//! Lichess open database catalog helpers.

use anyhow::{Context, Result};

/// One entry from the Lichess standard rated catalog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogEntry {
    /// Full download URL.
    pub url: String,
    /// Filename component (e.g. `lichess_db_standard_rated_2024-01.pgn.zst`).
    pub filename: String,
    /// Period label (e.g. `2024-01`).
    pub period_label: String,
}

/// Default Lichess standard rated catalog URL.
pub const LICHESS_CATALOG_URL: &str = "https://database.lichess.org/standard/list.txt";

/// Fetch the Lichess catalog list from the network.
pub async fn fetch_catalog() -> Result<String> {
    let text = reqwest::get(LICHESS_CATALOG_URL)
        .await
        .context("fetch lichess catalog")?
        .error_for_status()
        .context("lichess catalog HTTP error")?
        .text()
        .await
        .context("read lichess catalog body")?;
    Ok(text)
}

/// Parse catalog text into entries, optionally filtered by year.
pub fn parse_catalog(text: &str, year: Option<i32>) -> Vec<CatalogEntry> {
    let mut entries: Vec<CatalogEntry> = text
        .lines()
        .filter_map(|line| {
            let url = line.trim();
            if url.is_empty() || !url.starts_with("http") {
                return None;
            }
            let filename = url.rsplit('/').next()?.to_string();
            let period = extract_period(&filename)?;
            Some(CatalogEntry {
                url: url.to_string(),
                filename,
                period_label: period,
            })
        })
        .collect();

    if let Some(y) = year {
        let prefix = format!("{y}-");
        entries.retain(|e| e.period_label.starts_with(&prefix));
    }

    entries.sort_by(|a, b| a.period_label.cmp(&b.period_label));
    entries
}

fn extract_period(filename: &str) -> Option<String> {
    let stem = filename.strip_suffix(".pgn.zst")?;
    let period = stem.rsplit('_').next()?;
    if period.len() == 7 && period.as_bytes().get(4) == Some(&b'-') {
        Some(period.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn parse_catalog_filters_year() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tests/fixtures/lichess/list.txt");
        let text = fs::read_to_string(path).expect("fixture");
        let entries = parse_catalog(&text, Some(2024));
        assert_eq!(entries.len(), 4);
        assert_eq!(entries[0].period_label, "2024-01");
        assert_eq!(
            entries.last().expect("catalog has entries").period_label,
            "2024-12"
        );
    }

    #[test]
    fn extract_period_parses_filename() {
        assert_eq!(
            extract_period("lichess_db_standard_rated_2024-03.pgn.zst"),
            Some("2024-03".to_string())
        );
    }
}
