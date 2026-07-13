use serde::{Deserialize, Serialize};

/// Side to move or piece color.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Color {
    /// White pieces move up the board (increasing rank).
    White = 0,
    /// Black pieces move down the board (decreasing rank).
    Black = 1,
}

impl Color {
    /// Returns the opposite color.
    #[inline]
    pub fn flip(self) -> Color {
        match self {
            Color::White => Color::Black,
            Color::Black => Color::White,
        }
    }

    /// Index into two-element arrays (`0` = white, `1` = black).
    #[inline]
    pub fn index(self) -> usize {
        self as usize
    }

    /// Pawn rank step: `+1` for white, `-1` for black.
    #[inline]
    pub const fn pawn_step(self) -> i8 {
        match self {
            Color::White => 1,
            Color::Black => -1,
        }
    }

    /// Rank index (0..7) where pawns start for this color.
    #[inline]
    pub const fn starting_pawn_rank(self) -> u8 {
        match self {
            Color::White => 1,
            Color::Black => 6,
        }
    }

    /// Rank index (0..7) a pawn occupies immediately before promoting.
    #[inline]
    pub const fn promotion_rank(self) -> u8 {
        match self {
            Color::White => 6,
            Color::Black => 1,
        }
    }

    /// Rank index (0..7) of the back rank for this color.
    #[inline]
    pub const fn back_rank(self) -> u8 {
        match self {
            Color::White => 0,
            Color::Black => 7,
        }
    }
}

/// Piece kind independent of color.
///
/// Discriminants are pinned so `PieceKind` can index attack tables and
/// material-value arrays.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PieceKind {
    /// Pawn.
    Pawn = 0,
    /// Knight.
    Knight = 1,
    /// Bishop.
    Bishop = 2,
    /// Rook.
    Rook = 3,
    /// Queen.
    Queen = 4,
    /// King.
    King = 5,
}

impl PieceKind {
    /// Lowercase FEN/algebraic letter for this kind. Pawn is `'p'`.
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

    /// Parse a FEN/algebraic piece letter (case-insensitive for kind).
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

    /// Centipawn material value. King is `0` because checkmate is handled separately.
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

    /// Index into six-element piece-kind arrays.
    #[inline]
    pub const fn index(self) -> usize {
        self as usize
    }
}

/// A colored chess piece.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Piece {
    /// Piece color.
    pub color: Color,
    /// Piece kind.
    pub kind: PieceKind,
}

impl Piece {
    /// Construct a piece from color and kind.
    #[inline]
    pub fn new(color: Color, kind: PieceKind) -> Piece {
        Piece { color, kind }
    }

    /// FEN character: uppercase for white, lowercase for black.
    #[inline]
    pub fn to_fen_char(self) -> char {
        let c = self.kind.to_char();
        match self.color {
            Color::White => c.to_ascii_uppercase(),
            Color::Black => c,
        }
    }

    /// Parse a FEN piece character.
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
        assert_eq!(Color::White.flip(), Color::Black);
        assert_eq!(Color::Black.flip(), Color::White);
        assert_eq!(Color::White.flip().flip(), Color::White);
    }

    #[test]
    fn color_rank_helpers() {
        assert_eq!(Color::White.pawn_step(), 1);
        assert_eq!(Color::Black.pawn_step(), -1);
        assert_eq!(Color::White.starting_pawn_rank(), 1);
        assert_eq!(Color::Black.starting_pawn_rank(), 6);
        assert_eq!(Color::White.promotion_rank(), 6);
        assert_eq!(Color::Black.promotion_rank(), 1);
        assert_eq!(Color::White.back_rank(), 0);
        assert_eq!(Color::Black.back_rank(), 7);
    }

    #[test]
    fn piece_kind_char_roundtrip() {
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
