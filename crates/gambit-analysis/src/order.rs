//! Move ordering for alpha-beta efficiency.

use crate::report::MoveStat;
use gambit_db::{Move, MoveFlags, MoveList, PieceKind, Position};

const KILLER_SLOTS: usize = 2;

/// Killer moves per ply (quiet moves that caused beta cutoffs).
pub struct Killers {
    table: [[Option<Move>; KILLER_SLOTS]; 128],
}

impl Killers {
    /// Empty killer table.
    pub fn new() -> Self {
        Self {
            table: [[None; KILLER_SLOTS]; 128],
        }
    }

    /// Reset for a new game.
    pub fn clear(&mut self) {
        self.table = [[None; KILLER_SLOTS]; 128];
    }

    /// Record a killer at `ply`.
    pub fn store(&mut self, ply: u32, m: Move) {
        let idx = ply.min(127) as usize;
        if self.table[idx][0] == Some(m) {
            return;
        }
        self.table[idx][1] = self.table[idx][0];
        self.table[idx][0] = Some(m);
    }

    fn killer_score(&self, ply: u32, m: Move) -> i32 {
        let idx = ply.min(127) as usize;
        if self.table[idx][0] == Some(m) {
            return 900_000;
        }
        if self.table[idx][1] == Some(m) {
            return 800_000;
        }
        0
    }
}

impl Default for Killers {
    fn default() -> Self {
        Self::new()
    }
}

fn mvv_lva(m: Move, pos: &Position) -> i32 {
    if !m.flags.contains(MoveFlags::CAPTURE) {
        return 0;
    }
    let victim = pos
        .board
        .get(m.to)
        .map(|p| piece_order(p.kind))
        .unwrap_or(1);
    let attacker = pos
        .board
        .get(m.from)
        .map(|p| piece_order(p.kind))
        .unwrap_or(1);
    victim * 10 - attacker
}

fn piece_order(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Pawn => 1,
        PieceKind::Knight | PieceKind::Bishop => 3,
        PieceKind::Rook => 5,
        PieceKind::Queen => 9,
        PieceKind::King => 100,
    }
}

fn corpus_score(stat: &MoveStat) -> i32 {
    let total = stat.count.max(1) as i32;
    let wins = stat.white_wins as i32 + stat.black_wins as i32;
    stat.count as i32 * 100 + wins * 1000 / total
}

/// Sort moves in descending priority (best first).
pub fn order_moves(
    moves: &mut MoveList,
    pos: &Position,
    ply: u32,
    tt_move: Option<Move>,
    killers: &Killers,
    corpus: Option<&[MoveStat]>,
) {
    let slice = moves.as_slice();
    let mut scored: Vec<(i32, Move)> = slice
        .iter()
        .map(|&m| {
            let mut score = 0i32;
            if tt_move == Some(m) {
                score += 2_000_000;
            }
            score += killers.killer_score(ply, m);
            score += mvv_lva(m, pos);
            if let Some(stats) = corpus {
                for stat in stats {
                    if stat.uci.from == m.from
                        && stat.uci.to == m.to
                        && stat.uci.promotion == m.promotion
                    {
                        score += corpus_score(stat);
                        break;
                    }
                }
            }
            (score, m)
        })
        .collect();
    scored.sort_by_key(|b| std::cmp::Reverse(b.0));
    moves.clear();
    for (_, m) in scored {
        moves.push(m);
    }
}
