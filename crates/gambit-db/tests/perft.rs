//! Perft integration tests.

use gambit_db::{perft, Position};

const STARTPOS: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

#[test]
fn startpos_perft_d1() {
    let pos = Position::from_fen(STARTPOS).expect("valid fen");
    assert_eq!(perft(&pos, 1), 20);
}

#[test]
fn startpos_perft_d2() {
    let pos = Position::from_fen(STARTPOS).expect("valid fen");
    assert_eq!(perft(&pos, 2), 400);
}

#[test]
fn startpos_perft_d3() {
    let pos = Position::from_fen(STARTPOS).expect("valid fen");
    assert_eq!(perft(&pos, 3), 8902);
}

#[test]
fn startpos_perft_d4() {
    let pos = Position::from_fen(STARTPOS).expect("valid fen");
    assert_eq!(perft(&pos, 4), 197_281);
}

#[test]
fn kiwipete_perft_d1() {
    let pos = Position::from_fen(KIWIPETE).expect("valid fen");
    assert_eq!(perft(&pos, 1), 48);
}

#[test]
fn kiwipete_perft_d2() {
    let pos = Position::from_fen(KIWIPETE).expect("valid fen");
    assert_eq!(perft(&pos, 2), 2039);
}

#[test]
#[ignore]
fn kiwipete_perft_d5() {
    let pos = Position::from_fen(KIWIPETE).expect("valid fen");
    assert_eq!(perft(&pos, 5), 4_865_609);
}
