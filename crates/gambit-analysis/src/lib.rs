//! Chess analysis engine: search, evaluation, optional corpus book.

#![warn(missing_docs)]

mod book;
mod classify;
mod eval;
mod evaluator;
mod game;
mod limits;
mod order;
mod phase;
mod report;
mod search;
mod see;
mod tt;

pub use book::{write_book, CorpusBook};
pub use classify::{accuracy, classify_cp_loss, cp_loss_for_move, MoveClass};
pub use evaluator::{eval_from_uci, EvalSource, NativeEvaluator, PositionEvaluator};
pub use game::{GameAnalyzer, GamePly};
pub use limits::SearchLimits;
pub use phase::{detect_phase, GamePhase, DEFAULT_OPENING_PLY};
pub use report::{
    Analysis, GameReviewSummary, MoveStat, PlyAnalysis, PositionEval, Score, MATE_CP,
};
pub use search::Analyzer;
