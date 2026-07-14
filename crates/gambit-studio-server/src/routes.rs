//! Axum route handlers.

use crate::db::{
    games_by_position, get_game, hash_from_fen, health, list_filesets, list_sources_fast,
    lookup_position, opening_stats, reconstruct_job_from_filesets, run_bench, search_games,
    source_detail, source_id_by_name,
};
use crate::jobs::JobManager;
use crate::pool::PgPool;
use crate::types::{
    BenchResponse, FilesetView, GameDetail, GamesPage, HashFromFenRequest, HashFromFenResponse,
    HealthResponse, JobStarted, JobStatus, LoadYearRequest, OpeningMoveStat, PositionGamesPage,
    PositionHit, SourceDetail, SourceListItem, SyncCatalogRequest,
};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use gambit_ingest::{ImportOptions, IngestSession};
use std::path::PathBuf;
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;
use tracing::info;

/// Shared application state.
#[derive(Clone)]
pub struct AppState {
    pool: Arc<PgPool>,
    pg_uri: String,
    cache_dir: PathBuf,
    jobs: JobManager,
}

impl AppState {
    /// Build state from environment/config.
    pub fn new(pool: PgPool, pg_uri: String, cache_dir: PathBuf) -> Self {
        Self {
            pool: Arc::new(pool),
            pg_uri,
            cache_dir,
            jobs: JobManager::new(),
        }
    }

    async fn client(&self) -> Result<deadpool_postgres::Client, ApiError> {
        self.pool.get().await.map_err(ApiError::internal)
    }
}

/// API error wrapper.
pub struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn internal(err: anyhow::Error) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: err.to_string(),
        }
    }

    fn bad_request(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: msg.into(),
        }
    }

    fn not_found(msg: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: msg.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

/// Build the Axum router.
pub fn router(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/api/health", get(health_handler))
        .route("/api/sources", get(sources_handler))
        .route("/api/sources/{id}/summary", get(source_summary_handler))
        .route("/api/filesets", get(filesets_handler))
        .route("/api/filesets/sync", post(sync_catalog_handler))
        .route("/api/filesets/load-year", post(load_year_handler))
        .route("/api/jobs/active", get(active_job_handler))
        .route("/api/jobs/{id}", get(job_handler))
        .route("/api/games", get(games_handler))
        .route("/api/games/{id}", get(game_detail_handler))
        .route("/api/games/by-position/{hash}", get(games_by_position_handler))
        .route("/api/positions/hash", post(hash_from_fen_handler))
        .route("/api/positions/{hash}", get(position_handler))
        .route("/api/opening/{hash}", get(opening_handler))
        .route("/api/bench/queries", post(bench_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn health_handler(State(state): State<AppState>) -> Result<Json<HealthResponse>, ApiError> {
    let client = state.client().await?;
    let ok = health(&client).await.map_err(ApiError::internal)?;
    Ok(Json(HealthResponse {
        status: "ok",
        database_ok: ok,
    }))
}

async fn sources_handler(
    State(state): State<AppState>,
) -> Result<Json<Vec<SourceListItem>>, ApiError> {
    let client = state.client().await?;
    let sources = list_sources_fast(&client)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(sources))
}

async fn source_summary_handler(
    State(state): State<AppState>,
    Path(id): Path<i32>,
) -> Result<Json<SourceDetail>, ApiError> {
    let client = state.client().await?;
    let detail = source_detail(&client, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("source not found"))?;
    Ok(Json(detail))
}

#[derive(serde::Deserialize)]
struct FilesetsQuery {
    source_id: Option<i32>,
    source_name: Option<String>,
}

async fn filesets_handler(
    State(state): State<AppState>,
    Query(q): Query<FilesetsQuery>,
) -> Result<Json<Vec<FilesetView>>, ApiError> {
    let client = state.client().await?;
    let source_id = match (q.source_id, q.source_name) {
        (Some(id), _) => id,
        (None, Some(name)) => source_id_by_name(&client, &name)
            .await
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::not_found("source not found"))?,
        (None, None) => {
            return Err(ApiError::bad_request(
                "source_id or source_name is required",
            ));
        }
    };
    let filesets = list_filesets(&client, source_id)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(filesets))
}

async fn sync_catalog_handler(
    State(state): State<AppState>,
    Json(body): Json<SyncCatalogRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let pg_uri = state.pg_uri.clone();
    let source = body.source.clone();
    let year = body.year;
    let count = async {
        let mut session = IngestSession::connect(&pg_uri).await?;
        session.migrate().await?;
        let ids = session.sync_lichess_catalog(&source, year).await?;
        Ok::<_, anyhow::Error>(ids.len())
    }
    .await
    .map_err(ApiError::internal)?;
    Ok(Json(serde_json::json!({ "synced": count })))
}

async fn load_year_handler(
    State(state): State<AppState>,
    Json(body): Json<LoadYearRequest>,
) -> Result<Json<JobStarted>, ApiError> {
    if state.jobs.is_running() {
        if let Some(job) = state.jobs.active() {
            return Ok(Json(JobStarted { job_id: job.id }));
        }
        return Err(ApiError::bad_request("an ingest job is already running"));
    }

    let pg_uri = state.pg_uri.clone();
    let source = body.source.clone();
    let year = body.year;

    // Sync catalog before returning so sources/filesets exist immediately.
    {
        let mut session = IngestSession::connect(&pg_uri).await.map_err(ApiError::internal)?;
        session.migrate().await.map_err(ApiError::internal)?;
        session
            .sync_lichess_catalog(&source, year)
            .await
            .map_err(ApiError::internal)?;
    }

    let job_id = state.jobs.start(12, "starting fileset load");
    let cache_dir = state.cache_dir.clone();
    let jobs = state.jobs.clone();

    tokio::spawn(async move {
        let result = run_load_job(&pg_uri, &cache_dir, &source, year, &jobs).await;
        match result {
            Ok(total) => jobs.complete(format!("loaded {total} games across 12 shards")),
            Err(e) => jobs.fail(e.to_string()),
        }
    });

    Ok(Json(JobStarted { job_id }))
}

async fn run_load_job(
    pg_uri: &str,
    cache_dir: &PathBuf,
    source: &str,
    year: i32,
    jobs: &JobManager,
) -> anyhow::Result<u64> {
    let mut session = IngestSession::connect(pg_uri).await?;
    session.migrate().await?;

    let source_id = session.ensure_source(source).await?;
    let filesets = gambit_ingest::filesets::list_filesets(session.client(), source_id).await?;
    let year_prefix = format!("{year}-");
    let targets: Vec<_> = filesets
        .into_iter()
        .filter(|f| f.period_label.starts_with(&year_prefix))
        .collect();

    jobs.update(|j| j.total_shards = targets.len());

    let options = ImportOptions::default();
    let mut total_games = 0u64;

    for (i, fileset) in targets.iter().enumerate() {
        if fileset.status == "complete" {
            total_games += fileset.games_loaded as u64;
            continue;
        }

        let label = fileset.period_label.clone();
        jobs.update(|j| {
            j.current_shard = i + 1;
            j.message = format!("downloading shard {label}");
        });

        let jobs_dl = jobs.clone();
        let label_dl = label.clone();
        let download_progress: gambit_ingest::DownloadProgress = Box::new(
            move |downloaded, total| {
                let msg = match total {
                    Some(total) => format!(
                        "downloading shard {label_dl} ({:.1} / {:.1} GiB)",
                        downloaded as f64 / (1024.0 * 1024.0 * 1024.0),
                        total as f64 / (1024.0 * 1024.0 * 1024.0),
                    ),
                    None => format!(
                        "downloading shard {label_dl} ({:.1} GiB so far)",
                        downloaded as f64 / (1024.0 * 1024.0 * 1024.0),
                    ),
                };
                jobs_dl.update(|j| j.message = msg);
            },
        );

        let jobs_ing = jobs.clone();
        let label_ing = label.clone();
        let base_games = total_games;
        let ingest_progress: gambit_ingest::IngestProgress = Box::new(move |shard_games| {
            jobs_ing.update(|j| {
                j.games_loaded = base_games + shard_games as u64;
                j.message = format!(
                    "ingesting shard {label_ing} ({shard_games} games in shard so far…)"
                );
            });
        });

        jobs.update(|j| {
            j.message = format!("ingesting shard {label} (parsing PGN and loading positions…)");
        });

        let result = session
            .load_fileset_by_id(
                fileset.id,
                cache_dir,
                &options,
                &mut None,
                false,
                Some(download_progress),
                Some(ingest_progress),
            )
            .await?;

        let gpm = if result.ingest_elapsed.as_secs_f64() > 0.0 {
            Some(result.games_loaded as f64 / result.ingest_elapsed.as_secs_f64() * 60.0)
        } else {
            None
        };

        total_games += result.games_loaded as u64;
        jobs.update(|j| {
            j.games_loaded = total_games;
            j.games_per_min = gpm;
            j.message = format!("finished shard {label} · {total_games} games total");
        });
    }

    gambit_ingest::backfill_types(session.client(), source_id, &mut None).await?;
    session.refresh_stats().await?;
    info!(total_games, "fileset job complete");
    Ok(total_games)
}

#[derive(serde::Deserialize)]
struct ActiveJobQuery {
    source_name: Option<String>,
    year: Option<i32>,
}

async fn active_job_handler(
    State(state): State<AppState>,
    Query(q): Query<ActiveJobQuery>,
) -> Result<Json<Option<JobStatus>>, ApiError> {
    if let Some(job) = state.jobs.active() {
        return Ok(Json(Some(job)));
    }

    let Some(source_name) = q.source_name.filter(|s| !s.is_empty()) else {
        return Ok(Json(None));
    };
    let year = q.year.unwrap_or(2024);
    let client = state.client().await?;
    let Some(source_id) = source_id_by_name(&client, &source_name)
        .await
        .map_err(ApiError::internal)?
    else {
        return Ok(Json(None));
    };
    let job = reconstruct_job_from_filesets(&client, source_id, year)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(job))
}

async fn job_handler(
    State(state): State<AppState>,
    Path(id): Path<u64>,
) -> Result<Json<JobStatus>, ApiError> {
    state
        .jobs
        .get(id)
        .map(Json)
        .ok_or_else(|| ApiError::not_found("job not found"))
}

#[derive(serde::Deserialize)]
struct GamesQuery {
    player: Option<String>,
    source_id: Option<i32>,
    offset: Option<i64>,
    limit: Option<i64>,
}

async fn games_handler(
    State(state): State<AppState>,
    Query(q): Query<GamesQuery>,
) -> Result<Json<GamesPage>, ApiError> {
    let offset = q.offset.unwrap_or(0);
    let limit = q.limit.unwrap_or(20);
    let player = q.player.as_deref();
    let client = state.client().await?;
    let page = search_games(&client, player, q.source_id, offset, limit)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(page))
}

#[derive(serde::Deserialize)]
struct PositionGamesQuery {
    offset: Option<i64>,
    limit: Option<i64>,
}

async fn games_by_position_handler(
    State(state): State<AppState>,
    Path(hash): Path<i64>,
    Query(q): Query<PositionGamesQuery>,
) -> Result<Json<PositionGamesPage>, ApiError> {
    let client = state.client().await?;
    let page = games_by_position(&client, hash, q.offset.unwrap_or(0), q.limit.unwrap_or(20))
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(page))
}

async fn hash_from_fen_handler(
    Json(body): Json<HashFromFenRequest>,
) -> Result<Json<HashFromFenResponse>, ApiError> {
    let hash = hash_from_fen(&body.fen).map_err(ApiError::internal)?;
    Ok(Json(HashFromFenResponse { hash }))
}

async fn game_detail_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<GameDetail>, ApiError> {
    let client = state.client().await?;
    let game = get_game(&client, id)
        .await
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::not_found("game not found"))?;
    Ok(Json(game))
}

async fn position_handler(
    State(state): State<AppState>,
    Path(hash): Path<i64>,
) -> Result<Json<Vec<PositionHit>>, ApiError> {
    let client = state.client().await?;
    let hits = lookup_position(&client, hash)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(hits))
}

async fn opening_handler(
    State(state): State<AppState>,
    Path(hash): Path<i64>,
) -> Result<Json<Vec<OpeningMoveStat>>, ApiError> {
    let client = state.client().await?;
    let stats = opening_stats(&client, hash)
        .await
        .map_err(ApiError::internal)?;
    Ok(Json(stats))
}

async fn bench_handler(State(state): State<AppState>) -> Result<Json<BenchResponse>, ApiError> {
    let client = state.client().await?;
    let bench = run_bench(&client).await.map_err(ApiError::internal)?;
    Ok(Json(bench))
}
