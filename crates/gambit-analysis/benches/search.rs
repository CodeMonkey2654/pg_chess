#![allow(missing_docs)]

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use gambit_analysis::{Analyzer, SearchLimits};
use gambit_db::Position;

fn bench_search_depth_4_startpos(c: &mut Criterion) {
    let pos = Position::starting_position();
    c.bench_function("search_depth_4_startpos", |b| {
        b.iter(|| {
            let mut analyzer = Analyzer::new();
            black_box(analyzer.search(black_box(&pos), SearchLimits::depth(4)))
        });
    });
}

fn bench_search_depth_6_tactical(c: &mut Criterion) {
    let pos =
        Position::from_fen("r1bqkb1r/pppp1ppp/2n2n2/4p2Q/2B1P3/8/PPPP1PPP/RNB1K1NR w KQkq - 4 4")
            .expect("valid fen");
    c.bench_function("search_depth_6_tactical", |b| {
        b.iter(|| {
            let mut analyzer = Analyzer::new();
            black_box(analyzer.search(black_box(&pos), SearchLimits::depth(6)))
        });
    });
}

criterion_group!(
    benches,
    bench_search_depth_4_startpos,
    bench_search_depth_6_tactical
);
criterion_main!(benches);
