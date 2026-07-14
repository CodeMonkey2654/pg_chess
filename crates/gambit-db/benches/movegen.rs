#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gambit_db::{perft, ChessGame, Move, MoveList, Position};

fn bench_legal_moves(c: &mut Criterion) {
    let start = Position::starting_position();
    c.bench_function("legal_moves_startpos", |b| {
        b.iter(|| black_box(&start).legal_moves());
    });
}

fn bench_generate_legal_moves(c: &mut Criterion) {
    let mut start = Position::starting_position();
    c.bench_function("generate_legal_moves_startpos", |b| {
        b.iter(|| {
            let mut list = MoveList::new();
            black_box(&mut start).generate_legal_moves(black_box(&mut list));
            black_box(list.len())
        });
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
[Result "1-0"]

1. e4 e5 2. Nf3 Nc6 3. Bc4 Bc5 4. c3 Nf6 5. d4 exd4 6. cxd4 Bb4+ 7. Nc3 1-0
"#;
    let game = parse_pgn(PGN).expect("valid pgn");
    c.bench_function("explode_mainline_40_plies", |b| {
        b.iter(|| explode_mainline(black_box(&game)).expect("explode"));
    });
}

criterion_group!(
    benches,
    bench_legal_moves,
    bench_generate_legal_moves,
    bench_apply_move,
    bench_from_fen,
    bench_zobrist_hash,
    bench_game_replay,
    bench_perft_d3,
    bench_explode_mainline
);
criterion_main!(benches);
