use crate::fen::Position;
use crate::movement::{Move, MoveFlags};
use crate::san::error::SanError;
use crate::square::Square;
use crate::types::PieceKind;

/// Parse SAN in `pos`, returning the matching legal move.
pub fn parse_san(pos: &Position, san: &str) -> Result<Move, SanError> {
    let trimmed = san.trim();
    if trimmed.is_empty() {
        return Err(SanError::Empty);
    }

    let stripped = trimmed.trim_end_matches(['+', '#']);

    if stripped == "O-O" || stripped == "0-0" {
        return find_castle(pos, true);
    }
    if stripped == "O-O-O" || stripped == "0-0-0" {
        return find_castle(pos, false);
    }

    let (piece_kind, rest) = if let Some(first) = stripped.chars().next() {
        match first {
            'N' => (PieceKind::Knight, &stripped[1..]),
            'B' => (PieceKind::Bishop, &stripped[1..]),
            'R' => (PieceKind::Rook, &stripped[1..]),
            'Q' => (PieceKind::Queen, &stripped[1..]),
            'K' => (PieceKind::King, &stripped[1..]),
            'a'..='h' => (PieceKind::Pawn, stripped),
            _ => return Err(SanError::InvalidSyntax(trimmed.to_string())),
        }
    } else {
        return Err(SanError::InvalidSyntax(trimmed.to_string()));
    };

    let (disambiguation, capture, to_sq, promotion) = if piece_kind == PieceKind::Pawn {
        parse_pawn_parts(rest)?
    } else {
        parse_piece_parts(rest)?
    };

    let candidates: Vec<Move> = pos
        .legal_moves()
        .into_iter()
        .filter(|m| {
            let mover = pos.board.get(m.from);
            if piece_kind == PieceKind::Pawn {
                mover.is_some_and(|p| p.kind == PieceKind::Pawn)
            } else {
                mover.is_some_and(|p| p.kind == piece_kind)
            }
        })
        .filter(|m| m.to.to_algebraic() == to_sq)
        .filter(|m| m.promotion == promotion)
        .filter(|m| {
            if capture {
                m.flags.contains(MoveFlags::CAPTURE)
            } else {
                !m.flags.contains(MoveFlags::CAPTURE)
            }
        })
        .filter(|m| matches_disambiguation(m, &disambiguation))
        .collect();

    match candidates.len() {
        1 => Ok(candidates[0]),
        _ => Err(SanError::NoMatch(trimmed.to_string())),
    }
}

fn find_castle(pos: &Position, kingside: bool) -> Result<Move, SanError> {
    let flag = if kingside {
        MoveFlags::KING_CASTLE
    } else {
        MoveFlags::QUEEN_CASTLE
    };
    pos.legal_moves()
        .into_iter()
        .find(|m| m.flags.contains(flag))
        .ok_or_else(|| SanError::NoMatch(if kingside { "O-O" } else { "O-O-O" }.to_string()))
}

fn matches_disambiguation(m: &Move, disambiguation: &str) -> bool {
    if disambiguation.is_empty() {
        return true;
    }
    let from = m.from.to_algebraic();
    if disambiguation.len() == 1 {
        let c = disambiguation.as_bytes()[0];
        if (b'a'..=b'h').contains(&c) {
            return from.starts_with(disambiguation);
        }
        if (b'1'..=b'8').contains(&c) {
            return from.ends_with(disambiguation);
        }
    }
    from == disambiguation || from.starts_with(disambiguation) || from.ends_with(disambiguation)
}

fn parse_pawn_parts(rest: &str) -> Result<(String, bool, String, Option<PieceKind>), SanError> {
    if rest.len() == 2 && is_square(rest) {
        return Ok((String::new(), false, rest.to_string(), None));
    }

    if let Some(x_idx) = rest.find('x') {
        let from_file = &rest[0..x_idx];
        let after = &rest[x_idx + 1..];
        let (to_sq, promotion) = split_square_promotion(after)?;
        return Ok((from_file.to_string(), true, to_sq, promotion));
    }

    Err(SanError::InvalidSyntax(rest.to_string()))
}

fn parse_piece_parts(rest: &str) -> Result<(String, bool, String, Option<PieceKind>), SanError> {
    let mut s = rest;
    let mut disambiguation = String::new();

    while let Some(c) = s.chars().next() {
        if c.is_ascii_digit() || ('a'..='h').contains(&c) {
            if is_square(s) {
                break;
            }
            disambiguation.push(c);
            s = &s[c.len_utf8()..];
        } else {
            break;
        }
    }

    let capture = if let Some(stripped) = s.strip_prefix('x') {
        s = stripped;
        true
    } else {
        false
    };

    let (to_sq, promotion) = split_square_promotion(s)?;
    Ok((disambiguation, capture, to_sq, promotion))
}

fn split_square_promotion(s: &str) -> Result<(String, Option<PieceKind>), SanError> {
    if s.len() < 2 {
        return Err(SanError::InvalidSyntax(s.to_string()));
    }

    let eq_idx = s.find('=');
    let square_part = eq_idx.map(|i| &s[..i]).unwrap_or(s);
    if square_part.len() != 2 || !is_square(square_part) {
        return Err(SanError::InvalidSyntax(s.to_string()));
    }

    let promotion = if let Some(idx) = eq_idx {
        let promo_char = s
            .chars()
            .nth(idx + 1)
            .ok_or_else(|| SanError::InvalidSyntax(s.to_string()))?;
        Some(
            PieceKind::from_char(promo_char)
                .ok_or_else(|| SanError::InvalidSyntax(s.to_string()))?,
        )
    } else {
        None
    };

    Ok((square_part.to_string(), promotion))
}

fn is_square(s: &str) -> bool {
    Square::from_algebraic(s).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::san::format::to_san;

    #[test]
    fn roundtrips_opening_moves() {
        let mut pos = Position::starting_position();
        for san in ["e4", "e5", "Nf3", "Nc6", "Bb5"] {
            let m = parse_san(&pos, san).expect("parse san");
            assert_eq!(to_san(&pos, m), san);
            pos = pos.apply_move(m).expect("legal");
        }
    }

    #[test]
    fn parses_capture_notation() {
        let pos = Position::from_fen_syntax(
            "rnbqkbnr/ppp1pppp/8/3p4/4P3/8/PPPP1PPP/RNBQKBNR w KQkq d6 0 2",
        )
        .expect("syntax ok");
        let m = parse_san(&pos, "exd5").expect("parse");
        assert!(m.flags.contains(MoveFlags::CAPTURE));
    }
}
