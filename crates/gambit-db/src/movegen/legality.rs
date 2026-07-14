use crate::fen::Position;
use crate::movegen::castling::gen_castling;
use crate::movegen::movelist::MoveList;
use crate::movement::Move;

impl Position {
    /// All legal moves for the side to move.
    pub fn legal_moves(&self) -> Vec<Move> {
        let mut scratch = self.clone();
        let mut list = MoveList::new();
        scratch.generate_legal_moves(&mut list);
        list.to_vec()
    }

    /// Fill `out` with legal moves using in-place make/unmake (no position clone per candidate).
    pub fn generate_legal_moves(&mut self, out: &mut MoveList) {
        out.clear();
        let us = self.side_to_move;
        let mut candidates = self.pseudo_legal_moves();
        gen_castling(self, &mut candidates);

        for m in candidates {
            let Ok(undo) = self.make_move(m) else {
                continue;
            };
            let king_safe = self
                .king_square(us)
                .map(|k| !self.board.is_square_attacked(k, us.flip()))
                .unwrap_or(true);
            self.unmake_move(undo);
            if king_safe {
                out.push(m);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::fen::Position;
    use crate::movegen::movelist::MoveList;
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

    #[test]
    fn generate_matches_legal_moves_startpos() {
        let mut pos = Position::starting_position();
        let mut list = MoveList::new();
        pos.generate_legal_moves(&mut list);
        assert_eq!(list.len(), 20);
    }
}
