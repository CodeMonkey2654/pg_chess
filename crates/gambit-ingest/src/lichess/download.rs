//! Download Lichess shards to a local cache directory.

use anyhow::{Context, Result};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::info;

/// Optional callback invoked during download: `(bytes_received, total_bytes_if_known)`.
pub type DownloadProgress = Box<dyn FnMut(u64, Option<u64>) + Send>;

/// Optional callback invoked after each ingest batch: `(games_in_current_shard)`.
pub type IngestProgress = Box<dyn FnMut(usize) + Send>;

fn format_gib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

/// Download a remote URL to `cache_dir/filename`, returning path, size, and SHA-256.
pub async fn download_to_cache(
    url: &str,
    filename: &str,
    cache_dir: &Path,
    mut progress: Option<DownloadProgress>,
) -> Result<(PathBuf, i64, Vec<u8>)> {
    fs::create_dir_all(cache_dir)
        .await
        .with_context(|| format!("create cache dir {}", cache_dir.display()))?;

    let dest = cache_dir.join(filename);
    info!(url, path = %dest.display(), "downloading shard");

    let response = reqwest::get(url)
        .await
        .with_context(|| format!("GET {url}"))?
        .error_for_status()
        .with_context(|| format!("HTTP error for {url}"))?;

    let total = response.content_length();
    if let Some(total) = total {
        info!(total_gib = format_gib(total), "download size");
    }

    let mut stream = response.bytes_stream();
    let mut file = fs::File::create(&dest)
        .await
        .with_context(|| format!("create {}", dest.display()))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut last_progress = 0u64;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.with_context(|| format!("read chunk for {url}"))?;
        hasher.update(&chunk);
        downloaded += chunk.len() as u64;
        file.write_all(&chunk)
            .await
            .with_context(|| format!("write {}", dest.display()))?;

        if let Some(ref mut cb) = progress {
            let step = 64 * 1024 * 1024;
            if downloaded - last_progress >= step || total.is_some_and(|t| downloaded >= t) {
                cb(downloaded, total);
                last_progress = downloaded;
            }
        }
    }

    file.flush().await?;
    if let Some(ref mut cb) = progress {
        cb(downloaded, total);
    }

    let digest = hasher.finalize().to_vec();
    info!(
        path = %dest.display(),
        gib = format_gib(downloaded),
        "download complete"
    );
    Ok((dest, downloaded as i64, digest))
}

/// Resolve a cached file path if it already exists.
pub fn cached_path(cache_dir: &Path, filename: &str) -> PathBuf {
    cache_dir.join(filename)
}

/// Compute SHA-256 and size of an on-disk file.
pub async fn hash_file(path: &Path) -> Result<(i64, Vec<u8>)> {
    let bytes = fs::read(path)
        .await
        .with_context(|| format!("read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok((bytes.len() as i64, hasher.finalize().to_vec()))
}
