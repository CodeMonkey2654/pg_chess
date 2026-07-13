//! Square indexing and how we represent the board
//! 
//! This is not the most performant way, but it's the easiest to get correct as opposed to a bitboard
//! Convention: 0..63, a1 = 0, using Little-Endian Rank-File (LERF).
//!     square = rank * 8 + file
//!     file: 0..=7 maps to files a..=h
//!     rank: 0..=7 maps to ranks 1..=8
//! So a1=0, h1=7, a8=56, h8=63. 
//! 
//! Storage is a mailbox: a flat array of 64 optional Pieces `Option<Piece>`.
//! This is once again not efficient comparatively and focuses more on correctness.
//! Bitboards can be layered later for speed, but the public interface is important

use crate::types::{Color, Piece};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Square(pub u8);

impl Square{
    /// Build square needs to be from file or from rank (0..=7). Returning None means out of bounds
    #[inline]
    pub fn from_file_rank(file: u8, rank: u8) -> Option<Square> {
        if file < 8 && rank < 8 {
            Some(Square(rank * 8 + file))
        } else {
            None
        }
    }

    /// The file of the square(0..=7 so that a..=h) for a given square.
    #[inline]
    pub fn file(self) -> u8 {
        self.0 % 8
    }

    /// The rank of the square (0..=7 so that 1..=8) for a given square.
    #[inline]
    pub fn rank(self) -> u8 {
        self.0 / 8
    }

    /// Index into a 64-element array. Just the inner value as usize. 
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// Parse algebraic square notation like "e4" -> Square type. Case-insensitive on file.
    /// Return None if malformed
    pub fn from_algebraic(s: &str) -> Option<Square> {
        let bytes = s.as_bytes();
        if bytes.len() != 2 {
            return None;
        }

        let file = match bytes[0] {
            b'a'..=b'h' => bytes[0] - b'a',
            b'A'..=b'H' => bytes[0] - b'A',
            _ => return None,
        };

        let rank = match bytes[1] {
            b'1'..=b'8' => bytes[1] - b'1',
            _ => return None,
        };

        Square::from_file_rank(file, rank)
    }

    /// Emit algebraic notation, e.g. Square(0) -> "a1".
    pub fn to_algebraic(self) -> String {
        let file_char = (b'a' + self.file()) as char;
        let rank_char = (b'1' + self.rank()) as char;
        format!("{}{}", file_char, rank_char)
    }
}


/// Piece placement only - no move, castling, etc. Those live in Postition. This is just "what is where".
/// 'squares[i]' is the piece on square i or None if empty.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub squares: Vec<Option<Piece>>,
}

impl Board {
    pub fn empty() -> Board {
        Board {
            squares: vec![None; 64],
        }
    }

    #[inline]
    pub fn get(&self, sq: Square) -> Option<Piece> {
        self.squares[sq.index()]
    }

    #[inline]
    pub fn set(&mut self, sq: Square, piece: Piece) {
        self.squares[sq.index()] = Some(piece);
    }

    #[inline]
    pub fn clear(&mut self, sq: Square) -> Option<Piece> {
        self.squares[sq.index()].take()
    }

    pub fn starting_position() -> Board {
        use crate::types::{Color::*, PieceKind::*};
        let mut b = Board::empty();

        let back_rank = [Rook, Knight, Bishop, Queen, King, Bishop, Knight, Rook];
        for file in 0..8u8 {
            // White back rank on rank 0 (rank 1), white parns on rank 1 (rank 2).
            b.set(
                Square::from_file_rank(file, 0).unwrap(),
                Piece::new(White, back_rank[file as usize]),
            );
            b.set(
                Square::from_file_rank(file, 1).unwrap(),
                Piece::new(White, crate::types::PieceKind::Pawn),
            );

            // Black pawns on rank 6 (rank 7), black back rank on rank 7 (rank 8).
            b.set(
                Square::from_file_rank(file, 6).unwrap(),
                Piece::new(Black, crate::types::PieceKind::Pawn),
            );
            b.set(
                Square::from_file_rank(file, 7).unwrap(),
                Piece::new(Black, back_rank[file as usize]),
            );
        }
        b // in rust this is a return, developer is new so he needs to remember this isn't a syntax error and this comment is his punishment
    }

    pub fn king_square(&self, color: Color) -> Option<Square> {
        use crate::types::PieceKind::King;
        for i in 0..64u8 {
            if let Some(p) = self.squares[i as usize] {
                if p.color == color && p.kind == King {
                    return Some(Square(i));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Color, PieceKind};

    #[test]
    fn square_file_rank_math() {
        let a1 = Square(0);
        assert_eq!(a1.file(), 0);
        assert_eq!(a1.rank(), 0);

        let h8 = Square(63);
        assert_eq!(h8.file() , 7);
        assert_eq!(h8.rank(), 7);

        let e4 = Square::from_file_rank(4, 3).unwrap();
        assert_eq!(e4.0, 3 * 8 + 4); // 28
    }

    #[test]
    fn algebraic_roundtrip() {
        for i in 0..64u8 {
            let sq = Square(i);
            let s = sq.to_algebraic();
            assert_eq!(Square::from_algebraic(&s), Some(sq), "failed on {}", s);
        }

        assert_eq!(Square::from_algebraic("a1"), Some(Square(0)));
        assert_eq!(Square::from_algebraic("h8"), Some(Square(63)));
        assert_eq!(Square::from_algebraic("e4"), Some(Square(28)));
    }

    #[test]
    fn algebraic_rejects_junk() {
        assert_eq!(Square::from_algebraic("z9"), None);
        assert_eq!(Square::from_algebraic("e"), None);
        assert_eq!(Square::from_algebraic("e44"), None);
        assert_eq!(Square::from_algebraic(""), None);
    }

    #[test]
    fn starting_position_has_expected_pieces() {
        let b = Board::starting_position();

        // should total to 4 full ranks = 32 pieces
        let count = b.squares.iter().filter(|s| s.is_some()).count();
        assert_eq!(count, 32);

        // e1 is white king.
        let e1 = Square::from_algebraic("e1").unwrap();
        assert_eq!(
            b.get(e1),
            Some(Piece::new(Color::White, PieceKind::King))
        );

        // d8 is the black queen
        let d8 = Square::from_algebraic("d8").unwrap();
        assert_eq!(
            b.get(d8),
            Some(Piece::new(Color::Black, PieceKind::Queen))

        );

        // a1 is a white rook
        let a1 = Square::from_algebraic("a1").unwrap();
        assert_eq!(
            b.get(a1),
            Some(Piece::new(Color::White, PieceKind::Rook))
        );
    }

    #[test]
    fn king_square_fields_kings() {
        let b = Board::starting_position();
        assert_eq!(b.king_square(Color::White), Square::from_algebraic("e1"));
        assert_eq!(b.king_square(Color::Black), Square::from_algebraic("e8"));

        let empty = Board::empty();
        assert_eq!(empty.king_square(Color::White), None);
    }
}