//! Trait for position evaluation backends (native, external engine, etc.).

use crate::phase::GamePhase;
use crate::report::{PositionEval, Score};
use gambit_db::{Move, Position};

/// Backend that evaluates a single chess position.
pub trait PositionEvaluator: Send {
    /// Evaluate `pos` at the given ply and detected phase.
    fn evaluate(&mut self, pos: &Position, ply: u32, phase: GamePhase) -> PositionEval;
}

/// Native analyzer-backed evaluator.
pub struct NativeEvaluator {
    analyzer: crate::Analyzer,
    depth: u32,
}

impl NativeEvaluator {
    /// Create a native evaluator searching to `depth`.
    pub fn new(depth: u32) -> Self {
        Self {
            analyzer: crate::Analyzer::new(),
            depth,
        }
    }
}

impl PositionEvaluator for NativeEvaluator {
    fn evaluate(&mut self, pos: &Position, _ply: u32, _phase: GamePhase) -> PositionEval {
        let result = self
            .analyzer
            .search(pos, crate::SearchLimits::depth(self.depth));
        PositionEval {
            score: result.score,
            best_move: result.best_move,
            depth: result.depth,
            pv: result.pv,
            source: EvalSource::Native,
        }
    }
}

/// Source of an evaluation result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvalSource {
    /// External UCI engine (e.g. Stockfish).
    Stockfish,
    /// Syzygy tablebase.
    Syzygy,
    /// Corpus book statistics.
    Corpus,
    /// Native gambit-analysis search.
    Native,
}

impl EvalSource {
    /// PostgreSQL enum string.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stockfish => "stockfish",
            Self::Syzygy => "syzygy",
            Self::Corpus => "corpus",
            Self::Native => "native",
        }
    }
}

/// Convert a UCI search result into a position eval.
pub fn eval_from_uci(
    score_cp: Option<i32>,
    score_mate: Option<i32>,
    best_move: Move,
    pv: Vec<Move>,
    depth: u32,
) -> PositionEval {
    let score = match (score_mate, score_cp) {
        (Some(m), _) => Score::Mate(m),
        (None, Some(cp)) => Score::Cp(cp),
        (None, None) => Score::Cp(0),
    };
    PositionEval {
        score,
        best_move,
        depth,
        pv,
        source: EvalSource::Stockfish,
    }
}
