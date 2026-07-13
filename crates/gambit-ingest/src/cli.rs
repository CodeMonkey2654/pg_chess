//! CLI argument definitions.

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Bulk PGN ingest for the gambit PostgreSQL chess schema.
#[derive(Parser, Debug)]
#[command(name = "gambit-ingest", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

/// Subcommands.
#[derive(Subcommand, Debug)]
pub enum Command {
    /// Apply schema migrations.
    Migrate {
        /// PostgreSQL connection URI.
        #[arg(long, env = "DATABASE_URL")]
        pg_uri: String,
    },
    /// Import a PGN file into the database.
    Import {
        /// PostgreSQL connection URI.
        #[arg(long, env = "DATABASE_URL")]
        pg_uri: String,
        /// Source name (e.g. lichess_2024-01).
        #[arg(long)]
        source: String,
        /// PGN file path (use `-` for stdin).
        file: PathBuf,
        /// Parallel parse workers.
        #[arg(long, default_value_t = default_workers())]
        workers: usize,
        /// Games per COPY batch.
        #[arg(long, default_value_t = 5000)]
        batch_games: usize,
        /// Store full PGN text on each game row.
        #[arg(long)]
        store_pgn: bool,
        /// Stop on first parse error.
        #[arg(long)]
        fail_fast: bool,
    },
    /// Refresh opening move statistics materialized view.
    RefreshStats {
        /// PostgreSQL connection URI.
        #[arg(long, env = "DATABASE_URL")]
        pg_uri: String,
    },
    /// Benchmark parse throughput without database I/O.
    BenchParse {
        /// PGN file path.
        file: PathBuf,
        /// Parallel parse workers.
        #[arg(long, default_value_t = default_workers())]
        workers: usize,
    },
}

fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
