//! Static Exchange Evaluation for capture ordering and pruning.

use gambit_db::{Move, MoveFlags, PieceKind, Position};

fn piece_value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Pawn => 100,
        PieceKind::Knight | PieceKind::Bishop => 300,
        PieceKind::Rook => 500,
        PieceKind::Queen => 900,
        PieceKind::King => 10_000,
    }
}

/// SEE score for `capture` on `pos` (positive = capture wins material for mover).
pub fn see(pos: &Position, capture: Move) -> i32 {
    if !capture.flags.contains(MoveFlags::CAPTURE) {
        return 0;
    }
    let Some(victim) = pos.board.get(capture.to) else {
        return 0;
    };
    let victim_val = piece_value(victim.kind);
    let Some(attacker) = pos.board.get(capture.from) else {
        return 0;
    };
    let attacker_val = piece_value(attacker.kind);

    // Simplified SEE: victim value minus cheapest recapture estimate.
    let mut gain = victim_val - attacker_val;
    if gain < 0 {
        return gain;
    }

    // If square is defended by lower-value piece, adjust.
    if defended_by_lower(pos, capture.to, pos.side_to_move.flip()) {
        gain -= victim_val;
    }
    gain
}

fn defended_by_lower(pos: &Position, sq: gambit_db::Square, by: gambit_db::Color) -> bool {
    for (from, piece) in pos.board.iter_occupied() {
        if piece.color != by || from == sq {
            continue;
        }
        if attacks(pos, from, sq) && piece_value(piece.kind) < piece_value(
            pos.board.get(sq).map(|p| p.kind).unwrap_or(PieceKind::Pawn),
        ) {
            return true;
        }
    }
    false
}

fn attacks(pos: &Position, from: gambit_db::Square, to: gambit_db::Square) -> bool {
    let Some(piece) = pos.board.get(from) else {
        return false;
    };
    let df = (to.file() as i32 - from.file() as i32).abs();
    let dr = (to.rank() as i32 - from.rank() as i32).abs();
    match piece.kind {
        PieceKind::Pawn => {
            let dir: i32 = if piece.color == gambit_db::Color::White {
                1
            } else {
                -1
            };
            (to.rank() as i32 - from.rank() as i32) == dir && df == 1
        }
        PieceKind::Knight => (df == 2 && dr == 1) || (df == 1 && dr == 2),
        PieceKind::Bishop => df == dr && line_clear(pos, from, to),
        PieceKind::Rook => (df == 0 || dr == 0) && line_clear(pos, from, to),
        PieceKind::Queen => {
            (df == dr || df == 0 || dr == 0) && line_clear(pos, from, to)
        }
        PieceKind::King => df <= 1 && dr <= 1,
    }
}

fn line_clear(pos: &Position, from: gambit_db::Square, to: gambit_db::Square) -> bool {
    let df = (to.file() as i32 - from.file() as i32).signum();
    let dr = (to.rank() as i32 - from.rank() as i32).signum();
    let mut f = from.file() as i32 + df;
    let mut r = from.rank() as i32 + dr;
    while f != to.file() as i32 || r != to.rank() as i32 {
        if let Some(sq) = gambit_db::Square::from_file_rank(f as u8, r as u8) {
            if pos.board.get(sq).is_some() {
                return false;
            }
        }
        f += df;
        r += dr;
    }
    true
}
