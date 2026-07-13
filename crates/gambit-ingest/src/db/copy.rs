//! PostgreSQL COPY writers for staging tables.

use crate::headers::{copy_field, copy_field_req, StagingGameRow};
use anyhow::{Context, Result};
use bytes::Bytes;
use futures::SinkExt;
use gambit_db::{ExplodedGame, PlyRow, PositionRow};
use tokio_postgres::Client;

/// Write exploded games to staging tables via COPY.
pub async fn copy_staging_batch(
    client: &Client,
    games: &[StagingGameRow],
    positions: &[(i32, &PositionRow)],
    plies: &[(i32, &PlyRow)],
) -> Result<()> {
    copy_staging_games(client, games).await?;
    copy_staging_positions(client, positions).await?;
    copy_staging_plies(client, plies).await?;
    Ok(())
}

async fn copy_staging_games(client: &Client, games: &[StagingGameRow]) -> Result<()> {
    if games.is_empty() {
        return Ok(());
    }
    let mut data = String::new();
    for g in games {
        let row = format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            g.batch_seq,
            copy_field(g.pgn_text.as_deref()),
            copy_field(g.pgn_sha256.as_ref().map(|b| hex_bytes(b)).as_deref()),
            copy_field(g.pgn_byte_offset.map(|v| v.to_string()).as_deref()),
            copy_field(g.white.as_deref()),
            copy_field(g.black.as_deref()),
            copy_field(g.white_elo.map(|v| v.to_string()).as_deref()),
            copy_field(g.black_elo.map(|v| v.to_string()).as_deref()),
            copy_field(g.event.as_deref()),
            copy_field(g.site.as_deref()),
            copy_field(g.round.as_deref()),
            copy_field(g.game_date.map(|d| d.to_string()).as_deref()),
            copy_field_req(&g.result),
            copy_field(g.eco.as_deref()),
            g.ply_count,
        );
        data.push_str(&row);
    }
    copy_in(
        client,
        "COPY gambit.staging_games (batch_seq, pgn_text, pgn_sha256, pgn_byte_offset, \
         white, black, white_elo, black_elo, event, site, round, game_date, result, eco, ply_count) \
         FROM STDIN WITH (FORMAT text)",
        &data,
    )
    .await
}

async fn copy_staging_positions(client: &Client, positions: &[(i32, &PositionRow)]) -> Result<()> {
    if positions.is_empty() {
        return Ok(());
    }
    let mut data = String::new();
    for (batch_seq, pos) in positions {
        data.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            batch_seq,
            pos.ply,
            copy_field_req(&pos.fen),
            pos.hash as i64,
        ));
    }
    copy_in(
        client,
        "COPY gambit.staging_positions (batch_seq, ply, fen, hash) FROM STDIN WITH (FORMAT text)",
        &data,
    )
    .await
}

async fn copy_staging_plies(client: &Client, plies: &[(i32, &PlyRow)]) -> Result<()> {
    if plies.is_empty() {
        return Ok(());
    }
    let mut data = String::new();
    for (batch_seq, ply) in plies {
        data.push_str(&format!(
            "{}\t{}\t{}\t{}\n",
            batch_seq,
            ply.ply,
            copy_field_req(&ply.uci),
            copy_field_req(&ply.san),
        ));
    }
    copy_in(
        client,
        "COPY gambit.staging_plies (batch_seq, ply, uci, san) FROM STDIN WITH (FORMAT text)",
        &data,
    )
    .await
}

async fn copy_in(client: &Client, statement: &str, data: &str) -> Result<()> {
    let sink = client
        .copy_in(statement)
        .await
        .with_context(|| format!("start COPY: {statement}"))?;
    futures::pin_mut!(sink);
    sink.send(Bytes::from(data.to_string()))
        .await
        .context("COPY send")?;
    sink.close().await.context("COPY close")?;
    Ok(())
}

fn hex_bytes(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Staging rows built from a parsed batch.
pub type StagingBatch = (
    Vec<StagingGameRow>,
    Vec<(i32, PositionRow)>,
    Vec<(i32, PlyRow)>,
);

/// Build staging rows from parsed exploded games.
pub fn build_staging_rows(batch: &[(i32, ExplodedGame, Option<String>)]) -> StagingBatch {
    let mut games = Vec::with_capacity(batch.len());
    let mut positions = Vec::new();
    let mut plies = Vec::new();

    for (batch_seq, exploded, pgn_text) in batch {
        games.push(StagingGameRow::from_exploded(
            *batch_seq,
            exploded,
            pgn_text.clone(),
            None,
            None,
        ));
        for pos in &exploded.positions {
            positions.push((*batch_seq, pos.clone()));
        }
        for ply in &exploded.plies {
            plies.push((*batch_seq, ply.clone()));
        }
    }

    (games, positions, plies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_staging_empty_batch() {
        let (g, p, pl) = build_staging_rows(&[]);
        assert!(g.is_empty());
        assert!(p.is_empty());
        assert!(pl.is_empty());
    }
}
