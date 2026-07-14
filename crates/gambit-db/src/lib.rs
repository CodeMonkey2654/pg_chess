//! Core chess engine data structures: board, FEN, move generation, and game state.

#![warn(missing_docs)]

mod board;
mod fen;
mod game;
mod movegen;
mod movement;
mod perft;
mod pgn;
mod san;
mod square;
#[cfg(feature = "tablebase")]
mod tablebase;
mod types;
mod zobrist;

/// Common types for typical chess applications.
pub mod prelude {
    pub use crate::fen::{CastlingRights, FenError, Position};
    pub use crate::game::{ChessGame, GameStatus};
    pub use crate::movegen::{MoveList, Undo};
    pub use crate::movement::{Move, MoveFlags, MoveParseError};
    pub use crate::pgn::{
        explode_mainline, game_to_pgn, parse_pgn, parse_pgn_games, split_pgn_games, write_pgn,
        write_pgn_movetext, ExplodedGame, GameHeaders, PgnError, PgnGame, PgnMove, PgnMovetext,
        PlyRow, PositionRow,
    };
    pub use crate::san::{parse_san, to_san, SanError};
    pub use crate::square::Square;
    #[cfg(feature = "tablebase")]
    pub use crate::tablebase::{Tablebase, TablebaseError, Wdl};
    pub use crate::types::{Color, Piece, PieceKind};
}

#[cfg(feature = "tablebase")]
pub use tablebase::{Tablebase, TablebaseError, Wdl};

pub use fen::{CastlingRights, FenError, Position};
pub use game::{ChessGame, GameStatus};
pub use movegen::{MoveError, MoveList, Undo};
pub use movement::{Move, MoveFlags, MoveParseError};
pub use perft::perft;
pub use pgn::{
    explode_mainline, game_to_pgn, parse_pgn, parse_pgn_games, split_pgn_games, write_pgn,
    write_pgn_movetext, ExplodedGame, GameHeaders, PgnError, PgnGame, PgnMove, PgnMovetext, PlyRow,
    PositionRow,
};
pub use san::{parse_san, to_san, SanError};
pub use square::Square;
pub use types::{Color, Piece, PieceKind};
