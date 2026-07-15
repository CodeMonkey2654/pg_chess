//! High-throughput PGN bulk loader library for the gambit PostgreSQL schema.

pub mod analyze;
pub mod book;
pub mod cli;
pub mod db;
pub mod grpc;
pub mod headers;
pub mod lichess;
pub mod pipeline;
pub mod profile;
pub mod progress;
pub mod stream;

use anyhow::{Context, Result};
pub use db::backfill_types;
pub use analyze::{AnalyzeGameResult, AnalyzeOptions, analyze_batch, analyze_game};
pub use db::filesets::{self, FilesetRow};
use db::{
    build_staging_rows, copy_staging_batch, ensure_source, flush_staging_batch,
    refresh_opening_stats, run_migrations, truncate_staging, acquire_staging_lock,
    release_staging_lock,
};
pub use lichess::{
    cache_is_complete, cached_path, prefetch_download, CatalogEntry, MIN_COMPLETE_SHARD_BYTES,
};
pub use lichess::DownloadProgress;
pub use lichess::IngestProgress;
pub use progress::format_download_progress;
use lichess::{
    download_to_cache_with_retries, fetch_catalog, hash_file, parse_catalog,
};
use pipeline::{
    batch_games, parse_chunks_parallel, parse_path_parallel, GameProvenance, IngestStats,
    ParsedGame, RawGameChunk,
};
use profile::IngestProfile;
use std::path::{Path, PathBuf};
use std::time::Instant;
use stream::open_game_reader;
use tokio_postgres::Client;
use tracing::info;

/// Result of a full import operation.
#[derive(Debug, Clone)]
pub struct ImportResult {
    /// Parse statistics.
    pub parse: IngestStats,
    /// Games written to the database.
    pub games_loaded: usize,
    /// Positions written.
    pub positions_loaded: u64,
    /// Plies written.
    pub plies_loaded: u64,
    /// DB ingest wall time excluding backfill.
    pub ingest_elapsed: std::time::Duration,
    /// Backfill wall time when deferred types are enabled.
    pub backfill_elapsed: std::time::Duration,
}

/// Options controlling ingest behavior.
#[derive(Debug, Clone)]
pub struct ImportOptions {
    /// Parallel parse workers.
    pub workers: usize,
    /// Games per COPY batch.
    pub batch_games: usize,
    /// Concurrent shard loads (separate DB connections per shard).
    pub shard_concurrency: usize,
    /// Store full PGN text on each game row.
    pub store_pgn: bool,
    /// Stop on first parse error.
    pub fail_fast: bool,
    /// Cast chess types during INSERT instead of post-load backfill.
    pub eager_types: bool,
    /// Optional shard SHA-256 stamped on each game row.
    pub shard_sha256: Option<Vec<u8>>,
}

impl Default for ImportOptions {
    fn default() -> Self {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        Self {
            workers: cpus,
            batch_games: 5000,
            shard_concurrency: 1,
            store_pgn: false,
            fail_fast: false,
            eager_types: false,
            shard_sha256: None,
        }
    }
}

/// Connected ingest session bound to one PostgreSQL database.
pub struct IngestSession {
    client: Client,
    pg_uri: String,
}

impl IngestSession {
    /// Connect to PostgreSQL and return a session handle.
    pub async fn connect(pg_uri: &str) -> Result<Self> {
        Ok(Self {
            client: connect_client(pg_uri).await?,
            pg_uri: pg_uri.to_string(),
        })
    }

    /// Borrow the underlying PostgreSQL client.
    pub fn client(&self) -> &Client {
        &self.client
    }

    /// Apply pending schema migrations.
    pub async fn migrate(&self) -> Result<()> {
        run_migrations(&self.client).await
    }

    /// Ensure a source exists and return its id.
    pub async fn ensure_source(&self, name: &str) -> Result<i32> {
        ensure_source(&self.client, name).await
    }

    /// Import one PGN file into a source.
    pub async fn import_file(
        &mut self,
        source_id: i32,
        file: &Path,
        options: &ImportOptions,
        profile: &mut Option<IngestProfile>,
    ) -> Result<ImportResult> {
        let (games, parse_stats) = parse_path_parallel(
            file,
            options.workers,
            options.store_pgn,
            options.fail_fast,
            profile,
        )?;
        self.import_parsed_games(source_id, games, parse_stats, options, profile)
            .await
    }

    /// Import parsed games already in memory.
    pub async fn import_parsed_games(
        &mut self,
        source_id: i32,
        games: Vec<ParsedGame>,
        parse_stats: IngestStats,
        options: &ImportOptions,
        profile: &mut Option<IngestProfile>,
    ) -> Result<ImportResult> {
        let defer_types = !options.eager_types;
        let ingest_start = Instant::now();
        let mut total_games = 0usize;
        let mut total_positions = 0u64;
        let mut total_plies = 0u64;

        for batch in batch_games(games, options.batch_games) {
            let (game_count, pos_count, ply_count) = ingest_batch(
                &mut self.client,
                source_id,
                &batch,
                options.store_pgn,
                defer_types,
                profile,
            )
            .await?;
            total_games += game_count;
            total_positions += pos_count;
            total_plies += ply_count;
        }

        let ingest_elapsed = ingest_start.elapsed();
        let backfill_elapsed = if defer_types {
            let start = Instant::now();
            backfill_types(&self.client, source_id, profile).await?;
            start.elapsed()
        } else {
            std::time::Duration::ZERO
        };

        Ok(ImportResult {
            parse: parse_stats,
            games_loaded: total_games,
            positions_loaded: total_positions,
            plies_loaded: total_plies,
            ingest_elapsed,
            backfill_elapsed,
        })
    }

    /// Import one shard file with provenance, without running backfill.
    pub async fn import_shard_file(
        &mut self,
        source_id: i32,
        file: &Path,
        shard_sha256: Option<Vec<u8>>,
        options: &ImportOptions,
        profile: &mut Option<IngestProfile>,
    ) -> Result<ImportResult> {
        let mut local = options.clone();
        local.shard_sha256 = shard_sha256;
        self.import_shard_stream(source_id, file, &local, profile, None)
            .await
    }

    /// Stream-parse a shard and ingest in bounded batches without backfill.
    pub async fn import_shard_stream(
        &mut self,
        source_id: i32,
        file: &Path,
        options: &ImportOptions,
        profile: &mut Option<IngestProfile>,
        mut ingest_progress: Option<IngestProgress>,
    ) -> Result<ImportResult> {
        let defer_types = !options.eager_types;
        let parse_start = Instant::now();
        let mut reader = open_game_reader(file)?;
        let mut pending: Vec<ParsedGame> = Vec::new();
        let mut raw_batch: Vec<RawGameChunk> = Vec::new();
        let mut games_ok = 0u64;
        let mut games_err = 0u64;
        let mut positions = 0u64;
        let mut plies = 0u64;

        let ingest_start = Instant::now();
        let mut total_games = 0usize;
        let mut total_positions = 0u64;
        let mut total_plies = 0u64;

        let parse_batch_size = options.workers.saturating_mul(64).max(128);
        let provenance_base = GameProvenance {
            pgn_sha256: options.shard_sha256.clone(),
            pgn_byte_offset: None,
        };

        while let Some((chunk, offset)) = reader.next_game()? {
            raw_batch.push(RawGameChunk {
                text: chunk,
                byte_offset: offset,
            });

            if raw_batch.len() < parse_batch_size {
                continue;
            }

            let (parsed, err_count) = parse_chunks_parallel(
                &raw_batch,
                options.workers,
                options.store_pgn,
                &provenance_base,
                options.fail_fast,
                profile,
            )?;
            raw_batch.clear();
            games_err += err_count as u64;
            for game in parsed {
                games_ok += 1;
                positions += game.exploded.positions.len() as u64;
                plies += game.exploded.plies.len() as u64;
                pending.push(game);
            }

            while pending.len() >= options.batch_games {
                let batch: Vec<ParsedGame> =
                    pending.drain(..options.batch_games).collect();
                let (g, p, pl) = ingest_batch(
                    &mut self.client,
                    source_id,
                    &batch,
                    options.store_pgn,
                    defer_types,
                    profile,
                )
                .await?;
                total_games += g;
                total_positions += p;
                total_plies += pl;
                if let Some(ref mut cb) = ingest_progress {
                    cb(total_games);
                }
            }
        }

        if !raw_batch.is_empty() {
            let (parsed, err_count) = parse_chunks_parallel(
                &raw_batch,
                options.workers,
                options.store_pgn,
                &provenance_base,
                options.fail_fast,
                profile,
            )?;
            games_err += err_count as u64;
            for game in parsed {
                games_ok += 1;
                positions += game.exploded.positions.len() as u64;
                plies += game.exploded.plies.len() as u64;
                pending.push(game);
            }
        }

        while !pending.is_empty() {
            let take = options.batch_games.min(pending.len());
            let batch: Vec<ParsedGame> = pending.drain(..take).collect();
            let (g, p, pl) = ingest_batch(
                &mut self.client,
                source_id,
                &batch,
                options.store_pgn,
                defer_types,
                profile,
            )
            .await?;
            total_games += g;
            total_positions += p;
            total_plies += pl;
            if let Some(ref mut cb) = ingest_progress {
                cb(total_games);
            }
        }

        let parse_stats = IngestStats {
            games_ok,
            games_err,
            positions,
            plies,
            elapsed: parse_start.elapsed(),
        };

        Ok(ImportResult {
            parse: parse_stats,
            games_loaded: total_games,
            positions_loaded: total_positions,
            plies_loaded: total_plies,
            ingest_elapsed: ingest_start.elapsed(),
            backfill_elapsed: std::time::Duration::ZERO,
        })
    }

    /// Refresh opening move statistics.
    pub async fn refresh_stats(&self) -> Result<()> {
        refresh_opening_stats(&self.client).await
    }

    /// Sync Lichess catalog entries for a year into `gambit.filesets`.
    pub async fn sync_lichess_catalog(&self, source: &str, year: i32) -> Result<Vec<i64>> {
        let source_id = self.ensure_source(source).await?;
        let text = fetch_catalog().await?;
        let entries = parse_catalog(&text, Some(year));
        let mut ids = Vec::with_capacity(entries.len());
        for entry in entries {
            let id = filesets::upsert_fileset(
                &self.client,
                source_id,
                &entry.url,
                &entry.filename,
                &entry.period_label,
            )
            .await?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Sync catalog from provided text (for tests).
    pub async fn sync_catalog_text(&self, source: &str, text: &str, year: i32) -> Result<Vec<i64>> {
        let source_id = self.ensure_source(source).await?;
        let entries = parse_catalog(text, Some(year));
        let mut ids = Vec::with_capacity(entries.len());
        for entry in entries {
            let id = filesets::upsert_fileset(
                &self.client,
                source_id,
                &entry.url,
                &entry.filename,
                &entry.period_label,
            )
            .await?;
            ids.push(id);
        }
        Ok(ids)
    }

    /// Load a full-year Lichess fileset: download, ingest each shard, backfill once.
    pub async fn load_fileset_year(
        &mut self,
        source: &str,
        year: i32,
        cache_dir: &Path,
        options: &ImportOptions,
        profile: &mut Option<IngestProfile>,
        skip_complete: bool,
    ) -> Result<Vec<ImportResult>> {
        let source_id = self.ensure_source(source).await?;
        let targets =
            filesets::filesets_for_year(&self.client, source_id, year, skip_complete).await?;

        if targets.is_empty() {
            anyhow::bail!(
                "no filesets found for source {source} year {year}; run sync-catalog first"
            );
        }

        let pending: Vec<filesets::FilesetRow> = targets
            .into_iter()
            .filter(|f| !(skip_complete && f.status == "complete"))
            .collect();

        if pending.is_empty() {
            return Ok(Vec::new());
        }

        let concurrency = options.shard_concurrency.max(1);
        if concurrency == 1 {
            let mut results = Vec::new();
            let mut failures = 0usize;
            let mut prefetch = None;
            for (i, fileset) in pending.iter().enumerate() {
                if let Some(next) = pending.get(i + 1) {
                    let cached = cached_path(cache_dir, &next.filename);
                    if !cache_is_complete(
                        &cached,
                        next.byte_size,
                        next.sha256.as_deref(),
                    ) {
                        prefetch = Some(prefetch_download(
                            &next.remote_url,
                            &next.filename,
                            cache_dir,
                        ));
                    }
                }
                match self
                    .load_one_fileset(
                        source_id,
                        fileset,
                        cache_dir,
                        options,
                        profile,
                        None,
                        None,
                        prefetch.take(),
                    )
                    .await
                {
                    Ok(result) => results.push(result),
                    Err(e) => {
                        failures += 1;
                        tracing::error!(
                            fileset = fileset.period_label,
                            error = %e,
                            "shard load failed (continuing)"
                        );
                    }
                }
            }
            if results.is_empty() && failures > 0 {
                anyhow::bail!("all {failures} shard loads failed");
            }
            self.finish_year_ingest(source_id, options, profile).await?;
            return Ok(results);
        }

        use futures::stream::{self, StreamExt};
        let pg_uri = self.pg_uri.clone();
        let options = options.clone();
        let cache_dir = cache_dir.to_path_buf();

        let results: Vec<Result<ImportResult>> = stream::iter(pending)
            .map(|fileset| {
                let pg_uri = pg_uri.clone();
                let options = options.clone();
                let cache_dir = cache_dir.clone();
                async move {
                    let mut session = IngestSession::connect(&pg_uri).await?;
                    session
                        .load_one_fileset(
                            source_id,
                            &fileset,
                            &cache_dir,
                            &options,
                            &mut None,
                            None,
                            None,
                            None,
                        )
                        .await
                }
            })
            .buffer_unordered(concurrency)
            .collect()
            .await;

        let mut ok = Vec::new();
        let mut failures = 0usize;
        for result in results {
            match result {
                Ok(r) => ok.push(r),
                Err(e) => {
                    failures += 1;
                    tracing::error!(error = %e, "parallel shard load failed");
                }
            }
        }
        if ok.is_empty() && failures > 0 {
            anyhow::bail!("all {failures} parallel shard loads failed");
        }

        self.finish_year_ingest(source_id, &options, profile).await?;
        Ok(ok)
    }

    /// Backfill deferred types and refresh opening stats after a year load.
    pub async fn finish_year_ingest(
        &mut self,
        source_id: i32,
        options: &ImportOptions,
        profile: &mut Option<IngestProfile>,
    ) -> Result<()> {
        if !options.eager_types {
            backfill_types(&self.client, source_id, profile).await?;
        }
        refresh_opening_stats(&self.client).await?;
        Ok(())
    }

    /// Load a single fileset row by id.
    pub async fn load_fileset_by_id(
        &mut self,
        fileset_id: i64,
        cache_dir: &Path,
        options: &ImportOptions,
        profile: &mut Option<IngestProfile>,
        run_backfill: bool,
        download_progress: Option<DownloadProgress>,
        ingest_progress: Option<IngestProgress>,
        prefetched_download: Option<lichess::PrefetchHandle>,
    ) -> Result<ImportResult> {
        let fileset = filesets::get_fileset(&self.client, fileset_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("fileset {fileset_id} not found"))?;
        let source_id = fileset.source_id;
        let result = self
            .load_one_fileset(
                source_id,
                &fileset,
                cache_dir,
                options,
                profile,
                download_progress,
                ingest_progress,
                prefetched_download,
            )
            .await?;
        if run_backfill && !options.eager_types {
            backfill_types(&self.client, source_id, profile).await?;
        }
        Ok(result)
    }

    async fn load_one_fileset(
        &mut self,
        source_id: i32,
        fileset: &filesets::FilesetRow,
        cache_dir: &Path,
        options: &ImportOptions,
        profile: &mut Option<IngestProfile>,
        download_progress: Option<DownloadProgress>,
        ingest_progress: Option<IngestProgress>,
        prefetched_download: Option<lichess::PrefetchHandle>,
    ) -> Result<ImportResult> {
        let cached = cached_path(cache_dir, &fileset.filename);
        let (path, byte_size, sha256) = if cache_is_complete(
            &cached,
            fileset.byte_size,
            fileset.sha256.as_deref(),
        ) {
            if let (Some(size), Some(digest)) = (fileset.byte_size, fileset.sha256.as_ref()) {
                (cached, size, digest.clone())
            } else {
                let (size, digest) = hash_file(&cached).await?;
                (cached, size, digest)
            }
        } else if cached.exists() {
            tracing::warn!(
                fileset = fileset.period_label,
                path = %cached.display(),
                "removing incomplete cache file"
            );
            tokio::fs::remove_file(&cached)
                .await
                .with_context(|| format!("remove incomplete {}", cached.display()))?;
            Self::download_shard(
                &self.client,
                fileset,
                cache_dir,
                download_progress,
                prefetched_download,
            )
            .await?
        } else if let Some(handle) = prefetched_download {
            filesets::mark_download_started(&self.client, fileset.id).await?;
            match handle.await.context("prefetch download join")? {
                Ok(v) => v,
                Err(e) => {
                    let msg = format_error(&e);
                    filesets::mark_failed(&self.client, fileset.id, &msg).await?;
                    return Err(e);
                }
            }
        } else {
            Self::download_shard(
                &self.client,
                fileset,
                cache_dir,
                download_progress,
                None,
            )
            .await?
        };

        filesets::mark_download_complete(&self.client, fileset.id, byte_size, &sha256).await?;
        filesets::mark_ingest_started(&self.client, fileset.id).await?;

        let mut shard_opts = options.clone();
        shard_opts.eager_types = true;
        shard_opts.shard_sha256 = Some(sha256.clone());

        let started = Instant::now();
        let result = match self
            .import_shard_stream(source_id, &path, &shard_opts, profile, ingest_progress)
            .await
        {
            Ok(r) => r,
            Err(e) => {
                let msg = format_error(&e);
                filesets::mark_failed(&self.client, fileset.id, &msg).await?;
                return Err(e);
            }
        };

        let wall = started.elapsed().as_secs_f64();
        filesets::mark_ingest_complete(
            &self.client,
            fileset.id,
            result.games_loaded as i64,
            result.parse.games_err as i64,
            result.positions_loaded as i64,
            result.plies_loaded as i64,
        )
        .await?;
        filesets::record_ingest_run(
            &self.client,
            fileset.id,
            source_id,
            options.workers as i32,
            options.batch_games as i32,
            result.games_loaded as i64,
            result.positions_loaded as i64,
            wall,
        )
        .await?;

        info!(
            fileset = fileset.period_label,
            games = result.games_loaded,
            games_per_min = format!("{:.0}", result.games_loaded as f64 / wall * 60.0),
            "shard ingest complete"
        );
        Ok(result)
    }

    async fn download_shard(
        client: &Client,
        fileset: &filesets::FilesetRow,
        cache_dir: &Path,
        download_progress: Option<DownloadProgress>,
        prefetched_download: Option<lichess::PrefetchHandle>,
    ) -> Result<(PathBuf, i64, Vec<u8>)> {
        if let Some(handle) = prefetched_download {
            filesets::mark_download_started(client, fileset.id).await?;
            return handle.await.context("prefetch download join")?;
        }
        filesets::mark_download_started(client, fileset.id).await?;
        match download_to_cache_with_retries(
            &fileset.remote_url,
            &fileset.filename,
            cache_dir,
            download_progress,
        )
        .await
        {
            Ok(v) => Ok(v),
            Err(e) => {
                let msg = format_error(&e);
                filesets::mark_failed(client, fileset.id, &msg).await?;
                Err(e)
            }
        }
    }
}

/// Connect to PostgreSQL.
pub async fn connect_client(pg_uri: &str) -> Result<Client> {
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

/// Print a human-readable ingest summary to stdout.
pub fn print_summary(result: &ImportResult) {
    let games = result.games_loaded;
    let positions = result.positions_loaded;
    let plies = result.plies_loaded;
    let parse_stats = &result.parse;
    let ingest_elapsed = result.ingest_elapsed;
    let backfill_elapsed = result.backfill_elapsed;

    let ingest_secs = ingest_elapsed.as_secs_f64().max(f64::EPSILON);
    let backfill_secs = backfill_elapsed.as_secs_f64();
    let db_secs = (ingest_elapsed + backfill_elapsed)
        .as_secs_f64()
        .max(f64::EPSILON);
    let total_secs = (parse_stats.elapsed + ingest_elapsed + backfill_elapsed).as_secs_f64();

    println!();
    println!("=== Ingest Summary ===");
    println!("  Games loaded:      {games}");
    println!("  Parse errors:      {}", parse_stats.games_err);
    println!("  Positions loaded:  {positions}");
    println!("  Plies loaded:      {plies}");
    println!();
    println!("  Parse phase:");
    println!(
        "    elapsed:         {:.2}s",
        parse_stats.elapsed.as_secs_f64()
    );
    println!("    games/sec:       {:.0}", parse_stats.games_per_sec());
    println!(
        "    positions/sec:   {:.0}",
        parse_stats.positions_per_sec()
    );
    println!();
    println!("  Ingest phase (DB COPY + INSERT):");
    println!("    elapsed:         {:.2}s", ingest_secs);
    println!("    games/sec:       {:.0}", games as f64 / ingest_secs);
    println!(
        "    games/min:       {:.0}",
        games as f64 / ingest_secs * 60.0
    );
    println!("    positions/sec:   {:.0}", positions as f64 / ingest_secs);
    println!("    plies/sec:       {:.0}", plies as f64 / ingest_secs);
    if backfill_secs > 0.0 {
        println!();
        println!("  Backfill phase (chess types + indexes):");
        println!("    elapsed:         {:.2}s", backfill_secs);
        println!(
            "    positions/sec:   {:.0}",
            positions as f64 / backfill_secs.max(f64::EPSILON)
        );
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

async fn ingest_batch(
    client: &mut Client,
    source_id: i32,
    batch: &[ParsedGame],
    store_pgn: bool,
    defer_types: bool,
    profile: &mut Option<IngestProfile>,
) -> Result<(usize, u64, u64)> {
    acquire_staging_lock(client).await?;
    let result = ingest_batch_locked(client, source_id, batch, store_pgn, defer_types, profile).await;
    if let Err(e) = release_staging_lock(client).await {
        tracing::warn!(error = %e, "failed to release staging lock");
    }
    result
}

async fn ingest_batch_locked(
    client: &mut Client,
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
    let staging: Vec<(i32, gambit_db::ExplodedGame, Option<String>, GameProvenance)> = batch
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
                g.provenance.clone(),
            )
        })
        .collect();

    let (game_rows, pos_rows, ply_rows) = build_staging_rows(&staging);
    let prepare = prep_start.elapsed();

    let pos_refs: Vec<(i32, &gambit_db::PositionRow)> =
        pos_rows.iter().map(|(s, p)| (*s, p)).collect();
    let ply_refs: Vec<(i32, &gambit_db::PlyRow)> =
        ply_rows.iter().map(|(s, p)| (*s, p)).collect();

    if let Some(p) = profile {
        p.record("db.truncate_staging", truncate);
        p.record_count("ingest.prepare_staging", prepare, batch.len() as u64);
    }

    copy_staging_batch(client, &game_rows, &pos_refs, &ply_refs, profile).await?;
    flush_staging_batch(client, source_id, defer_types, profile).await
}

/// Format an error with its full cause chain for persistence in `filesets.error_message`.
pub fn format_error(err: &anyhow::Error) -> String {
    let mut msg = err.to_string();
    let mut src = err.source();
    while let Some(e) = src {
        msg.push_str(": ");
        msg.push_str(&e.to_string());
        src = e.source();
    }
    const MAX: usize = 2000;
    if msg.len() > MAX {
        msg.truncate(MAX - 3);
        msg.push_str("...");
    }
    msg
}
