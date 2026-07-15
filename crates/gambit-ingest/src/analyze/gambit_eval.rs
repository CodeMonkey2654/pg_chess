//! Native corpus + Syzygy + search evaluator (no external engines).

use crate::analyze::AnalyzeOptions;
use anyhow::Result;
use gambit_analysis::{
    detect_phase, CorpusBook, EvalSource, GamePhase, NativeEvaluator, PositionEval,
    PositionEvaluator, DEFAULT_OPENING_PLY,
};
use gambit_db::{Position, Tablebase, Wdl};
use std::path::PathBuf;
use std::sync::Arc;

/// Routes positions to corpus book, Syzygy tablebases, or native search.
pub struct GambitEvaluator {
    native: NativeEvaluator,
    book: Option<Arc<CorpusBook>>,
    tablebase: Option<Tablebase>,
    opening_ply: u32,
}

impl GambitEvaluator {
    /// Build evaluator from analyze options.
    pub fn new(options: &AnalyzeOptions) -> Result<Self> {
        let depth = options.depth;
        let mut native = NativeEvaluator::new(depth);
        let book = resolve_corpus_book(options).map(Arc::new);
        if let Some(ref b) = book {
            native = NativeEvaluator::with_book(depth, b.clone());
        }
        let tablebase = resolve_tablebase(options);

        Ok(Self {
            native,
            book,
            tablebase,
            opening_ply: DEFAULT_OPENING_PLY,
        })
    }
}

impl PositionEvaluator for GambitEvaluator {
    fn evaluate(&mut self, pos: &Position, ply: u32, _phase: GamePhase) -> PositionEval {
        let phase = detect_phase(pos, ply, self.opening_ply);

        if phase == GamePhase::Endgame {
            if let Some(ref tb) = self.tablebase {
                if let Some(ev) = probe_tablebase(tb, pos) {
                    return ev;
                }
            }
        }

        if phase == GamePhase::Opening {
            if let Some(ref book) = self.book {
                if let Some(stats) = book.lookup(pos.hash) {
                    if let Some(best) = stats.iter().max_by_key(|s| s.count) {
                        let shallow = self.native.evaluate(pos, ply, phase);
                        return PositionEval {
                            best_move: best.uci,
                            source: EvalSource::Corpus,
                            ..shallow
                        };
                    }
                }
            }
        }

        let mut ev = self.native.evaluate(pos, ply, phase);
        ev.source = EvalSource::Native;
        ev
    }
}

fn probe_tablebase(tb: &Tablebase, pos: &Position) -> Option<PositionEval> {
    let wdl = tb.probe_wdl(pos)?;
    let cp = match wdl {
        Wdl::Win => 500,
        Wdl::CursedWin => 50,
        Wdl::Draw => 0,
        Wdl::BlessedLoss => -50,
        Wdl::Loss => -500,
    };
    let legal = pos.legal_moves();
    let best_move = legal.first().copied()?;
    Some(PositionEval {
        score: gambit_analysis::Score::Cp(cp),
        best_move,
        depth: 0,
        pv: vec![best_move],
        source: EvalSource::Syzygy,
    })
}

/// Resolve corpus book path from options or environment.
pub fn resolve_corpus_book(options: &AnalyzeOptions) -> Option<CorpusBook> {
    let path = options
        .corpus_book
        .clone()
        .or_else(|| std::env::var("GAMBIT_CORPUS_BOOK").ok())
        .map(PathBuf::from)?;
    match CorpusBook::load(&path) {
        Ok(book) => Some(book),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "corpus book unavailable");
            None
        }
    }
}

fn resolve_tablebase(options: &AnalyzeOptions) -> Option<Tablebase> {
    let path = options
        .syzygy_path
        .clone()
        .or_else(|| std::env::var("GAMBIT_SYZYGY_PATH").ok())
        .map(PathBuf::from)?;
    match Tablebase::open(&path) {
        Ok(tb) => Some(tb),
        Err(e) => {
            tracing::warn!(path = %path.display(), error = %e, "Syzygy tablebases unavailable");
            None
        }
    }
}
