//! Gambit Studio gRPC API server.

use gambit_proto::ingest_service_client::IngestServiceClient;
use gambit_proto::studio_service_server::StudioServiceServer;
use gambit_studio_server::{PgPool, StudioServer};
use std::env;
use std::net::SocketAddr;
use tonic::transport::{Channel, Server};
use tonic_web::GrpcWebLayer;
use tower_http::cors::{Any, CorsLayer};
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
    let ingest_addr = env::var("INGEST_ADDR").unwrap_or_else(|_| "http://127.0.0.1:8082".into());
    let addr: SocketAddr = env::var("STUDIO_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;

    let pool = PgPool::new(&pg_uri)?;
    let ingest_channel = Channel::from_shared(ingest_addr)?.connect().await?;
    let ingest_client = IngestServiceClient::new(ingest_channel);
    let studio = StudioServer::new(pool, ingest_client);
    let service = StudioServiceServer::new(studio);

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    info!(%addr, %pg_uri, "gambit-studio-server listening");
    Server::builder()
        .accept_http1(true)
        .layer(cors)
        .layer(GrpcWebLayer::new())
        .add_service(service)
        .serve(addr)
        .await?;
    Ok(())
}
