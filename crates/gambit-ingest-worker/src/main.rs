//! gRPC ingest worker for Gambit Studio.

use gambit_ingest_worker::IngestWorker;
use gambit_proto::ingest_service_server::IngestServiceServer;
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let pg_uri = env::var("DATABASE_URL").unwrap_or_else(|_| {
        format!(
            "postgres://{}@127.0.0.1:28818/postgres",
            env::var("USERNAME").unwrap_or_else(|_| "postgres".into())
        )
    });
    let cache_dir = env::var("GAMBIT_CACHE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(".cache/lichess"));
    let addr: SocketAddr = env::var("INGEST_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8082".into())
        .parse()?;

    let worker = IngestWorker::new(pg_uri.clone(), cache_dir);
    let service = IngestServiceServer::new(worker);

    info!(%addr, %pg_uri, "gambit-ingest-worker listening");
    Server::builder().add_service(service).serve(addr).await?;
    Ok(())
}
