use crate::fen::{CastlingRights, Position};
use crate::square::Square;
use crate::types::{Color, Piece};
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
        let mut rng = SplitMix64::new(0xC0FF_EE12_3456_5678);
        let mut pieces = [[[0u64; 64]; 6]; 2];

        for color_row in &mut pieces {
            for kind_row in color_row {
                for key in kind_row {
                    *key = rng.next();
                }
            }
        }

        let black_to_move = rng.next();
        let mut castling = [0u64; 16];
        for key in &mut castling {
            *key = rng.next();
        }
        let mut en_passant_file = [0u64; 8];
        for key in &mut en_passant_file {
            *key = rng.next();
        }
        ZobristKeys {
            pieces,
            black_to_move,
            castling,
            en_passant_file,
        }
    })
}

/// Zobrist key for a piece on a square.
pub fn piece_key(piece: Piece, sq: Square) -> u64 {
    keys().pieces[piece.color.index()][piece.kind.index()][sq.index()]
}

/// Zobrist key toggled when black is to move.
pub fn black_to_move_key() -> u64 {
    keys().black_to_move
}

/// Zobrist key for a castling-rights bit mask.
pub fn castling_key(mask: usize) -> u64 {
    keys().castling[mask]
}

/// Zobrist key for an en passant file.
pub fn en_passant_file_key(file: u8) -> u64 {
    keys().en_passant_file[file as usize]
}

/// Bit mask for castling rights.
pub fn castling_mask(c: &CastlingRights) -> usize {
    let mut mask = 0usize;
    if c.white_kingside {
        mask |= 1;
    }
    if c.white_queenside {
        mask |= 2;
    }
    if c.black_kingside {
        mask |= 4;
    }
    if c.black_queenside {
        mask |= 8;
    }
    mask
}

impl Position {
    /// Compute Zobrist hash from scratch.
    pub fn zobrist_hash(&self) -> u64 {
        let k = keys();
        let mut h: u64 = 0;

        for (sq, p) in self.board.iter_occupied() {
            h ^= k.pieces[p.color.index()][p.kind.index()][sq.index()];
        }

        if self.side_to_move == Color::Black {
            h ^= k.black_to_move;
        }

        h ^= k.castling[castling_mask(&self.castling)];

        if let Some(ep) = self.en_passant {
            h ^= k.en_passant_file[ep.file() as usize];
        }
        h
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::Position;
    use crate::movement::Move;

    #[test]
    fn zobrist_is_deterministic_and_distinguishing() {
        let start = Position::starting_position();
        assert_eq!(
            start.zobrist_hash(),
            Position::starting_position().zobrist_hash()
        );
        let after = start
            .apply_move(Move::from_uci("e2e4").expect("valid uci"))
            .expect("legal move");
        assert_ne!(start.zobrist_hash(), after.zobrist_hash());
    }

    #[test]
    fn zobrist_side_to_move_matters() {
        let w = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").expect("valid fen");
        let b = Position::from_fen("4k3/8/8/8/8/8/8/4K3 b - - 0 1").expect("valid fen");
        assert_ne!(w.zobrist_hash(), b.zobrist_hash());
    }

    #[test]
    fn zobrist_direct_piece_key_xor() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").expect("valid fen");
        let mut h = 0u64;
        for (sq, p) in pos.board.iter_occupied() {
            h ^= piece_key(p, sq);
        }
        if pos.side_to_move == Color::Black {
            h ^= black_to_move_key();
        }
        h ^= castling_key(castling_mask(&pos.castling));
        assert_eq!(pos.zobrist_hash(), h);
    }

    #[test]
    fn hash_is_set_at_construction() {
        let s = Position::starting_position();
        assert_eq!(s.hash, s.zobrist_hash());
        let f =
            Position::from_fen("r1bqkbnr/pppp1ppp/2n5/4p3/4P3/5N2/PPPP1PPP/RNBQKB1R w KQkq - 2 3")
                .expect("valid fen");
        assert_eq!(f.hash, f.zobrist_hash());
    }

    #[test]
    fn incremental_hash_matches_oracle_over_random_games() {
        let mut seed: u64 = 0xDEADBEEF;
        let mut rng = || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };

        for _game in 0..50 {
            let mut pos = Position::starting_position();
            for _ply in 0..80 {
                let moves = pos.legal_moves();
                if moves.is_empty() {
                    break;
                }
                let slice = moves.as_slice();
                let m = slice[(rng() as usize) % slice.len()];
                pos = pos.apply_move(m).expect("legal move");
                assert_eq!(pos.hash, pos.zobrist_hash(), "hash mismatch after {m:?}");
            }
        }
    }
}
