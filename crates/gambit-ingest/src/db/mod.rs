//! Database connection, source management, and partition setup.

mod copy;

pub use copy::{build_staging_rows, copy_staging_batch};

use anyhow::{Context, Result};
use tokio_postgres::Client;

/// Ensure a source exists and return its id.
pub async fn ensure_source(client: &Client, name: &str) -> Result<i32> {
    let row = client
        .query_one(
            "INSERT INTO gambit.sources (name) VALUES ($1) \
             ON CONFLICT (name) DO UPDATE SET name = EXCLUDED.name \
             RETURNING id",
            &[&name],
        )
        .await
        .context("upsert source")?;
    let id: i32 = row.get(0);
    client
        .execute(
            "SELECT gambit.ensure_source_partitions($1, $2)",
            &[&id, &name],
        )
        .await
        .context("create partitions")?;
    Ok(id)
}

/// Apply schema migration SQL from the repo.
pub async fn run_migration(client: &Client, sql: &str) -> Result<()> {
    client.batch_execute(sql).await.context("run migration")?;
    Ok(())
}

/// Refresh opening move statistics materialized view.
pub async fn refresh_opening_stats(client: &Client) -> Result<()> {
    client
        .batch_execute("REFRESH MATERIALIZED VIEW gambit.opening_moves")
        .await
        .context("refresh opening_moves")?;
    Ok(())
}

/// Truncate all staging tables.
pub async fn truncate_staging(client: &Client) -> Result<()> {
    client
        .batch_execute(
            "TRUNCATE gambit.staging_games, gambit.staging_positions, gambit.staging_plies",
        )
        .await
        .context("truncate staging")?;
    Ok(())
}

/// Insert games from staging and return batch_seq → game_id mapping.
pub async fn insert_games_from_staging(client: &Client, source_id: i32) -> Result<Vec<(i32, i64)>> {
    client
        .batch_execute(
            "CREATE TEMP TABLE IF NOT EXISTS _batch_id_map (
                batch_seq int PRIMARY KEY,
                game_id bigint NOT NULL
            ) ON COMMIT DROP",
        )
        .await?;

    let rows = client
        .query(
            "WITH inserted AS (
                INSERT INTO gambit.games (
                    source_id, pgn_text, pgn_sha256, pgn_byte_offset,
                    white, black, white_elo, black_elo,
                    event, site, round, game_date, result, eco, ply_count
                )
                SELECT
                    $1, pgn_text, pgn_sha256, pgn_byte_offset,
                    white, black, white_elo, black_elo,
                    event, site, round, game_date, result, eco, ply_count
                FROM gambit.staging_games
                ORDER BY batch_seq
                RETURNING id
            ),
            numbered AS (
                SELECT id, row_number() OVER (ORDER BY id) AS rn FROM inserted
            ),
            staged AS (
                SELECT batch_seq, row_number() OVER (ORDER BY batch_seq) AS rn
                FROM gambit.staging_games
            )
            INSERT INTO _batch_id_map (batch_seq, game_id)
            SELECT s.batch_seq, n.id
            FROM staged s
            JOIN numbered n ON s.rn = n.rn
            RETURNING batch_seq, game_id",
            &[&source_id],
        )
        .await
        .context("insert games from staging")?;

    Ok(rows
        .iter()
        .map(|r| (r.get::<_, i32>(0), r.get::<_, i64>(1)))
        .collect())
}

/// Insert positions from staging using batch id map.
pub async fn insert_positions_from_staging(client: &Client, source_id: i32) -> Result<u64> {
    let n = client
        .execute(
            "INSERT INTO gambit.positions (game_id, source_id, ply, position, hash, fen)
            SELECT m.game_id, $1, sp.ply, sp.fen::chess_position, sp.hash, sp.fen
            FROM gambit.staging_positions sp
            JOIN _batch_id_map m ON m.batch_seq = sp.batch_seq",
            &[&source_id],
        )
        .await
        .context("insert positions from staging")?;
    Ok(n)
}

/// Insert plies from staging using batch id map.
pub async fn insert_plies_from_staging(client: &Client, source_id: i32) -> Result<u64> {
    let n = client
        .execute(
            "INSERT INTO gambit.plies (game_id, source_id, ply, move, san, uci)
            SELECT m.game_id, $1, sp.ply, sp.uci::chess_move, sp.san, sp.uci
            FROM gambit.staging_plies sp
            JOIN _batch_id_map m ON m.batch_seq = sp.batch_seq",
            &[&source_id],
        )
        .await
        .context("insert plies from staging")?;
    Ok(n)
}

/// Flush a staging batch: COPY → INSERT games/positions/plies → truncate staging.
pub async fn flush_staging_batch(client: &Client, source_id: i32) -> Result<(usize, u64, u64)> {
    let map = insert_games_from_staging(client, source_id).await?;
    let pos = insert_positions_from_staging(client, source_id).await?;
    let pl = insert_plies_from_staging(client, source_id).await?;
    truncate_staging(client).await?;
    Ok((map.len(), pos, pl))
}
