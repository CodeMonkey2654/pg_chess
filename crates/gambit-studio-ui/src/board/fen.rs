//! FEN placement parsing for board rendering.

/// Parse the piece-placement field of a FEN into 64 cells (LERF: a1=0 … h8=63).
pub fn fen_to_squares(fen: &str) -> [Option<char>; 64] {
    let mut squares = [None; 64];
    let placement = fen.split_whitespace().next().unwrap_or(fen);

    for (fen_rank_idx, rank_str) in placement.split('/').enumerate() {
        if fen_rank_idx >= 8 {
            break;
        }
        let rank = 7 - fen_rank_idx;
        let mut file = 0u8;
        for ch in rank_str.chars() {
            if ch.is_ascii_digit() {
                file += ch.to_digit(10).unwrap_or(0) as u8;
            } else if ch.is_ascii_alphabetic() {
                if file < 8 {
                    squares[rank as usize * 8 + file as usize] = Some(ch);
                    file += 1;
                }
            }
        }
    }

    squares
}

/// Return true if the FEN character is a white piece.
pub fn is_white_piece(ch: char) -> bool {
    ch.is_ascii_uppercase()
}

/// Find the king square for `side` ("white" | "black") as algebraic notation.
pub fn king_square(fen: &str, side: &str) -> Option<String> {
    let target = if side == "white" { 'K' } else { 'k' };
    let squares = fen_to_squares(fen);
    for (idx, piece) in squares.iter().enumerate() {
        if *piece == Some(target) {
            return Some(index_to_algebraic(idx));
        }
    }
    None
}

/// Convert LERF index to algebraic (e.g. 0 → "a1").
pub fn index_to_algebraic(idx: usize) -> String {
    let rank = (idx / 8) as u8;
    let file = (idx % 8) as u8;
    format!("{}{}", (b'a' + file) as char, (b'1' + rank) as char)
}

/// Parse algebraic notation to LERF index.
pub fn algebraic_to_index(alg: &str) -> Option<usize> {
    let bytes = alg.as_bytes();
    if bytes.len() != 2 {
        return None;
    }
    let file = bytes[0].to_ascii_lowercase().checked_sub(b'a')?;
    let rank = bytes[1].checked_sub(b'1')?;
    if file < 8 && rank < 8 {
        Some(rank as usize * 8 + file as usize)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startpos_has_pieces() {
        let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
        let sq = fen_to_squares(fen);
        assert_eq!(sq[0], Some('R'));
        assert_eq!(sq[4], Some('K'));
        assert_eq!(sq[56], Some('r'));
    }
}
