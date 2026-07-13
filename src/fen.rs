//! A board position in FEN (Forsyth-Edwards Notation) is a standard oneline of etxt encoding
//!     1. What's on each Square
//!     2. Side to move and castling rights
//!     3. En passant target - square a pawn just skipped if any
//!     4. Move clocks - halfmove clock (for the 50 move rule) and fullmove number
//! Format is a six-space separated fields. The starting_position would be 
//! rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1
//! 1. Piece placement, rank 8 down to rank 1, ranks by /. Within a rank, files a -> h. Letters are pieces (uppercase White), digits are runs of empty squares
//! 2. Active color: w or b
//! 3. Castling any of KQkq or - if none
//! 4. En passant target square in algebraic (e.g. e3) or -
//! 5. Plies since last capture or pawn move
//! 6. Fullmove number: starts at 1, increments after black moves.
//! My system has 1 as index 0 while FEN starts at 8 so think of that.

use crate::board::{Board, Square};
use crate::types::Color;
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CastlingRights {
    pub white_kingside: bool,
    pub white_queenside: bool,
    pub black_kingside: bool,
    pub black_queenside: bool,
}

impl CastlingRights {
    pub fn none() -> Self {
        CastlingRights {
            white_kingside: false,
            white_queenside: false,
            black_kingside: false,
            black_queenside: false,
        }
    }

    pub fn all() -> Self {
        CastlingRights {
            white_kingside: true,
            white_queenside: true,
            black_kingside: true,
            black_queenside: true,
        }
    }

    pub fn to_fen(self) -> String {
        let mut s = String::new();
        if self.white_kingside {
            s.push('K');
        }
        if self.white_queenside {
            s.push('Q');
        }
        if self.black_kingside {
            s.push('k');
        }
        if self.black_queenside {
            s.push('q');
        }
        if s.is_empty() {
            s.push('-');
        }
        s
    }

    pub fn from_fen(s: &str) -> Option<CastlingRights> {
        let mut c = CastlingRights::none();
        if s == "-" {
            return Some(c);
        }
        for ch in s.chars() {
            match ch {
                'K' => c.white_kingside = true,
                'Q' => c.white_queenside = true, 
                'k' => c.black_kingside = true,
                'q' => c.black_queenside = true,
                _ => return None
            }
        }
        Some(c)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub board: Board,
    pub side_to_move: Color,
    pub castling: CastlingRights,
    pub en_passant: Option<Square>, // The square a capturing pawn moves TO....
    pub halfmove_clock: u32,
    pub fullmove_number: u32,
    pub hash: u64,
}

impl Position {
    pub fn starting_position() -> Position {
        let mut pos = Position {
            board: Board::starting_position(),
            side_to_move: Color::White,
            castling: CastlingRights::all(),
            en_passant: None,
            halfmove_clock: 0,
            fullmove_number: 1,
            hash: 0,
        };
        pos.hash = pos.zobrist_hash();
        pos
    }

    pub fn to_fen(&self) -> String{
        let mut fen = String::new();

        // Piece placement (REVERSE RANK CALEB GOD)
        for rank in (0..8u8).rev() {
            let mut empty_run = 0u8;
            for file in 0..8u8 {
                let sq = Square::from_file_rank(file, rank).unwrap();
                match self.board.get(sq) {
                    Some(piece) => {
                        if empty_run > 0 {
                            // flush digit of empty squares
                            fen.push((b'0' + empty_run) as char);
                            empty_run = 0;
                        }
                        fen.push(piece.to_fen_char());
                    }
                    None => empty_run += 1,
                }
            }
            if empty_run > 0 {
                fen.push((b'0' + empty_run) as char);
            }
            if rank >0 {
                fen.push('/');
            }
        }

        // active color
        fen.push(' ');
        fen.push(match self.side_to_move {
            Color::White => 'w',
            Color::Black => 'b',
        });

        // castling
        fen.push(' ');
        fen.push_str(&self.castling.to_fen());

        // en passant
        fen.push(' ');
        match self.en_passant {
            Some(sq) => fen.push_str(&sq.to_algebraic()),
            None => fen.push('-'),
        }

        // clocks
        fen.push(' ');
        fen.push_str(&self.halfmove_clock.to_string());
        fen.push(' ');
        fen.push_str(&self.fullmove_number.to_string());

        fen
    }

    pub fn from_fen(fen: &str) -> Option<Position> {
        let fields: Vec<&str> = fen.split_whitespace().collect();
        if fields.len() != 6 {
            return None;
        }

        // pieces
        let ranks: Vec<&str> = fields[0]. split('/').collect();
        if ranks.len() != 8 {
            return None;
        }

        let mut board = Board::empty();
        for (i, rank_str) in ranks.iter().enumerate() {
            let rank_index = 7 - i as u8; // Flip for FEN to us
            let mut file = 0u8;
            for ch in rank_str.chars() {
                if let Some(digit) = ch.to_digit(10) {
                    file += digit as u8;
                    if file > 8 {
                        return None; // overfilled rank
                    }
                } else {
                    // a piece found not run
                    let piece = crate::types::Piece::from_fen_char(ch)?;
                    let sq = Square::from_file_rank(file, rank_index)?;
                    board.set(sq, piece);
                    file += 1;
                    if file > 8 {
                        return None;
                    }
                }
            }
            if file != 8 {
                return None; // Not exactly 8 is not a valid position
            }
        }


        // active color
        let side_to_move = match fields[1] {
            "w" => Color::White,
            "b" => Color::Black,
            _ => return None,
        };

        // Castling
        let castling = CastlingRights::from_fen(fields[2])?;

        // en passant target!
        let en_passant = if fields[3]== "-" {
            None
        } else {
            Some(Square::from_algebraic(fields[3])?)
        };

        // numbers
        let halfmove_clock: u32 = fields[4].parse().ok()?;
        let fullmove_number: u32 = fields[5].parse().ok()?;
        let mut pos = Position {
            board,
            side_to_move,
            castling,
            en_passant,
            halfmove_clock,
            fullmove_number,
            hash: 0,
        };
        pos.hash = pos.zobrist_hash();
        Some(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STARTING_POSITION: &str =
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

    #[test]
    fn starting_position_fen_roundtrip() {
        let pos = Position::starting_position();
        assert_eq!(pos.to_fen(), STARTING_POSITION);

        let parsed = Position::from_fen(STARTING_POSITION).unwrap();
        assert_eq!(parsed, pos);
    }

    #[test]
    fn rank_order_is_not_flipped() {
        // A position with a lone white rook on a1 and black rook on a8.
        // If the rank flip were wrong, these would swap.
        let fen = "r7/8/8/8/8/8/8/R7 w - - 0 1";
        let pos = Position::from_fen(fen).unwrap();

        use crate::types::{Color, PieceKind};
        let a1 = Square::from_algebraic("a1").unwrap();
        let a8 = Square::from_algebraic("a8").unwrap();
        assert_eq!(
            pos.board.get(a1),
            Some(crate::types::Piece::new(Color::White, PieceKind::Rook))
        );
        assert_eq!(
            pos.board.get(a8),
            Some(crate::types::Piece::new(Color::Black, PieceKind::Rook))
        );
        // And it must round-trip back to the same string.
        assert_eq!(pos.to_fen(), fen);
    }

    #[test]
    fn parses_all_fen_fields() {
        // After 1. e4: black to move, en passant target e3, fullmove 1.
        let fen = "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1";
        let pos = Position::from_fen(fen).unwrap();

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
        let pos = Position::from_fen(fen).unwrap();
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
        assert!(Position::from_fen("").is_none());
        assert!(Position::from_fen("too few fields").is_none());
        // Only 7 ranks:
        assert!(Position::from_fen("8/8/8/8/8/8/8 w - - 0 1").is_none());
        // Rank overfilled (9 files):
        assert!(Position::from_fen("9/8/8/8/8/8/8/8 w - - 0 1").is_none());
        // Bad active color:
        assert!(
            Position::from_fen("8/8/8/8/8/8/8/8 x - - 0 1").is_none()
        );
        // Rank underfilled (only 7 files on first rank):
        assert!(Position::from_fen("7/8/8/8/8/8/8/8 w - - 0 1").is_none());
    }
}