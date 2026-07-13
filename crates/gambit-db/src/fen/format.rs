use crate::fen::{CastlingRights, Position};
use crate::square::Square;
use crate::types::Color;

/// Serialize a position to FEN.
pub fn to_fen(pos: &Position) -> String {
    let mut fen = String::new();

    for rank in (0..8u8).rev() {
        let mut empty_run = 0u8;
        for file in 0..8u8 {
            let sq = Square(rank * 8 + file);
            match pos.board.get(sq) {
                Some(piece) => {
                    if empty_run > 0 {
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
        if rank > 0 {
            fen.push('/');
        }
    }

    fen.push(' ');
    fen.push(match pos.side_to_move {
        Color::White => 'w',
        Color::Black => 'b',
    });

    fen.push(' ');
    fen.push_str(&castling_to_fen(pos.castling));

    fen.push(' ');
    match pos.en_passant {
        Some(sq) => fen.push_str(&sq.to_algebraic()),
        None => fen.push('-'),
    }

    fen.push(' ');
    fen.push_str(&pos.halfmove_clock.to_string());
    fen.push(' ');
    fen.push_str(&pos.fullmove_number.to_string());

    fen
}

fn castling_to_fen(c: CastlingRights) -> String {
    let mut s = String::new();
    if c.white_kingside {
        s.push('K');
    }
    if c.white_queenside {
        s.push('Q');
    }
    if c.black_kingside {
        s.push('k');
    }
    if c.black_queenside {
        s.push('q');
    }
    if s.is_empty() {
        s.push('-');
    }
    s
}
