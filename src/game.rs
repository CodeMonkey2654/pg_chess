use crate::fen::Position;
use crate::movement::{Move};
use crate::movegen::{MoveError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameStatus {
    Ongoing,
    Checkmate,
    Stalemate,
    FiftyMoveDraw,
    ThreefoldRepetition,
    InsufficientMaterial,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChessGame {
    pub start: Position,
    pub moves: Vec<Move>,
}

impl ChessGame {
    pub fn new() -> Self {
        Self { start: Position::starting_position(), moves: Vec::new() }
    }

    pub fn from_position(start: Position) -> Self {
        Self { start, moves: Vec::new() }
    }

    pub fn current_position(&self) -> Position {
        let mut pos = self.start.clone();
        for &m in &self.moves {
            pos = pos
                .apply_move(m)
                .expect("stored game history contains an illegal move (corrupt game)")
        }
        pos
    }

    pub fn position_hashes(&self) -> Vec<u64> {
        let mut hashes = Vec::with_capacity(self.moves.len() + 1);
        let mut position = self.start.clone();
        hashes.push(position.hash);

        for &m in &self.moves {
            position = position.apply_move(m).expect("corrupt game history");
            hashes.push(position.hash);
        }
        hashes
    }

    pub fn play(&mut self, m: Move) -> Result<(), MoveError> {
        let current = self.current_position();
        current.apply_move(m)?;
        self.moves.push(m);
        Ok(())
    }

    pub fn is_threefold_repetition(&self) -> bool {

        let mut counts: HashMap<u64, u8> = HashMap::new();
        let mut position = self.start.clone();

        let tally = |h: u64, counts: &mut HashMap<u64, u8>| -> bool {
            let c = counts.entry(h).or_insert(0);
            *c += 1;
            *c >= 3
        };

        if tally(position.hash, &mut counts) {
            return true;
        }
        for &m in &self.moves {
            position = position.apply_move(m).expect("corrupt game history");
            if tally(position.hash, &mut counts) {
                return true; // early out the instance it hits 3
            }
        }
        false
    }

    pub fn status(&self) -> GameStatus {
        let position = self.current_position();
        if position.is_checkmate() {
            return GameStatus::Checkmate;
        }
        if position.is_stalemate() {
            return GameStatus::Stalemate;
        }
        if position.is_insufficient_material() {
            return GameStatus::InsufficientMaterial;
        }
        if position.is_fifty_move_draw() {
            return GameStatus::FiftyMoveDraw;
        }
        if self.is_threefold_repetition() {
            return GameStatus::ThreefoldRepetition;
        }
        GameStatus::Ongoing
    }
}

impl Default for ChessGame {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::movement::Move;

    #[test]
    fn threefold_by_knight_shuffle() {
        // Both sides shuffle knights out and back twice, repeating the start
        // position three times total.
        let mut g = ChessGame::new();
        let seq = ["g1f3","g8f6","f3g1","f6g8", "g1f3","g8f6","f3g1","f6g8"];
        for uci in seq {
            g.play(Move::from_uci(uci).unwrap()).unwrap();
        }
        assert!(g.is_threefold_repetition());
        assert_eq!(g.status(), GameStatus::ThreefoldRepetition);
    }

    #[test]
    fn new_game_is_ongoing() {
        assert_eq!(ChessGame::new().status(), GameStatus::Ongoing);
    }

    #[test]
    fn two_occurrences_is_not_threefold() {
        // Knights out and back ONCE: the start position occurs twice, not three
        // times. Must NOT be flagged.
        let mut g = ChessGame::new();
        for uci in ["g1f3","g8f6","f3g1","f6g8"] {
            g.play(Move::from_uci(uci).unwrap()).unwrap();
        }
        assert!(!g.is_threefold_repetition());
        assert_eq!(g.status(), GameStatus::Ongoing);
    }

    #[test]
    fn threefold_triggers_on_third_occurrence() {
        // Shuffle back to start a THIRD time. Now flagged.
        let mut g = ChessGame::new();
        let seq = ["g1f3","g8f6","f3g1","f6g8",  // start seen 2x
                   "g1f3","g8f6","f3g1","f6g8"];  // start seen 3x
        for uci in seq {
            g.play(Move::from_uci(uci).unwrap()).unwrap();
        }
        assert!(g.is_threefold_repetition());
    }

    #[test]
    fn distinct_positions_never_threefold() {
        // A normal opening with no repetition must return false.
        let mut g = ChessGame::new();
        for uci in ["e2e4","e7e5","g1f3","b8c6","f1b5","a7a6"] {
            g.play(Move::from_uci(uci).unwrap()).unwrap();
        }
        assert!(!g.is_threefold_repetition());
    }
}