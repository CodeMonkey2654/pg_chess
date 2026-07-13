use crate::fen::{CastlingRights, Position};
use crate::square::Square;
use crate::types::Color;
use thiserror::Error;

/// Error parsing a FEN string.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FenError {
    /// Wrong number of fields.
    #[error("FEN must have exactly 6 fields")]
    WrongFieldCount,
    /// Wrong number of ranks in placement.
    #[error("piece placement must have exactly 8 ranks")]
    WrongRankCount,
    /// Rank has wrong number of files.
    #[error("rank has invalid file count")]
    InvalidRankWidth,
    /// Invalid piece character.
    #[error("invalid piece character")]
    InvalidPiece,
    /// Invalid active color.
    #[error("invalid active color")]
    InvalidActiveColor,
    /// Invalid castling field.
    #[error("invalid castling rights")]
    InvalidCastling,
    /// Castling rights inconsistent with board placement.
    #[error("castling rights inconsistent with board")]
    InconsistentCastlingRights,
    /// Invalid en passant square.
    #[error("invalid en passant square")]
    InvalidEnPassant,
    /// Invalid halfmove clock.
    #[error("invalid halfmove clock")]
    InvalidHalfmoveClock,
    /// Invalid fullmove number.
    #[error("invalid fullmove number")]
    InvalidFullmoveNumber,
    /// Wrong number of kings for a side.
    #[error("invalid king count for {color:?}: expected 1, found {count}")]
    InvalidKingCount {
        /// Side with invalid king count.
        color: crate::types::Color,
        /// Kings found on the board.
        count: u8,
    },
    /// Pawn on first or eighth rank.
    #[error("pawn on back rank")]
    PawnOnBackRank,
    /// More than 16 pieces for one side.
    #[error("too many pieces for {color:?}: {count}")]
    TooManyPieces {
        /// Side with too many pieces.
        color: crate::types::Color,
        /// Piece count.
        count: u8,
    },
    /// Both kings in check simultaneously.
    #[error("both sides cannot be in check")]
    BothSidesInCheck,
    /// Side not to move is in check.
    #[error("side not to move is in check")]
    SideNotToMoveInCheck,
}

/// Parse a FEN string into a [`Position`].
pub fn parse_fen(fen: &str) -> Result<Position, FenError> {
    let fields: Vec<&str> = fen.split_whitespace().collect();
    if fields.len() != 6 {
        return Err(FenError::WrongFieldCount);
    }

    let ranks: Vec<&str> = fields[0].split('/').collect();
    if ranks.len() != 8 {
        return Err(FenError::WrongRankCount);
    }

    let mut board = crate::board::Board::empty();
    for (i, rank_str) in ranks.iter().enumerate() {
        let rank_index = 7 - i as u8;
        let mut file = 0u8;
        for ch in rank_str.chars() {
            if let Some(digit) = ch.to_digit(10) {
                file += digit as u8;
                if file > 8 {
                    return Err(FenError::InvalidRankWidth);
                }
            } else {
                let piece = crate::types::Piece::from_fen_char(ch).ok_or(FenError::InvalidPiece)?;
                let sq = Square::from_file_rank(file, rank_index).ok_or(FenError::InvalidPiece)?;
                board.set(sq, piece);
                file += 1;
                if file > 8 {
                    return Err(FenError::InvalidRankWidth);
                }
            }
        }
        if file != 8 {
            return Err(FenError::InvalidRankWidth);
        }
    }

    let side_to_move = match fields[1] {
        "w" => Color::White,
        "b" => Color::Black,
        _ => return Err(FenError::InvalidActiveColor),
    };

    let castling = parse_castling(fields[2])?;

    let en_passant = if fields[3] == "-" {
        None
    } else {
        Some(Square::from_algebraic(fields[3]).ok_or(FenError::InvalidEnPassant)?)
    };

    let halfmove_clock: u32 = fields[4]
        .parse()
        .map_err(|_| FenError::InvalidHalfmoveClock)?;
    let fullmove_number: u32 = fields[5]
        .parse()
        .map_err(|_| FenError::InvalidFullmoveNumber)?;

    let white_king = board.king_square(Color::White);
    let black_king = board.king_square(Color::Black);

    let mut pos = Position {
        board,
        side_to_move,
        castling,
        en_passant,
        halfmove_clock,
        fullmove_number,
        hash: 0,
        white_king,
        black_king,
    };
    pos.hash = pos.zobrist_hash();
    Ok(pos)
}

fn parse_castling(s: &str) -> Result<CastlingRights, FenError> {
    let mut c = CastlingRights::none();
    if s == "-" {
        return Ok(c);
    }
    for ch in s.chars() {
        match ch {
            'K' => c.white_kingside = true,
            'Q' => c.white_queenside = true,
            'k' => c.black_kingside = true,
            'q' => c.black_queenside = true,
            _ => return Err(FenError::InvalidCastling),
        }
    }
    Ok(c)
}
