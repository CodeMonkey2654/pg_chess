use crate::fen::Position;
use crate::movement::{Move, MoveFlags};
use crate::types::{Piece, PieceKind};

/// Format a legal move as SAN in `pos`.
pub fn to_san(pos: &Position, m: Move) -> String {
    if m.flags.contains(MoveFlags::KING_CASTLE) {
        let mut san = "O-O".to_string();
        san.push_str(check_suffix(pos, m));
        return san;
    }
    if m.flags.contains(MoveFlags::QUEEN_CASTLE) {
        let mut san = "O-O-O".to_string();
        san.push_str(check_suffix(pos, m));
        return san;
    }

    let mover = pos.board.get(m.from).expect("legal move has piece");
    let legal = pos.legal_moves();
    let mut san = String::new();

    if mover.kind == PieceKind::Pawn {
        if m.flags.contains(MoveFlags::CAPTURE) {
            san.push((b'a' + m.from.file()) as char);
            san.push('x');
        }
        san.push_str(&m.to.to_algebraic());
        if let Some(promo) = m.promotion {
            san.push('=');
            san.push(promo.to_char());
        }
    } else {
        san.push(mover.kind.to_char().to_ascii_uppercase());
        if needs_disambiguation(pos, &legal, m, mover) {
            san.push_str(&disambiguation(pos, &legal, m, mover));
        }
        if m.flags.contains(MoveFlags::CAPTURE) {
            san.push('x');
        }
        san.push_str(&m.to.to_algebraic());
        if let Some(promo) = m.promotion {
            san.push('=');
            san.push(promo.to_char());
        }
    }

    san.push_str(check_suffix(pos, m));
    san
}

fn check_suffix(pos: &Position, m: Move) -> &'static str {
    let mut after = pos.clone();
    if after.make_move(m).is_ok() {
        if after.is_checkmate() {
            return "#";
        }
        if after.is_in_check(after.side_to_move) {
            return "+";
        }
    }
    ""
}

fn needs_disambiguation(pos: &Position, legal: &[Move], m: Move, mover: Piece) -> bool {
    legal.iter().any(|other| {
        other.to == m.to
            && pos
                .board
                .get(other.from)
                .is_some_and(|p| p.kind == mover.kind)
            && !(other.from == m.from && other.to == m.to && other.promotion == m.promotion)
    })
}

fn disambiguation(pos: &Position, legal: &[Move], m: Move, mover: Piece) -> String {
    let ambiguous: Vec<_> = legal
        .iter()
        .copied()
        .filter(|other| {
            other.to == m.to
                && pos
                    .board
                    .get(other.from)
                    .is_some_and(|p| p.kind == mover.kind)
        })
        .collect();

    let same_file = ambiguous.iter().any(|o| o.from.file() != m.from.file());
    let same_rank = ambiguous.iter().any(|o| o.from.rank() != m.from.rank());

    if !same_file {
        let f = (b'a' + m.from.file()) as char;
        return f.to_string();
    }
    if !same_rank {
        return (m.from.rank() + 1).to_string();
    }
    m.from.to_algebraic()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::Move;
    use crate::square::Square;

    #[test]
    fn formats_pawn_push() {
        let pos = Position::starting_position();
        let e2e4 = Move::from_uci("e2e4").expect("valid uci");
        let legal = pos
            .legal_moves()
            .into_iter()
            .find(|m| m.from == e2e4.from && m.to == e2e4.to)
            .expect("legal");
        assert_eq!(to_san(&pos, legal), "e4");
    }

    #[test]
    fn formats_kingside_castle() {
        let start = Position::from_fen("4k3/8/8/8/8/8/8/R3K2R w K - 0 1").expect("valid fen");
        let e1 = Square::from_algebraic("e1").expect("sq");
        let g1 = Square::from_algebraic("g1").expect("sq");
        let castle_mv = start
            .legal_moves()
            .into_iter()
            .find(|m| m.from == e1 && m.to == g1)
            .expect("castle");
        assert_eq!(to_san(&start, castle_mv), "O-O");
    }
}
