use crate::fen::Position;
use crate::movegen::MoveError;
use crate::movement::Move;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;

/// Terminal or ongoing game status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum GameStatus {
    /// Game in progress.
    Ongoing,
    /// Checkmate.
    Checkmate,
    /// Stalemate.
    Stalemate,
    /// Fifty-move rule draw.
    FiftyMoveDraw,
    /// Threefold repetition draw.
    ThreefoldRepetition,
    /// Insufficient mating material.
    InsufficientMaterial,
}

/// A chess game as a starting position plus move history.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChessGame {
    /// Starting position.
    pub start: Position,
    /// Resolved legal moves with flags, in order played.
    pub moves: Vec<Move>,
    current: Position,
    hash_history: Vec<u64>,
}

#[derive(Serialize, Deserialize)]
struct ChessGameFields {
    start: Position,
    moves: Vec<Move>,
}

impl Serialize for ChessGame {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        ChessGameFields {
            start: self.start.clone(),
            moves: self.moves.clone(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ChessGame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let fields = ChessGameFields::deserialize(deserializer)?;
        Ok(ChessGame::from_fields(fields.start, fields.moves))
    }
}

struct ReplayPositions<'a> {
    moves: &'a [Move],
    position: Position,
    index: usize,
    done: bool,
}

impl<'a> Iterator for ReplayPositions<'a> {
    type Item = Position;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        if self.index == 0 {
            self.index = 1;
            return Some(self.position.clone());
        }
        if self.index > self.moves.len() {
            self.done = true;
            return None;
        }
        let m = self.moves[self.index - 1];
        self.position.make_move(m).ok()?;
        self.index += 1;
        Some(self.position.clone())
    }
}

impl ChessGame {
    /// New game from the standard starting position.
    pub fn new() -> Self {
        let start = Position::starting_position();
        Self::from_position(start)
    }

    /// New game from a custom starting position.
    pub fn from_position(start: Position) -> Self {
        let hash_history = vec![start.hash];
        Self {
            current: start.clone(),
            start,
            moves: Vec::new(),
            hash_history,
        }
    }

    fn from_fields(start: Position, moves: Vec<Move>) -> Self {
        let mut game = Self::from_position(start);
        for m in moves {
            if game.play(m).is_err() {
                break;
            }
        }
        game
    }

    fn replay_positions(&self) -> ReplayPositions<'_> {
        ReplayPositions {
            moves: &self.moves,
            position: self.start.clone(),
            index: 0,
            done: false,
        }
    }

    /// Current position after all played moves.
    pub fn current_position(&self) -> Position {
        self.current.clone()
    }

    /// Reference to the current position (no clone).
    pub fn current(&self) -> &Position {
        &self.current
    }

    /// Every position in the game (start through current), as `(ply, position)` pairs.
    pub fn iter_positions(&self) -> impl Iterator<Item = (usize, Position)> + '_ {
        self.replay_positions().enumerate()
    }

    /// Zobrist hashes of every position in the game (including start).
    pub fn position_hashes(&self) -> Vec<u64> {
        self.hash_history.clone()
    }

    /// Play a legal move.
    pub fn play(&mut self, m: Move) -> Result<(), MoveError> {
        let legal = self.current.resolve_move(m)?;
        self.current.make_move(legal)?;
        self.moves.push(legal);
        self.hash_history.push(self.current.hash);
        Ok(())
    }

    /// Whether the same position has occurred three times.
    pub fn is_threefold_repetition(&self) -> bool {
        let mut counts: HashMap<u64, u8> = HashMap::new();

        for &hash in &self.hash_history {
            let c = counts.entry(hash).or_insert(0);
            *c += 1;
            if *c >= 3 {
                return true;
            }
        }
        false
    }

    /// Current game status including draw rules.
    pub fn status(&self) -> GameStatus {
        let position = &self.current;
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
        let mut g = ChessGame::new();
        let seq = [
            "g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8",
        ];
        for uci in seq {
            g.play(Move::from_uci(uci).expect("valid uci"))
                .expect("legal move");
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
        let mut g = ChessGame::new();
        for uci in ["g1f3", "g8f6", "f3g1", "f6g8"] {
            g.play(Move::from_uci(uci).expect("valid uci"))
                .expect("legal move");
        }
        assert!(!g.is_threefold_repetition());
        assert_eq!(g.status(), GameStatus::Ongoing);
    }

    #[test]
    fn threefold_triggers_on_third_occurrence() {
        let mut g = ChessGame::new();
        let seq = [
            "g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8",
        ];
        for uci in seq {
            g.play(Move::from_uci(uci).expect("valid uci"))
                .expect("legal move");
        }
        assert!(g.is_threefold_repetition());
    }

    #[test]
    fn distinct_positions_never_threefold() {
        let mut g = ChessGame::new();
        for uci in ["e2e4", "e7e5", "g1f3", "b8c6", "f1b5", "a7a6"] {
            g.play(Move::from_uci(uci).expect("valid uci"))
                .expect("legal move");
        }
        assert!(!g.is_threefold_repetition());
    }

    #[test]
    fn fifty_move_draw_status() {
        let fen = "4k3/8/8/8/8/8/8/R3K3 w - - 100 1";
        let game = ChessGame::from_position(Position::from_fen(fen).expect("valid fen"));
        assert_eq!(game.status(), GameStatus::FiftyMoveDraw);
    }

    #[test]
    fn current_position_is_incremental() {
        let mut g = ChessGame::new();
        g.play(Move::from_uci("e2e4").expect("valid uci"))
            .expect("legal move");
        assert_eq!(g.current_position().to_fen(), g.current().to_fen());
        assert!(g.current_position().to_fen().contains("4P3"));
    }

    #[test]
    fn serde_roundtrip_rebuilds_current() {
        let mut g = ChessGame::new();
        g.play(Move::from_uci("e2e4").expect("valid uci"))
            .expect("legal move");
        let json = serde_json::to_string(&g).expect("serialize");
        let restored: ChessGame = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(restored.current_position(), g.current_position());
        assert_eq!(restored.moves, g.moves);
    }

    #[test]
    fn position_hashes_tracks_incrementally() {
        let mut g = ChessGame::new();
        assert_eq!(g.position_hashes().len(), 1);
        g.play(Move::from_uci("e2e4").expect("valid uci"))
            .expect("legal move");
        assert_eq!(g.position_hashes().len(), 2);
    }
}
