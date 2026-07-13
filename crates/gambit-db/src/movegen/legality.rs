use crate::fen::Position;
use crate::movegen::castling::gen_castling;
use crate::movement::Move;

impl Position {
    /// All legal moves for the side to move.
    pub fn legal_moves(&self) -> Vec<Move> {
        let us = self.side_to_move;
        let mut candidates = self.pseudo_legal_moves();
        gen_castling(self, &mut candidates);

        let mut scratch = self.clone();
        let mut legal = Vec::with_capacity(candidates.len());

        for m in candidates {
            let Ok(undo) = scratch.make_move(m) else {
                continue;
            };
            let king_safe = scratch
                .king_square(us)
                .map(|k| !scratch.board.is_square_attacked(k, us.flip()))
                .unwrap_or(true);
            scratch.unmake_move(undo);
            if king_safe {
                legal.push(m);
            }
        }

        legal
    }
}

#[cfg(test)]
mod tests {
    use crate::fen::Position;
    use crate::square::Square;

    #[test]
    fn pinned_piece_cannot_move() {
        let pos = Position::from_fen_syntax("4r3/8/8/8/8/8/4N3/4K3 w - - 0 1").expect("syntax ok");
        let e2 = Square::from_algebraic("e2").expect("valid square");
        let knight_legal = pos
            .legal_moves()
            .into_iter()
            .filter(|m| m.from == e2)
            .count();
        assert_eq!(knight_legal, 0);
    }
}
