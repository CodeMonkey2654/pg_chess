//! Game phase detection for hybrid analysis routing.

use gambit_db::{PieceKind, Position};

/// Chess game phase for analysis routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GamePhase {
    /// Opening theory / corpus-heavy phase.
    Opening,
    /// Middlegame tactical/strategic phase.
    Middlegame,
    /// Endgame with reduced material or tablebase coverage.
    Endgame,
}

/// Default ply limit for opening phase.
pub const DEFAULT_OPENING_PLY: u32 = 14;

/// Piece count threshold for endgame detection.
pub const ENDGAME_PIECE_THRESHOLD: u32 = 7;

/// Classify game phase from position and ply.
pub fn detect_phase(pos: &Position, ply: u32, opening_ply_limit: u32) -> GamePhase {
    let pieces = count_pieces(pos);
    if pieces <= ENDGAME_PIECE_THRESHOLD {
        return GamePhase::Endgame;
    }
    if ply <= opening_ply_limit {
        return GamePhase::Opening;
    }
    GamePhase::Middlegame
}

/// Count non-king pieces on the board.
pub fn count_pieces(pos: &Position) -> u32 {
    let mut n = 0u32;
    for sq in 0..64u8 {
        if let Some(piece) = pos.board.get(gambit_db::Square(sq)) {
            if piece.kind != PieceKind::King {
                n += 1;
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use gambit_db::Position;

    #[test]
    fn startpos_is_opening() {
        let pos = Position::starting_position();
        assert_eq!(
            detect_phase(&pos, 1, DEFAULT_OPENING_PLY),
            GamePhase::Opening
        );
    }

    #[test]
    fn low_material_is_endgame() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").expect("fen");
        assert_eq!(
            detect_phase(&pos, 50, DEFAULT_OPENING_PLY),
            GamePhase::Endgame
        );
    }
}
