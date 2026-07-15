# Architecture

The project is a Cargo workspace with a Rust chess engine and adapters for PostgreSQL, Python, UCI, and analysis.

## Crates

- **gambit-db** (`crates/gambit-db/`): board, FEN, move generation, SAN/PGN, Zobrist hashing, game state. Published Rust API.
- **gambit-analysis** (`crates/gambit-analysis/`): in-memory search engine (negamax, TT, eval, optional corpus book).
- **gambit-ingest** (`crates/gambit-ingest/`): high-throughput PGN bulk loader for the `gambit` PostgreSQL schema; exports corpus books.
- **gambit-studio-server** (`crates/gambit-studio-server/`): gRPC API (tonic + grpc-web) for Gambit Studio queries; proxies ingest RPCs to the worker.
- **gambit-ingest-worker** (`crates/gambit-ingest-worker/`): gRPC ingest worker (Lichess sync/load, job streaming).
- **gambit-proto** (`crates/gambit-proto/`): shared protobuf definitions and generated stubs.
- **gambit-studio-ui** (`crates/gambit-studio-ui/`): Leptos WASM database browser (grpc-web client).
- **gambit-db-wasm** (`crates/gambit-db-wasm/`): WASM bindings for board replay in the browser.
- **pg_chess** (`crates/pg_chess/`): pgrx extension exposing `chess_*` SQL types and functions.
- **gambit-py** (`crates/gambit-py/`): PyO3 bindings for Python.
- **gambit-uci** (`crates/gambit-uci/`): UCI client for external engines and native UCI server (`gambit-analysis` binary).

## Module layout (gambit-db)

- `square`, `board`, `types` — LERF indexing, mailbox board with embedded occupancy bitboards
- `fen/` — parse, format, validate, `Position` with cached king squares and Zobrist hash
- `movement` — `Move`, UCI
- `movegen/` — pseudo-legal generation, bitboard-accelerated attacks, castling, legality filter, make/unmake, `MoveList`
- `san/`, `pgn/` — contextual notation (mainline + nested RAVs)
- `game` — `ChessGame` with incremental position and hash history
- `perft` — recursive legal-move correctness counting
- `tablebase` (feature `tablebase`) — Syzygy `.rtbw`/`.rtbz` probing via `shakmaty-syzygy`

## Module layout (gambit-analysis)

- `search` — iterative deepening negamax with quiescence
- `tt` — transposition table keyed on Zobrist hash
- `eval` — material + piece-square tables
- `order` — hash move, MVV-LVA, killers, corpus weights
- `book` — `.gbook` corpus loader (from `gambit.opening_moves` export)

## Public API

Use `gambit_db::prelude` for the common surface: `Position`, `Move`, `ChessGame`, SAN/PGN helpers.

- `Position::from_fen` always runs semantic validation (`Result`)
- `legal_moves()` returns `Vec<Move>`; `generate_legal_moves()` fills a stack `MoveList` for search
- Optional `tablebase` feature enables `Tablebase::open` / `probe_wdl` / `probe_dtz`

Use `gambit_analysis::Analyzer` for native search. See [analysis.md](analysis.md).

## Dependency direction

```
gambit-db (core)
    ↑
    ├── gambit-analysis
    ├── pg_chess
    ├── gambit-ingest  (+ export-book → .gbook)
    ├── gambit-py
    └── gambit-uci     (client + server binary)
            ↑
            └── depends on gambit-analysis
```

Adapters depend on `gambit-db`; the core engine has no pgrx/pyo3 dependency. Corpus statistics are aggregated in PostgreSQL and exported to `.gbook` for in-memory use—search never queries Postgres per node.

## Error handling

- Rust API: `Result` with `FenError`, `MoveError`, `SanError`, `PgnError`, `MoveParseError`
- SQL boundary: pgrx `error!()` for invalid input
