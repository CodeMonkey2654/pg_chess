//! Chess move representation and Notation
//! A "move" identifies a source square, destination square, optional promotion,
//! and other contextual flags populated by move gen or position resolution
//!
//! UCI is intrinsic to a move (e2e4, e7e8q)
//! SAN is Contextual: e4 -> Nf3 -> Nbd2 -> O-O -> exd8=Q+
//! SAN will also need a position

use crate::{board::Square};
use crate::types::PieceKind;
use serde::{Serialize, Deserialize};
use std::fmt;


// Note these are contextual properties, check and checkmate are a position result, not a mechanic of movement
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MoveFlags(u16);

impl MoveFlags {
    pub const NONE: Self = Self(0);
    pub const CAPTURE: Self = Self(1 << 0);
    pub const DOUBLE_PAWN_PUSH: Self = Self(1 << 1);
    pub const KING_CASTLE: Self = Self(1 << 2);
    pub const QUEEN_CASTLE: Self = Self(1 << 3);
    pub const EN_PASSANT: Self = Self(1 << 4);
    pub const PROMOTION: Self = Self(1 << 5);

    #[inline]
    pub const fn bits(self) -> u16 {
        self.0
    }

    #[inline]
    pub const fn contains(self, other: MoveFlags) -> bool {
        (self.0 & other.0) == other.0
    }

    #[inline]
    pub const fn insert(&mut self, other: MoveFlags) {
        self.0 |= other.0;
    }
}


/// Engine Core move representation. UCI determines a lot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Move {
    pub from: Square,
    pub to: Square,
    pub promotion: Option<PieceKind>,
    pub flags: MoveFlags,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveParseError {
    NonAscii,
    InvalidLength {
        actual: usize
    },
    InvalidFromSquare,
    InvalidToSquare,
    SameSourceAndDestination,
    InvalidPromotionPiece(char),
    InvalidPromotionGeometry,
    InconsistentPromotionFlag,
    ConflictingCastleFlags,
}

impl fmt::Display for MoveParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MoveParseError::NonAscii => {
                write!(f, "UCI Move must contain only ASCII Characters")
            }
            MoveParseError::InvalidLength { actual } => {
                write!(
                    f,
                    "UCI movem ust contain 4 or 5 characters, found {}",
                    actual
                )
            }
            MoveParseError::InvalidFromSquare => {
                write!(f, "Invalid UCI Source Square")
            }
            MoveParseError::InvalidToSquare => {
                write!(f, "Invalid UCI Target Square")
            }
            MoveParseError::SameSourceAndDestination => {
                write!(f, "The Source and Destination squares must be different")
            }
            MoveParseError::InvalidPromotionPiece(piece) => {
                write!(
                    f,
                    "Invalid UCI promotion piece '{}'; expected q, r, b, or n",
                    piece
                )
            }
            MoveParseError::InvalidPromotionGeometry => {
                write!(
                    f,
                    "promotion move must advance from the seventh to the eighth rank"
                )
            }
            MoveParseError::InconsistentPromotionFlag => {
                write!(f, "Promotion flag was supplied without a promotion piece")
            }
            MoveParseError::ConflictingCastleFlags => {
                write!(f, "a move cannot be both king and queen side")
            }
        }
    }
}

impl std::error::Error for MoveParseError {}

impl Move {
    pub fn new(
        from: Square,
        to: Square,
        promotion: Option<PieceKind>,
    ) -> Result<Self, MoveParseError> {
        Self::with_flags(from, to, promotion, MoveFlags::NONE)
    }

    pub fn with_flags(
        from: Square,
        to: Square,
        promotion: Option<PieceKind>,
        mut flags: MoveFlags,
    ) -> Result<Self, MoveParseError> {
        if from == to {
            return Err(MoveParseError::SameSourceAndDestination)
        }

        match promotion {
            Some(piece) => {
                if !matches!(
                    piece,
                    PieceKind::Queen
                        | PieceKind::Rook
                        | PieceKind::Bishop
                        | PieceKind::Knight
                ) {
                    return Err(MoveParseError::InvalidPromotionPiece(
                        piece.to_char()
                    ));
                }

                if !Self::has_valid_promotion_geometry(from, to) {
                    return Err(MoveParseError::InvalidPromotionGeometry);
                }

                flags.insert(MoveFlags::PROMOTION)
            }
            None if flags.contains(MoveFlags::PROMOTION) => {
                return Err(MoveParseError::InconsistentPromotionFlag);
            }
            None => {}
        }

        if flags.contains(MoveFlags::KING_CASTLE) && flags.contains(MoveFlags::QUEEN_CASTLE) {
            return Err(MoveParseError::ConflictingCastleFlags);
        }
        
        //En Passant is always a capture
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

    /// Parse long algebraic/UCI notion.
    /// 
    /// Parsing UCI does not infer capture, castling, en passant, or
    /// double-pawn-push flags because those require a Position
    pub fn from_uci(uci: &str) -> Result<Self, MoveParseError> {
        if !uci.is_ascii() {
            return Err(MoveParseError::NonAscii);
        }

        if uci.len() != 4 && uci.len() != 5 {
            return Err(MoveParseError::InvalidLength { actual: uci.len() });
        }

        let from = Square::from_algebraic(&uci[0..2])
            .ok_or(MoveParseError::InvalidFromSquare)?;

        let to = Square::from_algebraic(&uci[2..4])
            .ok_or(MoveParseError::InvalidToSquare)?;

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

    pub fn to_uci(self) -> String {
        let mut uci = String::with_capacity(if self.promotion.is_some() {
            5
        } else {
            4
        });

        uci.push_str(&self.from.to_algebraic());
        uci.push_str(&self.to.to_algebraic());

        if let Some(promotion) = self.promotion {
            uci.push(promotion.to_char());
        }
        uci
    }

    fn has_valid_promotion_geometry(from: Square, to: Square) -> bool {
        let rank_is_valid = matches!(
            (from.rank(), to.rank()),
            (6,7) | (1,0)
        );

        let file_distance = from.file().abs_diff(to.file());

        // Promotion can be straight pawn or diagonal capture
        rank_is_valid && file_distance <= 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_quiet_uci_move() {
        let uci_move = Move::from_uci("e2e4").unwrap();

        assert_eq!(uci_move.from, Square::from_algebraic("e2").unwrap());
        assert_eq!(uci_move.to, Square::from_algebraic("e4").unwrap());
        assert_eq!(uci_move.promotion, None);
        assert_eq!(uci_move.flags, MoveFlags::NONE);
        assert_eq!(uci_move.to_uci(), "e2e4");
    }

    #[test]
    fn parses_promotion_uci_move() {
        let uci_move = Move::from_uci("e7e8q").unwrap();

        assert_eq!(uci_move.from, Square::from_algebraic("e7").unwrap());
        assert_eq!(uci_move.to, Square::from_algebraic("e8").unwrap());
        assert_eq!(uci_move.promotion, Some(PieceKind::Queen));
    
        assert!(uci_move.flags.contains(MoveFlags::PROMOTION));
        assert!(!uci_move.flags.contains(MoveFlags::CAPTURE));
        assert_eq!(uci_move.to_uci(), "e7e8q");
    }

    #[test]
    fn formats_uppercase_uci_input() {
        let uci_move = Move::from_uci("E7E8Q").unwrap();
        assert_eq!(uci_move.to_uci(), "e7e8q");
    }

    #[test]
    fn parses_black_promotion() {
        let uci_move = Move::from_uci("e2e1r").unwrap();
        assert_eq!(uci_move.promotion, Some(PieceKind::Rook));
        assert_eq!(uci_move.to_uci(), "e2e1r");
    }

    #[test]
    fn uci_does_not_guess_contextual_flags() {
        let castle_shaped_move = Move::from_uci("e1g1").unwrap();

        assert!(
            !castle_shaped_move
                .flags
                .contains(MoveFlags::KING_CASTLE)
        );
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
            Square::from_algebraic("e5").unwrap(),
            Square::from_algebraic("d6").unwrap(),
            None,
            MoveFlags::EN_PASSANT,
        ).unwrap();

        assert!(uci_move.flags.contains(MoveFlags::EN_PASSANT));
        assert!(uci_move.flags.contains(MoveFlags::CAPTURE));
    }

    #[test]
    fn rejects_conflicts_castle_flags() {
        let mut flags = MoveFlags::KING_CASTLE;
        flags.insert(MoveFlags::QUEEN_CASTLE);

        let result = Move::with_flags(
            Square::from_algebraic("e1").unwrap(),
            Square::from_algebraic("g1").unwrap(),
            None,
            flags,
        );

        assert_eq!(
            result,
            Err(MoveParseError::ConflictingCastleFlags)
        );
    }
}