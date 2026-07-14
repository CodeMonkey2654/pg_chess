//! Gambit Studio REST API server.

use axum::Router;
use gambit_studio_server::{router, AppState, PgPool};
use std::env;
use std::net::SocketAddr;
use std::path::PathBuf;
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
    let addr: SocketAddr = env::var("STUDIO_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".into())
        .parse()?;

    let pool = PgPool::new(&pg_uri)?;
    let state = AppState::new(pool, pg_uri.clone(), cache_dir);
    let app: Router = router(state);

    info!(%addr, %pg_uri, "gambit-studio-server listening");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
