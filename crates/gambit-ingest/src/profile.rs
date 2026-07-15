//! Cumulative step timings for ingest profiling.

use std::collections::BTreeMap;
use std::time::Duration;

/// Named step with accumulated wall time and optional event count.
#[derive(Debug, Default, Clone)]
struct Step {
    duration: Duration,
    count: u64,
}

/// Aggregated profile across an ingest run.
#[derive(Debug, Default, Clone)]
pub struct IngestProfile {
    steps: BTreeMap<String, Step>,
}

impl IngestProfile {
    /// Record elapsed time for a named step.
    pub fn record(&mut self, name: &str, duration: Duration) {
        self.record_count(name, duration, 1);
    }

    /// Record elapsed time and an associated count (rows, bytes, batches, etc.).
    pub fn record_count(&mut self, name: &str, duration: Duration, count: u64) {
        let step = self.steps.entry(name.to_string()).or_default();
        step.duration += duration;
        step.count += count;
    }

    /// Total tracked wall time across all steps.
    pub fn total(&self) -> Duration {
        self.steps.values().map(|s| s.duration).sum()
    }

    /// Print a ranked breakdown table to stdout.
    pub fn print_report(&self) {
        self.print_report_with_wall(None, None);
    }

    /// Print breakdown with optional wall-clock parse/ingest durations for context.
    pub fn print_report_with_wall(
        &self,
        parse_wall: Option<Duration>,
        ingest_wall: Option<Duration>,
    ) {
        let tracked_ms: f64 = self
            .steps
            .iter()
            .filter(|(name, _)| *name != "ingest.total" && *name != "backfill.total")
            .map(|(_, s)| s.duration.as_secs_f64() * 1000.0)
            .sum();
        let mut rows: Vec<_> = self
            .steps
            .iter()
            .filter(|(name, _)| *name != "ingest.total" && *name != "backfill.total")
            .collect();
        rows.sort_by_key(|b| std::cmp::Reverse(b.1.duration));

        println!();
        println!("=== Ingest Profile ===");
        if let Some(w) = parse_wall {
            println!("  Parse wall clock:  {:.2}s", w.as_secs_f64());
        }
        if let Some(w) = ingest_wall {
            println!("  Ingest wall clock: {:.2}s", w.as_secs_f64());
        }
        println!(
            "  (Step times accumulate across batches; % of tracked total {:.1}s)",
            tracked_ms / 1000.0
        );
        println!(
            "  {:<32} {:>10} {:>8} {:>10}",
            "Step", "ms", "% tracked", "count"
        );
        println!("  {}", "-".repeat(64));

        for (name, step) in &rows {
            let ms = step.duration.as_secs_f64() * 1000.0;
            let pct = if tracked_ms > 0.0 {
                ms / tracked_ms * 100.0
            } else {
                0.0
            };
            let count = if step.count > 0 {
                step.count.to_string()
            } else {
                "-".to_string()
            };
            println!("  {:<32} {:>10.1} {:>7.1}% {:>10}", name, ms, pct, count);
        }

        if let Some(w) = ingest_wall {
            let ingest_ms = w.as_secs_f64() * 1000.0;
            if ingest_ms > 0.0 {
                println!("  {}", "-".repeat(64));
                println!("  Top steps as % of ingest wall clock:");
                for (name, step) in rows.iter().take(5) {
                    let ms = step.duration.as_secs_f64() * 1000.0;
                    println!("    {:<28} {:>6.1}%", name, ms / ingest_ms * 100.0);
                }
            }
        }

        println!("  {}", "-".repeat(64));
        println!("  {:<32} {:>10.1}", "TOTAL (tracked steps)", tracked_ms);
        println!();
    }
}
