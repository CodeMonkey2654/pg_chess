# Data types

The core Rust model uses a small set of chess-specific types that map cleanly to SQL.

## Core chess types

- Color: `White` or `Black`
- PieceKind: `Pawn`, `Knight`, `Bishop`, `Rook`, `Queen`, `King`
- Piece: a combination of color and piece kind
- Square: a board square identified by a 0..63 index, with helpers for algebraic notation such as `e4`
- Board: a 64-square mailbox board with optional pieces on each square
- Position: a full chess position including the board, side to move, castling rights, en passant target, and clocks
- Move: a move with source square, destination square, optional promotion, and move flags
- ChessGame: a starting position plus a move history

## SQL-facing types

- `chess_position`: serialized as FEN text
- `chess_move`: serialized as UCI text
- `chess_game`: serialized as a simple text form of `FEN | move1 move2 ...`
- `chess_move_class`: move quality enum (`best`, `good`, `inaccuracy`, `mistake`, `blunder`)
- `chess_eval_source`: evaluation backend enum (`stockfish`, `syzygy`, `corpus`, `native`)
- `chess_analysis_status`: game analysis job status enum

## Analysis functions (pg_chess extension)

- `chess_classify_cp_loss(cp_loss int) → chess_move_class label`
- `chess_accuracy_from_classes(classes text[]) → real`
- `chess_eval_to_cp(cp int, mate_plies int) → int`

## Notes

- FEN is the main representation for positions.
- UCI is the main representation for moves.
- The extension distinguishes syntax validity from full chess legality. A move can be valid UCI without being legal in a specific position.
