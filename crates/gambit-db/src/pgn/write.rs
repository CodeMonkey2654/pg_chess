use crate::fen::Position;
use crate::game::ChessGame;
use crate::pgn::parse::{PgnMove, PgnMovetext};
use crate::san::to_san;
use crate::types::Color;
use std::collections::HashMap;

/// Write a PGN string from headers and a chess game mainline.
pub fn write_pgn(headers: &HashMap<String, String>, game: &ChessGame) -> String {
    let movetext = game_to_movetext(game);
    write_pgn_movetext(headers, &movetext)
}

/// Write a PGN string from headers and parsed movetext (including variations).
pub fn write_pgn_movetext(headers: &HashMap<String, String>, movetext: &PgnMovetext) -> String {
    let mut out = String::new();
    let mut tags: Vec<_> = headers.iter().collect();
    tags.sort_by(|a, b| a.0.cmp(b.0));

    for (tag, value) in tags {
        out.push('[');
        out.push_str(tag);
        out.push_str(" \"");
        out.push_str(&value.replace('"', "\\\""));
        out.push_str("\"]\n");
    }
    out.push('\n');

    append_movetext(&mut out, movetext, &Position::starting_position(), true);

    if let Some(result) = headers.get("Result") {
        out.push(' ');
        out.push_str(result);
    }

    out
}

fn append_movetext(out: &mut String, movetext: &PgnMovetext, start: &Position, is_root: bool) {
    let mut pos = start.clone();
    let mut move_number = pos.fullmove_number;
    let mut white_to_move = pos.side_to_move == Color::White;
    let mut first_token = is_root;

    for pm in &movetext.moves {
        if white_to_move {
            if !first_token {
                out.push(' ');
            }
            out.push_str(&format!("{move_number}."));
            first_token = false;
        } else {
            out.push_str(" ...");
        }

        out.push(' ');
        out.push_str(&pm.san);

        if let Some(nag) = pm.nag {
            out.push_str(&format!(" ${nag}"));
        }
        if let Some(comment) = &pm.comment {
            out.push_str(" { ");
            out.push_str(comment);
            out.push_str(" }");
        }

        let fork = pos.clone();
        for var in &pm.variations {
            out.push_str(" (");
            append_movetext(out, var, &fork, false);
            out.push(')');
        }

        pos = pos.apply_move(pm.resolved).expect("movetext legal");
        white_to_move = !white_to_move;
        if white_to_move {
            move_number += 1;
        }
    }
}

fn game_to_movetext(game: &ChessGame) -> PgnMovetext {
    let mut pos = game.start.clone();
    let moves = game
        .moves
        .iter()
        .map(|&m| {
            let san = to_san(&pos, m);
            pos = pos.apply_move(m).expect("game history legal");
            PgnMove {
                san,
                resolved: m,
                nag: None,
                comment: None,
                variations: Vec::new(),
            }
        })
        .collect();
    PgnMovetext { moves }
}

/// Build a minimal PGN from a chess game.
pub fn game_to_pgn(game: &ChessGame) -> String {
    let mut headers = HashMap::new();
    headers.insert("Event".to_string(), "?".to_string());
    headers.insert("Site".to_string(), "?".to_string());
    headers.insert("Date".to_string(), "????.??.??".to_string());
    headers.insert("Round".to_string(), "?".to_string());
    headers.insert("White".to_string(), "?".to_string());
    headers.insert("Black".to_string(), "?".to_string());
    headers.insert("Result".to_string(), "*".to_string());
    write_pgn(&headers, game)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pgn::parse::parse_pgn;

    #[test]
    fn roundtrips_mainline() {
        let pgn = "[Event \"?\"]\n\n1. e4 e5 2. Nf3 *";
        let game = parse_pgn(pgn).expect("parse");
        let out = write_pgn_movetext(&game.headers, &game.movetext);
        assert!(out.contains("1. e4"));
        assert!(out.contains("e5"));
        assert!(out.contains("Nf3"));
    }

    #[test]
    fn roundtrips_variation() {
        let pgn = "[Event \"?\"]\n\n1. e4 (1. d4 d5) e5 *";
        let game = parse_pgn(pgn).expect("parse");
        let out = write_pgn_movetext(&game.headers, &game.movetext);
        assert!(out.contains("1. d4"));
        assert!(out.contains("d5"));
        let reparsed = parse_pgn(&format!("{out}")).expect("reparse");
        assert_eq!(reparsed.movetext, game.movetext);
    }
}
