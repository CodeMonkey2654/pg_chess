//! Transposition table keyed on Zobrist hash.

use gambit_db::Move;

const DEFAULT_TT_BITS: usize = 21;

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
    mask: usize,
}

impl TranspositionTable {
    /// Allocate default table (~64 MB of entries).
    pub fn new() -> Self {
        Self::with_bits(DEFAULT_TT_BITS)
    }

    /// Allocate table with `2^bits` entries.
    pub fn with_bits(bits: usize) -> Self {
        let bits = bits.clamp(16, 24);
        let size = 1usize << bits;
        Self {
            entries: vec![TtEntry::default(); size],
            mask: size - 1,
        }
    }

    /// Clear all entries (new game).
    pub fn clear(&mut self) {
        for e in &mut self.entries {
            *e = TtEntry::default();
        }
    }

    fn index(&self, key: u64) -> usize {
        (key as usize) & self.mask
    }

    /// Probe for a cutoff score at `depth`.
    pub fn probe(&self, key: u64, depth: i32, alpha: i32, beta: i32) -> Option<i32> {
        let entry = &self.entries[self.index(key)];
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
        let idx = self.index(key);
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
        let entry = &self.entries[self.index(key)];
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
