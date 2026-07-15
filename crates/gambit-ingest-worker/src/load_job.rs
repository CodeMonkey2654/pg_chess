//! Full-year fileset load job runner.

use crate::jobs::JobManager;
use anyhow::Result;
use gambit_ingest::{
    cache_is_complete, cached_path, format_download_progress, prefetch_download, ImportOptions,
    IngestSession,
};
use std::path::Path;
use tracing::info;

/// Run a full-year load job, updating `jobs` as shards progress.
pub async fn run_load_job(
    pg_uri: &str,
    cache_dir: &Path,
    source: &str,
    year: i32,
    jobs: &JobManager,
) -> Result<u64> {
    let mut session = IngestSession::connect(pg_uri).await?;
    session.migrate().await?;

    let source_id = session.ensure_source(source).await?;
    let targets =
        gambit_ingest::filesets::filesets_for_year(session.client(), source_id, year, true).await?;

    jobs.update(|j| j.total_shards = targets.len() as u32);

    let options = ImportOptions {
        shard_concurrency: 1,
        ..ImportOptions::default()
    };
    let mut total_games = 0u64;
    let mut prefetch = None;

    let pending: Vec<_> = targets.iter().filter(|f| f.status != "complete").collect();

    for (i, fileset) in pending.iter().enumerate() {
        if let Some(next) = pending.get(i + 1) {
            let cached = cached_path(cache_dir, &next.filename);
            if !cache_is_complete(&cached, next.byte_size, next.sha256.as_deref()) {
                prefetch = Some(prefetch_download(
                    &next.remote_url,
                    &next.filename,
                    cache_dir,
                ));
            }
        }

        let shard_index = targets.iter().position(|f| f.id == fileset.id).unwrap_or(i);
        let label = fileset.period_label.clone();
        jobs.update(|j| {
            j.current_shard = (shard_index + 1) as u32;
            j.message = format!("downloading shard {label}");
        });

        let jobs_dl = jobs.clone();
        let label_dl = label.clone();
        let download_progress: gambit_ingest::DownloadProgress =
            Box::new(move |downloaded, total| {
                let msg = format_download_progress(&label_dl, downloaded, total);
                jobs_dl.update(|j| j.message = msg);
            });

        let jobs_ing = jobs.clone();
        let label_ing = label.clone();
        let base_games = total_games;
        let ingest_progress: gambit_ingest::IngestProgress = Box::new(move |shard_games| {
            jobs_ing.update(|j| {
                j.games_loaded = base_games + shard_games as u64;
                j.message =
                    format!("ingesting shard {label_ing} ({shard_games} games in shard so far…)");
            });
        });

        jobs.update(|j| {
            j.message = format!("ingesting shard {label} (parsing PGN and loading positions…)");
        });

        let result = session
            .load_fileset_by_id(
                fileset.id,
                cache_dir,
                &options,
                &mut None,
                false,
                Some(download_progress),
                Some(ingest_progress),
                prefetch.take(),
            )
            .await?;

        let gpm = if result.ingest_elapsed.as_secs_f64() > 0.0 {
            Some(result.games_loaded as f64 / result.ingest_elapsed.as_secs_f64() * 60.0)
        } else {
            None
        };

        total_games += result.games_loaded as u64;
        jobs.update(|j| {
            j.games_loaded = total_games;
            j.games_per_min = gpm;
            j.message = format!("finished shard {label} · {total_games} games total");
        });
    }

    for fileset in &targets {
        if fileset.status == "complete" {
            total_games += fileset.games_loaded as u64;
        }
    }

    session
        .finish_year_ingest(source_id, &options, &mut None)
        .await?;
    info!(total_games, "fileset job complete");
    Ok(total_games)
}
