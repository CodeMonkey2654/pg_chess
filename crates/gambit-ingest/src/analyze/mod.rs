//! Game analysis pipeline: load plies, evaluate, bulk-write to DB.

mod copy;
pub mod gambit_eval;

use anyhow::{Context, Result};
use copy::flush_analysis_batch;
use futures::stream::{self, StreamExt};
use gambit_analysis::{GameAnalyzer, GamePly, GameReviewSummary};
use gambit_eval::GambitEvaluator;
use tokio_postgres::Client;

/// Options for analyzing games.
#[derive(Debug, Clone)]
pub struct AnalyzeOptions {
    /// Search depth (plies).
    pub depth: u32,
    /// Path to `.gbook` corpus export.
    pub corpus_book: Option<String>,
    /// Syzygy tablebase directory.
    pub syzygy_path: Option<String>,
    /// Parallel native analysis workers.
    pub workers: usize,
}

impl Default for AnalyzeOptions {
    fn default() -> Self {
        Self {
            depth: 12,
            corpus_book: std::env::var("GAMBIT_CORPUS_BOOK").ok(),
            syzygy_path: std::env::var("GAMBIT_SYZYGY_PATH").ok(),
            workers: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(2),
        }
    }
}

/// Result of analyzing one game.
#[derive(Debug, Clone)]
pub struct AnalyzeGameResult {
    /// Game id analyzed.
    pub game_id: i64,
    /// Summary with per-ply results.
    pub summary: GameReviewSummary,
}

/// Analyze a single game and persist results to `gambit.plies` / `gambit.games`.
pub async fn analyze_game(
    client: &Client,
    game_id: i64,
    options: &AnalyzeOptions,
) -> Result<AnalyzeGameResult> {
    client
        .execute(
            "UPDATE gambit.games SET analysis_status = 'running' WHERE id = $1",
            &[&game_id],
        )
        .await
        .context("mark game running")?;

    let row = client
        .query_opt(
            "SELECT source_id FROM gambit.games WHERE id = $1",
            &[&game_id],
        )
        .await?
        .context("game not found")?;
    let source_id: i32 = row.get(0);

    let plies = load_game_plies(client, game_id, source_id).await?;

    let summary = tokio::task::spawn_blocking({
        let options = options.clone();
        move || run_analysis(&plies, &options)
    })
    .await
    .context("analysis task join")??;

    write_analysis(client, source_id, &[(game_id, &summary)]).await?;

    Ok(AnalyzeGameResult { game_id, summary })
}

async fn write_analysis(
    client: &Client,
    source_id: i32,
    games: &[(i64, &GameReviewSummary)],
) -> Result<()> {
    client
        .execute("TRUNCATE gambit.staging_ply_analysis", &[])
        .await?;
    flush_analysis_batch(client, source_id, games).await
}

async fn load_game_plies(client: &Client, game_id: i64, source_id: i32) -> Result<Vec<GamePly>> {
    let start_fen: String = client
        .query_opt(
            "SELECT fen FROM gambit.positions
             WHERE game_id = $1 AND source_id = $2 AND ply = 0",
            &[&game_id, &source_id],
        )
        .await?
        .map(|r| r.get(0))
        .unwrap_or_else(|| "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1".to_string());

    let move_rows = client
        .query(
            "SELECT ply, uci FROM gambit.plies
             WHERE game_id = $1 AND source_id = $2 ORDER BY ply",
            &[&game_id, &source_id],
        )
        .await?;

    let mut plies = Vec::with_capacity(move_rows.len());
    let mut fen = start_fen;
    for row in move_rows {
        let ply: i32 = row.get(0);
        let uci: String = row.get(1);
        plies.push(GamePly {
            ply: ply as u32,
            uci: uci.clone(),
            fen_before: fen.clone(),
        });
        let next = gambit_db::Position::from_fen(&fen)
            .ok()
            .and_then(|pos| {
                gambit_db::Move::from_uci(&uci)
                    .ok()
                    .and_then(|mv| pos.apply_move(mv).ok())
            })
            .map(|p| p.to_fen())
            .unwrap_or(fen);
        fen = next;
    }
    Ok(plies)
}

fn run_analysis(plies: &[GamePly], options: &AnalyzeOptions) -> Result<GameReviewSummary> {
    let evaluator = GambitEvaluator::new(options)?;
    let mut analyzer = GameAnalyzer::new(evaluator);
    analyzer.analyze(plies).map_err(|e| anyhow::anyhow!(e))
}

/// Analyze up to `limit` pending games for a source.
pub async fn analyze_batch(
    client: &Client,
    source_id: i32,
    limit: usize,
    options: &AnalyzeOptions,
) -> Result<Vec<AnalyzeGameResult>> {
    let rows = client
        .query(
            "SELECT id FROM gambit.games
             WHERE source_id = $1 AND analysis_status IN ('none', 'failed')
             ORDER BY id LIMIT $2",
            &[&source_id, &(limit as i64)],
        )
        .await?;

    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let game_ids: Vec<i64> = rows.iter().map(|r| r.get(0)).collect();
    for &game_id in &game_ids {
        client
            .execute(
                "UPDATE gambit.games SET analysis_status = 'running' WHERE id = $1",
                &[&game_id],
            )
            .await?;
    }

    let plies_by_game = load_plies_batch(client, &game_ids, source_id).await?;

    let analyzed = stream::iter(game_ids)
        .map(|game_id| {
            let plies = plies_by_game.get(&game_id).cloned().unwrap_or_default();
            let options = options.clone();
            async move {
                match tokio::task::spawn_blocking(move || run_analysis(&plies, &options)).await {
                    Ok(Ok(summary)) => Ok((game_id, summary)),
                    Ok(Err(e)) => Err((game_id, e)),
                    Err(e) => Err((game_id, anyhow::anyhow!(e))),
                }
            }
        })
        .buffer_unordered(options.workers.max(1))
        .collect::<Vec<_>>()
        .await;

    let mut flush_batch: Vec<(i64, GameReviewSummary)> = Vec::new();
    let mut failed_ids: Vec<i64> = Vec::new();

    for outcome in analyzed {
        match outcome {
            Ok((game_id, summary)) => flush_batch.push((game_id, summary)),
            Err((game_id, e)) => {
                tracing::warn!(game_id, error = %e, "game analysis failed");
                failed_ids.push(game_id);
            }
        }
    }

    for game_id in failed_ids {
        let _ = client
            .execute(
                "UPDATE gambit.games SET analysis_status = 'failed' WHERE id = $1",
                &[&game_id],
            )
            .await;
    }

    if !flush_batch.is_empty() {
        let refs: Vec<(i64, &GameReviewSummary)> =
            flush_batch.iter().map(|(id, s)| (*id, s)).collect();
        if let Err(e) = write_analysis(client, source_id, &refs).await {
            for (game_id, _) in &flush_batch {
                let _ = client
                    .execute(
                        "UPDATE gambit.games SET analysis_status = 'failed' WHERE id = $1",
                        &[game_id],
                    )
                    .await;
            }
            return Err(e);
        }
    }

    let mut results = Vec::with_capacity(flush_batch.len());
    for (game_id, summary) in flush_batch {
        results.push(AnalyzeGameResult { game_id, summary });
    }

    Ok(results)
}

async fn load_plies_batch(
    client: &Client,
    game_ids: &[i64],
    source_id: i32,
) -> Result<std::collections::HashMap<i64, Vec<GamePly>>> {
    let mut out = std::collections::HashMap::new();
    for &game_id in game_ids {
        out.insert(game_id, load_game_plies(client, game_id, source_id).await?);
    }
    Ok(out)
}

