//! Hybrid Stockfish + native position evaluator.

use crate::analyze::{resolve_engine_path, AnalyzeOptions};
use anyhow::Result;
use gambit_analysis::{
    detect_phase, eval_from_uci, GamePhase, NativeEvaluator, PositionEval, PositionEvaluator,
    DEFAULT_OPENING_PLY,
};
use gambit_db::Position;
use gambit_uci::EnginePool;

/// Routes positions to Stockfish when available, else native search.
pub struct HybridEvaluator {
    pool: Option<EnginePool>,
    native: NativeEvaluator,
    depth: u32,
}

impl HybridEvaluator {
    /// Build evaluator from analyze options.
    pub fn new(options: &AnalyzeOptions) -> Result<Self> {
        let depth = options.depth;
        let pool = resolve_engine_path(options).and_then(|path| {
            match EnginePool::spawn(&path, &[], options.workers) {
                Ok(p) => Some(p),
                Err(e) => {
                    tracing::warn!(error = %e, "Stockfish unavailable, using native search");
                    None
                }
            }
        });
        Ok(Self {
            pool,
            native: NativeEvaluator::new(depth),
            depth,
        })
    }
}

impl PositionEvaluator for HybridEvaluator {
    fn evaluate(&mut self, pos: &Position, ply: u32, phase: GamePhase) -> PositionEval {
        let _phase = phase;
        let fen = pos.to_fen();
        if let Some(pool) = &self.pool {
            let search_depth = match detect_phase(pos, ply, DEFAULT_OPENING_PLY) {
                GamePhase::Opening => self.depth.saturating_sub(4).max(6),
                GamePhase::Endgame => self.depth,
                GamePhase::Middlegame => self.depth,
            };
            if let Ok(result) = pool.search_depth_with_info(&fen, &[], search_depth) {
                return eval_from_uci(
                    result.info.score_cp,
                    result.info.score_mate,
                    result.result.bestmove,
                    result.info.pv,
                    result.info.depth.unwrap_or(search_depth),
                );
            }
        }
        self.native.evaluate(pos, ply, phase)
    }
}
