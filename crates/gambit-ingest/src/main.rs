//! gambit-ingest — high-throughput PGN bulk loader.

use anyhow::Result;
use clap::Parser;
use gambit_ingest::{
    cli::{Cli, Command},
    book, pipeline, profile, ImportOptions, IngestSession,
};
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();
    match cli.command {
        Command::Migrate { pg_uri } => {
            let session = IngestSession::connect(&pg_uri).await?;
            session.migrate().await?;
            info!("schema migrations applied");
            Ok(())
        }
        Command::Import {
            pg_uri,
            source,
            file,
            workers,
            batch_games,
            store_pgn,
            fail_fast,
            profile,
            eager_types,
        } => {
            let mut session = IngestSession::connect(&pg_uri).await?;
            let source_id = session.ensure_source(&source).await?;
            info!(source, source_id, "source ready");

            let mut prof = if profile {
                Some(profile::IngestProfile::default())
            } else {
                None
            };
            let options = ImportOptions {
                workers,
                batch_games,
                store_pgn,
                fail_fast,
                eager_types,
                shard_sha256: None,
            };
            let result = session
                .import_file(source_id, &file, &options, &mut prof)
                .await?;
            info!(
                games = result.games_loaded,
                errors = result.parse.games_err,
                "import complete"
            );
            gambit_ingest::print_summary(&result);
            if let Some(p) = prof {
                p.print_report_with_wall(
                    Some(result.parse.elapsed),
                    Some(result.ingest_elapsed + result.backfill_elapsed),
                );
            }
            Ok(())
        }
        Command::RefreshStats { pg_uri } => {
            let session = IngestSession::connect(&pg_uri).await?;
            session.refresh_stats().await?;
            info!("opening_moves materialized view refreshed");
            Ok(())
        }
        Command::BenchParse { file, workers } => {
            let (games, stats) =
                pipeline::parse_path_parallel(&file, workers, false, false, &mut None)?;
            println!("games parsed: {}", games.len());
            println!("errors: {}", stats.games_err);
            println!("positions: {}", stats.positions);
            println!("plies: {}", stats.plies);
            println!("elapsed: {:.2}s", stats.elapsed.as_secs_f64());
            println!("games/sec: {:.0}", stats.games_per_sec());
            println!("positions/sec: {:.0}", stats.positions_per_sec());
            Ok(())
        }
        Command::ExportBook { pg_uri, output } => {
            let session = IngestSession::connect(&pg_uri).await?;
            let positions = book::export_book(session.client(), &output).await?;
            info!(
                positions,
                output = %output.display(),
                "corpus book exported"
            );
            Ok(())
        }
        Command::SyncCatalog {
            pg_uri,
            source,
            year,
        } => {
            let session = IngestSession::connect(&pg_uri).await?;
            session.migrate().await?;
            let ids = session.sync_lichess_catalog(&source, year).await?;
            info!(count = ids.len(), source, year, "catalog synced");
            Ok(())
        }
        Command::LoadFileset {
            pg_uri,
            source,
            year,
            cache_dir,
            workers,
            batch_games,
            profile,
            fileset_id,
        } => {
            let mut session = IngestSession::connect(&pg_uri).await?;
            session.migrate().await?;
            let mut prof = if profile {
                Some(profile::IngestProfile::default())
            } else {
                None
            };
            let options = ImportOptions {
                workers,
                batch_games,
                store_pgn: false,
                fail_fast: false,
                eager_types: false,
                shard_sha256: None,
            };

            if let Some(id) = fileset_id {
                let result = session
                    .load_fileset_by_id(id, &cache_dir, &options, &mut prof, true, None, None)
                    .await?;
                gambit_ingest::print_summary(&result);
            } else {
                session.sync_lichess_catalog(&source, year).await?;
                let results = session
                    .load_fileset_year(&source, year, &cache_dir, &options, &mut prof)
                    .await?;
                let total_games: usize = results.iter().map(|r| r.games_loaded).sum();
                info!(shards = results.len(), total_games, "fileset load complete");
                for (i, result) in results.iter().enumerate() {
                    println!("\n--- Shard {} ---", i + 1);
                    gambit_ingest::print_summary(result);
                }
            }
            Ok(())
        }
    }
}
