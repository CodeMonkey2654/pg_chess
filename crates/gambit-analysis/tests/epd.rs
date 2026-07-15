//! EPD-style tactical positions for native search regression.

use gambit_analysis::{Analyzer, SearchLimits};
use gambit_db::Position;

#[test]
fn epd_startpos_finds_e4() {
    let mut analyzer = Analyzer::new();
    let pos = Position::starting_position();
    let result = analyzer.search(&pos, SearchLimits::depth(6));
    assert_eq!(result.best_move.to_uci(), "e2e4");
}

#[test]
fn epd_search_completes_under_time_limit() {
    let mut analyzer = Analyzer::new();
    let pos = Position::from_fen("rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1")
        .expect("valid fen");
    let result = analyzer.search(&pos, SearchLimits::movetime(50));
    assert!(result.depth >= 1);
    assert!(result.nodes > 0);
}
