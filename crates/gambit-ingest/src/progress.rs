//! Download / ingest progress message formatting.

/// Format a shard download progress message for the job UI.
pub fn format_download_progress(label: &str, downloaded: u64, total: Option<u64>) -> String {
    match total {
        Some(total) => format!(
            "downloading shard {label} ({:.1} / {:.1} GiB)",
            downloaded as f64 / (1024.0 * 1024.0 * 1024.0),
            total as f64 / (1024.0 * 1024.0 * 1024.0),
        ),
        None => format!(
            "downloading shard {label} ({:.1} GiB so far)",
            downloaded as f64 / (1024.0 * 1024.0 * 1024.0),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn download_progress_with_total() {
        let msg = format_download_progress(
            "2024-01",
            2 * 1024 * 1024 * 1024,
            Some(30 * 1024 * 1024 * 1024),
        );
        assert!(msg.contains("2024-01"));
        assert!(msg.contains("2.0"));
        assert!(msg.contains("30.0"));
    }

    #[test]
    fn download_progress_unknown_total() {
        let msg = format_download_progress("2024-02", 1024 * 1024 * 1024, None);
        assert!(msg.contains("2024-02"));
        assert!(msg.contains("so far"));
    }
}
