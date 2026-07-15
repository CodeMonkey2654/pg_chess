//! Fileset and ingest run tracking.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use tokio_postgres::Client;

/// Row from `gambit.filesets`.
#[derive(Debug, Clone)]
pub struct FilesetRow {
    /// Primary key.
    pub id: i64,
    /// Owning source id.
    pub source_id: i32,
    /// Remote download URL.
    pub remote_url: String,
    /// Archive filename.
    pub filename: String,
    /// Period label (e.g. `2024-03`).
    pub period_label: String,
    /// Compressed byte size when known.
    pub byte_size: Option<i64>,
    /// SHA-256 of downloaded file.
    pub sha256: Option<Vec<u8>>,
    /// Lifecycle status.
    pub status: String,
    /// Games successfully loaded.
    pub games_loaded: i64,
    /// Parse errors during ingest.
    pub games_errors: i64,
    /// Positions loaded.
    pub positions_loaded: i64,
    /// Plies loaded.
    pub plies_loaded: i64,
    /// Last error message if failed.
    pub error_message: Option<String>,
}

/// Upsert a catalog entry into `gambit.filesets`.
pub async fn upsert_fileset(
    client: &Client,
    source_id: i32,
    remote_url: &str,
    filename: &str,
    period_label: &str,
) -> Result<i64> {
    let row = client
        .query_one(
            "INSERT INTO gambit.filesets (source_id, remote_url, filename, period_label)
             VALUES ($1, $2, $3, $4)
             ON CONFLICT (remote_url) DO UPDATE
             SET source_id = EXCLUDED.source_id,
                 filename = EXCLUDED.filename,
                 period_label = EXCLUDED.period_label
             RETURNING id",
            &[&source_id, &remote_url, &filename, &period_label],
        )
        .await
        .context("upsert fileset")?;
    Ok(row.get(0))
}

/// List filesets for a source, ordered by period.
pub async fn list_filesets(client: &Client, source_id: i32) -> Result<Vec<FilesetRow>> {
    let rows = client
        .query(
            "SELECT id, source_id, remote_url, filename, period_label, byte_size, sha256,
                    status, games_loaded, games_errors, positions_loaded, plies_loaded, error_message
             FROM gambit.filesets
             WHERE source_id = $1
             ORDER BY period_label",
            &[&source_id],
        )
        .await
        .context("list filesets")?;

    Ok(rows.iter().map(map_fileset_row).collect())
}

/// Load filesets for a calendar year, skipping complete unless retrying failures.
pub async fn filesets_for_year(
    client: &Client,
    source_id: i32,
    year: i32,
    skip_complete: bool,
) -> Result<Vec<FilesetRow>> {
    let year_prefix = format!("{year}-");
    Ok(list_filesets(client, source_id)
        .await?
        .into_iter()
        .filter(|f| f.period_label.starts_with(&year_prefix))
        .filter(|f| !(skip_complete && f.status == "complete"))
        .collect())
}

/// Fetch one fileset by id.
pub async fn get_fileset(client: &Client, fileset_id: i64) -> Result<Option<FilesetRow>> {
    let rows = client
        .query(
            "SELECT id, source_id, remote_url, filename, period_label, byte_size, sha256,
                    status, games_loaded, games_errors, positions_loaded, plies_loaded, error_message
             FROM gambit.filesets
             WHERE id = $1",
            &[&fileset_id],
        )
        .await
        .context("get fileset")?;
    Ok(rows.first().map(map_fileset_row))
}

/// Mark fileset download started.
pub async fn mark_download_started(client: &Client, fileset_id: i64) -> Result<()> {
    client
        .execute(
            "UPDATE gambit.filesets
             SET status = 'downloading',
                 download_started_at = now(),
                 error_message = NULL
             WHERE id = $1",
            &[&fileset_id],
        )
        .await
        .context("mark download started")?;
    Ok(())
}

/// Mark fileset download complete.
pub async fn mark_download_complete(
    client: &Client,
    fileset_id: i64,
    byte_size: i64,
    sha256: &[u8],
) -> Result<()> {
    client
        .execute(
            "UPDATE gambit.filesets
             SET status = 'downloaded',
                 download_completed_at = now(),
                 byte_size = $2,
                 sha256 = $3
             WHERE id = $1",
            &[&fileset_id, &byte_size, &sha256],
        )
        .await
        .context("mark download complete")?;
    Ok(())
}

/// Mark fileset ingest started.
pub async fn mark_ingest_started(client: &Client, fileset_id: i64) -> Result<()> {
    client
        .execute(
            "UPDATE gambit.filesets
             SET status = 'ingesting',
                 ingest_started_at = now(),
                 error_message = NULL
             WHERE id = $1",
            &[&fileset_id],
        )
        .await
        .context("mark ingest started")?;
    Ok(())
}

/// Mark fileset ingest complete with metrics.
pub async fn mark_ingest_complete(
    client: &Client,
    fileset_id: i64,
    games_loaded: i64,
    games_errors: i64,
    positions_loaded: i64,
    plies_loaded: i64,
) -> Result<()> {
    client
        .execute(
            "UPDATE gambit.filesets
             SET status = 'complete',
                 ingest_completed_at = now(),
                 games_loaded = $2,
                 games_errors = $3,
                 positions_loaded = $4,
                 plies_loaded = $5
             WHERE id = $1",
            &[
                &fileset_id,
                &games_loaded,
                &games_errors,
                &positions_loaded,
                &plies_loaded,
            ],
        )
        .await
        .context("mark ingest complete")?;
    Ok(())
}

/// Mark fileset failed with an error message.
pub async fn mark_failed(client: &Client, fileset_id: i64, message: &str) -> Result<()> {
    client
        .execute(
            "UPDATE gambit.filesets
             SET status = 'failed',
                 error_message = $2
             WHERE id = $1",
            &[&fileset_id, &message],
        )
        .await
        .context("mark fileset failed")?;
    Ok(())
}

/// Record an ingest run for performance tracking.
pub async fn record_ingest_run(
    client: &Client,
    fileset_id: i64,
    source_id: i32,
    workers: i32,
    batch_games: i32,
    games_loaded: i64,
    positions_loaded: i64,
    wall_seconds: f64,
) -> Result<i64> {
    let games_per_min = if wall_seconds > 0.0 {
        games_loaded as f64 / wall_seconds * 60.0
    } else {
        0.0
    };
    let positions_per_sec = if wall_seconds > 0.0 {
        positions_loaded as f64 / wall_seconds
    } else {
        0.0
    };

    let row = client
        .query_one(
            "INSERT INTO gambit.ingest_runs (
                fileset_id, source_id, finished_at, workers, batch_games,
                games_per_min, positions_per_sec, wall_seconds
             ) VALUES ($1, $2, now(), $3, $4, $5, $6, $7)
             RETURNING id",
            &[
                &fileset_id,
                &source_id,
                &workers,
                &batch_games,
                &games_per_min,
                &positions_per_sec,
                &wall_seconds,
            ],
        )
        .await
        .context("record ingest run")?;
    Ok(row.get(0))
}

fn map_fileset_row(row: &tokio_postgres::Row) -> FilesetRow {
    FilesetRow {
        id: row.get(0),
        source_id: row.get(1),
        remote_url: row.get(2),
        filename: row.get(3),
        period_label: row.get(4),
        byte_size: row.get(5),
        sha256: row.get(6),
        status: row.get(7),
        games_loaded: row.get(8),
        games_errors: row.get(9),
        positions_loaded: row.get(10),
        plies_loaded: row.get(11),
        error_message: row.get(12),
    }
}

/// Timestamp helper for API responses.
pub type Timestamp = DateTime<Utc>;
