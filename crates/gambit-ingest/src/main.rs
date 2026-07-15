//! gambit-ingest — high-throughput PGN bulk loader.

use anyhow::Result;
use clap::Parser;
use gambit_ingest::{
    book,
    cli::{Cli, Command},
    pipeline, profile, ImportOptions, IngestSession,
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
                ..ImportOptions::default()
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
            ingest_addr,
            source,
            year,
        } => {
            let mut client = gambit_ingest::grpc::connect(&ingest_addr).await?;
            let count = gambit_ingest::grpc::sync_catalog(&mut client, &source, year).await?;
            info!(count, source, year, "catalog synced via ingest worker");
            Ok(())
        }
        Command::LoadFileset {
            ingest_addr,
            source,
            year,
            cache_dir,
            workers,
            batch_games,
            profile,
            fileset_id,
        } => {
            if profile {
                eprintln!("note: --profile is not supported via gRPC load; use direct import for profiling");
            }
            let mut client = gambit_ingest::grpc::connect(&ingest_addr).await?;
            let (shards, total_games) = gambit_ingest::grpc::load_fileset(
                &mut client,
                &source,
                year,
                &cache_dir,
                workers,
                batch_games,
                fileset_id,
            )
            .await?;
            info!(
                shards,
                total_games, source, year, "fileset load complete via ingest worker"
            );
            Ok(())
        }
        Command::AnalyzeGame {
            pg_uri,
            game_id,
            depth,
            engine,
        } => {
            let session = IngestSession::connect(&pg_uri).await?;
            let options = gambit_ingest::analyze::AnalyzeOptions {
                depth,
                engine_path: engine,
                ..Default::default()
            };
            let result =
                gambit_ingest::analyze::analyze_game(session.client(), game_id, &options).await?;
            info!(
                game_id = result.game_id,
                plies = result.summary.plies.len(),
                white_acc = ?result.summary.accuracy_white,
                black_acc = ?result.summary.accuracy_black,
                "game analyzed"
            );
            Ok(())
        }
        Command::AnalyzeBatch {
            pg_uri,
            source,
            limit,
            depth,
            engine,
        } => {
            let session = IngestSession::connect(&pg_uri).await?;
            let source_id = session.ensure_source(&source).await?;
            let options = gambit_ingest::analyze::AnalyzeOptions {
                depth,
                engine_path: engine,
                ..Default::default()
            };
            let results = gambit_ingest::analyze::analyze_batch(
                session.client(),
                source_id,
                limit,
                &options,
            )
            .await?;
            info!(count = results.len(), source, "batch analysis complete");
            Ok(())
        }
    }
}
