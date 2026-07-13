use crate::fen::Position;
use crate::movement::{Move, MoveFlags};
use crate::square::{offset, Square};
use crate::types::{Color, PieceKind};

pub(crate) const KNIGHT_DELTAS: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (-1, 2),
    (1, -2),
    (-2, -1),
    (-1, -2),
    (-2, 1),
];

pub(crate) const KING_DELTAS: [(i8, i8); 8] = [
    (0, 1),
    (0, -1),
    (1, 0),
    (-1, 0),
    (1, 1),
    (-1, 1),
    (-1, -1),
    (1, -1),
];

pub(crate) const BISHOP_DIRECTIONS: [(i8, i8); 4] = [(1, 1), (-1, 1), (-1, -1), (1, -1)];

pub(crate) const ROOK_DIRECTIONS: [(i8, i8); 4] = [(0, 1), (1, 0), (-1, 0), (0, -1)];

/// Build a pseudo-legal move; generator bugs surface as invalid geometry in debug builds.
#[inline]
pub(crate) fn mk(from: Square, to: Square, promotion: Option<PieceKind>, flags: MoveFlags) -> Move {
    match Move::with_flags(from, to, promotion, flags) {
        Ok(m) => m,
        Err(_) => {
            debug_assert!(
                false,
                "move generator produced a geometrically invalid move"
            );
            Move {
                from,
                to,
                promotion,
                flags,
            }
        }
    }
}

fn push_promotions(from: Square, to: Square, extra: MoveFlags, out: &mut Vec<Move>) {
    for kind in [
        PieceKind::Queen,
        PieceKind::Rook,
        PieceKind::Bishop,
        PieceKind::Knight,
    ] {
        out.push(mk(from, to, Some(kind), extra));
    }
}

impl Position {
    /// All pseudo-legal moves for the side to move (no castling, no legality filter).
    pub(crate) fn pseudo_legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::with_capacity(48);
        let us = self.side_to_move;

        for sq in Square::ALL {
            let piece = match self.board.get(sq) {
                Some(p) if p.color == us => p,
                _ => continue,
            };
            match piece.kind {
                PieceKind::Pawn => push_pawn_moves(self, sq, us, &mut moves),
                PieceKind::Knight => push_leaper_moves(self, sq, us, &KNIGHT_DELTAS, &mut moves),
                PieceKind::Bishop => {
                    push_slider_moves(self, sq, us, &BISHOP_DIRECTIONS, &mut moves);
                }
                PieceKind::Rook => push_slider_moves(self, sq, us, &ROOK_DIRECTIONS, &mut moves),
                PieceKind::Queen => {
                    push_slider_moves(self, sq, us, &BISHOP_DIRECTIONS, &mut moves);
                    push_slider_moves(self, sq, us, &ROOK_DIRECTIONS, &mut moves);
                }
                PieceKind::King => push_leaper_moves(self, sq, us, &KING_DELTAS, &mut moves),
            }
        }
        moves
    }
}

fn push_pawn_moves(pos: &Position, from: Square, color: Color, out: &mut Vec<Move>) {
    let forward = color.pawn_step();
    let starting_rank = color.starting_pawn_rank();
    let promotion_rank = color.promotion_rank();
    let is_promotion = from.rank() == promotion_rank;

    if let Some(one) = offset(from, 0, forward) {
        if pos.board.get(one).is_none() {
            if is_promotion {
                push_promotions(from, one, MoveFlags::PROMOTION, out);
            } else {
                out.push(mk(from, one, None, MoveFlags::NONE));
            }

            if from.rank() == starting_rank {
                if let Some(two) = offset(from, 0, forward * 2) {
                    if pos.board.get(two).is_none() {
                        out.push(mk(from, two, None, MoveFlags::DOUBLE_PAWN_PUSH));
                    }
                }
            }
        }
    }

    for file_direction in [-1i8, 1] {
        let Some(to) = offset(from, file_direction, forward) else {
            continue;
        };

        if let Some(p) = pos.board.get(to) {
            if p.color != color && p.kind != PieceKind::King {
                if is_promotion {
                    let mut flags = MoveFlags::NONE;
                    flags.insert(MoveFlags::PROMOTION);
                    flags.insert(MoveFlags::CAPTURE);
                    push_promotions(from, to, flags, out);
                } else {
                    out.push(mk(from, to, None, MoveFlags::CAPTURE));
                }
            }
            continue;
        }
        if Some(to) == pos.en_passant {
            out.push(mk(from, to, None, MoveFlags::EN_PASSANT));
        }
    }
}

fn push_leaper_moves(
    pos: &Position,
    from: Square,
    color: Color,
    deltas: &[(i8, i8)],
    out: &mut Vec<Move>,
) {
    for &(file_offset, rank_offset) in deltas {
        let Some(to) = offset(from, file_offset, rank_offset) else {
            continue;
        };
        match pos.board.get(to) {
            Some(p) if p.color == color => {}
            Some(_) => out.push(mk(from, to, None, MoveFlags::CAPTURE)),
            None => out.push(mk(from, to, None, MoveFlags::NONE)),
        }
    }
}

fn push_slider_moves(
    pos: &Position,
    from: Square,
    color: Color,
    deltas: &[(i8, i8)],
    out: &mut Vec<Move>,
) {
    for &(file_direction, rank_direction) in deltas {
        let mut current = from;
        while let Some(to) = offset(current, file_direction, rank_direction) {
            match pos.board.get(to) {
                Some(p) if p.color == color => break,
                Some(_) => {
                    out.push(mk(from, to, None, MoveFlags::CAPTURE));
                    break;
                }
                None => {
                    out.push(mk(from, to, None, MoveFlags::NONE));
                    current = to;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::Position;

    fn pseudo_count(fen: &str) -> usize {
        Position::from_fen_syntax(fen)
            .expect("syntax ok")
            .pseudo_legal_moves()
            .len()
    }

    #[test]
    fn starting_position_has_20_pseudo_legal_moves() {
        assert_eq!(pseudo_count(&Position::starting_position().to_fen()), 20);
    }

    #[test]
    fn lone_center_knight_has_8() {
        assert_eq!(pseudo_count("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1"), 8 + 5);
    }

    #[test]
    fn corner_knight_has_2() {
        let moves = Position::from_fen_syntax("4k3/8/8/4K3/8/8/8/N7 w - - 0 1")
            .expect("syntax ok")
            .pseudo_legal_moves();

        let knight_moves = moves
            .iter()
            .filter(|m| m.from == Square::from_algebraic("a1").expect("valid square"))
            .count();

        assert_eq!(knight_moves, 2);
    }

    #[test]
    fn rook_slides_and_stops_at_capture() {
        let position =
            Position::from_fen_syntax("4k3/8/8/p7/8/8/8/R3K3 w - - 0 1").expect("syntax ok");
        let moves: Vec<_> = position
            .pseudo_legal_moves()
            .into_iter()
            .filter(|m| m.from == Square::from_algebraic("a1").expect("valid square"))
            .collect();

        assert_eq!(moves.len(), 7);
        assert!(moves.iter().any(|m| {
            m.to == Square::from_algebraic("a5").expect("valid square")
                && m.flags.contains(MoveFlags::CAPTURE)
        }));
    }

    #[test]
    fn pawn_double_push_flagged() {
        let position =
            Position::from_fen_syntax("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1").expect("syntax ok");
        let e2 = Square::from_algebraic("e2").expect("valid square");
        let e4 = Square::from_algebraic("e4").expect("valid square");
        let double = position
            .pseudo_legal_moves()
            .into_iter()
            .find(|m| m.from == e2 && m.to == e4)
            .expect("double push exists");
        assert!(double.flags.contains(MoveFlags::DOUBLE_PAWN_PUSH));
    }

    #[test]
    fn pawn_promotion_expands_to_4() {
        let position =
            Position::from_fen_syntax("3k4/4P3/8/8/8/8/8/4K3 w - - 0 1").expect("syntax ok");
        let e7 = Square::from_algebraic("e7").expect("valid square");
        let promotions: Vec<_> = position
            .pseudo_legal_moves()
            .into_iter()
            .filter(|m| m.from == e7 && m.promotion.is_some())
            .collect();

        assert_eq!(promotions.len(), 4);
        assert!(promotions
            .iter()
            .all(|m| m.flags.contains(MoveFlags::PROMOTION)));
    }

    #[test]
    fn en_passant_guaranteed_with_flag() {
        let position =
            Position::from_fen_syntax("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").expect("syntax ok");
        let e5 = Square::from_algebraic("e5").expect("valid square");
        let d6 = Square::from_algebraic("d6").expect("valid square");

        let en_passant = position
            .pseudo_legal_moves()
            .into_iter()
            .find(|m| m.from == e5 && m.to == d6)
            .expect("en passant exists");
        assert!(en_passant.flags.contains(MoveFlags::EN_PASSANT));
        assert!(en_passant.flags.contains(MoveFlags::CAPTURE));
    }
}
