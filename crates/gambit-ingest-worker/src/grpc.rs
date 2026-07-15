//! gRPC IngestService implementation.

use crate::db::{connect_client, reconstruct_job_from_filesets, source_id_by_name};
use crate::jobs::JobManager;
use crate::load_job::run_load_job;
use gambit_ingest::{ImportOptions, IngestSession};
use gambit_proto::ingest_service_server::IngestService;
use gambit_proto::{
    GetActiveJobRequest, GetJobRequest, JobStarted, JobStatus, LoadFilesetRequest,
    LoadFilesetResponse, LoadYearRequest, OptionalJobStatus, SyncCatalogRequest,
    SyncCatalogResponse, WatchJobRequest,
};
use std::path::PathBuf;
use std::pin::Pin;
use tokio::sync::broadcast::error::RecvError;
use tokio_stream::Stream;
use tonic::{Request, Response, Status};

/// Shared ingest worker state.
#[derive(Clone)]
pub struct IngestWorker {
    pg_uri: String,
    cache_dir: PathBuf,
    jobs: JobManager,
}

impl IngestWorker {
    /// Build worker state from config.
    pub fn new(pg_uri: String, cache_dir: PathBuf) -> Self {
        Self {
            pg_uri,
            cache_dir,
            jobs: JobManager::new(),
        }
    }
}

#[tonic::async_trait]
impl IngestService for IngestWorker {
    type WatchJobStream = Pin<Box<dyn Stream<Item = Result<JobStatus, Status>> + Send + 'static>>;

    async fn sync_catalog(
        &self,
        request: Request<SyncCatalogRequest>,
    ) -> Result<Response<SyncCatalogResponse>, Status> {
        let req = request.into_inner();
        let pg_uri = self.pg_uri.clone();
        let source = req.source;
        let year = req.year;

        let count = async {
            let mut session = IngestSession::connect(&pg_uri)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            session
                .migrate()
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            let ids = session
                .sync_lichess_catalog(&source, year)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            Ok::<_, Status>(ids.len() as u32)
        }
        .await?;

        Ok(Response::new(SyncCatalogResponse { synced: count }))
    }

    async fn load_year(
        &self,
        request: Request<LoadYearRequest>,
    ) -> Result<Response<JobStarted>, Status> {
        if self.jobs.is_running() {
            if let Some(job) = self.jobs.active() {
                return Ok(Response::new(JobStarted { job_id: job.id }));
            }
            return Err(Status::failed_precondition(
                "an ingest job is already running",
            ));
        }

        let req = request.into_inner();
        let pg_uri = self.pg_uri.clone();
        let source = req.source.clone();
        let year = req.year;

        {
            let mut session = IngestSession::connect(&pg_uri)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            session
                .migrate()
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            session
                .sync_lichess_catalog(&source, year)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
        }

        let job_id = self.jobs.start(12, "starting fileset load");
        let cache_dir = self.cache_dir.clone();
        let jobs = self.jobs.clone();
        let pg = pg_uri.clone();
        let src = source.clone();

        tokio::spawn(async move {
            let result = run_load_job(&pg, &cache_dir, &src, year, &jobs).await;
            match result {
                Ok(total) => jobs.complete(format!("loaded {total} games across 12 shards")),
                Err(e) => jobs.fail(e.to_string()),
            }
        });

        Ok(Response::new(JobStarted { job_id }))
    }

    async fn load_fileset(
        &self,
        request: Request<LoadFilesetRequest>,
    ) -> Result<Response<LoadFilesetResponse>, Status> {
        let req = request.into_inner();
        let mut session = IngestSession::connect(&self.pg_uri)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        session
            .migrate()
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        let options = ImportOptions {
            workers: req.workers as usize,
            batch_games: req.batch_games as usize,
            store_pgn: false,
            fail_fast: false,
            eager_types: false,
            shard_sha256: None,
            ..ImportOptions::default()
        };
        let cache_dir = PathBuf::from(req.cache_dir);

        if let Some(id) = req.fileset_id {
            let result = session
                .load_fileset_by_id(id, &cache_dir, &options, &mut None, true, None, None, None)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            return Ok(Response::new(LoadFilesetResponse {
                shards_loaded: 1,
                total_games: result.games_loaded as u64,
            }));
        }

        session
            .sync_lichess_catalog(&req.source, req.year)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let results = session
            .load_fileset_year(&req.source, req.year, &cache_dir, &options, &mut None, false)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let total_games: u64 = results.iter().map(|r| r.games_loaded as u64).sum();

        Ok(Response::new(LoadFilesetResponse {
            shards_loaded: results.len() as u32,
            total_games,
        }))
    }

    async fn get_job(
        &self,
        request: Request<GetJobRequest>,
    ) -> Result<Response<JobStatus>, Status> {
        let job_id = request.into_inner().job_id;
        self.jobs
            .get(job_id)
            .map(Response::new)
            .ok_or_else(|| Status::not_found("job not found"))
    }

    async fn get_active_job(
        &self,
        request: Request<GetActiveJobRequest>,
    ) -> Result<Response<OptionalJobStatus>, Status> {
        if let Some(job) = self.jobs.active() {
            return Ok(Response::new(OptionalJobStatus { job: Some(job) }));
        }

        let req = request.into_inner();
        let Some(source_name) = req.source_name.filter(|s| !s.is_empty()) else {
            return Ok(Response::new(OptionalJobStatus { job: None }));
        };
        let year = req.year.unwrap_or(2024);

        let client = connect_client(&self.pg_uri)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        let Some(source_id) = source_id_by_name(&client, &source_name)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
        else {
            return Ok(Response::new(OptionalJobStatus { job: None }));
        };
        let job = reconstruct_job_from_filesets(&client, source_id, year)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(OptionalJobStatus { job }))
    }

    async fn watch_job(
        &self,
        request: Request<WatchJobRequest>,
    ) -> Result<Response<Self::WatchJobStream>, Status> {
        let req = request.into_inner();

        let initial = if req.job_id > 0 {
            self.jobs.get(req.job_id)
        } else if let (Some(name), year) = (req.source_name, req.year) {
            let client = connect_client(&self.pg_uri)
                .await
                .map_err(|e| Status::internal(e.to_string()))?;
            if let Some(source_id) = source_id_by_name(&client, &name)
                .await
                .map_err(|e| Status::internal(e.to_string()))?
            {
                reconstruct_job_from_filesets(&client, source_id, year.unwrap_or(2024))
                    .await
                    .map_err(|e| Status::internal(e.to_string()))?
            } else {
                None
            }
        } else {
            self.jobs.active()
        };

        let Some(start) = initial else {
            return Err(Status::not_found("job not found"));
        };

        let mut rx = self.jobs.subscribe();
        let start_done = start.status == "complete" || start.status == "failed";

        let stream = async_stream::try_stream! {
            yield start;
            if start_done {
                return;
            }
            loop {
                match rx.recv().await {
                    Ok(status) => {
                        let done = status.status == "complete" || status.status == "failed";
                        yield status;
                        if done {
                            break;
                        }
                    }
                    Err(RecvError::Lagged(_)) => continue,
                    Err(RecvError::Closed) => break,
                }
            }
        };

        Ok(Response::new(Box::pin(stream)))
    }
}
