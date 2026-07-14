//! Transposition table keyed on Zobrist hash.

use gambit_db::Move;

const TT_BITS: usize = 20;
const TT_SIZE: usize = 1 << TT_BITS;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone, Copy)]
struct TtEntry {
    key: u64,
    depth: i8,
    score: i32,
    bound: Bound,
    best_move: Option<Move>,
}

impl Default for TtEntry {
    fn default() -> Self {
        Self {
            key: 0,
            depth: -1,
            score: 0,
            bound: Bound::Exact,
            best_move: None,
        }
    }
}

/// Fixed-size transposition table.
pub struct TranspositionTable {
    entries: Vec<TtEntry>,
}

impl TranspositionTable {
    /// Allocate a new table.
    pub fn new() -> Self {
        Self {
            entries: vec![TtEntry::default(); TT_SIZE],
        }
    }

    /// Clear all entries (new game).
    pub fn clear(&mut self) {
        for e in &mut self.entries {
            *e = TtEntry::default();
        }
    }

    fn index(key: u64) -> usize {
        (key as usize) & (TT_SIZE - 1)
    }

    /// Probe for a cutoff score at `depth`.
    pub fn probe(&self, key: u64, depth: i32, alpha: i32, beta: i32) -> Option<i32> {
        let entry = &self.entries[Self::index(key)];
        if entry.key != key || entry.depth < depth as i8 {
            return None;
        }
        let score = entry.score;
        match entry.bound {
            Bound::Exact => Some(score),
            Bound::Lower if score >= beta => Some(score),
            Bound::Upper if score <= alpha => Some(score),
            _ => None,
        }
    }

    /// Store a search result.
    pub fn store(
        &mut self,
        key: u64,
        depth: i32,
        score: i32,
        alpha: i32,
        beta: i32,
        best_move: Option<Move>,
    ) {
        let idx = Self::index(key);
        let entry = &mut self.entries[idx];
        if entry.key == key && entry.depth > depth as i8 {
            return;
        }
        let bound = if score <= alpha {
            Bound::Upper
        } else if score >= beta {
            Bound::Lower
        } else {
            Bound::Exact
        };
        *entry = TtEntry {
            key,
            depth: depth as i8,
            score,
            bound,
            best_move,
        };
    }

    /// Best move hint for move ordering (may be stale).
    pub fn best_move(&self, key: u64) -> Option<Move> {
        let entry = &self.entries[Self::index(key)];
        if entry.key == key {
            entry.best_move
        } else {
            None
        }
    }
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new()
    }
}
