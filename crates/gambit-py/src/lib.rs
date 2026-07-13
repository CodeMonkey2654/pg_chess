//! Python bindings for the gambit-db chess engine.

use gambit_db::{
    parse_pgn as db_parse_pgn, ChessGame, Move as DbMove, MoveError, MoveParseError, PgnError,
    PgnGame as DbPgnGame, Position as DbPosition,
};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyType};

fn move_parse_err(err: MoveParseError) -> PyErr {
    PyValueError::new_err(err.to_string())
}

fn move_err(err: MoveError) -> PyErr {
    PyValueError::new_err(err.to_string())
}

fn pgn_err(err: PgnError) -> PyErr {
    PyValueError::new_err(err.to_string())
}

/// A chess position in FEN form with move generation.
#[pyclass]
struct Position {
    inner: DbPosition,
}

#[pymethods]
impl Position {
    #[classmethod]
    fn from_fen(_cls: &Bound<'_, PyType>, fen: &str) -> PyResult<Self> {
        DbPosition::from_fen(fen)
            .map(|inner| Self { inner })
            .map_err(|e| PyValueError::new_err(format!("invalid FEN '{fen}': {e}")))
    }

    fn to_fen(&self) -> String {
        self.inner.to_fen()
    }

    fn legal_moves(&self) -> Vec<Move> {
        self.inner
            .legal_moves()
            .into_iter()
            .map(|m| Move { inner: m })
            .collect()
    }

    fn apply_move(&self, mv: &Move) -> PyResult<Position> {
        self.inner
            .apply_move(mv.inner)
            .map(|inner| Position { inner })
            .map_err(move_err)
    }

    fn __repr__(&self) -> String {
        format!("Position('{}')", self.inner.to_fen())
    }

    fn __str__(&self) -> String {
        self.inner.to_fen()
    }
}

/// A chess move in UCI notation.
#[pyclass]
struct Move {
    inner: DbMove,
}

#[pymethods]
impl Move {
    #[classmethod]
    fn from_uci(_cls: &Bound<'_, PyType>, uci: &str) -> PyResult<Self> {
        DbMove::from_uci(uci)
            .map(|inner| Self { inner })
            .map_err(move_parse_err)
    }

    fn to_uci(&self) -> String {
        self.inner.to_uci()
    }

    fn __repr__(&self) -> String {
        format!("Move('{}')", self.inner.to_uci())
    }

    fn __str__(&self) -> String {
        self.inner.to_uci()
    }
}

/// A chess game as a starting position plus move history.
#[pyclass]
struct Game {
    inner: ChessGame,
}

#[pymethods]
impl Game {
    #[new]
    fn new() -> Self {
        Self {
            inner: ChessGame::new(),
        }
    }

    fn play(&mut self, mv: &Move) -> PyResult<()> {
        self.inner.play(mv.inner).map_err(move_err)
    }

    fn fen(&self) -> String {
        self.inner.current_position().to_fen()
    }

    fn __repr__(&self) -> String {
        format!(
            "Game(fen='{}', moves={})",
            self.fen(),
            self.inner.moves.len()
        )
    }
}

/// A parsed PGN game with headers and movetext.
#[pyclass]
struct PgnGame {
    inner: DbPgnGame,
}

#[pymethods]
impl PgnGame {
    #[getter]
    fn headers(&self, py: Python<'_>) -> PyResult<Py<PyDict>> {
        let dict = PyDict::new(py);
        for (key, value) in &self.inner.headers {
            dict.set_item(key, value)?;
        }
        Ok(dict.unbind())
    }

    fn to_game(&self) -> PyResult<Game> {
        self.inner
            .to_chess_game()
            .map(|inner| Game { inner })
            .map_err(pgn_err)
    }

    fn __repr__(&self) -> String {
        format!(
            "PgnGame(headers={}, moves={})",
            self.inner.headers.len(),
            self.inner.movetext.moves.len()
        )
    }
}

/// Parse a single PGN game from text.
#[pyfunction]
fn parse_pgn(text: &str) -> PyResult<PgnGame> {
    db_parse_pgn(text)
        .map(|inner| PgnGame { inner })
        .map_err(pgn_err)
}

/// Python module `gambit`.
#[pymodule]
fn gambit(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<Position>()?;
    m.add_class::<Move>()?;
    m.add_class::<Game>()?;
    m.add_class::<PgnGame>()?;
    m.add_function(wrap_pyfunction!(parse_pgn, m)?)?;
    Ok(())
}
