//! Lichess open database integration.

pub mod catalog;
pub mod download;

pub use catalog::{fetch_catalog, parse_catalog, CatalogEntry, LICHESS_CATALOG_URL};
pub use download::{
    cache_is_complete, cached_path, download_to_cache, download_to_cache_with_retries, hash_file,
    prefetch_download, DownloadProgress, IngestProgress, PrefetchHandle, MIN_COMPLETE_SHARD_BYTES,
};
