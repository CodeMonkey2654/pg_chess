use serde::{Deserialize, Serialize};

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    White = 0,
    Black = 1,
}

impl Color {
    #[inline]
    pub fn flip(self) -> Color {
        // 0 ^ 1 = 1 (white -> black), and 1 ^ 1 = 0 (black to white) is slightly better assembly, but I can't read it well
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    /// FEN structures use case to encode color: Upper = White, Lower = Black
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }
}

/// Chess piece by kind, independent by color
/// We have to pinn discriminants so PieceKind can index into attach tables
/// and material-value arrays.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PieceKind {
    Pawn = 0,
    Knight = 1,
    Bishop = 2,
    Rook = 3,
    Queen = 4,
    King = 5,
}

impl PieceKind {
    /// Lowercase FEN/algebraic letter for this kind. Pawn is 'p'.
    #[inline]
    pub fn to_char(self) -> char {
        match self {
            PieceKind::Pawn => 'p',
            PieceKind::Knight => 'n', 
            PieceKind::Bishop => 'b',
            PieceKind::Rook => 'r',
            PieceKind::Queen => 'q',
            PieceKind::King => 'k',
        }
    }

    #[inline]
    pub fn from_char(c: char) -> Option<PieceKind> {
        Some(match c.to_ascii_lowercase() {
            'p' => PieceKind::Pawn,
            'n' => PieceKind::Knight,
            'b' => PieceKind::Bishop,
            'r' => PieceKind::Rook,
            'q' => PieceKind::Queen,
            'k' => PieceKind::King,
            _ => return None,
        })
    }

    /// Centipawn material value. Kind is 0 here because you never win one in legal play;
    /// check/checkmate is handled separately. 
    #[inline]
    pub fn value(self) -> i32 {
        match self {
            PieceKind::Pawn => 100,
            PieceKind::Knight => 320,
            PieceKind::Bishop => 330,
            PieceKind::Rook => 500,
            PieceKind::Queen => 900,
            PieceKind::King => 0,
        }
    }

    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A concrete pice is a color + kind. 
/// 
/// We keep it as a struct rather than a single 0..12 code because it would be annoying
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Piece {
    pub color: Color,
    pub kind: PieceKind,
}

impl Piece {
    #[inline]
    pub fn new(color: Color, kind: PieceKind) -> Piece {
        Piece { color, kind }
    }

    /// FEN Character: Uppercase for White, lowercase for Black.
    /// e.g. White Knight  -> 'N', Black pawn -> 'p'.
    #[inline]
    pub fn to_fen_char(self) -> char {
        let c = self.kind.to_char();
        match self.color {
            Color::White => c.to_ascii_uppercase(),
            Color::Black => c,
        }
    }

    #[inline]
    pub fn from_fen_char(c: char) -> Option<Piece> {
        let kind = PieceKind::from_char(c)?;
        let color = if c.is_ascii_uppercase() {
            Color::White
        } else {
            Color::Black
        };
        Some(Piece::new(color, kind))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn color_flip_is_involution() {
        // Flipping twice returns original
        assert_eq!(Color::White.flip(), Color::Black);
        assert_eq!(Color::Black.flip(), Color::White);
        assert_eq!(Color::White.flip().flip(), Color::White);
    }

    #[test]
    fn piece_kind_char_roundtrip() {
        // To and from FEN form checks no logic error here, that would be a womp womp
        for &k in &[
            PieceKind::Pawn,
            PieceKind::Knight,
            PieceKind::Bishop,
            PieceKind::Rook,
            PieceKind::Queen,
            PieceKind::King,
        ] {
            assert_eq!(PieceKind::from_char(k.to_char()), Some(k));
        }
    }

    #[test]
    fn piece_fen_char_roundtrip() {
        let white_knight = Piece::new(Color::White, PieceKind::Knight);
        assert_eq!(white_knight.to_fen_char(), 'N');
        assert_eq!(Piece::from_fen_char('N'), Some(white_knight));

        let black_pawn = Piece::new(Color::Black, PieceKind::Pawn);
        assert_eq!(black_pawn.to_fen_char(), 'p');
        assert_eq!(Piece::from_fen_char('p'), Some(black_pawn));
    }

    #[test]
    fn rejects_chunk_chars() {
        assert_eq!(PieceKind::from_char('x'), None);
        assert_eq!(PieceKind::from_char('9'), None);
    }
}