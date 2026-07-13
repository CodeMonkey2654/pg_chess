//! Semantic validation of chess positions beyond FEN syntax.

use crate::fen::{FenError, Position};
use crate::square::Square;
use crate::types::{Color, PieceKind};

/// Validate a parsed position for chess legality.
pub fn validate_position(pos: &Position) -> Result<(), FenError> {
    validate_kings(pos)?;
    validate_pawn_placement(pos)?;
    validate_piece_counts(pos)?;
    validate_check_state(pos)?;
    validate_castling_rights(pos)?;
    validate_en_passant(pos)?;
    Ok(())
}

fn validate_kings(pos: &Position) -> Result<(), FenError> {
    let mut white_kings = 0u8;
    let mut black_kings = 0u8;

    for (_, piece) in pos.board.iter_occupied() {
        if piece.kind == PieceKind::King {
            match piece.color {
                Color::White => white_kings += 1,
                Color::Black => black_kings += 1,
            }
        }
    }

    if white_kings != 1 {
        return Err(FenError::InvalidKingCount {
            color: Color::White,
            count: white_kings,
        });
    }
    if black_kings != 1 {
        return Err(FenError::InvalidKingCount {
            color: Color::Black,
            count: black_kings,
        });
    }
    Ok(())
}

fn validate_pawn_placement(pos: &Position) -> Result<(), FenError> {
    for (sq, piece) in pos.board.iter_occupied() {
        if piece.kind == PieceKind::Pawn {
            let rank = sq.rank();
            if rank == 0 || rank == 7 {
                return Err(FenError::PawnOnBackRank);
            }
        }
    }
    Ok(())
}

fn validate_piece_counts(pos: &Position) -> Result<(), FenError> {
    let mut white = 0u8;
    let mut black = 0u8;
    for (_, piece) in pos.board.iter_occupied() {
        match piece.color {
            Color::White => white += 1,
            Color::Black => black += 1,
        }
    }
    if white > 16 {
        return Err(FenError::TooManyPieces {
            color: Color::White,
            count: white,
        });
    }
    if black > 16 {
        return Err(FenError::TooManyPieces {
            color: Color::Black,
            count: black,
        });
    }
    Ok(())
}

fn validate_check_state(pos: &Position) -> Result<(), FenError> {
    let white_in_check = pos.is_in_check(Color::White);
    let black_in_check = pos.is_in_check(Color::Black);

    if white_in_check && black_in_check {
        return Err(FenError::BothSidesInCheck);
    }

    match pos.side_to_move {
        Color::White if black_in_check => return Err(FenError::SideNotToMoveInCheck),
        Color::Black if white_in_check => return Err(FenError::SideNotToMoveInCheck),
        _ => {}
    }

    Ok(())
}

fn validate_castling_rights(pos: &Position) -> Result<(), FenError> {
    if pos.castling.white_kingside && !king_on_home(pos, Color::White) {
        return Err(FenError::InconsistentCastlingRights);
    }
    if pos.castling.white_queenside && !king_on_home(pos, Color::White) {
        return Err(FenError::InconsistentCastlingRights);
    }
    if pos.castling.black_kingside && !king_on_home(pos, Color::Black) {
        return Err(FenError::InconsistentCastlingRights);
    }
    if pos.castling.black_queenside && !king_on_home(pos, Color::Black) {
        return Err(FenError::InconsistentCastlingRights);
    }

    if pos.castling.white_kingside && !rook_on_corner(pos, Color::White, true) {
        return Err(FenError::InconsistentCastlingRights);
    }
    if pos.castling.white_queenside && !rook_on_corner(pos, Color::White, false) {
        return Err(FenError::InconsistentCastlingRights);
    }
    if pos.castling.black_kingside && !rook_on_corner(pos, Color::Black, true) {
        return Err(FenError::InconsistentCastlingRights);
    }
    if pos.castling.black_queenside && !rook_on_corner(pos, Color::Black, false) {
        return Err(FenError::InconsistentCastlingRights);
    }

    Ok(())
}

fn king_on_home(pos: &Position, color: Color) -> bool {
    let file = 4;
    let rank = color.back_rank();
    Square::from_file_rank(file, rank)
        .and_then(|sq| pos.board.get(sq))
        .is_some_and(|p| p.color == color && p.kind == PieceKind::King)
}

fn rook_on_corner(pos: &Position, color: Color, kingside: bool) -> bool {
    let file = if kingside { 7 } else { 0 };
    let rank = color.back_rank();
    Square::from_file_rank(file, rank)
        .and_then(|sq| pos.board.get(sq))
        .is_some_and(|p| p.color == color && p.kind == PieceKind::Rook)
}

fn validate_en_passant(pos: &Position) -> Result<(), FenError> {
    let Some(ep) = pos.en_passant else {
        return Ok(());
    };

    let ep_rank = ep.rank();
    let ep_file = ep.file();

    // Rank of the opponent pawn that just double-pushed (0-indexed).
    let (expected_ep_rank, opponent_pawn_rank) = match pos.side_to_move {
        Color::White => (5, 4),
        Color::Black => (2, 3),
    };

    if ep_rank != expected_ep_rank {
        return Err(FenError::InvalidEnPassant);
    }

    let has_opponent_pawn = Square::from_file_rank(ep_file, opponent_pawn_rank)
        .and_then(|sq| pos.board.get(sq))
        .is_some_and(|p| p.color == pos.side_to_move.flip() && p.kind == PieceKind::Pawn);

    if !has_opponent_pawn {
        return Err(FenError::InvalidEnPassant);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::parse::parse_fen;

    #[test]
    fn rejects_missing_white_king() {
        let pos = parse_fen("8/8/8/8/8/8/8/4k2r w - - 0 1").expect("syntax ok");
        assert!(matches!(
            validate_position(&pos),
            Err(FenError::InvalidKingCount { .. })
        ));
    }

    #[test]
    fn rejects_side_not_to_move_in_check() {
        let pos = parse_fen("4r3/8/8/8/8/8/8/4K1k1 b - - 0 1").expect("syntax ok");
        assert_eq!(validate_position(&pos), Err(FenError::SideNotToMoveInCheck));
    }

    #[test]
    fn accepts_white_to_move_in_check() {
        let pos = parse_fen("4r3/8/8/8/8/8/8/4K1k1 w - - 0 1").expect("syntax ok");
        assert!(validate_position(&pos).is_ok());
    }

    #[test]
    fn rejects_pawn_on_back_rank() {
        let pos = parse_fen("8/8/8/8/8/8/8/P3K2k w - - 0 1").expect("syntax ok");
        assert_eq!(validate_position(&pos), Err(FenError::PawnOnBackRank));
    }

    #[test]
    fn accepts_starting_position() {
        let pos = parse_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1")
            .expect("syntax ok");
        assert!(validate_position(&pos).is_ok());
    }

    #[test]
    fn accepts_en_passant_after_e4() {
        let pos = parse_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1")
            .expect("syntax ok");
        assert!(validate_position(&pos).is_ok());
    }
}
