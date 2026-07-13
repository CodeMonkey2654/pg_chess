//! Precomputed attack bitboards for leapers.

use crate::square::Square;
use std::sync::LazyLock;

pub(crate) static KNIGHT_ATTACKS: LazyLock<[u64; 64]> =
    LazyLock::new(|| compute_leaper_attacks(&KNIGHT_DELTAS));
pub(crate) static KING_ATTACKS: LazyLock<[u64; 64]> =
    LazyLock::new(|| compute_leaper_attacks(&KING_DELTAS));

const KNIGHT_DELTAS: [(i8, i8); 8] = [
    (1, 2),
    (2, 1),
    (2, -1),
    (-1, 2),
    (1, -2),
    (-2, -1),
    (-1, -2),
    (-2, 1),
];

const KING_DELTAS: [(i8, i8); 8] = [
    (0, 1),
    (0, -1),
    (1, 0),
    (-1, 0),
    (1, 1),
    (-1, 1),
    (-1, -1),
    (1, -1),
];

fn compute_leaper_attacks(deltas: &[(i8, i8)]) -> [u64; 64] {
    let mut table = [0u64; 64];
    for sq in 0..64u8 {
        table[sq as usize] = leaper_attacks(Square(sq), deltas);
    }
    table
}

fn leaper_attacks(from: Square, deltas: &[(i8, i8)]) -> u64 {
    let mut attacks = 0u64;
    for &(file_offset, rank_offset) in deltas {
        if let Some(to) = offset(from, file_offset, rank_offset) {
            attacks |= 1u64 << to.index();
        }
    }
    attacks
}

fn offset(sq: Square, file_offset: i8, rank_offset: i8) -> Option<Square> {
    let file = sq.file() as i8 + file_offset;
    let rank = sq.rank() as i8 + rank_offset;
    if (0..8).contains(&file) && (0..8).contains(&rank) {
        Square::from_file_rank(file as u8, rank as u8)
    } else {
        None
    }
}
