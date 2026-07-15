use serde::{Deserialize, Serialize};

/// A board square in Little-Endian Rank-File (LERF) indexing: `square = rank * 8 + file`.
///
/// `a1 = 0`, `h1 = 7`, `a8 = 56`, `h8 = 63`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Square(pub u8);

impl Square {
    /// All 64 squares in index order (`a1` through `h8`).
    pub const ALL: [Square; 64] = {
        let mut squares = [Square(0); 64];
        let mut i = 0u8;
        while i < 64 {
            squares[i as usize] = Square(i);
            i += 1;
        }
        squares
    };

    /// Build a square from file and rank (both 0..7). Returns `None` if out of bounds.
    #[inline]
    pub fn from_file_rank(file: u8, rank: u8) -> Option<Square> {
        if file < 8 && rank < 8 {
            Some(Square(rank * 8 + file))
        } else {
            None
        }
    }

    /// File index 0..7 (`a`..`h`).
    #[inline]
    pub fn file(self) -> u8 {
        self.0 % 8
    }

    /// Rank index 0..7 (rank 1..8).
    #[inline]
    pub fn rank(self) -> u8 {
        self.0 / 8
    }

    /// Index into a 64-element array.
    #[inline]
    pub fn index(self) -> usize {
        self.0 as usize
    }

    /// Parse algebraic notation like `"e4"`. Case-insensitive on the file letter.
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

    /// Emit algebraic notation, e.g. `Square(0)` -> `"a1"`.
    pub fn to_algebraic(self) -> String {
        let file_char = (b'a' + self.file()) as char;
        let rank_char = (b'1' + self.rank()) as char;
        format!("{file_char}{rank_char}")
    }
}

/// Offset a square by file and rank deltas. Returns `None` if the result leaves the board.
#[inline]
pub(crate) fn offset(sq: Square, file_offset: i8, rank_offset: i8) -> Option<Square> {
    let file = sq.file() as i8 + file_offset;
    let rank = sq.rank() as i8 + rank_offset;
    if (0..8).contains(&file) && (0..8).contains(&rank) {
        Square::from_file_rank(file as u8, rank as u8)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn square_file_rank_math() {
        let a1 = Square(0);
        assert_eq!(a1.file(), 0);
        assert_eq!(a1.rank(), 0);

        let h8 = Square(63);
        assert_eq!(h8.file(), 7);
        assert_eq!(h8.rank(), 7);

        let e4 = Square::from_file_rank(4, 3).expect("valid square");
        assert_eq!(e4.0, 3 * 8 + 4);
    }

    #[test]
    fn algebraic_roundtrip() {
        for sq in Square::ALL {
            let s = sq.to_algebraic();
            assert_eq!(Square::from_algebraic(&s), Some(sq), "failed on {s}");
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
}
