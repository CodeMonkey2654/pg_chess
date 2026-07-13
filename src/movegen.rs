use crate::board::{Board, Square};
use crate::fen::Position;
use crate::types::{Color, PieceKind, Piece};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveError {
    Illegal,
}

impl std::fmt::Display for MoveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MoveError::Illegal => write!(f, "move is not legal in this position"),
        }
    }
}

impl std::error::Error for MoveError {}

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
                if p.color != color && p.kind != PieceKind::King {
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

        b.clear(m.from);

        if m.flags.contains(MoveFlags::EN_PASSANT) {
            if let Some(capture_sq) = Square::from_file_rank(m.to.file(), m.from.rank()) {
                b.clear(capture_sq);
            }
        }

        let placed = match m.promotion {
            Some(kind) => Piece { color: mover.color, kind },
            None => mover,
        };
        b.set(m.to, Some(placed).expect("piece placement is known"));

        // Castling
        if m.flags.contains(MoveFlags::KING_CASTLE) {
            // King goes e-> ; rook goes a -> d on same rank.
            let rank = m.from.rank();
            if let(Some(rook_from), Some(rook_to)) = (
                Square::from_file_rank(7, rank), // h-file
                Square::from_file_rank(5, rank), // f-file
            ) {
                let rook = b.get(rook_from);
                b.clear(rook_from);
                b.set(rook_to, rook.expect("Rook is already declared and should NEVER be None"));
            }

        } else if m.flags.contains(MoveFlags::QUEEN_CASTLE) {
            let rank = m.from.rank();
            if let (Some(rook_from), Some(rook_to)) = (
                Square::from_file_rank(0, rank), // a-file
                Square::from_file_rank(3, rank), // d-file
            ) {
                let rook = b.get(rook_from);
                b.clear(rook_from);
                b.set(rook_to, rook.expect("Rook is already declared and should NEVER be None"));
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
            Some(p) if p.color == us && p.kind == PieceKind::King => {}
            _ => return,
        }
        if self.board.is_square_attacked(king_sq, them) {
            return; // cannot castle out of check
        }
        let (king_right, queen_right) = match us {
            Color::White => (self.castling.white_kingside, self.castling.white_queenside),
            Color::Black => (self.castling.black_kingside, self.castling.black_queenside),
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
                    Some(k) => !after.is_square_attacked(k, us.flip()),
                    None => true, //kingless test positions: no filter plz
                }
            })
            .collect()
    }

    pub fn apply_move(&self, m: Move) -> Result<Position, MoveError> {
        let legal = self.legal_moves().into_iter().find(|lm| lm.from == m.from && lm.to == m.to && lm.promotion == m.promotion).ok_or(MoveError::Illegal)?;

        let new_board = self.board_after(legal);
        let mover = self.board.get(legal.from).expect("legal move has a mover");
        let is_pawn = mover.kind == PieceKind::Pawn;
        let is_capture = legal.flags.contains(MoveFlags::CAPTURE);

        let halfmove_clock = if is_pawn || is_capture {
            0
        } else {
            self.halfmove_clock + 1
        };

        let fullmove_number = match self.side_to_move {
            Color::White => self.fullmove_number,
            Color::Black => self.fullmove_number + 1,
        };

        let en_passant = if legal.flags.contains(MoveFlags::DOUBLE_PAWN_PUSH) {
            let mid_rank = (legal.from.rank() + legal.to.rank()) / 2;
            Square::from_file_rank(legal.from.file(), mid_rank)
        } else {
            None
        };

        let mut castling = self.castling;
        if mover.kind == PieceKind::King {
            match mover.color {
                Color::White => { castling.white_kingside = false; castling.white_queenside = false; }
                Color::Black => { castling.black_kingside = false; castling.black_queenside = false; }
            }
        }

        let corner_still_has_rook = |sq_file: u8, sq_rank: u8, color: Color| -> bool {
            match Square::from_file_rank(sq_file, sq_rank) {
                Some(sq) => matches!(new_board.get(sq),
                        Some(p) if p.color == color && p.kind == PieceKind::Rook
                    ),
                None => false,
            }
        };
        if !corner_still_has_rook(7, 0, Color::White) {
            castling.white_kingside = false;
        }
        if !corner_still_has_rook(0, 0, Color::White) {
            castling.white_queenside = false; 
        }
        if !corner_still_has_rook(7, 7, Color::Black) {
            castling.black_kingside = false;
        }
        if !corner_still_has_rook(0, 7, Color::Black) {
            castling.black_queenside = false;
        }

        Ok(Position {
            board: new_board,
            side_to_move: self.side_to_move.flip(),
            castling,
            en_passant,
            halfmove_clock,
            fullmove_number
        })
    }

    pub fn is_checkmate(&self) -> bool {
        self.is_in_check(self.side_to_move) && self.legal_moves().is_empty()
    }

    pub fn is_stalemate(&self) -> bool {
        !self.is_in_check(self.side_to_move) && self.legal_moves().is_empty()
    }

    pub fn is_fifty_move_draw(&self) -> bool {
        self.halfmove_clock >= 100
    }

    pub fn is_insufficient_material(&self) -> bool {
        let mut minors = 0u32;
        let mut bishops_light = 0u32;
        let mut bishops_dark = 0u32;
        for i in 0..64u8 {
            let sq = Square(i);
            if let Some(p) = self.board.get(sq) {
                match p.kind {
                    PieceKind::King => {}
                    PieceKind::Knight => minors += 1,
                    PieceKind::Bishop => {
                        minors += 1;
                        if (sq.file() + sq.rank()) % 2 == 0 { bishops_dark += 1; }
                        else { bishops_light += 1; }
                    }
                    _ => return false,
                }
            }
        }
        match minors {
            0 => true,
            1 => true,
            2 => bishops_light == 0 || bishops_dark == 0, //if both bishops are same color
            _ => false,
        }
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

  #[test]
    fn apply_rejects_illegal_move() {
        let pos = Position::starting_position();
        // e2 to e5 is not a legal pawn move.
        let bad = Move::new(
            Square::from_algebraic("e2").unwrap(),
            Square::from_algebraic("e5").unwrap(),
            None,
        ).unwrap();
        assert_eq!(pos.apply_move(bad), Err(MoveError::Illegal));
    }

    #[test]
    fn double_push_sets_en_passant_target() {
        let pos = Position::starting_position();
        let e2e4 = Move::new(
            Square::from_algebraic("e2").unwrap(),
            Square::from_algebraic("e4").unwrap(),
            None,
        ).unwrap();
        let after = pos.apply_move(e2e4).unwrap();
        assert_eq!(after.en_passant, Some(Square::from_algebraic("e3").unwrap()));
        assert_eq!(after.side_to_move, Color::Black);
    }

    #[test]
    fn en_passant_target_clears_next_move() {
        let pos = Position::starting_position();
        let e2e4 = Move::new(Square::from_algebraic("e2").unwrap(),
                             Square::from_algebraic("e4").unwrap(), None).unwrap();
        let after1 = pos.apply_move(e2e4).unwrap();
        // Black replies a7a6 (not a double push) -> EP target must clear.
        let a7a6 = Move::new(Square::from_algebraic("a7").unwrap(),
                             Square::from_algebraic("a6").unwrap(), None).unwrap();
        let after2 = after1.apply_move(a7a6).unwrap();
        assert_eq!(after2.en_passant, None);
    }

    #[test]
    fn king_move_loses_castling_rights() {
        // White king e1 can step to e2; both white rights vanish.
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1").unwrap();
        let ke1e2 = Move::new(Square::from_algebraic("e1").unwrap(),
                              Square::from_algebraic("e2").unwrap(), None).unwrap();
        let after = pos.apply_move(ke1e2).unwrap();
        assert!(!after.castling.white_kingside);
        assert!(!after.castling.white_queenside);
    }

    #[test]
    fn capturing_rook_removes_opponent_right() {
        // White rook a1 captures black rook a8; black loses queenside right.
        let pos = Position::from_fen("r3k3/8/8/8/8/8/8/R3K3 w Qq - 0 1").unwrap();
        let rxa8 = Move::new(Square::from_algebraic("a1").unwrap(),
                             Square::from_algebraic("a8").unwrap(), None).unwrap();
        let after = pos.apply_move(rxa8).unwrap();
        assert!(!after.castling.black_queenside); // the missed-case check
    }

    #[test]
    fn halfmove_clock_resets_on_capture_and_pawn() {
        // Knight shuffle increments; then a pawn move resets to 0.
        let pos = Position::from_fen("4k3/8/8/8/8/5N2/4P3/4K3 w - - 5 10").unwrap();
        let nf3g5 = Move::new(Square::from_algebraic("f3").unwrap(),
                              Square::from_algebraic("g5").unwrap(), None).unwrap();
        let after_knight = pos.apply_move(nf3g5).unwrap();
        assert_eq!(after_knight.halfmove_clock, 6); // incremented

        let e2e4 = Move::new(Square::from_algebraic("e2").unwrap(),
                             Square::from_algebraic("e4").unwrap(), None).unwrap();
        let after_pawn = pos.apply_move(e2e4).unwrap();
        assert_eq!(after_pawn.halfmove_clock, 0); // reset
    }

    #[test]
    fn scholars_mate_is_checkmate() {
        // 1. e4 e5 2. Bc4 Nc6 3. Qh5 Nf6?? 4. Qxf7#
        let moves = ["e2e4","e7e5","f1c4","b8c6","d1h5","g8f6","h5f7"];
        let mut pos = Position::starting_position();
        for uci in moves {
            let m = Move::from_uci(uci).unwrap();
            pos = pos.apply_move(m).unwrap();
        }
        assert!(pos.is_checkmate());
    }

    #[test]
    fn zobrist_is_deterministic_and_distinguishing() {
        let start = Position::starting_position();
        // Same position hashes identically.
        assert_eq!(start.zobrist_hash(), Position::starting_position().zobrist_hash());
        // A different position hashes differently (overwhelmingly likely).
        let after = start.apply_move(Move::from_uci("e2e4").unwrap()).unwrap();
        assert_ne!(start.zobrist_hash(), after.zobrist_hash());
    }

    #[test]
    fn zobrist_side_to_move_matters() {
        // Identical board layout but different side-to-move must differ.
        let w = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        let b = Position::from_fen("4k3/8/8/8/8/8/8/4K3 b - - 0 1").unwrap();
        assert_ne!(w.zobrist_hash(), b.zobrist_hash());
    }

    #[test]
    fn insufficient_material_detects_bare_kings() {
        let pos = Position::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
        assert!(pos.is_insufficient_material());
        // King + rook is sufficient.
        let rook = Position::from_fen("4k3/8/8/8/8/8/8/R3K3 w - - 0 1").unwrap();
        assert!(!rook.is_insufficient_material());
    }
}