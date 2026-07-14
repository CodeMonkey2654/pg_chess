//! Search output types.

use gambit_db::Move;

/// Centipawn or mate score from the side to move's perspective.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Score {
    /// Centipawns (positive = side to move is better).
    Cp(i32),
    /// Forced mate in N plies (positive = side to move mates).
    Mate(i32),
}

impl Score {
    /// Mate score for side to move winning in `plies`.
    pub fn mate_in(plies: i32) -> Self {
        Score::Mate(plies)
    }

    /// Whether this is a winning mate for side to move.
    pub fn is_mate(&self) -> bool {
        matches!(self, Score::Mate(_))
    }
}

/// Corpus statistics for a single move (when book feature enabled).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MoveStat {
    /// Move played in corpus games.
    pub uci: Move,
    /// Times this move was played from the position.
    pub count: u64,
    /// White win count in corpus.
    pub white_wins: u64,
    /// Black win count in corpus.
    pub black_wins: u64,
    /// Draw count in corpus.
    pub draws: u64,
}

/// Result of analyzing a position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    /// Best move found.
    pub best_move: Move,
    /// Score of the principal variation.
    pub score: Score,
    /// Principal variation (best line).
    pub pv: Vec<Move>,
    /// Depth reached (full-width plies).
    pub depth: u32,
    /// Nodes searched.
    pub nodes: u64,
    /// Wall time in milliseconds.
    pub time_ms: u64,
    /// Corpus move stats at root (if book loaded).
    pub corpus: Option<Vec<MoveStat>>,
}
