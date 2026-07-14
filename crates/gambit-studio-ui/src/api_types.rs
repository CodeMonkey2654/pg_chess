//! Shared API types mirroring gambit-studio-server responses.

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub database_ok: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceListItem {
    pub id: i32,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SourceDetail {
    pub id: i32,
    pub name: String,
    pub games: i64,
    pub positions: i64,
    pub plies: i64,
    pub positions_table_size: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FilesetView {
    pub id: i64,
    pub source_id: i32,
    pub remote_url: String,
    pub filename: String,
    pub period_label: String,
    pub status: String,
    pub games_loaded: i64,
    pub games_errors: i64,
    pub positions_loaded: i64,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobStatus {
    pub id: u64,
    pub status: String,
    pub message: String,
    pub current_shard: usize,
    pub total_shards: usize,
    pub games_loaded: u64,
    pub games_per_min: Option<f64>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct JobStarted {
    pub job_id: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GameListItem {
    pub id: i64,
    pub white: Option<String>,
    pub black: Option<String>,
    pub result: String,
    pub game_date: Option<String>,
    pub ply_count: i32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GamesPage {
    pub games: Vec<GameListItem>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PlyView {
    pub ply: i32,
    pub san: String,
    pub uci: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GameDetail {
    pub id: i64,
    pub source_id: i32,
    pub white: Option<String>,
    pub black: Option<String>,
    pub result: String,
    pub event: Option<String>,
    pub plies: Vec<PlyView>,
    pub start_fen: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PositionHit {
    pub game_id: i64,
    pub white: Option<String>,
    pub black: Option<String>,
    pub ply: i32,
    pub fen: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PositionGamesPage {
    pub hits: Vec<PositionHit>,
    pub total: i64,
    pub offset: i64,
    pub limit: i64,
    pub has_more: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OpeningMoveStat {
    pub move_uci: String,
    pub count: i64,
    pub white_wins: i64,
    pub black_wins: i64,
    pub draws: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HashFromFenResponse {
    pub hash: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BenchResult {
    pub id: String,
    pub title: String,
    pub description: String,
    pub latency_ms: f64,
    pub rows: i64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BenchResponse {
    pub results: Vec<BenchResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SyncResponse {
    pub synced: usize,
}
