# Architecture

The extension is split into a small Rust domain model and a PostgreSQL-facing adapter layer.

## Layers

- The Rust chess model lives in the modules under src. It handles board representation, FEN parsing, move representation, move generation, and simple game state.
- The PostgreSQL layer is centered in src/api.rs. This file defines the custom SQL types and exposes the public SQL functions.

## Module outline

- src/types.rs: basic chess primitives such as color, piece kind, and piece.
- src/board.rs: square indexing and the mailbox board representation.
- src/fen.rs: position state plus FEN parsing and formatting.
- src/movement.rs: move representation and UCI parsing.
- src/movegen.rs: move generation and legality checks.
- src/game.rs: simple game history and status tracking.
- src/api.rs: pgrx entry points and SQL-facing wrappers.

## Dependency direction

The domain modules do not depend on pgrx. They are plain Rust logic. The pgrx layer imports those modules and exposes them over SQL.

## Error handling

SQL functions use pgrx error handling. Invalid input or illegal moves raise PostgreSQL errors rather than silently returning a partial result.

## What is stored

Positions, moves, and game history are the main persisted concepts. The board is stored implicitly as part of a position.

## What is derived

Legal moves, checkmate/stalemate checks, and game status are computed from the current position rather than stored as separate state.

