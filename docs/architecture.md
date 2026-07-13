# Architecture

The project is a Cargo workspace with a Rust chess engine and adapters for PostgreSQL, Python, and UCI.

## Crates

- **gambit-db** (`crates/gambit-db/`): board, FEN, move generation, SAN/PGN, Zobrist hashing, game state. Published Rust API.
- **gambit-ingest** (`crates/gambit-ingest/`): high-throughput PGN bulk loader for the `gambit` PostgreSQL schema.
- **pg_chess** (`crates/pg_chess/`): pgrx extension exposing `chess_*` SQL types and functions.
- **gambit-py** (`crates/gambit-py/`): PyO3 bindings for Python.
- **gambit-uci** (`crates/gambit-uci/`): UCI protocol helpers (external engine integration).

## Module layout (gambit-db)

- `square`, `board`, `types` — LERF indexing, mailbox board with embedded occupancy bitboards
- `fen/` — parse, format, validate, `Position` with cached king squares and Zobrist hash
- `movement` — `Move`, UCI
- `movegen/` — pseudo-legal generation, bitboard-accelerated attacks, castling, legality filter, make/unmake
- `san/`, `pgn/` — contextual notation (mainline + nested RAVs)
- `game` — `ChessGame` with incremental position and hash history
- `perft` — recursive legal-move correctness counting
- `tablebase` (feature `tablebase`) — Syzygy `.rtbw`/`.rtbz` probing via `shakmaty-syzygy`

## Public API

Use `gambit_db::prelude` for the common surface: `Position`, `Move`, `ChessGame`, SAN/PGN helpers.

- `Position::from_fen` always runs semantic validation (`Result`)
- `legal_moves()` returns `Vec<Move>`
- Optional `tablebase` feature enables `Tablebase::open` / `probe_wdl` / `probe_dtz`

## Dependency direction

```
gambit-db (engine)
    ↑
    ├── pg_chess
    ├── gambit-ingest
    ├── gambit-py
    └── gambit-uci
```

Adapters depend on `gambit-db`; the engine has no pgrx/pyo3 dependency.

## Error handling

- Rust API: `Result` with `FenError`, `MoveError`, `SanError`, `PgnError`, `MoveParseError`
- SQL boundary: pgrx `error!()` for invalid input
