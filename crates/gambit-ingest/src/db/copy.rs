//! PostgreSQL COPY writers for staging tables.

use crate::headers::{copy_field, copy_field_req, StagingGameRow};
use crate::pipeline::GameProvenance;
use crate::profile::IngestProfile;
use anyhow::{Context, Result};
use bytes::Bytes;
use futures::SinkExt;
use gambit_db::{ExplodedGame, PlyRow, PositionRow};
use rayon::prelude::*;
use std::time::{Duration, Instant};
use tokio_postgres::Client;

/// Timing for one COPY step (format buffer + send to Postgres).
#[derive(Debug, Clone, Copy)]
pub struct CopyStepTiming {
    /// Time to build the COPY text buffer.
    pub format: Duration,
    /// Time to send buffer via COPY protocol.
    pub send: Duration,
    /// Bytes in the COPY payload.
    pub bytes: u64,
}

/// Write exploded games to staging tables via COPY.
pub async fn copy_staging_batch(
    client: &Client,
    games: &[StagingGameRow],
    positions: &[(i32, &PositionRow)],
    plies: &[(i32, &PlyRow)],
    profile: &mut Option<IngestProfile>,
) -> Result<()> {
    let g = copy_staging_games(client, games).await?;
    let p = copy_staging_positions(client, positions).await?;
    let pl = copy_staging_plies(client, plies).await?;

    if let Some(prof) = profile {
        prof.record_count("copy.format_games", g.format, g.bytes);
        prof.record_count("copy.send_games", g.send, g.bytes);
        prof.record_count("copy.format_positions", p.format, p.bytes);
        prof.record_count("copy.send_positions", p.send, p.bytes);
        prof.record_count("copy.format_plies", pl.format, pl.bytes);
        prof.record_count("copy.send_plies", pl.send, pl.bytes);
    }
    Ok(())
}

async fn copy_staging_games(client: &Client, games: &[StagingGameRow]) -> Result<CopyStepTiming> {
    if games.is_empty() {
        return Ok(CopyStepTiming {
            format: Duration::ZERO,
            send: Duration::ZERO,
            bytes: 0,
        });
    }
    let fmt_start = Instant::now();
    let parts: Vec<String> = games
        .par_iter()
        .map(|g| {
            format!(
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
            )
        })
        .collect();
    let data = parts.concat();
    let format = fmt_start.elapsed();
    let bytes = data.len() as u64;
    let send = copy_in(
        client,
        "COPY gambit.staging_games (batch_seq, pgn_text, pgn_sha256, pgn_byte_offset, \
         white, black, white_elo, black_elo, event, site, round, game_date, result, eco, ply_count) \
         FROM STDIN WITH (FORMAT text)",
        &data,
    )
    .await?;
    Ok(CopyStepTiming {
        format,
        send,
        bytes,
    })
}

async fn copy_staging_positions(
    client: &Client,
    positions: &[(i32, &PositionRow)],
) -> Result<CopyStepTiming> {
    if positions.is_empty() {
        return Ok(CopyStepTiming {
            format: Duration::ZERO,
            send: Duration::ZERO,
            bytes: 0,
        });
    }
    let fmt_start = Instant::now();
    let parts: Vec<String> = positions
        .par_iter()
        .map(|(batch_seq, pos)| {
            format!(
                "{}\t{}\t{}\t{}\n",
                batch_seq,
                pos.ply,
                copy_field_req(&pos.fen),
                pos.hash as i64,
            )
        })
        .collect();
    let data = parts.concat();
    let format = fmt_start.elapsed();
    let bytes = data.len() as u64;
    let send = copy_in(
        client,
        "COPY gambit.staging_positions (batch_seq, ply, fen, hash) FROM STDIN WITH (FORMAT text)",
        &data,
    )
    .await?;
    Ok(CopyStepTiming {
        format,
        send,
        bytes,
    })
}

async fn copy_staging_plies(client: &Client, plies: &[(i32, &PlyRow)]) -> Result<CopyStepTiming> {
    if plies.is_empty() {
        return Ok(CopyStepTiming {
            format: Duration::ZERO,
            send: Duration::ZERO,
            bytes: 0,
        });
    }
    let fmt_start = Instant::now();
    let parts: Vec<String> = plies
        .par_iter()
        .map(|(batch_seq, ply)| {
            format!(
                "{}\t{}\t{}\t{}\n",
                batch_seq,
                ply.ply,
                copy_field_req(&ply.uci),
                copy_field_req(&ply.san),
            )
        })
        .collect();
    let data = parts.concat();
    let format = fmt_start.elapsed();
    let bytes = data.len() as u64;
    let send = copy_in(
        client,
        "COPY gambit.staging_plies (batch_seq, ply, uci, san) FROM STDIN WITH (FORMAT text)",
        &data,
    )
    .await?;
    Ok(CopyStepTiming {
        format,
        send,
        bytes,
    })
}

async fn copy_in(client: &Client, statement: &str, data: &str) -> Result<Duration> {
    let start = Instant::now();
    let sink = client
        .copy_in(statement)
        .await
        .with_context(|| format!("start COPY: {statement}"))?;
    futures::pin_mut!(sink);
    sink.send(Bytes::from(data.to_string()))
        .await
        .context("COPY send")?;
    sink.close().await.context("COPY close")?;
    Ok(start.elapsed())
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
pub fn build_staging_rows(
    batch: &[(i32, ExplodedGame, Option<String>, GameProvenance)],
) -> StagingBatch {
    let mut games = Vec::with_capacity(batch.len());
    let mut positions = Vec::new();
    let mut plies = Vec::new();

    for (batch_seq, exploded, pgn_text, provenance) in batch {
        games.push(StagingGameRow::from_exploded(
            *batch_seq,
            exploded,
            pgn_text.clone(),
            provenance.pgn_sha256.clone(),
            provenance.pgn_byte_offset,
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
