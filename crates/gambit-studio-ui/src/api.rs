//! gRPC client for the Gambit Studio API (grpc-web).

use crate::grpc_web::{server_streaming, unary};
use gambit_proto::{
    BenchResponse, Empty, GameDetail, GamesByPositionRequest, GamesPage, GetActiveJobRequest,
    GetGameRequest, GetSourceSummaryRequest, HashFromFenRequest,
    HashFromFenResponse, HealthResponse, JobStarted, JobStatus, ListFilesetsRequest,
    ListFilesetsResponse, ListSourcesResponse, LoadYearRequest, OpeningStatsRequest,
    OpeningStatsResponse, OptionalJobStatus,
    PositionGamesPage, SearchGamesRequest, SourceDetail, SyncCatalogRequest, SyncCatalogResponse,
    WatchJobRequest,
};

pub async fn fetch_health() -> Result<HealthResponse, String> {
    unary("Health", &Empty {}).await
}

pub async fn fetch_sources() -> Result<Vec<gambit_proto::SourceListItem>, String> {
    let resp: ListSourcesResponse = unary("ListSources", &Empty {}).await?;
    Ok(resp.sources)
}

pub async fn fetch_source_detail(id: i32) -> Result<SourceDetail, String> {
    unary(
        "GetSourceSummary",
        &GetSourceSummaryRequest { source_id: id },
    )
    .await
}

pub async fn fetch_filesets_by_name(
    source_name: &str,
) -> Result<Vec<gambit_proto::FilesetView>, String> {
    let resp: ListFilesetsResponse = unary(
        "ListFilesets",
        &ListFilesetsRequest {
            source_id: None,
            source_name: Some(source_name.to_string()),
        },
    )
    .await?;
    Ok(resp.filesets)
}

pub async fn sync_catalog(source: &str, year: i32) -> Result<SyncCatalogResponse, String> {
    unary(
        "SyncCatalog",
        &SyncCatalogRequest {
            source: source.to_string(),
            year,
        },
    )
    .await
}

pub async fn load_year(source: &str, year: i32) -> Result<JobStarted, String> {
    unary(
        "LoadYear",
        &LoadYearRequest {
            source: source.to_string(),
            year,
        },
    )
    .await
}

pub async fn fetch_active_job(source_name: &str, year: i32) -> Result<Option<JobStatus>, String> {
    let resp: OptionalJobStatus = unary(
        "GetActiveJob",
        &GetActiveJobRequest {
            source_name: Some(source_name.to_string()),
            year: Some(year),
        },
    )
    .await?;
    Ok(resp.job)
}

pub async fn watch_job(
    job_id: u64,
    source_name: Option<String>,
    year: Option<i32>,
    mut on_status: impl FnMut(JobStatus) -> bool,
) -> Result<(), String> {
    server_streaming(
        "WatchJob",
        &WatchJobRequest {
            job_id,
            source_name,
            year,
        },
        |status| on_status(status),
    )
    .await
}

pub async fn fetch_games(
    player: Option<&str>,
    source_id: Option<i32>,
    offset: i64,
    limit: i64,
) -> Result<GamesPage, String> {
    unary(
        "SearchGames",
        &SearchGamesRequest {
            player: player.map(str::to_string),
            source_id,
            offset,
            limit,
        },
    )
    .await
}

pub async fn fetch_game(id: i64) -> Result<GameDetail, String> {
    unary("GetGame", &GetGameRequest { game_id: id }).await
}

pub async fn hash_from_fen(fen: &str) -> Result<i64, String> {
    let resp: HashFromFenResponse = unary(
        "HashFromFen",
        &HashFromFenRequest {
            fen: fen.to_string(),
        },
    )
    .await?;
    Ok(resp.hash)
}

pub async fn fetch_opening_stats(hash: i64) -> Result<Vec<gambit_proto::OpeningMoveStat>, String> {
    let resp: OpeningStatsResponse = unary("OpeningStats", &OpeningStatsRequest { hash }).await?;
    Ok(resp.stats)
}

pub async fn fetch_games_by_position(
    hash: i64,
    offset: i64,
    limit: i64,
) -> Result<PositionGamesPage, String> {
    unary(
        "GamesByPosition",
        &GamesByPositionRequest {
            hash,
            offset,
            limit,
        },
    )
    .await
}

pub async fn run_bench() -> Result<BenchResponse, String> {
    unary("RunBench", &Empty {}).await
}

pub async fn analyze_game(game_id: i64, depth: u32) -> Result<gambit_proto::AnalyzeGameResponse, String> {
    unary(
        "AnalyzeGame",
        &gambit_proto::AnalyzeGameRequest { game_id, depth },
    )
    .await
}
