//! Move generation, legality filtering, and move application.

mod attack_tables;
mod attacks;
mod castling;
mod error;
mod legality;
mod pseudo;
mod state;

pub use error::MoveError;

use crate::fen::Position;
use crate::movement::Move;
use crate::types::PieceKind;

impl Position {
    pub(crate) fn resolve_move(&self, m: Move) -> Result<Move, MoveError> {
        self.legal_moves()
            .into_iter()
            .find(|lm| lm.from == m.from && lm.to == m.to && lm.promotion == m.promotion)
            .ok_or(MoveError::Illegal)
    }

    /// Apply a legal move, returning the resulting position.
    pub fn apply_move(&self, m: Move) -> Result<Position, MoveError> {
        let legal = self.resolve_move(m)?;
        let mut pos = self.clone();
        pos.make_move(legal)?;
        Ok(pos)
    }

    /// Whether the side to move is checkmated.
    pub fn is_checkmate(&self) -> bool {
        self.is_in_check(self.side_to_move) && self.legal_moves().is_empty()
    }

    /// Whether the side to move is stalemated.
    pub fn is_stalemate(&self) -> bool {
        !self.is_in_check(self.side_to_move) && self.legal_moves().is_empty()
    }

    /// Whether the fifty-move rule applies (100 halfmoves without capture or pawn move).
    pub fn is_fifty_move_draw(&self) -> bool {
        self.halfmove_clock >= 100
    }

    /// Whether neither side has sufficient mating material.
    pub fn is_insufficient_material(&self) -> bool {
        let mut minors = 0u32;
        let mut bishops_light = 0u32;
        let mut bishops_dark = 0u32;
        for (sq, p) in self.board.iter_occupied() {
            match p.kind {
                PieceKind::King => {}
                PieceKind::Knight => minors += 1,
                PieceKind::Bishop => {
                    minors += 1;
                    if (sq.file() + sq.rank()) % 2 == 0 {
                        bishops_dark += 1;
                    } else {
                        bishops_light += 1;
                    }
                }
                _ => return false,
            }
        }
        match minors {
            0 => true,
            1 => true,
            2 => bishops_light == 0 || bishops_dark == 0,
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::Move;
    use crate::square::Square;
    use crate::types::Color;

    #[test]
    fn detects_back_rank_mate() {
        let pos = Position::from_fen("4R1k1/5ppp/8/8/8/8/8/6K1 b - - 0 1").expect("valid fen");
        assert!(pos.is_in_check(Color::Black));
        assert!(pos.is_checkmate());
        assert!(!pos.is_stalemate());
    }

    #[test]
    fn detects_stalemate() {
        let pos = Position::from_fen("k7/2Q5/2K5/8/8/8/8/8 b - - 0 1").expect("valid fen");
        assert!(!pos.is_in_check(Color::Black));
        assert!(pos.is_stalemate());
        assert!(!pos.is_checkmate());
    }

    #[test]
    fn apply_rejects_illegal_move() {
        let pos = Position::starting_position();
        let bad = Move::new(
            Square::from_algebraic("e2").expect("valid square"),
            Square::from_algebraic("e5").expect("valid square"),
            None,
        )
        .expect("valid geometry");
        assert_eq!(pos.apply_move(bad), Err(MoveError::Illegal));
    }

    #[test]
    fn double_push_sets_en_passant_target() {
        let pos = Position::starting_position();
        let e2e4 = Move::new(
            Square::from_algebraic("e2").expect("valid square"),
            Square::from_algebraic("e4").expect("valid square"),
            None,
        )
        .expect("valid geometry");
        let after = pos.apply_move(e2e4).expect("legal move");
        assert_eq!(
            after.en_passant,
            Some(Square::from_algebraic("e3").expect("valid square"))
        );
        assert_eq!(after.side_to_move, Color::Black);
    }

    #[test]
    fn en_passant_target_clears_next_move() {
        let pos = Position::starting_position();
        let e2e4 = Move::new(
            Square::from_algebraic("e2").expect("valid square"),
            Square::from_algebraic("e4").expect("valid square"),
            None,
        )
        .expect("valid geometry");
        let after1 = pos.apply_move(e2e4).expect("legal move");
        let a7a6 = Move::new(
            Square::from_algebraic("a7").expect("valid square"),
            Square::from_algebraic("a6").expect("valid square"),
            None,
        )
        .expect("valid geometry");
        let after2 = after1.apply_move(a7a6).expect("legal move");
        assert_eq!(after2.en_passant, None);
    }

    #[test]
    fn king_move_loses_castling_rights() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1").expect("valid fen");
        let ke1e2 = Move::new(
            Square::from_algebraic("e1").expect("valid square"),
            Square::from_algebraic("e2").expect("valid square"),
            None,
        )
        .expect("valid geometry");
        let after = pos.apply_move(ke1e2).expect("legal move");
        assert!(!after.castling.white_kingside);
        assert!(!after.castling.white_queenside);
    }

    #[test]
    fn capturing_rook_removes_opponent_right() {
        let pos = Position::from_fen("r3k3/8/8/8/8/8/8/R3K3 w Qq - 0 1").expect("valid fen");
        let rxa8 = Move::new(
            Square::from_algebraic("a1").expect("valid square"),
            Square::from_algebraic("a8").expect("valid square"),
            None,
        )
        .expect("valid geometry");
        let after = pos.apply_move(rxa8).expect("legal move");
        assert!(!after.castling.black_queenside);
    }

    #[test]
    fn halfmove_clock_resets_on_capture_and_pawn() {
        let pos = Position::from_fen("4k3/8/8/8/8/5N2/4P3/4K3 w - - 5 10").expect("valid fen");
        let nf3g5 = Move::new(
            Square::from_algebraic("f3").expect("valid square"),
            Square::from_algebraic("g5").expect("valid square"),
            None,
        )
        .expect("valid geometry");
        let after_knight = pos.apply_move(nf3g5).expect("legal move");
        assert_eq!(after_knight.halfmove_clock, 6);

        let e2e4 = Move::new(
            Square::from_algebraic("e2").expect("valid square"),
            Square::from_algebraic("e4").expect("valid square"),
            None,
        )
        .expect("valid geometry");
        let after_pawn = pos.apply_move(e2e4).expect("legal move");
        assert_eq!(after_pawn.halfmove_clock, 0);
    }

    #[test]
    fn scholars_mate_is_checkmate() {
        let moves = ["e2e4", "e7e5", "f1c4", "b8c6", "d1h5", "g8f6", "h5f7"];
        let mut pos = Position::starting_position();
        for uci in moves {
            let m = Move::from_uci(uci).expect("valid uci");
            pos = pos.apply_move(m).expect("legal move");
        }
        assert!(pos.is_checkmate());
    }

    #[test]
    fn insufficient_material_detects_bare_kings() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").expect("valid fen");
        assert!(pos.is_insufficient_material());
        let rook = Position::from_fen("4k3/8/8/8/8/8/8/R3K3 w - - 0 1").expect("valid fen");
        assert!(!rook.is_insufficient_material());
    }
}
