//! Walk a game's plies and produce per-move analysis.

use crate::classify::{accuracy, classify_cp_loss, cp_loss_for_move, MoveClass};
use crate::evaluator::PositionEvaluator;
use crate::phase::{detect_phase, DEFAULT_OPENING_PLY};
use crate::report::{GameReviewSummary, PlyAnalysis};
use gambit_db::{Move, Position};

/// Input ply for game analysis.
#[derive(Debug, Clone)]
pub struct GamePly {
    /// Ply number (1-based).
    pub ply: u32,
    /// UCI move played.
    pub uci: String,
    /// FEN before this move (position to evaluate).
    pub fen_before: String,
}

/// Analyze all plies in a game using the given evaluator backend.
pub struct GameAnalyzer<E: PositionEvaluator> {
    evaluator: E,
    opening_ply_limit: u32,
}

impl<E: PositionEvaluator> GameAnalyzer<E> {
    /// Create a game analyzer with default opening ply limit.
    pub fn new(evaluator: E) -> Self {
        Self {
            evaluator,
            opening_ply_limit: DEFAULT_OPENING_PLY,
        }
    }

    /// Set opening phase ply limit.
    pub fn with_opening_ply_limit(mut self, limit: u32) -> Self {
        self.opening_ply_limit = limit;
        self
    }

    /// Analyze all plies and return per-ply results plus accuracy summary.
    pub fn analyze(&mut self, plies: &[GamePly]) -> Result<GameReviewSummary, String> {
        let mut results = Vec::with_capacity(plies.len());
        let mut white_classes = Vec::new();
        let mut black_classes = Vec::new();

        for ply in plies {
            let pos = Position::from_fen(&ply.fen_before).map_err(|e| e.to_string())?;
            let phase = detect_phase(&pos, ply.ply, self.opening_ply_limit);
            let before = self.evaluator.evaluate(&pos, ply.ply, phase);
            let eval_before = before.eval_cp();

            let mv = Move::from_uci(&ply.uci).map_err(|e| e.to_string())?;
            let after_pos = pos.apply_move(mv).map_err(|e| e.to_string())?;
            let after_phase = detect_phase(&after_pos, ply.ply, self.opening_ply_limit);
            let after = self
                .evaluator
                .evaluate(&after_pos, ply.ply + 1, after_phase);
            let eval_after = after.eval_cp();

            let mover_was_white = ply.ply % 2 == 1;
            let cp_loss = cp_loss_for_move(eval_before, eval_after, mover_was_white);
            let move_class = classify_cp_loss(cp_loss);

            if mover_was_white {
                white_classes.push(move_class);
            } else {
                black_classes.push(move_class);
            }

            results.push(PlyAnalysis {
                ply: ply.ply,
                eval_before,
                eval_after,
                best_move: before.best_move,
                cp_loss,
                move_class,
                depth: before.depth,
                source: before.source,
            });
        }

        let blunders_white = white_classes
            .iter()
            .filter(|c| **c == MoveClass::Blunder)
            .count() as u32;
        let blunders_black = black_classes
            .iter()
            .filter(|c| **c == MoveClass::Blunder)
            .count() as u32;

        Ok(GameReviewSummary {
            plies: results,
            accuracy_white: accuracy(&white_classes),
            accuracy_black: accuracy(&black_classes),
            blunders_white,
            blunders_black,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evaluator::NativeEvaluator;

    #[test]
    fn analyze_empty_game() {
        let mut analyzer = GameAnalyzer::new(NativeEvaluator::new(2));
        let summary = analyzer.analyze(&[]).expect("analyze");
        assert!(summary.plies.is_empty());
        assert_eq!(summary.accuracy_white, None);
    }
}
