//! Mate puzzle correctness tests.

use gambit_analysis::{Analyzer, Score, SearchLimits};
use gambit_db::{Move, Position};

fn find_move(pos: &Position, depth: u32) -> Move {
    let mut analyzer = Analyzer::new();
    analyzer.search(pos, SearchLimits::depth(depth)).best_move
}

fn is_mate_in_one(pos: &Position, mv: Move) -> bool {
    let after = pos.apply_move(mv).expect("legal");
    after.is_checkmate()
}

#[test]
fn mate_in_1_queen_back_rank() {
    let pos = Position::from_fen("6k1/4Q3/5K2/8/8/8/8/8 w - - 0 1").expect("valid fen");
    let mv = find_move(&pos, 4);
    assert!(
        is_mate_in_one(&pos, mv),
        "expected mate in 1, got {}",
        mv.to_uci()
    );
}

#[test]
fn mate_in_1_queen_corner() {
    let pos = Position::from_fen("7k/5Q2/6K1/8/8/8/8/8 w - - 0 1").expect("valid fen");
    let mv = find_move(&pos, 4);
    assert!(
        is_mate_in_one(&pos, mv),
        "expected mate in 1, got {}",
        mv.to_uci()
    );
}

#[test]
fn terminal_checkmate_has_no_legal_moves() {
    let pos = Position::from_fen("6k1/4Q3/5K2/8/8/8/8/8 w - - 0 1").expect("valid fen");
    let mv = find_move(&pos, 4);
    let after = pos.apply_move(mv).expect("legal");
    assert!(after.is_checkmate());
    assert!(after.legal_moves().is_empty());
}

#[test]
fn startpos_eval_near_equal() {
    let pos = Position::starting_position();
    let mut analyzer = Analyzer::new();
    let result = analyzer.search(&pos, SearchLimits::depth(4));
    if let Score::Cp(cp) = result.score {
        assert!((-50..=50).contains(&cp));
    }
}

#[test]
fn same_position_same_score() {
    let pos = Position::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1")
        .expect("valid fen");
    let mut a = Analyzer::new();
    let mut b = Analyzer::new();
    let sa = a.search(&pos, SearchLimits::depth(5));
    let sb = b.search(&pos, SearchLimits::depth(5));
    assert_eq!(sa.score, sb.score);
}
