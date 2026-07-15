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
        /// Print per-step timing breakdown.
        #[arg(long)]
        profile: bool,
        /// Cast FEN/UCI to chess types during INSERT (slower; default defers to post-import backfill).
        #[arg(long)]
        eager_types: bool,
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
    /// Export corpus move statistics to a `.gbook` file for gambit-analysis.
    ExportBook {
        /// PostgreSQL connection URI.
        #[arg(long, env = "DATABASE_URL")]
        pg_uri: String,
        /// Output book path.
        #[arg(long)]
        output: PathBuf,
    },
    /// Sync Lichess catalog entries for a year into gambit.filesets.
    SyncCatalog {
        /// Ingest worker gRPC address.
        #[arg(long, env = "INGEST_ADDR", default_value = "http://127.0.0.1:8082")]
        ingest_addr: String,
        /// Source name (e.g. lichess_standard_2024).
        #[arg(long)]
        source: String,
        /// Calendar year to sync (e.g. 2024).
        #[arg(long)]
        year: i32,
    },
    /// Download and ingest a Lichess fileset.
    LoadFileset {
        /// Ingest worker gRPC address.
        #[arg(long, env = "INGEST_ADDR", default_value = "http://127.0.0.1:8082")]
        ingest_addr: String,
        /// Source name shared by all shards.
        #[arg(long)]
        source: String,
        /// Calendar year to load (all 12 monthly shards).
        #[arg(long)]
        year: i32,
        /// Local cache directory for .pgn.zst downloads.
        #[arg(long, default_value = ".cache/lichess")]
        cache_dir: PathBuf,
        /// Parallel parse workers.
        #[arg(long, default_value_t = default_workers())]
        workers: usize,
        /// Games per COPY batch.
        #[arg(long, default_value_t = 5000)]
        batch_games: usize,
        /// Print per-step timing breakdown.
        #[arg(long)]
        profile: bool,
        /// Load only one fileset id (retry a failed shard).
        #[arg(long)]
        fileset_id: Option<i64>,
    },
    /// Analyze a single game (engine eval + move classification).
    AnalyzeGame {
        /// PostgreSQL connection URI.
        #[arg(long, env = "DATABASE_URL")]
        pg_uri: String,
        /// Game id to analyze.
        #[arg(long)]
        game_id: i64,
        /// Search depth in plies.
        #[arg(long, default_value_t = 12)]
        depth: u32,
        /// Stockfish executable path (default: stockfish or GAMBIT_STOCKFISH_PATH).
        #[arg(long, env = "GAMBIT_STOCKFISH_PATH")]
        engine: Option<String>,
    },
    /// Analyze a batch of unanalyzed games for a source.
    AnalyzeBatch {
        /// PostgreSQL connection URI.
        #[arg(long, env = "DATABASE_URL")]
        pg_uri: String,
        /// Source name.
        #[arg(long)]
        source: String,
        /// Max games to analyze.
        #[arg(long, default_value_t = 10)]
        limit: usize,
        /// Search depth in plies.
        #[arg(long, default_value_t = 12)]
        depth: u32,
        /// Stockfish executable path.
        #[arg(long, env = "GAMBIT_STOCKFISH_PATH")]
        engine: Option<String>,
    },
}

fn default_workers() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}
