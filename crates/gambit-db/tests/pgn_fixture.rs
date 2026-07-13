//! PGN fixture integration tests.

use gambit_db::parse_pgn;
use std::path::PathBuf;

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/pgn/sample.pgn")
}

#[test]
fn twic_sample_pgn_parses() {
    let text = std::fs::read_to_string(fixture_path()).expect("read fixture");
    let game = parse_pgn(&text).expect("parse pgn");
    assert!(game.movetext.moves.len() >= 10);
    let chess = game.to_chess_game().expect("replay mainline");
    assert!(chess.moves.len() >= 10);
}
