//! In-place make/unmake for move generation and search.

use crate::fen::{CastlingRights, Position};
use crate::movegen::error::MoveError;
use crate::movement::{Move, MoveFlags};
use crate::square::Square;
use crate::types::{Color, Piece, PieceKind};
use crate::zobrist::{
    black_to_move_key, castling_key, castling_mask, en_passant_file_key, piece_key,
};

/// Snapshot of position state before a move, used to restore via [`Position::unmake_move`].
#[derive(Debug, Clone)]
pub struct Undo {
    m: Move,
    mover: Piece,
    captured: Option<(Square, Piece)>,
    castling: CastlingRights,
    en_passant: Option<Square>,
    halfmove_clock: u32,
    fullmove_number: u32,
    hash: u64,
    side_to_move: Color,
    white_king: Option<Square>,
    black_king: Option<Square>,
    rook_from: Option<Square>,
    rook_to: Option<Square>,
    rook: Option<Piece>,
}

impl Position {
    /// Apply `m` in place. `m` must carry correct flags from move generation.
    pub fn make_move(&mut self, m: Move) -> Result<Undo, MoveError> {
        let mover = self.board.get(m.from).ok_or(MoveError::Illegal)?;

        let captured = if m.flags.contains(MoveFlags::EN_PASSANT) {
            Square::from_file_rank(m.to.file(), m.from.rank())
                .and_then(|sq| self.board.get(sq).map(|p| (sq, p)))
        } else if m.flags.contains(MoveFlags::CAPTURE) {
            self.board.get(m.to).map(|p| (m.to, p))
        } else {
            None
        };

        let (rook_from, rook_to, rook) = if m.flags.contains(MoveFlags::KING_CASTLE) {
            let rank = m.from.rank();
            let rf = Square::from_file_rank(7, rank);
            let rt = Square::from_file_rank(5, rank);
            let r = rf.and_then(|sq| self.board.get(sq));
            (rf, rt, r)
        } else if m.flags.contains(MoveFlags::QUEEN_CASTLE) {
            let rank = m.from.rank();
            let rf = Square::from_file_rank(0, rank);
            let rt = Square::from_file_rank(3, rank);
            let r = rf.and_then(|sq| self.board.get(sq));
            (rf, rt, r)
        } else {
            (None, None, None)
        };

        let undo = Undo {
            m,
            mover,
            captured,
            castling: self.castling,
            en_passant: self.en_passant,
            halfmove_clock: self.halfmove_clock,
            fullmove_number: self.fullmove_number,
            hash: self.hash,
            side_to_move: self.side_to_move,
            white_king: self.white_king,
            black_king: self.black_king,
            rook_from,
            rook_to,
            rook,
        };

        apply_board_move(
            &mut self.board,
            m,
            captured.map(|(sq, _)| sq),
            rook_from,
            rook_to,
        );

        let is_pawn = mover.kind == PieceKind::Pawn;
        let is_capture = m.flags.contains(MoveFlags::CAPTURE);

        self.halfmove_clock = if is_pawn || is_capture {
            0
        } else {
            self.halfmove_clock + 1
        };

        self.fullmove_number = match self.side_to_move {
            Color::White => self.fullmove_number,
            Color::Black => self.fullmove_number + 1,
        };

        self.en_passant = if m.flags.contains(MoveFlags::DOUBLE_PAWN_PUSH) {
            let mid_rank = (m.from.rank() + m.to.rank()) / 2;
            Square::from_file_rank(m.from.file(), mid_rank)
        } else {
            None
        };

        self.hash ^= black_to_move_key();

        let mut castling = self.castling;
        if mover.kind == PieceKind::King {
            castling.revoke_all(mover.color);
        }

        let corner_still_has_rook = |sq_file: u8, sq_rank: u8, color: Color| -> bool {
            match Square::from_file_rank(sq_file, sq_rank) {
                Some(sq) => matches!(
                    self.board.get(sq),
                    Some(p) if p.color == color && p.kind == PieceKind::Rook
                ),
                None => false,
            }
        };
        if !corner_still_has_rook(7, Color::White.back_rank(), Color::White) {
            castling.white_kingside = false;
        }
        if !corner_still_has_rook(0, Color::White.back_rank(), Color::White) {
            castling.white_queenside = false;
        }
        if !corner_still_has_rook(7, Color::Black.back_rank(), Color::Black) {
            castling.black_kingside = false;
        }
        if !corner_still_has_rook(0, Color::Black.back_rank(), Color::Black) {
            castling.black_queenside = false;
        }

        self.hash ^= castling_key(castling_mask(&undo.castling));
        self.hash ^= castling_key(castling_mask(&castling));

        if let Some(old_ep) = undo.en_passant {
            self.hash ^= en_passant_file_key(old_ep.file());
        }

        if let Some(new_ep) = self.en_passant {
            self.hash ^= en_passant_file_key(new_ep.file());
        }

        self.hash ^= piece_key(mover, m.from);

        if let Some((cap_sq, captured_piece)) = undo.captured {
            self.hash ^= piece_key(captured_piece, cap_sq);
        }

        let placed = match m.promotion {
            Some(kind) => Piece {
                color: mover.color,
                kind,
            },
            None => mover,
        };

        self.hash ^= piece_key(placed, m.to);

        if let (Some(rf), Some(rt), Some(rook_piece)) = (rook_from, rook_to, rook) {
            self.hash ^= piece_key(rook_piece, rf);
            self.hash ^= piece_key(rook_piece, rt);
        }

        self.castling = castling;

        if mover.kind == PieceKind::King {
            match mover.color {
                Color::White => self.white_king = Some(m.to),
                Color::Black => self.black_king = Some(m.to),
            }
        }

        self.side_to_move = self.side_to_move.flip();

        debug_assert_eq!(self.hash, self.zobrist_hash(), "incremental hash diverged");
        Ok(undo)
    }

    /// Restore position state from `undo` after [`Self::make_move`].
    pub fn unmake_move(&mut self, undo: Undo) {
        let m = undo.m;

        self.side_to_move = undo.side_to_move;
        self.castling = undo.castling;
        self.en_passant = undo.en_passant;
        self.halfmove_clock = undo.halfmove_clock;
        self.fullmove_number = undo.fullmove_number;
        self.hash = undo.hash;
        self.white_king = undo.white_king;
        self.black_king = undo.black_king;

        self.board.clear(m.to);

        if let (Some(rf), Some(rt)) = (undo.rook_from, undo.rook_to) {
            if let Some(rook_piece) = undo.rook {
                self.board.clear(rt);
                self.board.set(rf, rook_piece);
            }
        }

        if let Some((cap_sq, piece)) = undo.captured {
            self.board.set(cap_sq, piece);
        }

        self.board.set(m.from, undo.mover);
    }
}

fn apply_board_move(
    board: &mut crate::board::Board,
    m: Move,
    ep_capture_sq: Option<Square>,
    rook_from: Option<Square>,
    rook_to: Option<Square>,
) {
    let Some(mover) = board.get(m.from) else {
        return;
    };

    board.clear(m.from);

    if let Some(capture_sq) = ep_capture_sq {
        board.clear(capture_sq);
    } else if m.flags.contains(MoveFlags::CAPTURE) {
        board.clear(m.to);
    }

    let placed = match m.promotion {
        Some(kind) => Piece {
            color: mover.color,
            kind,
        },
        None => mover,
    };
    board.set(m.to, placed);

    if let (Some(rf), Some(rt)) = (rook_from, rook_to) {
        if let Some(rook) = board.clear(rf) {
            board.set(rt, rook);
        }
    }
}

#[cfg(test)]
fn board_after(pos: &Position, m: Move) -> crate::board::Board {
    use crate::movement::MoveFlags;

    let mut b = pos.board.clone();
    let Some(mover) = b.get(m.from) else {
        return b;
    };

    b.clear(m.from);

    if m.flags.contains(MoveFlags::EN_PASSANT) {
        if let Some(capture_sq) = Square::from_file_rank(m.to.file(), m.from.rank()) {
            b.clear(capture_sq);
        }
    } else if m.flags.contains(MoveFlags::CAPTURE) {
        b.clear(m.to);
    }

    let placed = match m.promotion {
        Some(kind) => Piece {
            color: mover.color,
            kind,
        },
        None => mover,
    };
    b.set(m.to, placed);

    if m.flags.contains(MoveFlags::KING_CASTLE) {
        let rank = m.from.rank();
        if let (Some(rook_from), Some(rook_to)) = (
            Square::from_file_rank(7, rank),
            Square::from_file_rank(5, rank),
        ) {
            if let Some(rook) = b.clear(rook_from) {
                b.set(rook_to, rook);
            }
        }
    } else if m.flags.contains(MoveFlags::QUEEN_CASTLE) {
        let rank = m.from.rank();
        if let (Some(rook_from), Some(rook_to)) = (
            Square::from_file_rank(0, rank),
            Square::from_file_rank(3, rank),
        ) {
            if let Some(rook) = b.clear(rook_from) {
                b.set(rook_to, rook);
            }
        }
    }

    b
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::Position;
    use crate::movement::Move;
    use crate::types::Color;

    #[test]
    fn make_unmake_roundtrip_matches_board_after() {
        let pos = Position::from_fen_syntax(
            "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
        )
        .expect("syntax ok");
        for m in pos.legal_moves().into_iter() {
            let mut scratch = pos.clone();
            let undo = scratch.make_move(m).expect("pseudo-legal");
            let expected_board = board_after(&pos, m);
            assert_eq!(scratch.board, expected_board);

            scratch.unmake_move(undo);
            assert_eq!(scratch, pos);
        }
    }

    #[test]
    fn must_address_check() {
        let pos = Position::from_fen_syntax("4r3/8/8/8/8/8/8/4K3 w - - 0 1").expect("syntax ok");
        let legal = pos.legal_moves();
        for m in &legal {
            let after = board_after(&pos, *m);
            let k = after.king_square(Color::White).expect("king exists");
            assert!(!after.is_square_attacked(k, Color::Black));
        }
    }

    #[test]
    fn make_unmake_preserves_hash() {
        let pos = Position::starting_position();
        let e2e4 = Move::from_uci("e2e4").expect("valid uci");
        let legal = pos
            .legal_moves()
            .into_iter()
            .find(|m| m.from == e2e4.from && m.to == e2e4.to)
            .expect("e2e4 legal");

        let mut scratch = pos.clone();
        let undo = scratch.make_move(legal).expect("legal");
        assert_eq!(scratch.hash, scratch.zobrist_hash());
        scratch.unmake_move(undo);
        assert_eq!(scratch.hash, pos.hash);
    }
}
