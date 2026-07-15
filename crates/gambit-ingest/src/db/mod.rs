//! Database connection, source management, and partition setup.

mod copy;
pub mod filesets;
mod staging_lock;

pub use copy::{build_staging_rows, copy_staging_batch};
pub use staging_lock::{acquire_staging_lock, release_staging_lock};
pub use filesets::{
    get_fileset, list_filesets, mark_download_complete, mark_download_started, mark_failed,
    mark_ingest_complete, mark_ingest_started, record_ingest_run, upsert_fileset, FilesetRow,
};

use anyhow::{Context, Result};
use std::time::Instant;
use tokio_postgres::Client;

use crate::profile::IngestProfile;

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

/// Flush a staging batch: INSERT games/positions/plies → truncate staging.
pub async fn flush_staging_batch(
    client: &mut Client,
    source_id: i32,
    defer_types: bool,
    profile: &mut Option<IngestProfile>,
) -> Result<(usize, u64, u64)> {
    let tx_start = Instant::now();
    let tx = client.transaction().await.context("begin batch tx")?;

    tx.batch_execute(
        "CREATE TEMP TABLE IF NOT EXISTS _batch_id_map (
            batch_seq int PRIMARY KEY,
            game_id bigint NOT NULL
        )",
    )
    .await?;
    tx.batch_execute("TRUNCATE _batch_id_map")
        .await
        .context("truncate batch id map")?;
    let tx_setup = tx_start.elapsed();

    let games_start = Instant::now();
    let rows = tx
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
        .map_err(|e| anyhow::anyhow!("insert games from staging: {e}"))?;

    let game_count = rows.len();
    let insert_games = games_start.elapsed();

    let pos_start = Instant::now();
    let pos_sql = if defer_types {
        "INSERT INTO gambit.positions (game_id, source_id, ply, position, hash, fen)
            SELECT m.game_id, $1, sp.ply, NULL, sp.hash, sp.fen
            FROM gambit.staging_positions sp
            JOIN _batch_id_map m ON m.batch_seq = sp.batch_seq"
    } else {
        "INSERT INTO gambit.positions (game_id, source_id, ply, position, hash, fen)
            SELECT m.game_id, $1, sp.ply, sp.fen::chess_position, sp.hash, sp.fen
            FROM gambit.staging_positions sp
            JOIN _batch_id_map m ON m.batch_seq = sp.batch_seq"
    };
    let pos = tx
        .execute(pos_sql, &[&source_id])
        .await
        .map_err(|e| anyhow::anyhow!("insert positions from staging: {e}"))?;
    let insert_positions = pos_start.elapsed();

    let pl_start = Instant::now();
    let pl_sql = if defer_types {
        "INSERT INTO gambit.plies (game_id, source_id, ply, move, san, uci)
            SELECT m.game_id, $1, sp.ply, NULL, sp.san, sp.uci
            FROM gambit.staging_plies sp
            JOIN _batch_id_map m ON m.batch_seq = sp.batch_seq"
    } else {
        "INSERT INTO gambit.plies (game_id, source_id, ply, move, san, uci)
            SELECT m.game_id, $1, sp.ply, sp.uci::chess_move, sp.san, sp.uci
            FROM gambit.staging_plies sp
            JOIN _batch_id_map m ON m.batch_seq = sp.batch_seq"
    };
    let pl = tx
        .execute(pl_sql, &[&source_id])
        .await
        .map_err(|e| anyhow::anyhow!("insert plies from staging: {e}"))?;
    let insert_plies = pl_start.elapsed();

    let commit_start = Instant::now();
    tx.batch_execute(
        "TRUNCATE gambit.staging_games, gambit.staging_positions, gambit.staging_plies",
    )
    .await
    .context("truncate staging")?;

    tx.commit().await.context("commit batch tx")?;
    let truncate_tx = commit_start.elapsed();

    if let Some(p) = profile {
        p.record("db.tx_setup", tx_setup);
        p.record_count("db.insert_games", insert_games, game_count as u64);
        p.record_count("db.insert_positions", insert_positions, pos);
        p.record_count("db.insert_plies", insert_plies, pl);
        p.record("db.truncate_staging_tx", truncate_tx);
    }
    Ok((game_count, pos, pl))
}

/// Materialize deferred chess_position / chess_move columns after bulk load.
pub async fn backfill_types(
    client: &Client,
    source_id: i32,
    profile: &mut Option<IngestProfile>,
) -> Result<(i64, i64)> {
    let pos_start = Instant::now();
    let pos_row = client
        .query_one("SELECT gambit.backfill_positions($1)", &[&source_id])
        .await
        .context("backfill positions")?;
    let pos_count: i64 = pos_row.get(0);
    let backfill_positions = pos_start.elapsed();

    let pl_start = Instant::now();
    let pl_row = client
        .query_one("SELECT gambit.backfill_plies($1)", &[&source_id])
        .await
        .context("backfill plies")?;
    let pl_count: i64 = pl_row.get(0);
    let backfill_plies = pl_start.elapsed();

    let idx_start = Instant::now();
    client
        .execute("SELECT gambit.ensure_position_indexes($1)", &[&source_id])
        .await
        .context("ensure position indexes")?;
    let ensure_indexes = idx_start.elapsed();

    if let Some(p) = profile {
        p.record_count(
            "db.backfill_positions",
            backfill_positions,
            pos_count as u64,
        );
        p.record_count("db.backfill_plies", backfill_plies, pl_count as u64);
        p.record("db.ensure_position_indexes", ensure_indexes);
    }

    Ok((pos_count, pl_count))
}

/// Apply schema migration SQL from the repo.
async fn seed_applied_migrations(client: &Client) -> Result<()> {
    // Existing dev DBs created before schema_migrations tracking: treat core schema as applied.
    let has_sources: bool = client
        .query_one(
            "SELECT EXISTS(
                SELECT 1 FROM information_schema.tables
                WHERE table_schema = 'gambit' AND table_name = 'sources'
            )",
            &[],
        )
        .await?
        .get(0);

    if has_sources {
        client
            .execute(
                "INSERT INTO gambit.schema_migrations (filename)
                 VALUES ('001_core.sql')
                 ON CONFLICT (filename) DO NOTHING",
                &[],
            )
            .await?;
    }
    Ok(())
}

/// Apply pending schema migration files in order.
pub async fn run_migrations(client: &Client) -> Result<()> {
    client
        .batch_execute(
            "CREATE SCHEMA IF NOT EXISTS gambit;
             CREATE TABLE IF NOT EXISTS gambit.schema_migrations (
                 filename text PRIMARY KEY,
                 applied_at timestamptz NOT NULL DEFAULT now()
             );",
        )
        .await
        .context("ensure schema_migrations table")?;

    seed_applied_migrations(client).await?;

    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../schema/migrations");
    let mut files: Vec<_> = std::fs::read_dir(&dir)
        .with_context(|| format!("read migrations dir {}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "sql"))
        .collect();
    files.sort();

    for path in files {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_string();

        let applied: bool = client
            .query_one(
                "SELECT EXISTS(SELECT 1 FROM gambit.schema_migrations WHERE filename = $1)",
                &[&filename],
            )
            .await
            .context("check migration status")?
            .get(0);

        if applied {
            continue;
        }

        let sql = std::fs::read_to_string(&path)
            .with_context(|| format!("read migration {}", path.display()))?;
        client
            .batch_execute(&sql)
            .await
            .with_context(|| format!("run migration {}", path.display()))?;
        client
            .execute(
                "INSERT INTO gambit.schema_migrations (filename) VALUES ($1)",
                &[&filename],
            )
            .await
            .with_context(|| format!("record migration {filename}"))?;
    }
    Ok(())
}
