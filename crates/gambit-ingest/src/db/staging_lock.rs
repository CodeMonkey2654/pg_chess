//! Serialize access to global UNLOGGED staging tables across connections.

use anyhow::{Context, Result};
use tokio_postgres::Client;

/// Advisory lock key for gambit ingest staging (`gambit` + `01`).
const STAGING_LOCK_KEY: i64 = 0x6761_6D62_6974_0001;

/// Acquire the global staging advisory lock (blocks until available).
pub async fn acquire_staging_lock(client: &Client) -> Result<()> {
    client
        .query_one("SELECT pg_advisory_lock($1)", &[&STAGING_LOCK_KEY])
        .await
        .context("acquire staging advisory lock")?;
    Ok(())
}

/// Release the global staging advisory lock.
pub async fn release_staging_lock(client: &Client) -> Result<()> {
    client
        .query_one("SELECT pg_advisory_unlock($1)", &[&STAGING_LOCK_KEY])
        .await
        .context("release staging advisory lock")?;
    Ok(())
}
