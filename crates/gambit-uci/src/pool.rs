//! Pool of UCI engine processes for concurrent analysis.

use crate::client::UciEngine;
use crate::parse::{SearchResult, SearchWithInfo, UciError};
use std::sync::{Arc, Mutex};

/// Shared pool of UCI engine worker processes.
pub struct EnginePool {
    engines: Vec<Arc<Mutex<UciEngine>>>,
    next: Mutex<usize>,
}

impl EnginePool {
    /// Spawn `workers` engine processes and complete UCI handshake for each.
    pub fn spawn(command: impl AsRef<str>, args: &[&str], workers: usize) -> Result<Self, UciError> {
        let workers = workers.max(1);
        let mut engines = Vec::with_capacity(workers);
        for _ in 0..workers {
            let engine = UciEngine::spawn(command.as_ref(), args)?;
            engines.push(Arc::new(Mutex::new(engine)));
        }
        Ok(Self {
            engines,
            next: Mutex::new(0),
        })
    }

    /// Number of engines in the pool.
    pub fn len(&self) -> usize {
        self.engines.len()
    }

    /// Whether the pool has no engines.
    pub fn is_empty(&self) -> bool {
        self.engines.is_empty()
    }

    fn acquire(&self) -> Arc<Mutex<UciEngine>> {
        let mut idx = self.next.lock().expect("pool lock");
        let engine = self.engines[*idx].clone();
        *idx = (*idx + 1) % self.engines.len();
        engine
    }

    /// Search a position to the given depth using a pooled engine.
    pub fn search_depth(
        &self,
        fen: &str,
        moves: &[&str],
        depth: u32,
    ) -> Result<SearchResult, UciError> {
        let engine = self.acquire();
        let mut guard = engine.lock().expect("engine lock");
        guard.search_depth(fen, moves, depth)
    }

    /// Search with info line captured from the final depth.
    pub fn search_depth_with_info(
        &self,
        fen: &str,
        moves: &[&str],
        depth: u32,
    ) -> Result<SearchWithInfo, UciError> {
        let engine = self.acquire();
        let mut guard = engine.lock().expect("engine lock");
        guard.search_depth_with_info(fen, moves, depth)
    }

    /// Search with MultiPV enabled.
    pub fn search_depth_multipv(
        &self,
        fen: &str,
        moves: &[&str],
        depth: u32,
        multipv: u32,
    ) -> Result<SearchWithInfo, UciError> {
        let engine = self.acquire();
        let mut guard = engine.lock().expect("engine lock");
        guard.search_depth_multipv(fen, moves, depth, multipv)
    }
}
