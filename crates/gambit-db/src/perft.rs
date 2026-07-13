use crate::fen::Position;

/// Perft: count legal move sequences to `depth` plies from `position`.
pub fn perft(position: &Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let mut count = 0u64;
    let mut pos = position.clone();
    for m in position.legal_moves() {
        let Ok(undo) = pos.make_move(m) else {
            continue;
        };
        count += perft_in_place(&mut pos, depth - 1);
        pos.unmake_move(undo);
    }
    count
}

fn perft_in_place(pos: &mut Position, depth: u32) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves: Vec<_> = pos.legal_moves().into_iter().collect();
    let mut count = 0u64;
    for m in moves {
        let Ok(undo) = pos.make_move(m) else {
            continue;
        };
        count += perft_in_place(pos, depth - 1);
        pos.unmake_move(undo);
    }
    count
}
