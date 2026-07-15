//! PostgreSQL COPY writer for analysis staging.

use crate::headers::{copy_field, copy_field_req};
use anyhow::{Context, Result};
use bytes::Bytes;
use futures::SinkExt;
use gambit_analysis::GameReviewSummary;
use std::time::Instant;
use tokio_postgres::Client;

/// One row for `gambit.staging_ply_analysis`.
#[derive(Debug, Clone)]
pub struct StagingPlyAnalysisRow {
    /// Game id.
    pub game_id: i64,
    /// Ply number.
    pub ply: i32,
    /// Eval before move (centipawns).
    pub eval_before: Option<i16>,
    /// Eval after move (centipawns).
    pub eval_after: Option<i16>,
    /// Best move UCI.
    pub best_move: String,
    /// Centipawn loss.
    pub cp_loss: Option<i16>,
    /// Move class label.
    pub move_class: String,
    /// Search depth.
    pub eval_depth: Option<i16>,
    /// Eval backend label.
    pub eval_source: String,
}

/// Build staging rows from analyzed games.
pub fn build_staging_rows(games: &[(i64, &GameReviewSummary)]) -> Vec<StagingPlyAnalysisRow> {
    let total_plies: usize = games.iter().map(|(_, s)| s.plies.len()).sum();
    let mut rows = Vec::with_capacity(total_plies);
    for (game_id, summary) in games {
        for ply in &summary.plies {
            rows.push(StagingPlyAnalysisRow {
                game_id: *game_id,
                ply: ply.ply as i32,
                eval_before: Some(ply.eval_before as i16),
                eval_after: Some(ply.eval_after as i16),
                best_move: ply.best_move.to_uci(),
                cp_loss: Some(ply.cp_loss as i16),
                move_class: ply.move_class.as_str().to_string(),
                eval_depth: Some(ply.depth as i16),
                eval_source: ply.source.as_str().to_string(),
            });
        }
    }
    rows
}

/// COPY rows into `gambit.staging_ply_analysis`.
pub async fn copy_staging_ply_analysis(
    client: &Client,
    rows: &[StagingPlyAnalysisRow],
) -> Result<()> {
    if rows.is_empty() {
        return Ok(());
    }

    let mut data = String::with_capacity(rows.len() * 64);
    for row in rows {
        data.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            row.game_id,
            row.ply,
            copy_field_i16(row.eval_before),
            copy_field_i16(row.eval_after),
            copy_field_req(&row.best_move),
            copy_field_i16(row.cp_loss),
            copy_field_req(&row.move_class),
            copy_field_i16(row.eval_depth),
            copy_field_req(&row.eval_source),
        ));
    }

    let sink = client
        .copy_in(
            "COPY gambit.staging_ply_analysis (
                game_id, ply, eval_before, eval_after, best_move,
                cp_loss, move_class, eval_depth, eval_source
            ) FROM STDIN WITH (FORMAT text)",
        )
        .await
        .context("start COPY staging_ply_analysis")?;
    futures::pin_mut!(sink);
    sink.send(Bytes::from(data))
        .await
        .context("COPY staging_ply_analysis send")?;
    sink.close()
        .await
        .context("COPY staging_ply_analysis close")?;
    Ok(())
}

/// Persist analyzed games: COPY staging → merge → per-game rollup.
pub async fn flush_analysis_batch(
    client: &Client,
    source_id: i32,
    games: &[(i64, &GameReviewSummary)],
) -> Result<()> {
    if games.is_empty() {
        return Ok(());
    }

    let rows = build_staging_rows(games);
    let copy_start = Instant::now();
    copy_staging_ply_analysis(client, &rows).await?;
    tracing::debug!(
        plies = rows.len(),
        games = games.len(),
        copy_ms = copy_start.elapsed().as_millis(),
        "analysis staging COPY complete"
    );

    client
        .query_one("SELECT gambit.merge_ply_analysis($1)", &[&source_id])
        .await
        .context("merge ply analysis")?;

    for (game_id, _) in games {
        client
            .execute("SELECT gambit.rollup_game_analysis($1)", &[game_id])
            .await
            .with_context(|| format!("rollup game analysis for game {game_id}"))?;
    }

    Ok(())
}

fn copy_field_i16(value: Option<i16>) -> String {
    copy_field(value.map(|v| v.to_string()).as_deref())
}

#[cfg(test)]
mod tests {
    use super::*;
    use gambit_analysis::{EvalSource, GameReviewSummary, MoveClass, PlyAnalysis};
    use gambit_db::Move;

    #[test]
    fn build_rows_from_summary() {
        let summary = GameReviewSummary {
            plies: vec![PlyAnalysis {
                ply: 1,
                eval_before: 20,
                eval_after: 10,
                best_move: Move::from_uci("e2e4").expect("mv"),
                cp_loss: 10,
                move_class: MoveClass::Good,
                depth: 12,
                source: EvalSource::Native,
            }],
            accuracy_white: Some(90.0),
            accuracy_black: None,
            blunders_white: 0,
            blunders_black: 0,
        };
        let rows = build_staging_rows(&[(42, &summary)]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].game_id, 42);
        assert_eq!(rows[0].move_class, "good");
    }
}
