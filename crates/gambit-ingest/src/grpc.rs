//! gRPC client helpers for the ingest worker.

use gambit_proto::ingest_service_client::IngestServiceClient;
use gambit_proto::{LoadFilesetRequest, SyncCatalogRequest};
use tonic::transport::Channel;

/// Connect to the ingest worker.
pub async fn connect(ingest_addr: &str) -> anyhow::Result<IngestServiceClient<Channel>> {
    let channel = Channel::from_shared(ingest_addr.to_string())?
        .connect()
        .await?;
    Ok(IngestServiceClient::new(channel))
}

/// Sync Lichess catalog via the ingest worker.
pub async fn sync_catalog(
    client: &mut IngestServiceClient<Channel>,
    source: &str,
    year: i32,
) -> anyhow::Result<u32> {
    let resp = client
        .sync_catalog(SyncCatalogRequest {
            source: source.to_string(),
            year,
        })
        .await?
        .into_inner();
    Ok(resp.synced)
}

/// Load filesets via the ingest worker.
pub async fn load_fileset(
    client: &mut IngestServiceClient<Channel>,
    source: &str,
    year: i32,
    cache_dir: &std::path::Path,
    workers: usize,
    batch_games: usize,
    fileset_id: Option<i64>,
) -> anyhow::Result<(u32, u64)> {
    let resp = client
        .load_fileset(LoadFilesetRequest {
            source: source.to_string(),
            year,
            cache_dir: cache_dir.display().to_string(),
            workers: workers as u32,
            batch_games: batch_games as u32,
            fileset_id,
        })
        .await?
        .into_inner();
    Ok((resp.shards_loaded, resp.total_games))
}
