# pg_chess

`pg_chess` is a PostgreSQL extension written in Rust with [`pgrx`](https://github.com/pgcentralfoundation/pgrx). It introduces chess-specific PostgreSQL types and functions backed by a chess domain model in Rust.

The project is under active development. The current implementation is focused on foundation representations and text formats:
- pieces, colors, and types of pieces (piece kinds)
- squares using Little-Endian Rank-File indexing
- A 64 square mailbox board
- complete chess positions represented using [FEN](https://en.wikipedia.org/wiki/Forsyth%E2%80%93Edwards_Notation)
- Chess moves represented using [UCI Notation](https://en.wikipedia.org/wiki/Universal_Chess_Interface)
- PostgreSQL custom types for positions and moves
- SQL helpers for constructing and inspecting those values

## Status

Current Version: Pre-Alpha

The extension currently provides or plan to provides before calling v0.1.0:

| Capability | Status |
| ---------- | ------ |
| Piece and Color Types | Implemented |
| Algebraic square parsing and formatting | Implemented |
| Mailbox board representation | Implemented |
| Starting board construction | Implemented |
| FEN Parsing and Formatting | Implemented |
| PostgreSQL `chess_position` type | Implemented |
| UCI Move Parsing and Formatting | Implemented |
| PostgreSQL `chess_move` type | Implemented |
| Move flags | Partially implemented |
| Move application | Not implemented |
| Pseudo-legal move generation | Not implemented |
| Legal move generation | Not implemented |
| Check and Checkmate Detection | Not implemented |
| SAN Support | Not implemented |
| PGN support | Not implemented |
| Game model | Not implemented |
| Full PG API | Not implemented |
| Operators + operator classes | Not implemented |
| Indexes + Optimizations | Not implemented |


## Notation Validity vs Chess Legality

The extension distinguishes whether a notation is properly formatted (Notation Validity) from whether the mvoe is legal in chess (Chess Legality).

For example:
```sql
SELECT chess_is_valid_uci('e2e4');
```

returns `true` because `e2e4` is properly formatted UCI. It doesn't show that:
- a piece exists on `e2`
- that piece belongs to the side to move
- the piece can move to `e4`
- the move leaves its king safe

This also extends to `chess_is_valid_fen`, which currently checks whether the input can be parsed by the extension. It doesn't show that the represented position could occur in a legal chess game.

## Quick Start

### Pre-reqs

You need:
- Rust and Cargo
- A supproted PostgreSQL installation
- `cargo-pgrx`
- the PostgreSQL development files requried by `pgrx`

Install `cargo-pgrx`:

```bash
cargo install --locked cargo-pgrx
```

Initialize `pgrx` against your PostgreSQL installation:

```bash
cargo pgrx init
```

Run the extention in a development PostgreSQL instance:

```bash
cargo pgrx run
```

Run tests:

```bash
cargo test
cargo pgrx test
```

Run formatting and lint checks:

```bash
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
```

The exact PG versions supported should be from `Cargo.toml` and the local cargo-pgrx config.

## Architecture

The repository is divided into a `pgrx` adapter layer and a chess domain model. The domain model represents chess and notation parsing. The SQL layer owns PG type integration, functions, and conversion of Rust errors into PostgreSQL errors. 

There are other docs that I may or may not write you can read in the docs folder if you're curious.

## Design principles

- Keep PostgreSQL layer thin
- Make invariants explicit
- Separate intrinsic and contextual move data (move has squares, and promotion piece, everything else come from the position)
- For Now - prefer correctness over optimization (Mailbox over bitmap for instance)


## Stability
There is none yet ;)