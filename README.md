# pg_chess

pg_chess is a small PostgreSQL extension for working with chess positions, moves, and simple games directly in SQL. It is implemented in Rust with pgrx and exposes custom PostgreSQL types plus helper functions.

## What it currently supports

- FEN parsing and formatting for chess positions
- UCI parsing and formatting for moves
- Custom SQL types: `chess_position`, `chess_move`, and `chess_game`
- Legal move generation from a position
- Move application and simple game tracking

## Status

This project is still pre-alpha. The core model is working, but features such as SAN/PGN support, richer game analysis, and broader polish are still pending.

## Quick start

### Prerequisites

- Rust and Cargo
- A supported PostgreSQL installation
- `cargo-pgrx`
- The PostgreSQL development files required by pgrx

### Install and run

```bash
cargo install --locked cargo-pgrx
cargo pgrx init
cargo pgrx run
```

### Try it in SQL

```sql
SELECT chess_to_fen(chess_start_position());
SELECT chess_to_fen(chess_apply_move(chess_start_position(), 'e2e4'));
SELECT count(*) FROM chess_legal_moves(chess_start_position());
```

## Notes

- `chess_is_valid_fen` and `chess_is_valid_uci` check formatting and parsing, not full chess legality.
- Applying an illegal move through SQL helpers raises an error.
- See the docs in the docs directory for a compact overview of the data model and SQL API.

## Development

```bash
cargo test
cargo pgrx test
cargo fmt --check
```