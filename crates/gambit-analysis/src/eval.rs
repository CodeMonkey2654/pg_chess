//! Static evaluation (material + piece-square tables).

use gambit_db::{Color, PieceKind, Position, Square};

const PAWN: i32 = 100;
const KNIGHT: i32 = 320;
const BISHOP: i32 = 330;
const ROOK: i32 = 500;
const QUEEN: i32 = 900;

fn piece_value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Pawn => PAWN,
        PieceKind::Knight => KNIGHT,
        PieceKind::Bishop => BISHOP,
        PieceKind::Rook => ROOK,
        PieceKind::Queen => QUEEN,
        PieceKind::King => 0,
    }
}

/// Piece-square table for white (mirrored for black).
const PST_PAWN: [i32; 64] = build_pst(&[
    [0, 0, 0, 0, 0, 0, 0, 0],
    [50, 50, 50, 50, 50, 50, 50, 50],
    [10, 10, 20, 30, 30, 20, 10, 10],
    [5, 5, 10, 25, 25, 10, 5, 5],
    [0, 0, 0, 20, 20, 0, 0, 0],
    [5, -5, -10, 0, 0, -10, -5, 5],
    [5, 10, 10, -20, -20, 10, 10, 5],
    [0, 0, 0, 0, 0, 0, 0, 0],
]);

const PST_KNIGHT: [i32; 64] = build_pst(&[
    [-50, -40, -30, -30, -30, -30, -40, -50],
    [-40, -20, 0, 0, 0, 0, -20, -40],
    [-30, 0, 10, 15, 15, 10, 0, -30],
    [-30, 5, 15, 20, 20, 15, 5, -30],
    [-30, 0, 15, 20, 20, 15, 0, -30],
    [-30, 5, 10, 15, 15, 10, 5, -30],
    [-40, -20, 0, 5, 5, 0, -20, -40],
    [-50, -40, -30, -30, -30, -30, -40, -50],
]);

const PST_BISHOP: [i32; 64] = build_pst(&[
    [-20, -10, -10, -10, -10, -10, -10, -20],
    [-10, 0, 0, 0, 0, 0, 0, -10],
    [-10, 0, 5, 10, 10, 5, 0, -10],
    [-10, 5, 5, 10, 10, 5, 5, -10],
    [-10, 0, 10, 10, 10, 10, 0, -10],
    [-10, 10, 10, 10, 10, 10, 10, -10],
    [-10, 5, 0, 0, 0, 0, 5, -10],
    [-20, -10, -10, -10, -10, -10, -10, -20],
]);

const PST_ROOK: [i32; 64] = build_pst(&[
    [0, 0, 0, 0, 0, 0, 0, 0],
    [5, 10, 10, 10, 10, 10, 10, 5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [-5, 0, 0, 0, 0, 0, 0, -5],
    [0, 0, 0, 5, 5, 0, 0, 0],
]);

const PST_QUEEN: [i32; 64] = build_pst(&[
    [-20, -10, -10, -5, -5, -10, -10, -20],
    [-10, 0, 0, 0, 0, 0, 0, -10],
    [-10, 0, 5, 5, 5, 5, 0, -10],
    [-5, 0, 5, 5, 5, 5, 0, -5],
    [0, 0, 5, 5, 5, 5, 0, -5],
    [-10, 5, 5, 5, 5, 5, 0, -10],
    [-10, 0, 5, 0, 0, 0, 0, -10],
    [-20, -10, -10, -5, -5, -10, -10, -20],
]);

const fn build_pst(rank_file: &[[i32; 8]; 8]) -> [i32; 64] {
    let mut out = [0i32; 64];
    let mut rank = 0;
    while rank < 8 {
        let mut file = 0;
        while file < 8 {
            out[rank * 8 + file] = rank_file[7 - rank][file];
            file += 1;
        }
        rank += 1;
    }
    out
}

fn pst(kind: PieceKind, sq: Square, color: Color) -> i32 {
    let idx = if color == Color::White {
        sq.index()
    } else {
        63 - sq.index()
    };
    match kind {
        PieceKind::Pawn => PST_PAWN[idx],
        PieceKind::Knight => PST_KNIGHT[idx],
        PieceKind::Bishop => PST_BISHOP[idx],
        PieceKind::Rook => PST_ROOK[idx],
        PieceKind::Queen => PST_QUEEN[idx],
        PieceKind::King => 0,
    }
}

/// Evaluate from white's perspective (positive = white better).
pub fn evaluate(pos: &Position) -> i32 {
    let mut score = 0i32;
    for (sq, piece) in pos.board.iter_occupied() {
        let val = piece_value(piece.kind) + pst(piece.kind, sq, piece.color);
        if piece.color == Color::White {
            score += val;
        } else {
            score -= val;
        }
    }
    if pos.side_to_move == Color::White {
        score
    } else {
        -score
    }
}
