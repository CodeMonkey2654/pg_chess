pub(crate) mod bitboard;

use crate::square::Square;
use crate::types::{Color, Piece, PieceKind};
use bitboard::BitboardBoard;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

fn serialize_squares<S>(squares: &[Option<Piece>; 64], serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    squares.as_slice().serialize(serializer)
}

fn deserialize_squares<'de, D>(deserializer: D) -> Result<[Option<Piece>; 64], D::Error>
where
    D: Deserializer<'de>,
{
    let vec: Vec<Option<Piece>> = Vec::deserialize(deserializer)?;
    vec.try_into().map_err(|v: Vec<Option<Piece>>| {
        serde::de::Error::invalid_length(v.len(), &"an array of length 64")
    })
}

/// Piece placement only — castling, en passant, and clocks live in [`crate::fen::Position`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Board {
    pub(crate) squares: [Option<Piece>; 64],
    occupancy: BitboardBoard,
}

mod board_squares {
    use super::{deserialize_squares, serialize_squares, BitboardBoard, Board};
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(board: &Board, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serialize_squares(&board.squares, serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Board, D::Error>
    where
        D: Deserializer<'de>,
    {
        let squares = deserialize_squares(deserializer)?;
        Ok(Board {
            occupancy: BitboardBoard::from_squares(&squares),
            squares,
        })
    }
}

impl Serialize for Board {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        board_squares::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for Board {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        board_squares::deserialize(deserializer)
    }
}

impl Board {
    /// Empty board with no pieces.
    pub fn empty() -> Board {
        Board {
            squares: [None; 64],
            occupancy: BitboardBoard::default(),
        }
    }

    /// Piece on `sq`, if any.
    #[inline]
    pub fn get(&self, sq: Square) -> Option<Piece> {
        self.squares[sq.index()]
    }

    /// Place a piece on `sq`.
    #[inline]
    pub fn set(&mut self, sq: Square, piece: Piece) {
        self.squares[sq.index()] = Some(piece);
        self.occupancy.set(sq, piece);
    }

    /// Remove and return the piece on `sq`.
    #[inline]
    pub fn clear(&mut self, sq: Square) -> Option<Piece> {
        let removed = self.squares[sq.index()].take();
        if removed.is_some() {
            self.occupancy.clear(sq);
        }
        removed
    }

    /// Iterate occupied squares with their pieces.
    pub fn iter_occupied(&self) -> impl Iterator<Item = (Square, Piece)> + '_ {
        Square::ALL
            .into_iter()
            .filter_map(|sq| self.get(sq).map(|p| (sq, p)))
    }

    /// Cached occupancy bitboards.
    pub(crate) fn occupancy(&self) -> &BitboardBoard {
        &self.occupancy
    }

    /// Standard starting piece placement.
    pub fn starting_position() -> Board {
        use crate::types::{Color::*, PieceKind::*};
        let mut b = Board::empty();

        let back_rank = [Rook, Knight, Bishop, Queen, King, Bishop, Knight, Rook];
        for file in 0..8u8 {
            b.set(
                Square(Color::White.back_rank() * 8 + file),
                Piece::new(White, back_rank[file as usize]),
            );
            b.set(
                Square(Color::White.starting_pawn_rank() * 8 + file),
                Piece::new(White, Pawn),
            );
            b.set(
                Square(Color::Black.starting_pawn_rank() * 8 + file),
                Piece::new(Black, Pawn),
            );
            b.set(
                Square(Color::Black.back_rank() * 8 + file),
                Piece::new(Black, back_rank[file as usize]),
            );
        }
        b
    }

    /// Square of the king for `color`, if present.
    pub fn king_square(&self, color: Color) -> Option<Square> {
        self.iter_occupied()
            .find(|(_, p)| p.color == color && p.kind == PieceKind::King)
            .map(|(sq, _)| sq)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Color, PieceKind};

    #[test]
    fn starting_position_has_expected_pieces() {
        let b = Board::starting_position();

        let count = b.iter_occupied().count();
        assert_eq!(count, 32);

        let e1 = Square::from_algebraic("e1").expect("valid square");
        assert_eq!(b.get(e1), Some(Piece::new(Color::White, PieceKind::King)));

        let d8 = Square::from_algebraic("d8").expect("valid square");
        assert_eq!(b.get(d8), Some(Piece::new(Color::Black, PieceKind::Queen)));

        let a1 = Square::from_algebraic("a1").expect("valid square");
        assert_eq!(b.get(a1), Some(Piece::new(Color::White, PieceKind::Rook)));
    }

    #[test]
    fn king_square_finds_kings() {
        let b = Board::starting_position();
        assert_eq!(b.king_square(Color::White), Square::from_algebraic("e1"));
        assert_eq!(b.king_square(Color::Black), Square::from_algebraic("e8"));

        let empty = Board::empty();
        assert_eq!(empty.king_square(Color::White), None);
    }
}
