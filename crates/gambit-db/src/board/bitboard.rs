//! Occupancy bitboards mirroring the mailbox board.

use crate::square::Square;
use crate::types::{Color, Piece, PieceKind};

/// Occupancy bitboards mirroring a mailbox board.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct BitboardBoard {
    white: u64,
    black: u64,
    pawns: u64,
    knights: u64,
    bishops: u64,
    rooks: u64,
    queens: u64,
    kings: u64,
}

impl BitboardBoard {
    /// Build bitboards from a mailbox board.
    pub(crate) fn from_squares(squares: &[Option<Piece>; 64]) -> Self {
        let mut bb = Self::default();
        for (idx, piece) in squares.iter().enumerate() {
            if let Some(p) = piece {
                bb.set(Square(idx as u8), *p);
            }
        }
        bb
    }

    /// Set piece on square.
    pub(crate) fn set(&mut self, sq: Square, piece: Piece) {
        self.clear(sq);
        let mask = 1u64 << sq.index();
        match piece.color {
            Color::White => self.white |= mask,
            Color::Black => self.black |= mask,
        }
        match piece.kind {
            PieceKind::Pawn => self.pawns |= mask,
            PieceKind::Knight => self.knights |= mask,
            PieceKind::Bishop => self.bishops |= mask,
            PieceKind::Rook => self.rooks |= mask,
            PieceKind::Queen => self.queens |= mask,
            PieceKind::King => self.kings |= mask,
        }
    }

    /// Remove piece from square.
    pub(crate) fn clear(&mut self, sq: Square) {
        let mask = !(1u64 << sq.index());
        self.white &= mask;
        self.black &= mask;
        self.pawns &= mask;
        self.knights &= mask;
        self.bishops &= mask;
        self.rooks &= mask;
        self.queens &= mask;
        self.kings &= mask;
    }

    /// Combined occupancy for `color`.
    pub(crate) fn occupancy(&self, color: Color) -> u64 {
        match color {
            Color::White => self.white,
            Color::Black => self.black,
        }
    }

    /// Piece bitboard for `color` and `kind`.
    pub(crate) fn pieces(&self, color: Color, kind: PieceKind) -> u64 {
        let color_mask = self.occupancy(color);
        let kind_mask = match kind {
            PieceKind::Pawn => self.pawns,
            PieceKind::Knight => self.knights,
            PieceKind::Bishop => self.bishops,
            PieceKind::Rook => self.rooks,
            PieceKind::Queen => self.queens,
            PieceKind::King => self.kings,
        };
        color_mask & kind_mask
    }

    /// All occupied squares.
    pub(crate) fn all_occupied(&self) -> u64 {
        self.white | self.black
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Board;

    #[test]
    fn startpos_has_32_occupied() {
        let board = Board::starting_position();
        let bb = board.occupancy();
        assert_eq!(bb.all_occupied().count_ones(), 32);
        assert_eq!(bb.occupancy(Color::White).count_ones(), 16);
    }
}
