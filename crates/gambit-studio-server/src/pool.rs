//! PostgreSQL connection pool for read-heavy API handlers.

use anyhow::{Context, Result};
use deadpool_postgres::{Config, Pool, Runtime};
use tokio_postgres::NoTls;

/// Shared connection pool.
#[derive(Clone)]
pub struct PgPool {
    inner: Pool,
}

impl PgPool {
    /// Create a pool from a PostgreSQL URI.
    pub fn new(pg_uri: &str) -> Result<Self> {
        let mut cfg = Config::new();
        cfg.url = Some(pg_uri.to_string());
        cfg.pool = Some(deadpool_postgres::PoolConfig {
            max_size: 16,
            ..Default::default()
        });
        let pool = cfg
            .create_pool(Some(Runtime::Tokio1), NoTls)
            .context("create postgres pool")?;
        Ok(Self { inner: pool })
    }

    /// Check out one connection from the pool.
    pub async fn get(&self) -> Result<deadpool_postgres::Client> {
        self.inner.get().await.context("pool checkout")
    }
}
