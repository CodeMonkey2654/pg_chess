//! Shared API types for Gambit Studio.

use serde::{Deserialize, Serialize};

/// Health check response.
#[derive(Debug, Serialize)]
pub struct HealthResponse {
    /// Service status.
    pub status: &'static str,
    /// Whether PostgreSQL is reachable.
    pub database_ok: bool,
}

/// Lightweight source row (no aggregate counts).
#[derive(Debug, Serialize)]
pub struct SourceListItem {
    /// Source id.
    pub id: i32,
    /// Source name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
}

/// Source summary row with counts (from /summary or legacy list).
#[derive(Debug, Serialize)]
pub struct SourceSummary {
    /// Source id.
    pub id: i32,
    /// Source name.
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Total games loaded.
    pub games: i64,
    /// Total positions.
    pub positions: i64,
    /// Total plies.
    pub plies: i64,
}

/// Detailed source metrics.
#[derive(Debug, Serialize)]
pub struct SourceDetail {
    /// Source id.
    pub id: i32,
    /// Source name.
    pub name: String,
    /// Game count.
    pub games: i64,
    /// Position count.
    pub positions: i64,
    /// Ply count.
    pub plies: i64,
    /// Approximate positions table size.
    pub positions_table_size: String,
}

/// Fileset shard status for the UI.
#[derive(Debug, Serialize)]
pub struct FilesetView {
    /// Fileset id.
    pub id: i64,
    /// Source id.
    pub source_id: i32,
    /// Remote URL.
    pub remote_url: String,
    /// Filename.
    pub filename: String,
    /// Period label (YYYY-MM).
    pub period_label: String,
    /// Status string.
    pub status: String,
    /// Games loaded.
    pub games_loaded: i64,
    /// Parse errors.
    pub games_errors: i64,
    /// Positions loaded.
    pub positions_loaded: i64,
    /// Error message if failed.
    pub error_message: Option<String>,
}

/// Sync catalog request body.
#[derive(Debug, Deserialize)]
pub struct SyncCatalogRequest {
    /// Source name.
    pub source: String,
    /// Calendar year.
    pub year: i32,
}

/// Load year request body.
#[derive(Debug, Deserialize)]
pub struct LoadYearRequest {
    /// Source name.
    pub source: String,
    /// Calendar year.
    pub year: i32,
}

/// Background job status.
#[derive(Debug, Clone, Serialize)]
pub struct JobStatus {
    /// Job id.
    pub id: u64,
    /// running | complete | failed.
    pub status: String,
    /// Human-readable progress message.
    pub message: String,
    /// Current shard index (1-based).
    pub current_shard: usize,
    /// Total shards in job.
    pub total_shards: usize,
    /// Games loaded so far in current job.
    pub games_loaded: u64,
    /// Latest games/min throughput.
    pub games_per_min: Option<f64>,
}

/// Game list item.
#[derive(Debug, Serialize)]
pub struct GameListItem {
    /// Game id.
    pub id: i64,
    /// White player.
    pub white: Option<String>,
    /// Black player.
    pub black: Option<String>,
    /// Result.
    pub result: String,
    /// Game date.
    pub game_date: Option<String>,
    /// Ply count.
    pub ply_count: i32,
}

/// Paginated games response.
#[derive(Debug, Serialize)]
pub struct GamesPage {
    /// Games in this page.
    pub games: Vec<GameListItem>,
    /// Total matching games.
    pub total: i64,
    /// Page offset.
    pub offset: i64,
    /// Page limit.
    pub limit: i64,
    /// Whether more pages exist.
    pub has_more: bool,
}

/// Paginated games at a position hash.
#[derive(Debug, Serialize)]
pub struct PositionGamesPage {
    /// Position hits in this page.
    pub hits: Vec<PositionHit>,
    /// Total games at this hash.
    pub total: i64,
    /// Page offset.
    pub offset: i64,
    /// Page limit.
    pub limit: i64,
    /// Whether more pages exist.
    pub has_more: bool,
}

/// One ply in a game.
#[derive(Debug, Serialize)]
pub struct PlyView {
    /// Ply index.
    pub ply: i32,
    /// SAN notation.
    pub san: String,
    /// UCI notation.
    pub uci: String,
}

/// Game detail with plies.
#[derive(Debug, Serialize)]
pub struct GameDetail {
    /// Game id.
    pub id: i64,
    /// Source id.
    pub source_id: i32,
    /// White player.
    pub white: Option<String>,
    /// Black player.
    pub black: Option<String>,
    /// Result.
    pub result: String,
    /// Event name.
    pub event: Option<String>,
    /// Plies in order.
    pub plies: Vec<PlyView>,
    /// Starting FEN.
    pub start_fen: String,
}

/// FEN → Zobrist hash request.
#[derive(Debug, Deserialize)]
pub struct HashFromFenRequest {
    /// FEN string.
    pub fen: String,
}

/// FEN → Zobrist hash response.
#[derive(Debug, Serialize)]
pub struct HashFromFenResponse {
    /// Zobrist hash as signed i64 (matches DB storage).
    pub hash: i64,
}

/// Position lookup hit.
#[derive(Debug, Serialize)]
pub struct PositionHit {
    /// Game id.
    pub game_id: i64,
    /// White player.
    pub white: Option<String>,
    /// Black player.
    pub black: Option<String>,
    /// Ply index.
    pub ply: i32,
    /// FEN string.
    pub fen: String,
}

/// Opening move stat row.
#[derive(Debug, Serialize)]
pub struct OpeningMoveStat {
    /// UCI move.
    pub move_uci: String,
    /// Total count.
    pub count: i64,
    /// White wins.
    pub white_wins: i64,
    /// Black wins.
    pub black_wins: i64,
    /// Draws.
    pub draws: i64,
}

/// One benchmark query result.
#[derive(Debug, Serialize)]
pub struct BenchResult {
    /// Stable query id (slug).
    pub id: String,
    /// Human-readable benchmark name.
    pub title: String,
    /// Why this query matters for chess-database UX or ops.
    pub description: String,
    /// Latency in milliseconds.
    pub latency_ms: f64,
    /// Rows returned.
    pub rows: i64,
}

/// Benchmark suite response.
#[derive(Debug, Serialize)]
pub struct BenchResponse {
    /// Individual query timings.
    pub results: Vec<BenchResult>,
}

/// Start job response.
#[derive(Debug, Serialize)]
pub struct JobStarted {
    /// Assigned job id.
    pub job_id: u64,
}
