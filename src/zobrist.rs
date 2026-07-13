use crate::board::Square;
use crate::fen::Position;
use crate::types::Color;
use std::sync::OnceLock;

struct ZobristKeys {
    pieces: [[[u64; 64]; 6]; 2],
    black_to_move: u64,
    castling: [u64; 16],
    en_passant_file: [u64; 8],
}

struct SplitMix64 {
    state: u64,
}
impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

fn keys() -> &'static ZobristKeys {
    static KEYS: OnceLock<ZobristKeys> = OnceLock::new();
    KEYS.get_or_init(|| {
        // Fix seed, NEVER CHANGE THIS
        let mut rng = SplitMix64::new(0xC0FFEE_1234_5678);
        let mut pieces = [[[0u64; 64]; 6]; 2];
        
        for c in 0..2 {
            for k in 0..6 {
                for s in 0..64 {
                    pieces[c][k][s] = rng.next();
                }
            }
        }

        let black_to_move = rng.next();
        let mut castling = [0u64; 16];
        for i in 0..16 {
            castling[i] = rng.next();
        }
        let mut en_passant_file = [0u64; 8];
        for i in 0..8 {
            en_passant_file[i] = rng.next();
        }
        ZobristKeys {pieces, black_to_move, castling, en_passant_file}
    })
}

impl Position {
    pub fn zobrist_hash(&self) -> u64 {
        let k = keys();
        let mut h: u64 = 0;

        // Occupied Squares
        for i in 0..64u8 {
            if let Some(p) = self.board.get(Square(i)) {
                h ^= k.pieces[p.color.index()][p.kind.index()][i as usize];
            }
        }

        if self.side_to_move == Color::Black {
            h ^= k.black_to_move;
        }

        let mut mask = 0usize;
        if self.castling.white_kingside { mask |= 1; }
        if self.castling.white_queenside { mask |= 2; }
        if self.castling.black_kingside { mask |= 4; }
        if self.castling.black_queenside { mask |= 8; }
        h ^= k.castling[mask];

        // En-passant: only file matters for repetition
        if let Some(ep) = self.en_passant {
            h ^= k.en_passant_file[ep.file() as usize];
        }
        h
    }
}