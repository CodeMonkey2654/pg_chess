//! Download Lichess shards to a local cache directory.

use anyhow::{Context, Result};
use futures::StreamExt;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::task::JoinHandle;
use tracing::{info, warn};

/// Optional callback invoked during download: `(bytes_received, total_bytes_if_known)`.
pub type DownloadProgress = Box<dyn FnMut(u64, Option<u64>) + Send>;

/// Optional callback invoked after each ingest batch: `(games_in_current_shard)`.
pub type IngestProgress = Box<dyn FnMut(usize) + Send>;

/// Background download task handle.
pub type PrefetchHandle = JoinHandle<Result<(PathBuf, i64, Vec<u8>)>>;

/// Lichess monthly shards are ~27–30 GiB compressed; smaller files are incomplete.
pub const MIN_COMPLETE_SHARD_BYTES: u64 = 5_000_000_000;

const MAX_DOWNLOAD_ATTEMPTS: u32 = 5;

/// Start downloading a shard in the background (no progress callback).
pub fn prefetch_download(url: &str, filename: &str, cache_dir: &Path) -> PrefetchHandle {
    let url = url.to_string();
    let filename = filename.to_string();
    let cache_dir = cache_dir.to_path_buf();
    tokio::spawn(
        async move { download_to_cache_with_retries(&url, &filename, &cache_dir, None).await },
    )
}

fn format_gib(bytes: u64) -> f64 {
    bytes as f64 / (1024.0 * 1024.0 * 1024.0)
}

/// True when an on-disk cache file looks complete for ingest.
pub fn cache_is_complete(
    path: &Path,
    known_byte_size: Option<i64>,
    known_sha256: Option<&[u8]>,
) -> bool {
    let Ok(meta) = std::fs::metadata(path) else {
        return false;
    };
    let size = meta.len();

    if let Some(expected) = known_byte_size {
        if size != expected as u64 {
            return false;
        }
    } else if size < MIN_COMPLETE_SHARD_BYTES {
        return false;
    }

    if let Some(expected) = known_sha256 {
        if let Ok(actual) = hash_file_sync(path) {
            return actual.1 == expected;
        }
        return false;
    }

    true
}

/// Download with retries and exponential backoff.
pub async fn download_to_cache_with_retries(
    url: &str,
    filename: &str,
    cache_dir: &Path,
    mut progress: Option<DownloadProgress>,
) -> Result<(PathBuf, i64, Vec<u8>)> {
    let mut last_err = None;
    for attempt in 1..=MAX_DOWNLOAD_ATTEMPTS {
        match download_to_cache(url, filename, cache_dir, progress.as_mut()).await {
            Ok(v) => return Ok(v),
            Err(e) => {
                warn!(
                    url,
                    filename,
                    attempt,
                    max = MAX_DOWNLOAD_ATTEMPTS,
                    error = %e,
                    "shard download failed"
                );
                last_err = Some(e);
                if attempt < MAX_DOWNLOAD_ATTEMPTS {
                    let delay = Duration::from_secs(2u64.pow(attempt));
                    tokio::time::sleep(delay).await;
                }
            }
        }
    }
    Err(last_err.unwrap_or_else(|| anyhow::anyhow!("download failed with no error")))
}

/// Download a remote URL to `cache_dir/filename`, returning path, size, and SHA-256.
pub async fn download_to_cache(
    url: &str,
    filename: &str,
    cache_dir: &Path,
    mut progress: Option<&mut DownloadProgress>,
) -> Result<(PathBuf, i64, Vec<u8>)> {
    fs::create_dir_all(cache_dir)
        .await
        .with_context(|| format!("create cache dir {}", cache_dir.display()))?;

    let dest = cache_dir.join(filename);
    let part = part_path(&dest);
    if part.exists() {
        fs::remove_file(&part)
            .await
            .with_context(|| format!("remove stale partial {}", part.display()))?;
    }

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
    let mut file = fs::File::create(&part)
        .await
        .with_context(|| format!("create {}", part.display()))?;
    let mut hasher = Sha256::new();
    let mut downloaded = 0u64;
    let mut last_progress = 0u64;

    let write_result: Result<()> = async {
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.with_context(|| format!("read chunk for {url}"))?;
            hasher.update(&chunk);
            downloaded += chunk.len() as u64;
            file.write_all(&chunk)
                .await
                .with_context(|| format!("write {}", part.display()))?;

            if let Some(cb) = progress.as_mut() {
                let step = 64 * 1024 * 1024;
                if downloaded - last_progress >= step || total.is_some_and(|t| downloaded >= t) {
                    cb(downloaded, total);
                    last_progress = downloaded;
                }
            }
        }
        file.flush().await.context("flush partial download")?;
        Ok(())
    }
    .await;

    if let Err(e) = write_result {
        let _ = fs::remove_file(&part).await;
        return Err(e);
    }

    if let Some(expected) = total {
        if downloaded != expected {
            let _ = fs::remove_file(&part).await;
            anyhow::bail!(
                "download incomplete for {url}: got {} bytes, expected {expected}",
                downloaded
            );
        }
    } else if downloaded < MIN_COMPLETE_SHARD_BYTES {
        let _ = fs::remove_file(&part).await;
        anyhow::bail!(
            "download suspiciously small for {url}: {} bytes",
            downloaded
        );
    }

    fs::rename(&part, &dest)
        .await
        .with_context(|| format!("rename {} -> {}", part.display(), dest.display()))?;

    if let Some(cb) = progress.as_mut() {
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

fn part_path(dest: &Path) -> PathBuf {
    let name = dest.file_name().and_then(|n| n.to_str()).unwrap_or("shard");
    dest.with_file_name(format!("{name}.part"))
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

fn hash_file_sync(path: &Path) -> Result<(i64, Vec<u8>)> {
    let bytes = std::fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok((bytes.len() as i64, hasher.finalize().to_vec()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_shard_bytes_threshold() {
        const { assert!(MIN_COMPLETE_SHARD_BYTES > 1_000_000_000) };
    }

    #[test]
    fn cache_complete_requires_size_when_unknown() {
        let dir = std::env::temp_dir().join("gambit_cache_test");
        let _ = std::fs::create_dir_all(&dir);
        let tiny = dir.join("tiny.zst");
        std::fs::write(&tiny, vec![0u8; 1024]).expect("write");
        assert!(!cache_is_complete(&tiny, None, None));
        let _ = std::fs::remove_file(&tiny);
    }
}
