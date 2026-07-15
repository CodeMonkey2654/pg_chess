//! Database query helpers.

use anyhow::{Context, Result};
use gambit_ingest::filesets;
use gambit_proto::{
    AnalyzeGameResponse, BenchResponse, BenchResult, FilesetView, GameAnalysisSummary, GameDetail,
    GameListItem, GamesPage, OpeningMoveStat, PlyView, PositionGamesPage, PositionHit,
    SourceDetail, SourceListItem,
};
use std::time::Instant;
use tokio_postgres::Client;

const START_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Health check against PostgreSQL.
pub async fn health(client: &Client) -> Result<bool> {
    client.simple_query("SELECT 1").await?;
    Ok(true)
}

/// Fast source list without aggregate counts.
pub async fn list_sources_fast(client: &Client) -> Result<Vec<SourceListItem>> {
    let rows = client
        .query(
            "SELECT id, name, description FROM gambit.sources ORDER BY name",
            &[],
        )
        .await
        .context("list sources fast")?;

    Ok(rows
        .iter()
        .map(|r| SourceListItem {
            id: r.get(0),
            name: r.get(1),
            description: r.get(2),
        })
        .collect())
}

/// Resolve source id by name.
pub async fn source_id_by_name(client: &Client, name: &str) -> Result<Option<i32>> {
    let rows = client
        .query("SELECT id FROM gambit.sources WHERE name = $1", &[&name])
        .await?;
    Ok(rows.first().map(|r| r.get(0)))
}

/// Detailed metrics for one source.
pub async fn source_detail(client: &Client, source_id: i32) -> Result<Option<SourceDetail>> {
    let rows = client
        .query(
            "SELECT s.id, s.name,
                    (SELECT count(*) FROM gambit.games g WHERE g.source_id = s.id),
                    (SELECT count(*) FROM gambit.positions p WHERE p.source_id = s.id),
                    (SELECT count(*) FROM gambit.plies pl WHERE pl.source_id = s.id),
                    pg_size_pretty(pg_total_relation_size('gambit.positions'))
             FROM gambit.sources s
             WHERE s.id = $1",
            &[&source_id],
        )
        .await
        .context("source detail")?;

    Ok(rows.first().map(|r| SourceDetail {
        id: r.get(0),
        name: r.get(1),
        games: r.get(2),
        positions: r.get(3),
        plies: r.get(4),
        positions_table_size: r.get(5),
    }))
}

/// List filesets for a source.
pub async fn list_filesets(client: &Client, source_id: i32) -> Result<Vec<FilesetView>> {
    let rows = filesets::list_filesets(client, source_id).await?;
    Ok(rows
        .into_iter()
        .map(|f| FilesetView {
            id: f.id,
            source_id: f.source_id,
            remote_url: f.remote_url,
            filename: f.filename,
            period_label: f.period_label,
            status: f.status,
            games_loaded: f.games_loaded,
            games_errors: f.games_errors,
            positions_loaded: f.positions_loaded,
            error_message: f.error_message,
        })
        .collect())
}

/// Search games with optional player and source filters.
pub async fn search_games(
    client: &Client,
    player: Option<&str>,
    source_id: Option<i32>,
    offset: i64,
    limit: i64,
) -> Result<GamesPage> {
    let limit = limit.clamp(1, 100);
    let pattern = player.filter(|s| !s.is_empty()).map(|p| format!("%{p}%"));

    let total: i64 = match (&pattern, source_id) {
        (Some(pat), Some(sid)) => client
            .query_one(
                "SELECT count(*) FROM gambit.games
                     WHERE source_id = $1 AND (white ILIKE $2 OR black ILIKE $2)",
                &[&sid, pat],
            )
            .await?
            .get(0),
        (Some(pat), None) => client
            .query_one(
                "SELECT count(*) FROM gambit.games
                     WHERE white ILIKE $1 OR black ILIKE $1",
                &[pat],
            )
            .await?
            .get(0),
        (None, Some(sid)) => client
            .query_one(
                "SELECT count(*) FROM gambit.games WHERE source_id = $1",
                &[&sid],
            )
            .await?
            .get(0),
        (None, None) => client
            .query_one("SELECT count(*) FROM gambit.games", &[])
            .await?
            .get(0),
    };

    let rows = match (&pattern, source_id) {
        (Some(pat), Some(sid)) => {
            client
                .query(
                    "SELECT id, white, black, result, game_date::text, ply_count
                     FROM gambit.games
                     WHERE source_id = $1 AND (white ILIKE $2 OR black ILIKE $2)
                     ORDER BY game_date DESC NULLS LAST, id DESC
                     OFFSET $3 LIMIT $4",
                    &[&sid, pat, &offset, &limit],
                )
                .await?
        }
        (Some(pat), None) => {
            client
                .query(
                    "SELECT id, white, black, result, game_date::text, ply_count
                     FROM gambit.games
                     WHERE white ILIKE $1 OR black ILIKE $1
                     ORDER BY game_date DESC NULLS LAST, id DESC
                     OFFSET $2 LIMIT $3",
                    &[pat, &offset, &limit],
                )
                .await?
        }
        (None, Some(sid)) => {
            client
                .query(
                    "SELECT id, white, black, result, game_date::text, ply_count
                     FROM gambit.games
                     WHERE source_id = $1
                     ORDER BY id DESC
                     OFFSET $2 LIMIT $3",
                    &[&sid, &offset, &limit],
                )
                .await?
        }
        (None, None) => {
            client
                .query(
                    "SELECT id, white, black, result, game_date::text, ply_count
                     FROM gambit.games
                     ORDER BY id DESC
                     OFFSET $1 LIMIT $2",
                    &[&offset, &limit],
                )
                .await?
        }
    };

    Ok(GamesPage {
        games: rows
            .iter()
            .map(|r| GameListItem {
                id: r.get(0),
                white: r.get(1),
                black: r.get(2),
                result: r.get(3),
                game_date: r.get::<_, Option<String>>(4),
                ply_count: r.get(5),
            })
            .collect(),
        total,
        offset,
        limit,
        has_more: offset + limit < total,
    })
}

/// Fetch one game with plies and real start FEN from positions ply 0.
pub async fn get_game(client: &Client, game_id: i64) -> Result<Option<GameDetail>> {
    let rows = client
        .query(
            "SELECT id, source_id, white, black, result, event,
                    analysis_status::text, accuracy_white, accuracy_black,
                    blunders_white, blunders_black, analyzed_at::text
             FROM gambit.games WHERE id = $1",
            &[&game_id],
        )
        .await?;
    let Some(row) = rows.first() else {
        return Ok(None);
    };

    let source_id: i32 = row.get(1);

    let plies = client
        .query(
            "SELECT ply, san, uci, eval_before, eval_after,
                    CASE WHEN best_move IS NULL THEN NULL ELSE chess_move_to_uci(best_move) END,
                    cp_loss, move_class::text
             FROM gambit.plies
             WHERE game_id = $1 AND source_id = $2 ORDER BY ply",
            &[&game_id, &source_id],
        )
        .await?;

    let start_fen = client
        .query_opt(
            "SELECT fen FROM gambit.positions
             WHERE game_id = $1 AND source_id = $2 AND ply = 0 LIMIT 1",
            &[&game_id, &source_id],
        )
        .await?
        .map(|r| r.get::<_, String>(0))
        .unwrap_or_else(|| START_FEN.to_string());

    Ok(Some(GameDetail {
        id: row.get(0),
        source_id: row.get(1),
        white: row.get(2),
        black: row.get(3),
        result: row.get(4),
        event: row.get(5),
        plies: plies
            .iter()
            .map(|p| PlyView {
                ply: p.get(0),
                san: p.get(1),
                uci: p.get(2),
                eval_before: p.get::<_, Option<i16>>(3).map(i32::from),
                eval_after: p.get::<_, Option<i16>>(4).map(i32::from),
                best_move: p.get(5),
                cp_loss: p.get::<_, Option<i16>>(6).map(i32::from),
                move_class: p.get(7),
            })
            .collect(),
        start_fen,
        analysis: Some(GameAnalysisSummary {
            status: row.get(6),
            accuracy_white: row.get(7),
            accuracy_black: row.get(8),
            blunders_white: row.get::<_, Option<i16>>(9).map(i32::from),
            blunders_black: row.get::<_, Option<i16>>(10).map(i32::from),
            analyzed_at: row.get(11),
        }),
    }))
}

/// Run engine analysis on a game and persist results.
pub async fn run_analyze_game(
    client: &Client,
    game_id: i64,
    depth: u32,
) -> Result<AnalyzeGameResponse> {
    let options = gambit_ingest::AnalyzeOptions {
        depth,
        ..Default::default()
    };
    let result = gambit_ingest::analyze_game(client, game_id, &options).await?;
    Ok(AnalyzeGameResponse {
        game_id: result.game_id,
        accuracy_white: result.summary.accuracy_white.map(|v| v as f32),
        accuracy_black: result.summary.accuracy_black.map(|v| v as f32),
        plies_analyzed: result.summary.plies.len() as u32,
    })
}

/// Lookup positions by Zobrist hash (limited sample).
pub async fn lookup_position(client: &Client, hash: i64) -> Result<Vec<PositionHit>> {
    let page = games_by_position(client, hash, 0, 50).await?;
    Ok(page.hits)
}

/// Paginated games reaching a position hash.
pub async fn games_by_position(
    client: &Client,
    hash: i64,
    offset: i64,
    limit: i64,
) -> Result<PositionGamesPage> {
    let limit = limit.clamp(1, 100);
    let total: i64 = client
        .query_one(
            "SELECT count(DISTINCT p.game_id) FROM gambit.positions p WHERE p.hash = $1",
            &[&hash],
        )
        .await?
        .get(0);

    let rows = client
        .query(
            "SELECT p.game_id, g.white, g.black, p.ply, p.fen
             FROM (
                 SELECT DISTINCT ON (game_id) game_id, ply, fen
                 FROM gambit.positions
                 WHERE hash = $1
                 ORDER BY game_id, ply
             ) p
             JOIN gambit.games g ON g.id = p.game_id
             ORDER BY g.game_date DESC NULLS LAST, p.game_id
             OFFSET $2 LIMIT $3",
            &[&hash, &offset, &limit],
        )
        .await?;

    Ok(PositionGamesPage {
        hits: rows
            .iter()
            .map(|r| PositionHit {
                game_id: r.get(0),
                white: r.get(1),
                black: r.get(2),
                ply: r.get(3),
                fen: r.get(4),
            })
            .collect(),
        total,
        offset,
        limit,
        has_more: offset + limit < total,
    })
}

/// Compute Zobrist hash for a FEN string.
pub fn hash_from_fen(fen: &str) -> Result<i64> {
    use gambit_db::prelude::Position;
    let pos = Position::from_fen(fen).context("parse fen")?;
    Ok(pos.zobrist_hash() as i64)
}

/// Opening move stats for a prefix hash.
pub async fn opening_stats(client: &Client, hash: i64) -> Result<Vec<OpeningMoveStat>> {
    let rows = client
        .query(
            "SELECT move_uci, count, white_wins, black_wins, draws
             FROM gambit.opening_moves
             WHERE prefix_hash = $1
             ORDER BY count DESC
             LIMIT 20",
            &[&hash],
        )
        .await?;

    Ok(rows
        .iter()
        .map(|r| OpeningMoveStat {
            move_uci: r.get(0),
            count: r.get(1),
            white_wins: r.get(2),
            black_wins: r.get(3),
            draws: r.get(4),
        })
        .collect())
}

/// Run a timed query benchmark suite.
pub async fn run_bench(client: &Client) -> Result<BenchResponse> {
    struct BenchQuery {
        id: &'static str,
        title: &'static str,
        description: &'static str,
        sql: &'static str,
    }

    let queries: &[BenchQuery] = &[
        BenchQuery {
            id: "corpus_size",
            title: "Corpus size",
            description: "Baseline count of loaded games — every dashboard and report depends on this being fast.",
            sql: "SELECT count(*) FROM gambit.games",
        },
        BenchQuery {
            id: "positions_scale",
            title: "Position index scale",
            description: "Positions are the largest table; count time reflects index health at millions of rows.",
            sql: "SELECT count(*) FROM gambit.positions",
        },
        BenchQuery {
            id: "player_search",
            title: "Player name search",
            description: "Powers the Games browser — users expect sub-second lookup when searching by player name.",
            sql: "SELECT id, white, black, result, game_date::text
                  FROM gambit.games
                  WHERE white ILIKE '%car%' OR black ILIKE '%car%'
                  ORDER BY game_date DESC NULLS LAST, id DESC
                  LIMIT 20",
        },
        BenchQuery {
            id: "recent_games_page",
            title: "Recent games (paginated)",
            description: "Simulates scrolling the game list — ordered scan with OFFSET/LIMIT on the games index.",
            sql: "SELECT id, white, black, result, game_date::text
                  FROM gambit.games
                  ORDER BY id DESC
                  OFFSET 500 LIMIT 20",
        },
        BenchQuery {
            id: "game_replay_plies",
            title: "Load game for replay",
            description: "Fetching every ply for one game is on the critical path when opening the board viewer.",
            sql: "SELECT ply, san, uci
                  FROM gambit.plies
                  WHERE game_id = (SELECT id FROM gambit.games ORDER BY ply_count DESC LIMIT 1)
                  ORDER BY ply",
        },
        BenchQuery {
            id: "position_transpositions",
            title: "Position transposition lookup",
            description: "Core chess-database query: find all games that reached the same position via Zobrist hash.",
            sql: "SELECT p.game_id, g.white, g.black, p.ply, p.fen
                  FROM gambit.positions p
                  JOIN gambit.games g ON g.id = p.game_id
                  WHERE p.hash = (SELECT hash FROM gambit.positions LIMIT 1)
                  LIMIT 50",
        },
        BenchQuery {
            id: "opening_explorer",
            title: "Opening explorer",
            description: "Popular continuations from a position — essential for opening prep and novelty hunting.",
            sql: "SELECT move_uci, count, white_wins, black_wins, draws
                  FROM gambit.opening_moves
                  WHERE prefix_hash = (SELECT prefix_hash FROM gambit.opening_moves LIMIT 1)
                  ORDER BY count DESC
                  LIMIT 20",
        },
        BenchQuery {
            id: "source_aggregation",
            title: "Per-source breakdown",
            description: "Dashboard aggregates games and positions per import batch — tests grouped JOIN performance.",
            sql: "SELECT s.name,
                         COALESCE(g.cnt, 0) AS games,
                         COALESCE(p.cnt, 0) AS positions
                  FROM gambit.sources s
                  LEFT JOIN (
                      SELECT source_id, count(*) AS cnt FROM gambit.games GROUP BY source_id
                  ) g ON g.source_id = s.id
                  LEFT JOIN (
                      SELECT source_id, count(*) AS cnt FROM gambit.positions GROUP BY source_id
                  ) p ON p.source_id = s.id
                  ORDER BY s.name",
        },
        BenchQuery {
            id: "result_filter",
            title: "Filter by result",
            description: "Endgame and opening studies often isolate decisive games — tests selective scans on result.",
            sql: "SELECT count(*) FROM gambit.games WHERE result = '1-0'",
        },
        BenchQuery {
            id: "date_range",
            title: "Date range filter",
            description: "Period analysis (e.g. all 2024 games) requires efficient use of the game_date index.",
            sql: "SELECT id, white, black, game_date::text
                  FROM gambit.games
                  WHERE game_date >= '2024-01-01' AND game_date < '2025-01-01'
                  ORDER BY game_date DESC
                  LIMIT 50",
        },
        BenchQuery {
            id: "eco_distribution",
            title: "ECO opening distribution",
            description: "Opening classification summaries group by ECO code — typical analytics aggregation query.",
            sql: "SELECT eco, count(*)::bigint AS games
                  FROM gambit.games
                  WHERE eco IS NOT NULL
                  GROUP BY eco
                  ORDER BY games DESC
                  LIMIT 20",
        },
        BenchQuery {
            id: "high_elo_filter",
            title: "High-rated games",
            description: "Training on strong players filters by Elo — tests nullable rating columns at scale.",
            sql: "SELECT id, white, black, white_elo, black_elo
                  FROM gambit.games
                  WHERE white_elo >= 2500 OR black_elo >= 2500
                  ORDER BY game_date DESC NULLS LAST
                  LIMIT 20",
        },
        BenchQuery {
            id: "head_to_head",
            title: "Head-to-head lookup",
            description: "Pairing queries (Player A vs Player B) are common in match prep and historical research.",
            sql: "SELECT id, white, black, result, game_date::text
                  FROM gambit.games
                  WHERE (white ILIKE '%nakamura%' AND black ILIKE '%carlsen%')
                     OR (white ILIKE '%carlsen%' AND black ILIKE '%nakamura%')
                  ORDER BY game_date DESC NULLS LAST
                  LIMIT 20",
        },
        BenchQuery {
            id: "positions_by_ply",
            title: "Positions at a given ply",
            description: "Opening-depth analysis asks 'what positions appear after move 20?' — ply-filtered index scan.",
            sql: "SELECT game_id, hash, fen
                  FROM gambit.positions
                  WHERE ply = 20
                  LIMIT 50",
        },
    ];

    let mut results = Vec::with_capacity(queries.len());

    for q in queries {
        let start = Instant::now();
        let rows = client.query(q.sql, &[]).await.with_context(|| q.id)?;
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;
        results.push(BenchResult {
            id: q.id.to_string(),
            title: q.title.to_string(),
            description: q.description.to_string(),
            latency_ms: elapsed,
            rows: rows.len() as i64,
        });
    }

    Ok(BenchResponse { results })
}
