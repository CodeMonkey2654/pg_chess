//! Lichess open database integration.

pub mod catalog;
pub mod download;

pub use catalog::{fetch_catalog, parse_catalog, CatalogEntry, LICHESS_CATALOG_URL};
pub use download::{cached_path, download_to_cache, hash_file, DownloadProgress, IngestProgress};
