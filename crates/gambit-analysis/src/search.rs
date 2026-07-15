//! Alpha-beta negamax search with iterative deepening and quiescence.

use crate::book::CorpusBook;
use crate::eval;
use crate::limits::SearchLimits;
use crate::order::{order_moves, Killers};
use crate::report::{Analysis, Score};
use crate::see;
use crate::tt::TranspositionTable;
use gambit_db::{Move, MoveFlags, MoveList, Position};
use std::time::{Duration, Instant};

const MATE_SCORE: i32 = 30_000;
const MAX_PLY: u32 = 64;
const ENDGAME_PIECES: u32 = 6;

fn count_pieces(pos: &Position) -> u32 {
    pos.board.iter_occupied().count() as u32
}

/// Chess analysis engine.
pub struct Analyzer {
    tt: TranspositionTable,
    killers: Killers,
    book: Option<CorpusBook>,
    book_cutoff_depth: u32,
    nodes: u64,
    stop: Option<Instant>,
    limits: SearchLimits,
    best_pv: Vec<Move>,
    best_score: i32,
    best_move: Option<Move>,
    depth_reached: u32,
}

impl Analyzer {
    /// New analyzer with default settings.
    pub fn new() -> Self {
        Self {
            tt: TranspositionTable::new(),
            killers: Killers::new(),
            book: None,
            book_cutoff_depth: 0,
            nodes: 0,
            stop: None,
            limits: SearchLimits::default(),
            best_pv: Vec::new(),
            best_score: 0,
            best_move: None,
            depth_reached: 0,
        }
    }

    /// Load a corpus book from disk.
    pub fn with_book(mut self, book: CorpusBook) -> Self {
        self.book = Some(book);
        self
    }

    /// Set ply limit for book-only cutoff (0 = disabled).
    pub fn book_cutoff_depth(mut self, depth: u32) -> Self {
        self.book_cutoff_depth = depth;
        self
    }

    /// Reset transposition table and killers for a new game.
    pub fn new_game(&mut self) {
        self.tt.clear();
        self.killers.clear();
    }

    /// Search `pos` within `limits`.
    pub fn search(&mut self, pos: &Position, limits: SearchLimits) -> Analysis {
        let start = Instant::now();
        self.limits = limits;
        self.nodes = 0;
        self.stop = if limits.movetime_ms > 0 {
            Some(start + Duration::from_millis(limits.movetime_ms))
        } else {
            None
        };
        self.best_pv.clear();
        self.best_move = None;
        self.best_score = 0;
        self.depth_reached = 0;

        let corpus_root = self
            .book
            .as_ref()
            .and_then(|b| b.lookup(pos.hash))
            .map(|s| s.to_vec());

        let mut search_pos = pos.clone();
        let max_depth = if limits.depth > 0 {
            limits.depth
        } else {
            MAX_PLY
        };

        for depth in 1..=max_depth {
            if self.should_stop() {
                break;
            }
            let window = if depth >= 4 && self.depth_reached > 0 {
                50
            } else {
                MATE_SCORE
            };
            let alpha = self.best_score.saturating_sub(window);
            let beta = self.best_score.saturating_add(window);
            let mut score = self.negamax(&mut search_pos, depth, alpha, beta, 0);
            if score <= alpha || score >= beta {
                score = self.negamax(&mut search_pos, depth, -MATE_SCORE, MATE_SCORE, 0);
            }
            if self.should_stop() {
                break;
            }
            self.depth_reached = depth;
            self.best_score = score;
            self.rebuild_pv(pos, depth);
        }

        let best_move = if let Some(m) = self.best_move {
            m
        } else {
            let mut list = MoveList::new();
            search_pos.generate_legal_moves(&mut list);
            list.as_slice()
                .first()
                .copied()
                .unwrap_or_else(|| Move::from_uci("e2e4").expect("startpos has moves"))
        };

        let score = if self.best_score.abs() >= MATE_SCORE - MAX_PLY as i32 {
            let plies = MATE_SCORE - self.best_score.abs();
            if self.best_score > 0 {
                Score::Mate(plies)
            } else {
                Score::Mate(-plies)
            }
        } else {
            Score::Cp(self.best_score)
        };

        Analysis {
            best_move,
            score,
            pv: self.best_pv.clone(),
            depth: self.depth_reached,
            nodes: self.nodes,
            time_ms: start.elapsed().as_millis() as u64,
            corpus: corpus_root,
        }
    }

    fn should_stop(&self) -> bool {
        if let Some(deadline) = self.stop {
            if Instant::now() >= deadline {
                return true;
            }
        }
        if self.limits.max_nodes > 0 && self.nodes >= self.limits.max_nodes {
            return true;
        }
        false
    }

    fn rebuild_pv(&mut self, pos: &Position, depth: u32) {
        self.best_pv.clear();
        let mut scratch = pos.clone();
        for _ in 0..depth {
            let key = scratch.hash;
            let Some(tt_mv) = self.tt.best_move(key) else {
                break;
            };
            let mut legal = MoveList::new();
            scratch.generate_legal_moves(&mut legal);
            let Some(mv) = legal.as_slice().iter().find(|m| {
                m.from == tt_mv.from && m.to == tt_mv.to && m.promotion == tt_mv.promotion
            }) else {
                break;
            };
            self.best_pv.push(*mv);
            let Ok(undo) = scratch.make_move(*mv) else {
                break;
            };
            scratch.unmake_move(undo);
        }
    }

    fn negamax(
        &mut self,
        pos: &mut Position,
        depth: u32,
        mut alpha: i32,
        beta: i32,
        ply: u32,
    ) -> i32 {
        self.nodes += 1;
        if self.should_stop() {
            return eval::evaluate(pos);
        }

        if pos.is_fifty_move_draw() || pos.is_insufficient_material() {
            return 0;
        }

        if pos.is_checkmate() {
            return -MATE_SCORE + ply as i32;
        }
        if pos.is_stalemate() {
            return 0;
        }

        if depth == 0 {
            return self.quiescence(pos, alpha, beta, ply);
        }

        let in_check = pos.is_in_check(pos.side_to_move);

        // Null-move pruning (skip when in check or low material).
        if depth >= 3 && !in_check && ply > 0 && count_pieces(pos) > ENDGAME_PIECES {
            let null_depth = depth - 3;
            let saved_stm = pos.side_to_move;
            let saved_hash = pos.hash;
            pos.side_to_move = saved_stm.flip();
            pos.hash = pos.zobrist_hash();
            let score = -self.negamax(pos, null_depth, -beta, -beta + 1, ply + 1);
            pos.side_to_move = saved_stm;
            pos.hash = saved_hash;
            if score >= beta {
                return beta;
            }
        }

        let key = pos.hash;
        if let Some(score) = self.tt.probe(key, depth as i32, alpha, beta) {
            return score;
        }

        let corpus = self.book.as_ref().and_then(|b| b.lookup(key));

        let mut moves = MoveList::new();
        pos.generate_legal_moves(&mut moves);
        if moves.is_empty() {
            return if pos.is_in_check(pos.side_to_move) {
                -MATE_SCORE + ply as i32
            } else {
                0
            };
        }

        let tt_move = self.tt.best_move(key);
        order_moves(&mut moves, pos, ply, tt_move, &self.killers, corpus);

        let mut best_move = None;
        let mut best_score = -MATE_SCORE;
        let mut moves_searched = 0;

        for m in moves.as_slice() {
            let is_capture = m.flags.contains(MoveFlags::CAPTURE);
            let is_quiet = !is_capture && !m.flags.contains(MoveFlags::PROMOTION);

            if is_capture && see::see(pos, *m) < 0 {
                continue;
            }

            let mut reduction = 0u32;
            if depth >= 3 && moves_searched >= 3 && is_quiet && !in_check {
                reduction = 1;
                if moves_searched >= 6 && depth >= 5 {
                    reduction = 2;
                }
            }

            let search_depth = depth - 1 - reduction;

            let Ok(undo) = pos.make_move(*m) else {
                continue;
            };
            let mut score = -self.negamax(pos, search_depth, -beta, -alpha, ply + 1);
            if reduction > 0 && score > alpha {
                score = -self.negamax(pos, depth - 1, -beta, -alpha, ply + 1);
            }
            pos.unmake_move(undo);

            if self.should_stop() {
                break;
            }

            if score > best_score {
                best_score = score;
                best_move = Some(*m);
            }
            if score > alpha {
                alpha = score;
                if ply == 0 {
                    self.best_move = Some(*m);
                }
            }
            if alpha >= beta {
                if !m.flags.contains(MoveFlags::CAPTURE) {
                    self.killers.store(ply, *m);
                }
                break;
            }
            moves_searched += 1;
        }

        if moves_searched == 0 {
            best_score = eval::evaluate(pos);
        }

        self.tt
            .store(key, depth as i32, best_score, alpha, beta, best_move);
        best_score
    }

    fn quiescence(&mut self, pos: &mut Position, mut alpha: i32, beta: i32, ply: u32) -> i32 {
        self.nodes += 1;
        if self.should_stop() {
            return eval::evaluate(pos);
        }

        if pos.is_checkmate() {
            return -MATE_SCORE + ply as i32;
        }
        if pos.is_stalemate() {
            return 0;
        }

        let stand_pat = eval::evaluate(pos);
        if stand_pat >= beta {
            return beta;
        }
        if alpha < stand_pat {
            alpha = stand_pat;
        }

        let mut moves = MoveList::new();
        pos.generate_legal_moves(&mut moves);

        let mut captures = MoveList::new();
        for m in moves.as_slice() {
            if m.flags.contains(MoveFlags::CAPTURE) {
                captures.push(*m);
            }
        }
        order_moves(&mut captures, pos, ply, None, &self.killers, None);

        for m in captures.as_slice() {
            if see::see(pos, *m) < 0 {
                continue;
            }
            let Ok(undo) = pos.make_move(*m) else {
                continue;
            };
            let score = -self.quiescence(pos, -beta, -alpha, ply + 1);
            pos.unmake_move(undo);
            if score >= beta {
                return beta;
            }
            if score > alpha {
                alpha = score;
            }
        }
        alpha
    }
}

impl Default for Analyzer {
    fn default() -> Self {
        Self::new()
    }
}

// Re-export for book module tests without tempfile
#[cfg(test)]
mod book_test {
    use super::*;
    use crate::book::write_book;
    use crate::report::MoveStat;
    use gambit_db::Move;

    #[test]
    fn book_roundtrip_via_analyzer() {
        let mv = Move::from_uci("e2e4").expect("valid");
        let rows = vec![(
            Position::starting_position().hash,
            vec![MoveStat {
                uci: mv,
                count: 50,
                white_wins: 20,
                black_wins: 15,
                draws: 15,
            }],
        )];
        let path = std::env::temp_dir().join("gambit_test.gbook");
        write_book(&path, &rows).expect("write");
        let book = CorpusBook::load(&path).expect("load");
        let _ = std::fs::remove_file(&path);
        let mut analyzer = Analyzer::new().with_book(book);
        let pos = Position::starting_position();
        let result = analyzer.search(&pos, SearchLimits::depth(3));
        assert!(result.corpus.is_some());
        assert!(!result.corpus.as_ref().expect("corpus").is_empty());
    }
}
