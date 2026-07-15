//! Search output types.

use crate::classify::MoveClass;
use crate::evaluator::EvalSource;
use gambit_db::Move;

/// Large centipawn value used to represent forced mate in loss calculations.
pub const MATE_CP: i32 = 30_000;

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

    /// Normalize to centipawns for classification arithmetic.
    pub fn to_cp(self) -> i32 {
        match self {
            Score::Cp(cp) => cp,
            Score::Mate(plies) => {
                if plies > 0 {
                    MATE_CP - plies
                } else {
                    -MATE_CP - plies
                }
            }
        }
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

/// Evaluation of a single position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionEval {
    /// Engine score from side to move.
    pub score: Score,
    /// Best move found.
    pub best_move: Move,
    /// Search depth reached.
    pub depth: u32,
    /// Principal variation.
    pub pv: Vec<Move>,
    /// Backend that produced this eval.
    pub source: EvalSource,
}

impl PositionEval {
    /// Centipawn score for persistence.
    pub fn eval_cp(&self) -> i32 {
        self.score.to_cp()
    }

    /// Mate plies if mate score, else None.
    pub fn mate_plies(&self) -> Option<i32> {
        match self.score {
            Score::Mate(p) => Some(p),
            Score::Cp(_) => None,
        }
    }
}

/// Per-ply analysis result for database persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlyAnalysis {
    /// Ply number (1-based).
    pub ply: u32,
    /// Eval before the move (side to move at position).
    pub eval_before: i32,
    /// Eval after the move (side to move at new position).
    pub eval_after: i32,
    /// Engine best move from the pre-move position.
    pub best_move: Move,
    /// Centipawn loss for the move played.
    pub cp_loss: i32,
    /// Move quality classification.
    pub move_class: MoveClass,
    /// Search depth used.
    pub depth: u32,
    /// Evaluation backend source.
    pub source: EvalSource,
}

/// Game-level analysis summary.
#[derive(Debug, Clone, PartialEq)]
pub struct GameReviewSummary {
    /// Per-ply analysis rows.
    pub plies: Vec<PlyAnalysis>,
    /// White accuracy percentage.
    pub accuracy_white: Option<f64>,
    /// Black accuracy percentage.
    pub accuracy_black: Option<f64>,
    /// White blunder count.
    pub blunders_white: u32,
    /// Black blunder count.
    pub blunders_black: u32,
}
