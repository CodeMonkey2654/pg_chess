//! Parallel PGN parse and batch accumulation.

use anyhow::Result;
use gambit_db::{explode_mainline, parse_pgn, split_pgn_games, ExplodedGame, PgnError};
use rayon::prelude::*;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Result of parsing one game from a PGN chunk.
pub struct ParsedGame {
    /// Original PGN text for optional storage.
    pub pgn_text: String,
    /// Exploded mainline data.
    pub exploded: ExplodedGame,
}

/// Statistics from a parse or ingest run.
#[derive(Debug, Default, Clone)]
pub struct IngestStats {
    /// Successfully parsed games.
    pub games_ok: u64,
    /// Skipped / failed games.
    pub games_err: u64,
    /// Total positions emitted.
    pub positions: u64,
    /// Total plies emitted.
    pub plies: u64,
    /// Wall-clock duration.
    pub elapsed: Duration,
}

impl IngestStats {
    /// Games processed per second.
    pub fn games_per_sec(&self) -> f64 {
        if self.elapsed.as_secs_f64() > 0.0 {
            self.games_ok as f64 / self.elapsed.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Positions processed per second.
    pub fn positions_per_sec(&self) -> f64 {
        if self.elapsed.as_secs_f64() > 0.0 {
            self.positions as f64 / self.elapsed.as_secs_f64()
        } else {
            0.0
        }
    }
}

/// Parse PGN file contents into exploded games (parallel).
pub fn parse_file_parallel(
    input: &str,
    workers: usize,
    store_pgn: bool,
    fail_fast: bool,
    profile: &mut Option<crate::profile::IngestProfile>,
) -> Result<(Vec<ParsedGame>, IngestStats)> {
    let split_start = Instant::now();
    let chunks: Vec<&str> = split_pgn_games(input);
    if let Some(p) = profile {
        p.record_count("parse.split_games", split_start.elapsed(), chunks.len() as u64);
    }

    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(workers)
        .build()
        .map_err(|e| anyhow::anyhow!("thread pool: {e}"))?;

    let games_ok = AtomicU64::new(0);
    let games_err = AtomicU64::new(0);
    let positions = AtomicU64::new(0);
    let plies = AtomicU64::new(0);
    let start = Instant::now();

    let parsed: Vec<ParsedGame> = pool.install(|| {
        chunks
            .par_iter()
            .filter_map(|chunk| match parse_one(chunk, store_pgn) {
                Ok(game) => {
                    games_ok.fetch_add(1, Ordering::Relaxed);
                    positions.fetch_add(game.exploded.positions.len() as u64, Ordering::Relaxed);
                    plies.fetch_add(game.exploded.plies.len() as u64, Ordering::Relaxed);
                    Some(game)
                }
                Err(e) => {
                    games_err.fetch_add(1, Ordering::Relaxed);
                    eprintln!("skip game: {e}");
                    if fail_fast {
                        panic!("fail-fast: {e}");
                    }
                    None
                }
            })
            .collect()
    });

    let elapsed = start.elapsed();
    if let Some(p) = profile {
        p.record_count("parse.explode_parallel", elapsed, parsed.len() as u64);
    }

    Ok((
        parsed,
        IngestStats {
            games_ok: games_ok.load(Ordering::Relaxed),
            games_err: games_err.load(Ordering::Relaxed),
            positions: positions.load(Ordering::Relaxed),
            plies: plies.load(Ordering::Relaxed),
            elapsed,
        },
    ))
}

fn parse_one(chunk: &str, store_pgn: bool) -> Result<ParsedGame, PgnError> {
    let pgn = parse_pgn(chunk)?;
    let exploded = explode_mainline(&pgn)?;
    Ok(ParsedGame {
        pgn_text: if store_pgn {
            chunk.to_string()
        } else {
            String::new()
        },
        exploded,
    })
}

/// Read a PGN file and parse in parallel.
pub fn parse_path_parallel(
    path: &Path,
    workers: usize,
    store_pgn: bool,
    fail_fast: bool,
    profile: &mut Option<crate::profile::IngestProfile>,
) -> Result<(Vec<ParsedGame>, IngestStats)> {
    let read_start = Instant::now();
    let input = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
    if let Some(p) = profile {
        p.record_count("parse.read_file", read_start.elapsed(), input.len() as u64);
    }
    parse_file_parallel(&input, workers, store_pgn, fail_fast, profile)
}

/// Split parsed games into batches of at most `batch_size` games.
pub fn batch_games(
    games: Vec<ParsedGame>,
    batch_size: usize,
) -> impl Iterator<Item = Vec<ParsedGame>> {
    let batch_size = batch_size.max(1);
    games
        .into_iter()
        .enumerate()
        .fold(Vec::new(), |mut acc, (i, g)| {
            if i % batch_size == 0 {
                acc.push(Vec::new());
            }
            if let Some(last) = acc.last_mut() {
                last.push(g);
            }
            acc
        })
        .into_iter()
        .filter(|b| !b.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINI: &str = "[Event \"A\"]\n\n1. e4 1-0\n\n[Event \"B\"]\n\n1. d4 1-0";

    #[test]
    fn parse_file_parallel_two_games() {
        let (games, stats) = parse_file_parallel(MINI, 2, false, false, &mut None).expect("parse");
        assert_eq!(games.len(), 2);
        assert_eq!(stats.games_ok, 2);
    }

    #[test]
    fn batch_games_splits_evenly() {
        let games: Vec<ParsedGame> = (0..5)
            .map(|_| ParsedGame {
                pgn_text: String::new(),
                exploded: ExplodedGame {
                    headers: Default::default(),
                    start_fen: String::new(),
                    positions: vec![],
                    plies: vec![],
                },
            })
            .collect();
        let batches: Vec<_> = batch_games(games, 2).collect();
        assert_eq!(batches.len(), 3);
        assert_eq!(batches[0].len(), 2);
        assert_eq!(batches[2].len(), 1);
    }
}
