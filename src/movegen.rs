use crate::board::{Board, Square};
use crate::fen::Position;
use crate::types::{Color, PieceKind};
use crate::movement::{Move, MoveFlags};

/// Move offsets using signed deltas and bounds-checked Square. Prevents wrap from a-h file
const KNIGHT_DELTAS: [(i8, i8); 8] = [
    (1, 2), (2, 1), (2, -1), (-1, 2), (1, -2), (-2, -1), (-1, -2), (-2, 1)
];

const KING_DELTAS: [(i8, i8); 8] = [
    (0, 1), (0, -1), (1, 0), (-1, 0), (1, 1), (-1, 1), (-1, -1), (1, -1)
];

// Direction based movements
const BISHOP_DIRECTIONS: [(i8, i8); 4] = [
    (1, 1), (-1, 1), (-1, -1), (1, -1)
];

const ROOK_DIRECTIONS: [(i8, i8); 4] = [
    (0, 1), (1, 0), (-1, 0), (0, -1)
];

#[inline]
fn offset(sq: Square, file_offset: i8, rank_offset: i8) -> Option<Square> {
    let file = sq.file() as i8 + file_offset;
    let rank = sq.rank() as i8 + rank_offset;
    if (0..8).contains(&file) && (0..8).contains(&rank) {
        Square::from_file_rank(file as u8, rank as u8)
    } else {
        None
    }
}

impl Position{
    /// All psuedo-legal moves for the side to move. No castling
    pub fn psuedo_legal_moves(&self) -> Vec<Move> {
        let mut moves = Vec::with_capacity(48);
        let us = self.side_to_move;

        for i in 0..64u8 {
            let sq: Square = Square(i);
            let piece = match self.board.get(sq) {
                Some(p) if p.color == us => p,
                _ => continue, // empty or enemy piece
            };
            match piece.kind {
                PieceKind::Pawn => self.push_pawn_moves(sq, us, &mut moves),
                PieceKind::Knight => self.push_leaper_moves(sq, us, &KNIGHT_DELTAS, &mut moves),
                PieceKind::Bishop => self.push_slider_moves(sq, us, &BISHOP_DIRECTIONS, &mut moves),
                PieceKind::Rook => self.push_slider_moves(sq, us, &ROOK_DIRECTIONS, &mut moves),
            PieceKind::Queen => { // m(Q) = {m(R)} | {m(B)}
                    self.push_slider_moves(sq, us, &BISHOP_DIRECTIONS, &mut moves);
                    self.push_slider_moves(sq, us, &ROOK_DIRECTIONS, &mut moves); // Queen acts as union of rook and bishop
                },
                PieceKind::King => self.push_leaper_moves(sq, us, &KING_DELTAS, &mut moves),
            }
        }
        moves
    }

    pub fn push_pawn_moves(&self, from: Square, color: Color, out: &mut Vec<Move>) {
        // Rules:
        // - white pawns move up, black down
        // - pawns have a rank where double push is an option
        // - promotion is available for 1 rank per color
        // - pieces ahead have to be empty
        // - diagonal capture is available
        // - shoulder to shoulder is avilable
        let forward: i8 = match color {
            Color::White => 1,
            Color::Black => -1,
        };

        let starting_rank: u8 = match color {
            Color::White => 1, // rank 2
            Color::Black => 6, // rank 7
        };

        let promotion_rank: i8 = match color {
            Color::White => 6, // about to move to 8
            Color::Black => 1, // about to move to 1
        };

        let is_promotion = from.rank() as i8 == promotion_rank;

        // single push if empty
        if let Some(one) = offset(from, 0, forward) {
            if self.board.get(one).is_none() {
                if is_promotion {
                    push_promotions(from, one, MoveFlags::PROMOTION, out);
                } else {
                    out.push(mk(from, one, None, MoveFlags::NONE));
                }

                // Double push if from start and both empty
                if from.rank() == starting_rank {
                    if let Some(two) = offset(from, 0, forward * 2) {
                        if self.board.get(two).is_none() {
                            out.push(mk(from, two, None, MoveFlags::DOUBLE_PAWN_PUSH));
                        }
                    }
                }
            }
        }

        // Diagonal captures (and promotion captures)
        for &file_direction in &[-1i8, 1] {
            let Some(to) = offset(from, file_direction, forward) else { continue };

            // Normal caps
            if let Some(p) = self.board.get(to) {
                if p.color != color {
                    if is_promotion {
                        let mut flags = MoveFlags::NONE;
                        flags.insert(MoveFlags::PROMOTION);
                        flags.insert(MoveFlags::CAPTURE);
                        push_promotions(from, to, flags, out);
                    } else {
                        out.push(mk(from, to, None, MoveFlags::CAPTURE))
                    }
                }
                continue; // occupied square is handled and can't also be en passant
            }
            // en passant
            if Some(to) == self.en_passant {
                out.push(mk(from, to, None, MoveFlags::EN_PASSANT));
            }
        }

    }

    pub fn push_leaper_moves(&self, from: Square, color: Color, deltas: &[(i8,i8)], out: &mut Vec<Move>) {
        for &(file_offset, rank_offset) in deltas {
            let Some(to) = offset(from, file_offset, rank_offset) else { continue };
            match self.board.get(to) {
                Some(p) if p.color == color => {} // blocked by friendly piece,
                Some(_) => out.push(mk(from, to, None, MoveFlags::CAPTURE)), // enemy: capture
                None => out.push(mk(from, to, None, MoveFlags::NONE)), // quiet move
            }
        }
    }

    pub fn push_slider_moves(&self, from: Square, color: Color, deltas: &[(i8,i8)], out: &mut Vec<Move>) {
        for &(file_direction, rank_direction) in deltas {
            let mut current = from;
            loop {
                let Some(to) = offset(current, file_direction, rank_direction) else { break };
                match self.board.get(to) {
                    Some(p) if p.color == color => break, // friendly block
                    Some(_) => {
                        out.push(mk(from, to, None, MoveFlags::CAPTURE));
                        break;
                    }
                    None => {
                        out.push(mk(from, to, None, MoveFlags::NONE));
                        current = to;
                    }
                }
            }
        }
    }
}


impl Board {
    pub fn is_square_attacked(&self, target: Square, by: Color) -> bool {
        // Pawn attacks. attacks two diagonal foward
        let pawn_back: i8 = match by {
            Color::White => -1,
            Color::Black => 1,
        };
        for file_direction in [-1i8, 1] {
            if let Some(sq) = offset(target, file_direction, pawn_back) {
                if let Some(p) = self.get(sq) {
                    if p.color == by && p.kind == PieceKind::Pawn {
                        return true;
                    }
                }
            }
        }

        // Knights
        for &(file_direction, rank_direction) in &KNIGHT_DELTAS {
            if let Some(sq) = offset(target, file_direction, rank_direction) {
                if let Some(p) = self.get(sq) {
                    if p.color == by && p.kind == PieceKind::Knight {
                        return true
                    }
                }
            }
        }

        // King
        for &(file_direction, rank_direction) in &KING_DELTAS {
            if let Some(sq) = offset(target, file_direction, rank_direction) {
                if let Some(p) = self.get(sq) {
                    if p.color == by && p.kind == PieceKind::King {
                        return true
                    }
                }
            }
        }

        // non-diagonal sliding
        if self.slider_hits(target, by, &ROOK_DIRECTIONS, PieceKind::Rook) {
            return true;
        }

        // Diagonal sliding
        if self.slider_hits(target, by, &BISHOP_DIRECTIONS, PieceKind::Bishop) {
            return true;
        }

        false
    }

    fn slider_hits(&self, target: Square, by: Color, dirs: &[(i8,i8)], line_kind: PieceKind) -> bool {
        for &(file_direction, rank_direction) in dirs {
            let mut current = target;
            loop {
                let Some(sq) = offset(current, file_direction, rank_direction) else { break };
                match self.get(sq) {
                    None => { current = sq; continue; }
                    Some(p) => {
                        // first blocker, attacks if enemy matching line piece. understanding is if queens stare at each other
                        if p.color == by && (p.kind == line_kind || p.kind == PieceKind::Queen) {
                            return true;
                        }
                        break; // any piece blocks hit scanning
                    }
                }
            }
        }
        false
    }
}

impl Position {
    pub fn is_in_check(&self, color: Color) -> bool {
        match self.board.king_square(color) {
            Some(k) => self.board.is_square_attacked(k, color.flip()),
            None => false, // no king on board, could be for tests
        }
    }

    /// Board after playing m, doesn't update anything other than board side
    fn board_after(&self, m: Move) -> Board {
        let mut b = self.board.clone();
        let mover = b.get(m.from).expect("move.from MUST hold a piece");

        b.set(m.from, None);

        if m.flags.contains(MoveFlags::EN_PASSANT) {
            if let Some(capture_sq) = Square::from_file_rank(m.to.file(), m.from.rank()) {
                b.set(capture_sq, None);
            }
        }

        let placed = match m.promotion {
            Some(kind) => Piece { color: mover.color, kind },
            None => mover,
        };
        b.set(m.to, Some(placed));

        // Castling
        if m.flags.contains(MoveFlags::KING_CASTLE) {
            // King goes e-> ; rook goes a -> d on same rank.
            let rank = m.from.rank();
            if let(Some(rook_from), Some(rook_to)) = (
                Square::from_file_rank(7, rank), // h-file
                Square::from_file_rank(5, rank), // f-file
            ) {
                let rook = b.get(rook_from);
                b.set(rook_from, None);
                b.set(rook_to, rook);
            }

        } else if m.flags.contains(MoveFlags::QUEEN_CASTLE) {
            let rank = m.from.rank();
            if let (Some(rook_from), Some(rook_to)) = (
                Square::from_file_rank(0, rank), // a-file
                Square::from_file_rank(3, rank), // d-file
            ) {
                let rook = b.get(rook_from);
                b.set(rook_from, None);
                b.set(rook_to, rook.un);
            }
        }

        b
    }

    fn gen_castling(&self, out: &mut Vec<Move>) {
        let us = self.side_to_move;
        let them = us.flip();
        let rank = match us {Color::White => 0, Color::Black => 7};

        let king_sq = match Square::from_file_rank(4, rank) { Some(s) => s, None => return };

        match self.board.get(king_sq) {
            Some(p) => if p.color == us && p.kind == PieceKind::King => {}
            _ => return,
        }
        if self.board.is_square_attacked(king_sq, them) {
            return; // cannot castle out of check
        }
        let (king_right, queen_right) = match us {
            Color::White => (self.castling.white_kingside, self.castling.white_queenside),
            Color::Black => (self.castling.black_kingside, self.castling.white_queenside),
        };
        // Kingside
        if king_right {
            let f = Square::from_file_rank(5, rank);
            let g = Square::from_file_rank(6, rank);
            if let (Some(f), Some(g)) = (f, g) {
                let empty = self.board.get(f).is_none() && self.board.get(g).is_none();
                let safe = !self.board.is_square_attacked(f, them) && !self.board.is_square_attacked(g, them);
                if empty && safe {
                    out.push(mk(king_sq, g, None, MoveFlags::KING_CASTLE));
                }
            }
        }

        // Queenside
        if queen_right {
            let b = Square::from_file_rank(1, rank);
            let c = Square::from_file_rank(2, rank);
            let d = Square::from_file_rank(3, rank);
            if let (Some(b), Some(c), Some(d)) = (b, c, d) {
                let empty = self.board.get(b).is_none() && self.board.get(c).is_none() && self.board.get(d).is_none();
                let safe = !self.board.is_square_attacked(d, them) && !self.board.is_square_attacked(c, them);
                if empty && safe {
                    out.push(mk(king_sq, c, None, MoveFlags::QUEEN_CASTLE));
                }
            }
        }
    }

    pub fn legal_moves(&self) -> Vec<Move> {
        let us = self.side_to_move;
        let mut candidates = self.psuedo_legal_moves();
        self.gen_castling(&mut candidates);

        candidates
            .into_iter()
            .filter(|&m| {
                let after = self.board_after(m);
                match after.king_square(us) {
                    Some(k) = after.is_square_attacked(k, us.flip()),
                    None => true, //kingless test positions: no filter plz
                }
            })
            .collect();
    }

    pub fn is_checkmate(&self) -> bool {
        self.is_in_check(self.side_to_move) && self.legal_moves().is_empty()
    }

    pub fn is_stalemate(&self) -> bool {
        !self.is_in_check(self.side_to_move) && self.legal_moves().is_empty()
    }
}

// Makes live here to force geometric construction, panics with generator bugs
#[inline]
fn mk(from: Square, to: Square, promotion: Option<PieceKind>, flags: MoveFlags) -> Move {
    Move::with_flags(from, to, promotion, flags)
        .expect("move generator produced a geometrically invalid move")
}

// Emits all 4 promotion moves for a from -> to pawn move, carrying extra flags (like capture)
fn push_promotions(from: Square, to: Square, extra: MoveFlags, out: &mut Vec<Move>) {
    for kind in [PieceKind::Queen, PieceKind::Rook, PieceKind::Bishop, PieceKind::Knight] {
        out.push(mk(from, to, Some(kind), extra));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fen::Position;

    // helper for my sanity
    fn perft_count(fen: &str) -> usize {
        Position::from_fen(fen).unwrap().psuedo_legal_moves().len()
    }

    #[test]
    fn starting_position_has_20_psuedo_legal_moves() {
        assert_eq!(perft_count(&Position::starting_position().to_fen()), 20);
    }

    #[test]
    fn lone_center_knight_has_8() {
        // White knight on d4, otherwise only kings (needed for "legal")
        assert_eq!(perft_count("4k3/8/8/8/3N4/8/8/4K3 w - - 0 1"), 8 + 5) //8 knight, 5 king on e1
    }

    #[test]
    fn corner_knight_has_2() {
        let moves = Position::from_fen("4k3/8/8/4K3/8/8/8/N7 w - - 0 1")
            .unwrap()
            .psuedo_legal_moves();

        let knight_moves = moves
            .iter()
            .filter(|m| m.from == Square::from_algebraic("a1").unwrap())
            .count();

        assert_eq!(knight_moves, 2);
    }

    #[test]
    fn rook_slides_and_stops_at_capture() {
        let position = Position::from_fen("4k3/8/8/p7/8/8/8/R3K3 w - - 0 1").unwrap();
        let moves: Vec<_> = position.psuedo_legal_moves().into_iter()
            .filter(|m| m.from == Square::from_algebraic("a1").unwrap())
            .collect();

        assert_eq!(moves.len(), 7);
        assert!(
            moves
            .iter()
            .any(|m| m.to == Square::from_algebraic("a5").unwrap() && m.flags.contains(MoveFlags::CAPTURE))
        );
    }

    #[test]
    fn pawn_double_push_flagged() {
        let position = Position::from_fen("4k3/8/8/8/8/8/4P3/4K3 w - - 0 1").unwrap();
        let e2 = Square::from_algebraic("e2").unwrap();
        let e4 = Square::from_algebraic("e4").unwrap();
        let double = position.psuedo_legal_moves().into_iter()
            .find(|m| m.from == e2 && m.to == e4).unwrap();
        assert!(double.flags.contains(MoveFlags::DOUBLE_PAWN_PUSH));
    }

    #[test]
    fn pawn_promotion_expands_to_4() {
        let position = Position::from_fen("3k4/4P3/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let e7 = Square::from_algebraic("e7").unwrap();
        let promotions: Vec<_> = position.psuedo_legal_moves()
            .into_iter()
            .filter(|m| m.from == e7 && m.promotion.is_some())
            .collect();

        assert_eq!(promotions.len(), 4);
        assert!(promotions.iter().all(|m| m.flags.contains(MoveFlags::PROMOTION)));
    }

    #[test]
    fn en_passant_guarunteed_with_flag() {
        // black played d7-d5, white pawn on e5 can take en passant on d6
        let position = Position::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap();
        let e5 = Square::from_algebraic("e5").unwrap();
        let d6 = Square::from_algebraic("d6").unwrap();

        let en_passant = position.psuedo_legal_moves().into_iter()
            .find(|m| m.from == e5 && m.to == d6).unwrap();
        assert!(en_passant.flags.contains(MoveFlags::EN_PASSANT));
        assert!(en_passant.flags.contains(MoveFlags::CAPTURE)); //constructure should imply this
    }

     #[test]
    fn detects_check() {
        // Black rook on e8 checks white king on e1 down the open e-file.
        let pos = Position::from_fen("4r3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert!(pos.is_in_check(Color::White));
    }

    #[test]
    fn no_false_check_when_blocked() {
        // Same rook, but a white pawn on e2 blocks the check.
        let pos = Position::from_fen("4r3/8/8/8/8/8/4P3/4K3 w - - 0 1").unwrap();
        assert!(!pos.is_in_check(Color::White));
    }

    #[test]
    fn pawn_attack_direction_is_correct() {
        // Black pawn on d2 attacks c1 and e1 (diagonally forward for black).
        let pos = Position::from_fen("4k3/8/8/8/8/8/3p4/8 w - - 0 1").unwrap();
        let c1 = Square::from_algebraic("c1").unwrap();
        let e1 = Square::from_algebraic("e1").unwrap();
        let d1 = Square::from_algebraic("d1").unwrap();
        assert!(pos.board.is_square_attacked(c1, Color::Black));
        assert!(pos.board.is_square_attacked(e1, Color::Black));
        assert!(!pos.board.is_square_attacked(d1, Color::Black)); // pawns don't attack straight
    }

    // Legality: pins and escaping check

    #[test]
    fn pinned_piece_cannot_move() {
        // White knight on e2 is pinned to the white king on e1 by black rook e8.
        // The knight has zero legal moves (any move exposes the king).
        let pos = Position::from_fen("4r3/8/8/8/8/8/4N3/4K3 w - - 0 1").unwrap();
        let e2 = Square::from_algebraic("e2").unwrap();
        let knight_legal = pos.legal_moves().into_iter()
            .filter(|m| m.from == e2).count();
        assert_eq!(knight_legal, 0);
    }

    #[test]
    fn must_address_check() {
        // White king e1 in check from rook e8; only legal moves get out of check.
        // King can go to d1/d2/f1/f2 (not e2, still on file; not squares off board).
        let pos = Position::from_fen("4r3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let legal = pos.legal_moves();
        assert!(!legal.is_empty());
        // Every legal move must leave the king off the e-file or block; verify
        // none of them keep the king in check by re-testing.
        for m in &legal {
            let after = pos.board_after(*m);
            let k = after.king_square(Color::White).unwrap();
            assert!(!after.is_square_attacked(k, Color::Black));
        }
    }

    // Castling

    #[test]
    fn kingside_castle_generated_when_legal() {
        // White: king e1, rook h1, f1/g1 empty, nothing attacking. KQkq rights.
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K2R w K - 0 1").unwrap();
        let e1 = Square::from_algebraic("e1").unwrap();
        let g1 = Square::from_algebraic("g1").unwrap();
        let castle = pos.legal_moves().into_iter()
            .find(|m| m.from == e1 && m.to == g1);
        assert!(castle.is_some());
        assert!(castle.unwrap().flags.contains(MoveFlags::KING_CASTLE));
    }

    #[test]
    fn cannot_castle_through_attacked_square() {
        // Black rook on f8 attacks f1: white king would cross f1 while castling
        // kingside, which is illegal even though f1/g1 are empty.
        let pos = Position::from_fen("5r2/8/8/8/8/8/8/4K2R w K - 0 1").unwrap();
        let e1 = Square::from_algebraic("e1").unwrap();
        let g1 = Square::from_algebraic("g1").unwrap();
        let castle = pos.legal_moves().into_iter()
            .find(|m| m.from == e1 && m.to == g1);
        assert!(castle.is_none()); // must NOT be offered
    }

    // Terminal states

    #[test]
    fn detects_back_rank_mate() {
        // Classic back-rank mate: black king g8 boxed by own pawns f7,g7,h7,
        // white rook delivers mate on e8. Black to move, checkmated.
        let pos = Position::from_fen("4R1k1/5ppp/8/8/8/8/8/6K1 b - - 0 1").unwrap();
        assert!(pos.is_in_check(Color::Black));
        assert!(pos.is_checkmate());
        assert!(!pos.is_stalemate());
    }

    #[test]
    fn detects_stalemate() {
        // Classic stalemate: black king a8, white queen c7 (not giving check),
        // white king c6. Black is not in check but has no legal move.
        let pos = Position::from_fen("k7/2Q5/2K5/8/8/8/8/8 b - - 0 1").unwrap();
        assert!(!pos.is_in_check(Color::Black));
        assert!(pos.is_stalemate());
        assert!(!pos.is_checkmate());
    }
}