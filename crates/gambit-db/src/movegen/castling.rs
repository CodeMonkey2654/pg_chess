use crate::fen::Position;
use crate::movegen::pseudo::mk;
use crate::movement::MoveFlags;
use crate::square::Square;
use crate::types::PieceKind;

/// Generate castling pseudo-moves when legal.
pub(crate) fn gen_castling(pos: &Position, out: &mut Vec<crate::movement::Move>) {
    let us = pos.side_to_move;
    let them = us.flip();
    let rank = us.back_rank();

    let king_sq = match Square::from_file_rank(4, rank) {
        Some(s) => s,
        None => return,
    };

    match pos.board.get(king_sq) {
        Some(p) if p.color == us && p.kind == PieceKind::King => {}
        _ => return,
    }
    if pos.board.is_square_attacked(king_sq, them) {
        return;
    }
    let (king_right, queen_right) = pos.castling.rights_for(us);

    if king_right {
        let f = Square::from_file_rank(5, rank);
        let g = Square::from_file_rank(6, rank);
        if let (Some(f), Some(g)) = (f, g) {
            let empty = pos.board.get(f).is_none() && pos.board.get(g).is_none();
            let safe =
                !pos.board.is_square_attacked(f, them) && !pos.board.is_square_attacked(g, them);
            if empty && safe {
                out.push(mk(king_sq, g, None, MoveFlags::KING_CASTLE));
            }
        }
    }

    if queen_right {
        let b = Square::from_file_rank(1, rank);
        let c = Square::from_file_rank(2, rank);
        let d = Square::from_file_rank(3, rank);
        if let (Some(b), Some(c), Some(d)) = (b, c, d) {
            let empty = pos.board.get(b).is_none()
                && pos.board.get(c).is_none()
                && pos.board.get(d).is_none();
            let safe =
                !pos.board.is_square_attacked(d, them) && !pos.board.is_square_attacked(c, them);
            if empty && safe {
                out.push(mk(king_sq, c, None, MoveFlags::QUEEN_CASTLE));
            }
        }
    }
}
