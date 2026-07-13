//! gambit-ingest — high-throughput PGN bulk loader.

mod cli;
mod db;
mod headers;
mod pipeline;
mod profile;

use anyhow::{Context, Result};
use clap::Parser;
use cli::{Cli, Command};
use db::{
    backfill_types, build_staging_rows, copy_staging_batch, ensure_source, flush_staging_batch,
    refresh_opening_stats, run_migrations, truncate_staging,
};
use pipeline::{batch_games, parse_path_parallel, IngestStats, ParsedGame};
use profile::IngestProfile;
use std::path::Path;
use std::time::Instant;
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
        Command::Migrate { pg_uri } => cmd_migrate(&pg_uri).await,
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
            cmd_import(
                &pg_uri,
                &source,
                &file,
                workers,
                batch_games,
                store_pgn,
                fail_fast,
                profile,
                eager_types,
            )
            .await
        }
        Command::RefreshStats { pg_uri } => cmd_refresh_stats(&pg_uri).await,
        Command::BenchParse { file, workers } => cmd_bench_parse(&file, workers),
    }
}

async fn connect(pg_uri: &str) -> Result<tokio_postgres::Client> {
    let (client, connection) = tokio_postgres::connect(pg_uri, tokio_postgres::NoTls)
        .await
        .context("connect to postgres")?;
    tokio::spawn(async move {
        if let Err(e) = connection.await {
            eprintln!("postgres connection error: {e}");
        }
    });
    Ok(client)
}

async fn cmd_migrate(pg_uri: &str) -> Result<()> {
    let client = connect(pg_uri).await?;
    run_migrations(&client).await?;
    info!("schema migrations applied");
    Ok(())
}

async fn cmd_import(
    pg_uri: &str,
    source: &str,
    file: &Path,
    workers: usize,
    batch_size: usize,
    store_pgn: bool,
    fail_fast: bool,
    enable_profile: bool,
    eager_types: bool,
) -> Result<()> {
    let defer_types = !eager_types;
    let mut profile = if enable_profile {
        Some(IngestProfile::default())
    } else {
        None
    };

    let mut client = connect(pg_uri).await?;
    let source_id = ensure_source(&client, source).await?;
    info!(source, source_id, defer_types, "source ready");

    let (games, parse_stats) = parse_path_parallel(
        file,
        workers,
        store_pgn,
        fail_fast,
        &mut profile,
    )?;
    info!(
        games = parse_stats.games_ok,
        errors = parse_stats.games_err,
        games_per_sec = format!("{:.0}", parse_stats.games_per_sec()),
        "parse complete"
    );

    let ingest_start = Instant::now();
    let mut total_games = 0usize;
    let mut total_positions = 0u64;
    let mut total_plies = 0u64;

    for batch in batch_games(games, batch_size) {
        let (game_count, pos_count, ply_count) = ingest_batch(
            &mut client,
            source_id,
            &batch,
            store_pgn,
            defer_types,
            &mut profile,
        )
        .await?;
        total_games += game_count;
        total_positions += pos_count;
        total_plies += ply_count;
    }

    let ingest_elapsed = ingest_start.elapsed();

    let backfill_elapsed = if defer_types {
        let backfill_start = Instant::now();
        info!("backfilling chess types");
        let (pos, pl) = backfill_types(&client, source_id, &mut profile).await?;
        let elapsed = backfill_start.elapsed();
        info!(
            positions = pos,
            plies = pl,
            elapsed_secs = elapsed.as_secs_f64(),
            "backfill complete"
        );
        elapsed
    } else {
        std::time::Duration::ZERO
    };

    if let Some(p) = profile.as_mut() {
        p.record("ingest.total", ingest_elapsed);
        if defer_types {
            p.record("backfill.total", backfill_elapsed);
        }
    }

    info!(
        games = total_games,
        positions = total_positions,
        plies = total_plies,
        elapsed_secs = ingest_elapsed.as_secs_f64(),
        games_per_sec = format!("{:.0}", total_games as f64 / ingest_elapsed.as_secs_f64()),
        "ingest complete"
    );

    print_summary(
        &parse_stats,
        total_games,
        total_positions,
        total_plies,
        ingest_elapsed,
        backfill_elapsed,
    );

    if let Some(p) = profile {
        p.print_report_with_wall(
            Some(parse_stats.elapsed),
            Some(ingest_elapsed + backfill_elapsed),
        );
    }
    Ok(())
}

async fn ingest_batch(
    client: &mut tokio_postgres::Client,
    source_id: i32,
    batch: &[ParsedGame],
    store_pgn: bool,
    defer_types: bool,
    profile: &mut Option<IngestProfile>,
) -> Result<(usize, u64, u64)> {
    let trunc_start = Instant::now();
    truncate_staging(client).await?;
    let truncate = trunc_start.elapsed();

    let prep_start = Instant::now();
    let staging: Vec<(i32, gambit_db::ExplodedGame, Option<String>)> = batch
        .iter()
        .enumerate()
        .map(|(i, g)| {
            (
                i as i32 + 1,
                g.exploded.clone(),
                if store_pgn {
                    Some(g.pgn_text.clone())
                } else {
                    None
                },
            )
        })
        .collect();

    let (game_rows, pos_rows, ply_rows) = build_staging_rows(&staging);
    let prepare = prep_start.elapsed();

    let pos_refs: Vec<(i32, &gambit_db::PositionRow)> =
        pos_rows.iter().map(|(s, p)| (*s, p)).collect();
    let ply_refs: Vec<(i32, &gambit_db::PlyRow)> = ply_rows.iter().map(|(s, p)| (*s, p)).collect();

    if let Some(p) = profile {
        p.record("db.truncate_staging", truncate);
        p.record_count("ingest.prepare_staging", prepare, batch.len() as u64);
    }

    copy_staging_batch(
        client,
        &game_rows,
        &pos_refs,
        &ply_refs,
        profile,
    )
    .await?;
    let (n, pos, pl) = flush_staging_batch(client, source_id, defer_types, profile).await?;
    Ok((n, pos, pl))
}

async fn cmd_refresh_stats(pg_uri: &str) -> Result<()> {
    let client = connect(pg_uri).await?;
    refresh_opening_stats(&client).await?;
    info!("opening_moves materialized view refreshed");
    Ok(())
}

fn cmd_bench_parse(file: &Path, workers: usize) -> Result<()> {
    let (games, stats) = parse_path_parallel(file, workers, false, false, &mut None)?;
    println!("games parsed: {}", games.len());
    println!("errors: {}", stats.games_err);
    println!("positions: {}", stats.positions);
    println!("plies: {}", stats.plies);
    println!("elapsed: {:.2}s", stats.elapsed.as_secs_f64());
    println!("games/sec: {:.0}", stats.games_per_sec());
    println!("positions/sec: {:.0}", stats.positions_per_sec());
    Ok(())
}

fn print_summary(
    parse_stats: &IngestStats,
    games: usize,
    positions: u64,
    plies: u64,
    ingest_elapsed: std::time::Duration,
    backfill_elapsed: std::time::Duration,
) {
    let ingest_secs = ingest_elapsed.as_secs_f64().max(f64::EPSILON);
    let backfill_secs = backfill_elapsed.as_secs_f64();
    let db_secs = (ingest_elapsed + backfill_elapsed).as_secs_f64().max(f64::EPSILON);
    let total_secs = (parse_stats.elapsed + ingest_elapsed + backfill_elapsed).as_secs_f64();

    println!();
    println!("=== Ingest Summary ===");
    println!("  Games loaded:      {games}");
    println!("  Parse errors:      {}", parse_stats.games_err);
    println!("  Positions loaded:  {positions}");
    println!("  Plies loaded:      {plies}");
    println!();
    println!("  Parse phase:");
    println!("    elapsed:         {:.2}s", parse_stats.elapsed.as_secs_f64());
    println!("    games/sec:       {:.0}", parse_stats.games_per_sec());
    println!("    positions/sec:   {:.0}", parse_stats.positions_per_sec());
    println!();
    println!("  Ingest phase (DB COPY + INSERT):");
    println!("    elapsed:         {:.2}s", ingest_secs);
    println!("    games/sec:       {:.0}", games as f64 / ingest_secs);
    println!("    games/min:       {:.0}", games as f64 / ingest_secs * 60.0);
    println!("    positions/sec:   {:.0}", positions as f64 / ingest_secs);
    println!("    plies/sec:       {:.0}", plies as f64 / ingest_secs);
    if backfill_secs > 0.0 {
        println!();
        println!("  Backfill phase (chess types + indexes):");
        println!("    elapsed:         {:.2}s", backfill_secs);
        println!("    positions/sec:   {:.0}", positions as f64 / backfill_secs.max(f64::EPSILON));
    }
    println!();
    println!("  DB total (ingest + backfill):");
    println!("    elapsed:         {:.2}s", db_secs);
    println!("    games/min:       {:.0}", games as f64 / db_secs * 60.0);
    println!("    positions/sec:   {:.0}", positions as f64 / db_secs);
    println!();
    println!("  End-to-end:        {:.2}s", total_secs);
    println!(
        "    positions/sec:   {:.0}",
        positions as f64 / total_secs.max(f64::EPSILON)
    );
}
