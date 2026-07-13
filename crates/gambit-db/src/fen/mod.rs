//! FEN position representation.

mod format;
mod parse;
mod validate;

pub use format::to_fen;
pub use parse::FenError;
pub use validate::validate_position;

use crate::board::Board;
use crate::square::Square;
use crate::types::Color;
use serde::{Deserialize, Serialize};

/// Castling availability for both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastlingRights {
    /// White may castle kingside.
    pub white_kingside: bool,
    /// White may castle queenside.
    pub white_queenside: bool,
    /// Black may castle kingside.
    pub black_kingside: bool,
    /// Black may castle queenside.
    pub black_queenside: bool,
}

impl CastlingRights {
    /// No castling rights.
    pub fn none() -> Self {
        CastlingRights {
            white_kingside: false,
            white_queenside: false,
            black_kingside: false,
            black_queenside: false,
        }
    }

    /// All four castling rights.
    pub fn all() -> Self {
        CastlingRights {
            white_kingside: true,
            white_queenside: true,
            black_kingside: true,
            black_queenside: true,
        }
    }

    /// Kingside and queenside rights for `color`.
    pub fn rights_for(self, color: Color) -> (bool, bool) {
        match color {
            Color::White => (self.white_kingside, self.white_queenside),
            Color::Black => (self.black_kingside, self.black_queenside),
        }
    }

    /// Revoke all castling rights for `color`.
    pub fn revoke_all(&mut self, color: Color) {
        match color {
            Color::White => {
                self.white_kingside = false;
                self.white_queenside = false;
            }
            Color::Black => {
                self.black_kingside = false;
                self.black_queenside = false;
            }
        }
    }
}

/// Full chess position: board, side to move, castling, en passant, clocks, and hash.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    /// Piece placement.
    pub board: Board,
    /// Side to move.
    pub side_to_move: Color,
    /// Castling rights.
    pub castling: CastlingRights,
    /// En passant target square (square the capturing pawn moves to).
    pub en_passant: Option<Square>,
    /// Halfmove clock (plies since last capture or pawn move).
    pub halfmove_clock: u32,
    /// Fullmove number (increments after black's move).
    pub fullmove_number: u32,
    /// Cached Zobrist hash.
    pub hash: u64,
    /// Cached white king square.
    pub(crate) white_king: Option<Square>,
    /// Cached black king square.
    pub(crate) black_king: Option<Square>,
}

impl Position {
    /// Standard starting position.
    pub fn starting_position() -> Position {
        let board = Board::starting_position();
        let white_king = board.king_square(Color::White);
        let black_king = board.king_square(Color::Black);
        let mut pos = Position {
            board,
            side_to_move: Color::White,
            castling: CastlingRights::all(),
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            hash: 0,
            white_king,
            black_king,
        };
        pos.hash = pos.zobrist_hash();
        pos
    }

    /// Parse FEN with semantic validation.
    pub fn from_fen(fen: &str) -> Result<Position, FenError> {
        let pos = parse::parse_fen(fen)?;
        validate_position(&pos)?;
        Ok(pos)
    }

    /// Parse FEN syntax only (no semantic validation). For tests and internal tooling.
    #[allow(dead_code)]
    pub(crate) fn from_fen_syntax(fen: &str) -> Result<Position, FenError> {
        parse::parse_fen(fen)
    }

    /// Whether FEN is syntactically valid and semantically legal.
    pub fn is_valid_fen(fen: &str) -> bool {
        Self::from_fen(fen).is_ok()
    }

    /// Serialize to FEN.
    pub fn to_fen(&self) -> String {
        to_fen(self)
    }

    /// Positions equivalent for transposition (ignores move clocks).
    pub fn equivalent_for_transposition(&self, other: &Position) -> bool {
        if self.board != other.board {
            return false;
        }
        if self.side_to_move != other.side_to_move {
            return false;
        }
        if self.castling != other.castling {
            return false;
        }
        let self_ep_file = self.en_passant.map(|s| s.file());
        let other_ep_file = other.en_passant.map(|s| s.file());
        self_ep_file == other_ep_file
    }

    /// Cached king square for `color`.
    pub fn king_square(&self, color: Color) -> Option<Square> {
        match color {
            Color::White => self.white_king,
            Color::Black => self.black_king,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Color, Piece, PieceKind};

    const STARTING_POSITION: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn starting_position_fen_roundtrip() {
        let pos = Position::starting_position();
        assert_eq!(pos.to_fen(), STARTING_POSITION);

        let parsed = Position::from_fen(STARTING_POSITION).expect("valid fen");
        assert_eq!(parsed, pos);
    }

    #[test]
    fn rank_order_is_not_flipped() {
        let fen = "r7/8/8/8/8/8/8/R7 w - - 0 1";
        let pos = Position::from_fen_syntax(fen).expect("syntax ok");

        let a1 = Square::from_algebraic("a1").expect("valid square");
        let a8 = Square::from_algebraic("a8").expect("valid square");
        assert_eq!(
            pos.board.get(a1),
            Some(Piece::new(Color::White, PieceKind::Rook))
        );
        assert_eq!(
            pos.board.get(a8),
            Some(Piece::new(Color::Black, PieceKind::Rook))
        );
        assert_eq!(pos.to_fen(), fen);
    }

    #[test]
    fn parses_all_fen_fields() {
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
        let pos = Position::from_fen_syntax(fen).expect("syntax ok");

        assert_eq!(pos.side_to_move, Color::Black);
        assert_eq!(pos.castling, CastlingRights::all());
        assert_eq!(pos.en_passant, Square::from_algebraic("e3"));
        assert_eq!(pos.halfmove_clock, 0);
        assert_eq!(pos.fullmove_number, 1);
        assert_eq!(pos.to_fen(), fen);
    }

    #[test]
    fn castling_subset_roundtrips() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w Kq - 5 10";
        let pos = Position::from_fen(fen).expect("valid fen");
        assert!(pos.castling.white_kingside);
        assert!(!pos.castling.white_queenside);
        assert!(!pos.castling.black_kingside);
        assert!(pos.castling.black_queenside);
        assert_eq!(pos.halfmove_clock, 5);
        assert_eq!(pos.fullmove_number, 10);
        assert_eq!(pos.to_fen(), fen);
    }

    #[test]
    fn rejects_malformed_fen() {
        assert!(Position::from_fen("").is_err());
        assert!(Position::from_fen("too few fields").is_err());
        assert!(Position::from_fen("8/8/8/8/8/8/8 w - - 0 1").is_err());
        assert!(Position::from_fen("9/8/8/8/8/8/8/8 w - - 0 1").is_err());
        assert!(Position::from_fen("8/8/8/8/8/8/8/8 x - - 0 1").is_err());
        assert!(Position::from_fen("7/8/8/8/8/8/8/8 w - - 0 1").is_err());
    }

    #[test]
    fn king_cache_set_at_construction() {
        let pos = Position::starting_position();
        assert_eq!(pos.king_square(Color::White), Square::from_algebraic("e1"));
        assert_eq!(pos.king_square(Color::Black), Square::from_algebraic("e8"));
    }
}
