//! Chess move representation and UCI notation.

use crate::square::Square;
use crate::types::PieceKind;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Contextual move properties set by move generation or position resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MoveFlags(u16);

impl MoveFlags {
    /// No special flags.
    pub const NONE: Self = Self(0);
    /// Capture of an enemy piece (including en passant).
    pub const CAPTURE: Self = Self(1 << 0);
    /// Pawn double-push from starting rank.
    pub const DOUBLE_PAWN_PUSH: Self = Self(1 << 1);
    /// Kingside castling.
    pub const KING_CASTLE: Self = Self(1 << 2);
    /// Queenside castling.
    pub const QUEEN_CASTLE: Self = Self(1 << 3);
    /// En passant capture.
    pub const EN_PASSANT: Self = Self(1 << 4);
    /// Pawn promotion.
    pub const PROMOTION: Self = Self(1 << 5);

    /// Raw flag bits.
    #[inline]
    pub const fn bits(self) -> u16 {
        self.0
    }

    /// Whether all bits in `other` are set in `self`.
    #[inline]
    pub const fn contains(self, other: MoveFlags) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Set flag bits from `other`.
    #[inline]
    pub const fn insert(&mut self, other: MoveFlags) {
        self.0 |= other.0;
    }
}

/// Engine core move: source, destination, optional promotion, and flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Move {
    /// Origin square.
    pub from: Square,
    /// Destination square.
    pub to: Square,
    /// Promotion piece, if any.
    pub promotion: Option<PieceKind>,
    /// Move flags.
    pub flags: MoveFlags,
}

/// Error parsing a UCI move string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MoveParseError {
    /// Non-ASCII input.
    #[error("UCI move must contain only ASCII characters")]
    NonAscii,
    /// Wrong string length.
    #[error("UCI move must contain 4 or 5 characters, found {actual}")]
    InvalidLength {
        /// Actual length received.
        actual: usize,
    },
    /// Invalid source square.
    #[error("invalid UCI source square")]
    InvalidFromSquare,
    /// Invalid destination square.
    #[error("invalid UCI target square")]
    InvalidToSquare,
    /// Source equals destination.
    #[error("source and destination squares must be different")]
    SameSourceAndDestination,
    /// Invalid promotion piece letter.
    #[error("invalid UCI promotion piece '{0}'; expected q, r, b, or n")]
    InvalidPromotionPiece(char),
    /// Promotion geometry invalid for from/to squares.
    #[error("promotion move must advance from the seventh to the eighth rank")]
    InvalidPromotionGeometry,
    /// Promotion flag without piece.
    #[error("promotion flag was supplied without a promotion piece")]
    InconsistentPromotionFlag,
    /// Both castle flags set.
    #[error("a move cannot be both king- and queenside castle")]
    ConflictingCastleFlags,
}

impl Move {
    /// Construct a move with no flags.
    pub fn new(
        from: Square,
        to: Square,
        promotion: Option<PieceKind>,
    ) -> Result<Self, MoveParseError> {
        Self::with_flags(from, to, promotion, MoveFlags::NONE)
    }

    /// Construct a move with explicit flags.
    pub fn with_flags(
        from: Square,
        to: Square,
        promotion: Option<PieceKind>,
        mut flags: MoveFlags,
    ) -> Result<Self, MoveParseError> {
        if from == to {
            return Err(MoveParseError::SameSourceAndDestination);
        }

        match promotion {
            Some(piece) => {
                if !matches!(
                    piece,
                    PieceKind::Queen | PieceKind::Rook | PieceKind::Bishop | PieceKind::Knight
                ) {
                    return Err(MoveParseError::InvalidPromotionPiece(piece.to_char()));
                }

                if !Self::has_valid_promotion_geometry(from, to) {
                    return Err(MoveParseError::InvalidPromotionGeometry);
                }

                flags.insert(MoveFlags::PROMOTION);
            }
            None if flags.contains(MoveFlags::PROMOTION) => {
                return Err(MoveParseError::InconsistentPromotionFlag);
            }
            None => {}
        }

        if flags.contains(MoveFlags::KING_CASTLE) && flags.contains(MoveFlags::QUEEN_CASTLE) {
            return Err(MoveParseError::ConflictingCastleFlags);
        }

        if flags.contains(MoveFlags::EN_PASSANT) {
            flags.insert(MoveFlags::CAPTURE);
        }
        Ok(Move {
            from,
            to,
            promotion,
            flags,
        })
    }

    /// Parse long algebraic / UCI notation. Does not infer capture or castle flags.
    pub fn from_uci(uci: &str) -> Result<Self, MoveParseError> {
        if !uci.is_ascii() {
            return Err(MoveParseError::NonAscii);
        }

        if uci.len() != 4 && uci.len() != 5 {
            return Err(MoveParseError::InvalidLength { actual: uci.len() });
        }

        let from = Square::from_algebraic(&uci[0..2]).ok_or(MoveParseError::InvalidFromSquare)?;

        let to = Square::from_algebraic(&uci[2..4]).ok_or(MoveParseError::InvalidToSquare)?;

        let promotion = if uci.len() == 5 {
            let promotion_char = uci.as_bytes()[4] as char;
            let piece = PieceKind::from_char(promotion_char)
                .ok_or(MoveParseError::InvalidPromotionPiece(promotion_char))?;
            Some(piece)
        } else {
            None
        };

        Move::new(from, to, promotion)
    }

    /// Format as UCI notation.
    pub fn to_uci(self) -> String {
        let mut uci = String::with_capacity(if self.promotion.is_some() { 5 } else { 4 });

        uci.push_str(&self.from.to_algebraic());
        uci.push_str(&self.to.to_algebraic());

        if let Some(promotion) = self.promotion {
            uci.push(promotion.to_char());
        }
        uci
    }

    fn has_valid_promotion_geometry(from: Square, to: Square) -> bool {
        let rank_is_valid = matches!((from.rank(), to.rank()), (6, 7) | (1, 0));

        let file_distance = from.file().abs_diff(to.file());

        rank_is_valid && file_distance <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quiet_uci_move() {
        let uci_move = Move::from_uci("e2e4").expect("valid uci");

        assert_eq!(
            uci_move.from,
            Square::from_algebraic("e2").expect("valid square")
        );
        assert_eq!(
            uci_move.to,
            Square::from_algebraic("e4").expect("valid square")
        );
        assert_eq!(uci_move.promotion, None);
        assert_eq!(uci_move.flags, MoveFlags::NONE);
        assert_eq!(uci_move.to_uci(), "e2e4");
    }

    #[test]
    fn parses_promotion_uci_move() {
        let uci_move = Move::from_uci("e7e8q").expect("valid uci");

        assert_eq!(
            uci_move.from,
            Square::from_algebraic("e7").expect("valid square")
        );
        assert_eq!(
            uci_move.to,
            Square::from_algebraic("e8").expect("valid square")
        );
        assert_eq!(uci_move.promotion, Some(PieceKind::Queen));

        assert!(uci_move.flags.contains(MoveFlags::PROMOTION));
        assert!(!uci_move.flags.contains(MoveFlags::CAPTURE));
        assert_eq!(uci_move.to_uci(), "e7e8q");
    }

    #[test]
    fn formats_uppercase_uci_input() {
        let uci_move = Move::from_uci("E7E8Q").expect("valid uci");
        assert_eq!(uci_move.to_uci(), "e7e8q");
    }

    #[test]
    fn parses_black_promotion() {
        let uci_move = Move::from_uci("e2e1r").expect("valid uci");
        assert_eq!(uci_move.promotion, Some(PieceKind::Rook));
        assert_eq!(uci_move.to_uci(), "e2e1r");
    }

    #[test]
    fn uci_does_not_guess_contextual_flags() {
        let castle_shaped_move = Move::from_uci("e1g1").expect("valid uci");

        assert!(!castle_shaped_move.flags.contains(MoveFlags::KING_CASTLE));
    }

    #[test]
    fn rejects_invalid_uci() {
        assert!(Move::from_uci("").is_err());
        assert!(Move::from_uci("e2").is_err());
        assert!(Move::from_uci("e2e9").is_err());
        assert!(Move::from_uci("e2e2").is_err());
        assert!(Move::from_uci("e7e8k").is_err());
        assert!(Move::from_uci("e6e7q").is_err());
        assert!(Move::from_uci("e7h8q").is_err());
    }

    #[test]
    fn en_passant_implied_capture() {
        let uci_move = Move::with_flags(
            Square::from_algebraic("e5").expect("valid square"),
            Square::from_algebraic("d6").expect("valid square"),
            None,
            MoveFlags::EN_PASSANT,
        )
        .expect("valid move");

        assert!(uci_move.flags.contains(MoveFlags::EN_PASSANT));
        assert!(uci_move.flags.contains(MoveFlags::CAPTURE));
    }

    #[test]
    fn rejects_conflicts_castle_flags() {
        let mut flags = MoveFlags::KING_CASTLE;
        flags.insert(MoveFlags::QUEEN_CASTLE);

        let result = Move::with_flags(
            Square::from_algebraic("e1").expect("valid square"),
            Square::from_algebraic("g1").expect("valid square"),
            None,
            flags,
        );

        assert_eq!(result, Err(MoveParseError::ConflictingCastleFlags));
    }
}
