use crate::board::Board;
use crate::fen::Position;
use crate::movegen::attack_tables::{KING_ATTACKS, KNIGHT_ATTACKS};
use crate::square::{offset, Square};
use crate::types::{Color, PieceKind};

pub(crate) const BISHOP_DIRECTIONS: [(i8, i8); 4] = [(1, 1), (-1, 1), (-1, -1), (1, -1)];

pub(crate) const ROOK_DIRECTIONS: [(i8, i8); 4] = [(0, 1), (1, 0), (-1, 0), (0, -1)];

impl Board {
    /// Whether `target` is attacked by a piece of `by`.
    pub fn is_square_attacked(&self, target: Square, by: Color) -> bool {
        let occ = self.occupancy();
        let target_idx = target.index();

        let pawn_back: i8 = -by.pawn_step();
        for file_direction in [-1i8, 1] {
            if let Some(sq) = offset(target, file_direction, pawn_back) {
                if let Some(p) = self.get(sq) {
                    if p.color == by && p.kind == PieceKind::Pawn {
                        return true;
                    }
                }
            }
        }

        let knights = occ.pieces(by, PieceKind::Knight);
        if (KNIGHT_ATTACKS[target_idx] & knights) != 0 {
            return true;
        }

        let kings = occ.pieces(by, PieceKind::King);
        if (KING_ATTACKS[target_idx] & kings) != 0 {
            return true;
        }

        if self.slider_hits(
            target,
            by,
            &ROOK_DIRECTIONS,
            PieceKind::Rook,
            occ.all_occupied(),
        ) {
            return true;
        }

        if self.slider_hits(
            target,
            by,
            &BISHOP_DIRECTIONS,
            PieceKind::Bishop,
            occ.all_occupied(),
        ) {
            return true;
        }

        false
    }

    fn slider_hits(
        &self,
        target: Square,
        by: Color,
        dirs: &[(i8, i8)],
        line_kind: PieceKind,
        occupied: u64,
    ) -> bool {
        for &(file_direction, rank_direction) in dirs {
            let mut current = target;
            while let Some(sq) = offset(current, file_direction, rank_direction) {
                let mask = 1u64 << sq.index();
                if occupied & mask == 0 {
                    current = sq;
                    continue;
                }
                if let Some(p) = self.get(sq) {
                    if p.color == by && (p.kind == line_kind || p.kind == PieceKind::Queen) {
                        return true;
                    }
                }
                break;
            }
        }
        false
    }
}

impl Position {
    /// Whether `color`'s king is in check.
    pub fn is_in_check(&self, color: Color) -> bool {
        match self.king_square(color) {
            Some(k) => self.board.is_square_attacked(k, color.flip()),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::Position;
    use crate::movement::MoveFlags;

    #[test]
    fn detects_check() {
        let pos = Position::from_fen_syntax("4r3/8/8/8/8/8/8/4K3 w - - 0 1").expect("syntax ok");
        assert!(pos.is_in_check(Color::White));
    }

    #[test]
    fn no_false_check_when_blocked() {
        let pos = Position::from_fen_syntax("4r3/8/8/8/8/8/4P3/4K3 w - - 0 1").expect("syntax ok");
        assert!(!pos.is_in_check(Color::White));
    }

    #[test]
    fn pawn_attack_direction_is_correct() {
        let pos = Position::from_fen_syntax("4k3/8/8/8/8/8/3p4/8 w - - 0 1").expect("syntax ok");
        let c1 = Square::from_algebraic("c1").expect("valid square");
        let e1 = Square::from_algebraic("e1").expect("valid square");
        let d1 = Square::from_algebraic("d1").expect("valid square");
        assert!(pos.board.is_square_attacked(c1, Color::Black));
        assert!(pos.board.is_square_attacked(e1, Color::Black));
        assert!(!pos.board.is_square_attacked(d1, Color::Black));
    }

    #[test]
    fn kingside_castle_generated_when_legal() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1").expect("valid fen");
        let e1 = Square::from_algebraic("e1").expect("valid square");
        let g1 = Square::from_algebraic("g1").expect("valid square");
        let castle = pos
            .legal_moves()
            .into_iter()
            .find(|m| m.from == e1 && m.to == g1);
        assert!(castle.is_some());
        assert!(castle
            .expect("castle")
            .flags
            .contains(MoveFlags::KING_CASTLE));
    }

    #[test]
    fn cannot_castle_through_attacked_square() {
        let pos = Position::from_fen_syntax("5r2/8/8/8/8/8/8/4K2R w K - 0 1").expect("syntax ok");
        let e1 = Square::from_algebraic("e1").expect("valid square");
        let g1 = Square::from_algebraic("g1").expect("valid square");
        let castle = pos
            .legal_moves()
            .into_iter()
            .find(|m| m.from == e1 && m.to == g1);
        assert!(castle.is_none());
    }
}
