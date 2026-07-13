#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gambit_db::{perft, ChessGame, Move, Position};

fn bench_legal_moves(c: &mut Criterion) {
    let start = Position::starting_position();
    c.bench_function("legal_moves_startpos", |b| {
        b.iter(|| black_box(&start).legal_moves());
    });
}

fn bench_apply_move(c: &mut Criterion) {
    let start = Position::starting_position();
    let mv = Move::from_uci("e2e4").expect("valid uci");
    c.bench_function("apply_move_startpos", |b| {
        b.iter(|| black_box(&start).apply_move(mv));
    });
}

fn bench_from_fen(c: &mut Criterion) {
    const FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    c.bench_function("from_fen_startpos", |b| {
        b.iter(|| Position::from_fen(black_box(FEN)));
    });
}

fn bench_zobrist_hash(c: &mut Criterion) {
    let start = Position::starting_position();
    c.bench_function("zobrist_hash_startpos", |b| {
        b.iter(|| black_box(&start).zobrist_hash());
    });
}

fn bench_game_replay(c: &mut Criterion) {
    let mut game = ChessGame::new();
    for _ in 0..20 {
        let pos = game.current_position();
        let mv = pos
            .legal_moves()
            .into_iter()
            .next()
            .expect("non-terminal position");
        game.play(mv).expect("legal move from legal_moves");
    }
    c.bench_function("game_replay_20_plies", |b| {
        b.iter(|| {
            let pos = black_box(&game).current_position();
            black_box(pos.zobrist_hash())
        });
    });
}

fn bench_perft_d3(c: &mut Criterion) {
    let start = Position::starting_position();
    c.bench_function("perft_startpos_d3", |b| {
        b.iter(|| perft(black_box(&start), 3));
    });
}

fn bench_explode_mainline(c: &mut Criterion) {
    use gambit_db::{explode_mainline, parse_pgn};
    const PGN: &str = r#"[Event "?"]
[Result "*"]

1. e4 e5 2. Nf3 Nc6 3. Bb5 a6 4. Ba4 Nf6 5. O-O Be7 6. Re1 b5 7. Bb3 d6 8. c3 O-O
9. h3 Nb8 10. d4 Nbd7 11. c4 c6 12. cxb5 axb5 13. Nc3 Bb7 14. Bg5 b4 15. Nb1 h6
16. Bh4 c5 17. dxe5 Nxe4 18. Bxe7 Qxe7 19. exd6 Qf6 20. Nbd2 Nxd6 21. Nc4 Nxc4
22. Bxc4 Nb6 23. Ne5 Rae8 24. Bxf7+ Rxf7 25. Nxf7 Rxe1+ 26. Qxe1 Kxf7 27. Qe3 Qg5
28. Qxg5 hxg5 29. b3 Ke6 30. a3 Kd6 31. axb4 cxb4 32. Ra5 Nd5 33. f3 Bc8 34. Kf2
Bf5 35. Ra7 g6 36. Ra6+ Kc5 37. Ke1 Nf4 38. g3 Nxh3 39. Kd2 Kb5 40. Rd6 Kc5 41. Ra6
Nf2 42. g4 Bd3 43. Rd6 Kc5 44. Ra6 Nh3 45. Kc3 1/2-1/2
"#;
    let game = parse_pgn(PGN).expect("valid pgn");
    c.bench_function("explode_mainline_40_plies", |b| {
        b.iter(|| explode_mainline(black_box(&game)).expect("explode"));
    });
}

criterion_group!(
    benches,
    bench_legal_moves,
    bench_apply_move,
    bench_from_fen,
    bench_zobrist_hash,
    bench_game_replay,
    bench_perft_d3,
    bench_explode_mainline
);
criterion_main!(benches);
