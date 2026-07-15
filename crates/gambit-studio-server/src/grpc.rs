//! gRPC StudioService implementation.

use crate::db::{
    games_by_position, get_game, get_position_eval, hash_from_fen, health, list_filesets,
    list_sources_fast, lookup_position, opening_stats, run_analyze_game, run_bench, search_games,
    source_detail, source_id_by_name,
};
use crate::pool::PgPool;
use gambit_proto::ingest_service_client::IngestServiceClient;
use gambit_proto::studio_service_server::StudioService;
use gambit_proto::{
    AnalyzeGameRequest, AnalyzeGameResponse, BenchResponse, Empty, GameDetail, GamesPage,
    GetActiveJobRequest, GetGameRequest, GetJobRequest, GetPositionEvalRequest, GetSourceSummaryRequest,
    HashFromFenRequest, HashFromFenResponse, HealthResponse, JobStarted, JobStatus,
    ListFilesetsRequest, ListFilesetsResponse, ListSourcesResponse, LoadYearRequest,
    LookupPositionRequest, LookupPositionResponse, OpeningStatsRequest, OpeningStatsResponse,
    OptionalJobStatus, PositionEvalResponse, PositionGamesPage, SearchGamesRequest, SourceDetail,
    SyncCatalogRequest, SyncCatalogResponse, WatchJobRequest,
};
use std::pin::Pin;
use std::sync::Arc;
use tokio_stream::{Stream, StreamExt};
use tonic::transport::Channel;
use tonic::{Request, Response, Status};

/// Shared studio server state.
#[derive(Clone)]
pub struct StudioServer {
    pool: Arc<PgPool>,
    ingest: IngestServiceClient<Channel>,
}

impl StudioServer {
    /// Build server state from a DB pool and ingest worker channel.
    pub fn new(pool: PgPool, ingest: IngestServiceClient<Channel>) -> Self {
        Self {
            pool: Arc::new(pool),
            ingest,
        }
    }

    async fn client(&self) -> Result<deadpool_postgres::Client, Status> {
        self.pool
            .get()
            .await
            .map_err(|e| Status::internal(e.to_string()))
    }
}

#[tonic::async_trait]
impl StudioService for StudioServer {
    type WatchJobStream = Pin<Box<dyn Stream<Item = Result<JobStatus, Status>> + Send + 'static>>;

    async fn health(&self, _request: Request<Empty>) -> Result<Response<HealthResponse>, Status> {
        let client = self.client().await?;
        let ok = health(&client)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(HealthResponse {
            status: "ok".to_string(),
            database_ok: ok,
        }))
    }

    async fn list_sources(
        &self,
        _request: Request<Empty>,
    ) -> Result<Response<ListSourcesResponse>, Status> {
        let client = self.client().await?;
        let sources = list_sources_fast(&client)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(ListSourcesResponse { sources }))
    }

    async fn get_source_summary(
        &self,
        request: Request<GetSourceSummaryRequest>,
    ) -> Result<Response<SourceDetail>, Status> {
        let id = request.into_inner().source_id;
        let client = self.client().await?;
        let detail = source_detail(&client, id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("source not found"))?;
        Ok(Response::new(detail))
    }

    async fn list_filesets(
        &self,
        request: Request<ListFilesetsRequest>,
    ) -> Result<Response<ListFilesetsResponse>, Status> {
        let req = request.into_inner();
        let client = self.client().await?;
        let source_id = match (req.source_id, req.source_name) {
            (Some(id), _) => id,
            (None, Some(name)) => source_id_by_name(&client, &name)
                .await
                .map_err(|e| Status::internal(e.to_string()))?
                .ok_or_else(|| Status::not_found("source not found"))?,
            (None, None) => {
                return Err(Status::invalid_argument(
                    "source_id or source_name is required",
                ));
            }
        };
        let filesets = list_filesets(&client, source_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(ListFilesetsResponse { filesets }))
    }

    async fn search_games(
        &self,
        request: Request<SearchGamesRequest>,
    ) -> Result<Response<GamesPage>, Status> {
        let req = request.into_inner();
        let client = self.client().await?;
        let page = search_games(
            &client,
            req.player.as_deref(),
            req.source_id,
            req.offset,
            req.limit,
            req.include_total.unwrap_or(false),
            req.cursor.as_deref(),
        )
        .await
        .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(page))
    }

    async fn get_game(
        &self,
        request: Request<GetGameRequest>,
    ) -> Result<Response<GameDetail>, Status> {
        let req = request.into_inner();
        let client = self.client().await?;
        let game = get_game(&client, req.game_id, req.max_plies)
            .await
            .map_err(|e| Status::internal(e.to_string()))?
            .ok_or_else(|| Status::not_found("game not found"))?;
        Ok(Response::new(game))
    }

    async fn games_by_position(
        &self,
        request: Request<gambit_proto::GamesByPositionRequest>,
    ) -> Result<Response<PositionGamesPage>, Status> {
        let req = request.into_inner();
        let client = self.client().await?;
        let page = games_by_position(
            &client,
            req.hash,
            req.source_id,
            req.offset,
            req.limit,
            req.cursor.as_deref(),
        )
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(page))
    }

    async fn hash_from_fen(
        &self,
        request: Request<HashFromFenRequest>,
    ) -> Result<Response<HashFromFenResponse>, Status> {
        let fen = request.into_inner().fen;
        let hash = hash_from_fen(&fen).map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(HashFromFenResponse { hash }))
    }

    async fn lookup_position(
        &self,
        request: Request<LookupPositionRequest>,
    ) -> Result<Response<LookupPositionResponse>, Status> {
        let hash = request.into_inner().hash;
        let client = self.client().await?;
        let hits = lookup_position(&client, hash)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(LookupPositionResponse { hits }))
    }

    async fn opening_stats(
        &self,
        request: Request<OpeningStatsRequest>,
    ) -> Result<Response<OpeningStatsResponse>, Status> {
        let hash = request.into_inner().hash;
        let client = self.client().await?;
        let stats = opening_stats(&client, hash)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(OpeningStatsResponse { stats }))
    }

    async fn run_bench(&self, _request: Request<Empty>) -> Result<Response<BenchResponse>, Status> {
        let client = self.client().await?;
        let bench = run_bench(&client)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(bench))
    }

    async fn analyze_game(
        &self,
        request: Request<AnalyzeGameRequest>,
    ) -> Result<Response<AnalyzeGameResponse>, Status> {
        let req = request.into_inner();
        let depth = if req.depth == 0 { 12 } else { req.depth };
        let client = self.client().await?;
        let resp = run_analyze_game(&client, req.game_id, depth)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }

    async fn get_position_eval(
        &self,
        request: Request<GetPositionEvalRequest>,
    ) -> Result<Response<PositionEvalResponse>, Status> {
        let req = request.into_inner();
        let depth = if req.depth == 0 { 10 } else { req.depth as u32 };
        let profile_id = if req.profile_id == 0 {
            1_i16
        } else {
            req.profile_id as i16
        };
        let hash = if req.hash != 0 {
            req.hash
        } else {
            hash_from_fen(&req.fen).map_err(|e| Status::invalid_argument(e.to_string()))?
        };
        let client = self.client().await?;
        let resp = get_position_eval(&client, &req.fen, hash, depth, profile_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(Response::new(resp))
    }

    async fn sync_catalog(
        &self,
        request: Request<SyncCatalogRequest>,
    ) -> Result<Response<SyncCatalogResponse>, Status> {
        let mut client = self.ingest.clone();
        let resp = client
            .sync_catalog(request)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;
        Ok(resp)
    }

    async fn load_year(
        &self,
        request: Request<LoadYearRequest>,
    ) -> Result<Response<JobStarted>, Status> {
        let mut client = self.ingest.clone();
        let resp = client.load_year(request).await.map_err(map_ingest_status)?;
        Ok(resp)
    }

    async fn get_job(
        &self,
        request: Request<GetJobRequest>,
    ) -> Result<Response<JobStatus>, Status> {
        let mut client = self.ingest.clone();
        let resp = client.get_job(request).await.map_err(map_ingest_status)?;
        Ok(resp)
    }

    async fn get_active_job(
        &self,
        request: Request<GetActiveJobRequest>,
    ) -> Result<Response<OptionalJobStatus>, Status> {
        let mut client = self.ingest.clone();
        let resp = client
            .get_active_job(request)
            .await
            .map_err(map_ingest_status)?;
        Ok(resp)
    }

    async fn watch_job(
        &self,
        request: Request<WatchJobRequest>,
    ) -> Result<Response<Self::WatchJobStream>, Status> {
        let mut client = self.ingest.clone();
        let response = client.watch_job(request).await.map_err(map_ingest_status)?;
        #[allow(clippy::result_large_err)]
        let stream = response
            .into_inner()
            .map(|item| item.map_err(|e| Status::internal(e.to_string())));
        Ok(Response::new(Box::pin(stream)))
    }
}

fn map_ingest_status(err: tonic::Status) -> Status {
    Status::new(err.code(), err.message())
}
