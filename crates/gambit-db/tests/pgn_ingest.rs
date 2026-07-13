//! PGN fixture integration tests for multi-game and FEN ingest paths.

use gambit_db::{explode_mainline, parse_pgn, parse_pgn_games, split_pgn_games};
use std::path::PathBuf;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/pgn")
        .join(name)
}

#[test]
fn twic_sample_pgn_parses() {
    let text = std::fs::read_to_string(fixture("sample.pgn")).expect("read fixture");
    let game = parse_pgn(&text).expect("parse pgn");
    assert!(game.movetext.moves.len() >= 10);
    let chess = game.to_chess_game().expect("replay mainline");
    assert!(chess.moves.len() >= 10);
}

#[test]
fn multi_game_fixture_splits() {
    let text = std::fs::read_to_string(fixture("multi_game.pgn")).expect("read fixture");
    let chunks = split_pgn_games(&text);
    assert_eq!(chunks.len(), 3);
    let games: Vec<_> = parse_pgn_games(&text).map(|r| r.expect("parse")).collect();
    assert_eq!(games.len(), 3);
    assert_eq!(
        games[0].headers.get("Event").map(String::as_str),
        Some("Game 1")
    );
    assert_eq!(
        games[2].headers.get("Result").map(String::as_str),
        Some("1/2-1/2")
    );
}

#[test]
fn fen_setup_fixture_explodes() {
    let text = std::fs::read_to_string(fixture("fen_setup.pgn")).expect("read fixture");
    let game = parse_pgn(&text).expect("parse pgn");
    let exploded = explode_mainline(&game).expect("explode");
    assert_eq!(exploded.plies.len(), 2);
    assert_eq!(exploded.positions.len(), 3);
    assert!(exploded.start_fen.contains("4k3"));
}

#[test]
fn sample_explode_produces_positions() {
    let text = std::fs::read_to_string(fixture("sample.pgn")).expect("read fixture");
    let game = parse_pgn(&text).expect("parse pgn");
    let exploded = explode_mainline(&game).expect("explode");
    assert_eq!(exploded.plies.len(), game.movetext.moves.len());
    assert_eq!(exploded.positions.len(), game.movetext.moves.len() + 1);
}
