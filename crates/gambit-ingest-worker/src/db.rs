//! PostgreSQL helpers for job reconstruction.

use anyhow::{Context, Result};
use gambit_proto::JobStatus;
use tokio_postgres::Client;

/// Resolve source id by name.
pub async fn source_id_by_name(client: &Client, name: &str) -> Result<Option<i32>> {
    let rows = client
        .query("SELECT id FROM gambit.sources WHERE name = $1", &[&name])
        .await?;
    Ok(rows.first().map(|r| r.get(0)))
}

/// Reconstruct job status from persisted fileset rows.
pub async fn reconstruct_job_from_filesets(
    client: &Client,
    source_id: i32,
    year: i32,
) -> Result<Option<JobStatus>> {
    let rows = client
        .query(
            "SELECT period_label, status, games_loaded
             FROM gambit.filesets
             WHERE source_id = $1
             ORDER BY period_label",
            &[&source_id],
        )
        .await?;

    let year_prefix = format!("{year}-");
    let shards: Vec<_> = rows
        .iter()
        .filter(|r| {
            let label: String = r.get(0);
            label.starts_with(&year_prefix)
        })
        .collect();

    if shards.is_empty() {
        return Ok(None);
    }

    let total_shards = shards.len();
    let games_loaded: i64 = shards.iter().map(|r| r.get::<_, i64>(2)).sum();
    let complete = shards
        .iter()
        .filter(|r| {
            let status: String = r.get(1);
            status == "complete"
        })
        .count();

    let active = shards.iter().find(|r| {
        let status: String = r.get(1);
        status == "downloading" || status == "ingesting"
    });

    let failed = shards.iter().find(|r| {
        let status: String = r.get(1);
        status == "failed"
    });

    if complete == total_shards {
        return Ok(Some(JobStatus {
            id: 0,
            status: "complete".to_string(),
            message: format!("loaded {games_loaded} games across {total_shards} shards"),
            current_shard: total_shards as u32,
            total_shards: total_shards as u32,
            games_loaded: games_loaded as u64,
            games_per_min: None,
        }));
    }

    if let Some(f) = failed {
        let label: String = f.get(0);
        let games: i64 = f.get(2);
        return Ok(Some(JobStatus {
            id: 0,
            status: "failed".to_string(),
            message: format!("shard {label} failed after {games} games"),
            current_shard: (complete + 1) as u32,
            total_shards: total_shards as u32,
            games_loaded: games_loaded as u64,
            games_per_min: None,
        }));
    }

    let (current_shard, message) = if let Some(a) = active {
        let label: String = a.get(0);
        let status: String = a.get(1);
        let games: i64 = a.get(2);
        (
            complete + 1,
            format!("{status} shard {label} ({games} games in shard)"),
        )
    } else {
        (
            complete + 1,
            format!("{complete}/{total_shards} shards complete · resuming…"),
        )
    };

    Ok(Some(JobStatus {
        id: 0,
        status: "running".to_string(),
        message,
        current_shard: current_shard as u32,
        total_shards: total_shards as u32,
        games_loaded: games_loaded as u64,
        games_per_min: None,
    }))
}

/// Connect a single postgres client for ingest operations.
pub async fn connect_client(pg_uri: &str) -> Result<Client> {
    let (client, connection) = tokio_postgres::connect(pg_uri, tokio_postgres::NoTls)
        .await
        .context("connect to postgres")?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("postgres connection error: {e}");
        }
    });
    Ok(client)
}
