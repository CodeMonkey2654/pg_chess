//! UCI move parsing helpers.

/// Parsed UCI move: from/to squares and optional promotion piece.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UciMove {
    pub from: String,
    pub to: String,
    pub promotion: Option<char>,
}

/// Parse a UCI move string (e.g. "e2e4", "e7e8q").
pub fn parse_uci(uci: &str) -> Option<UciMove> {
    if uci.len() < 4 {
        return None;
    }
    let from = uci[0..2].to_string();
    let to = uci[2..4].to_string();
    let promotion = if uci.len() >= 5 {
        uci.chars().nth(4)
    } else {
        None
    };
    Some(UciMove {
        from,
        to,
        promotion,
    })
}

/// If the move is castling, return the rook from/to squares.
pub fn castling_rook_move(king_from: &str, king_to: &str) -> Option<(String, String)> {
    match (king_from, king_to) {
        ("e1", "g1") => Some(("h1".into(), "f1".into())),
        ("e1", "c1") => Some(("a1".into(), "d1".into())),
        ("e8", "g8") => Some(("h8".into(), "f8".into())),
        ("e8", "c8") => Some(("a8".into(), "d8".into())),
        _ => None,
    }
}

/// Promotion piece options for a pawn move from `from` to `to`.
pub fn promotion_options(legal_moves: &[String], from: &str, to: &str) -> Vec<char> {
    let prefix = format!("{from}{to}");
    legal_moves
        .iter()
        .filter(|m| m.starts_with(&prefix) && m.len() == 5)
        .filter_map(|m| m.chars().nth(4))
        .collect()
}

/// Find the matching legal UCI move, defaulting promotion to queen.
pub fn matching_uci(
    legal_moves: &[String],
    from: &str,
    to: &str,
    promotion: Option<char>,
) -> Option<String> {
    let base = format!("{from}{to}");
    if let Some(p) = promotion {
        let full = format!("{base}{p}");
        if legal_moves.iter().any(|m| m == &full) {
            return Some(full);
        }
        return None;
    }
    let matches: Vec<&String> = legal_moves
        .iter()
        .filter(|m| m.starts_with(&base) && m.len() >= 4)
        .collect();
    if matches.len() == 1 {
        return Some(matches[0].clone());
    }
    matches.iter().find(|m| m.len() == 4).map(|m| (*m).clone())
}
