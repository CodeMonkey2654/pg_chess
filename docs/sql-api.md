# SQL API

The public SQL interface is small and centered around a few helpers. Pure functions are marked `IMMUTABLE` and `PARALLEL SAFE`.

## Position functions

| Function | Volatility |
|----------|------------|
| `chess_start_position()` | IMMUTABLE |
| `chess_from_fen(fen text)` | IMMUTABLE (strict semantic validation) |
| `chess_to_fen(position)` | IMMUTABLE |
| `chess_is_valid_fen(fen text)` | IMMUTABLE (strict) |
| `chess_fen_error(fen text)` | IMMUTABLE |
| `chess_side_to_move(position)` | IMMUTABLE |
| `chess_in_check(position)` | IMMUTABLE |
| `chess_is_checkmate(position)` | IMMUTABLE |
| `chess_is_stalemate(position)` | IMMUTABLE |
| `chess_apply_move(position, uci)` | IMMUTABLE |
| `chess_apply_san(position, san)` | IMMUTABLE |
| `chess_legal_move_count(position)` | IMMUTABLE |
| `chess_legal_moves(position)` | IMMUTABLE |
| `chess_placement(position)` | IMMUTABLE |

## Move functions

| Function | Volatility |
|----------|------------|
| `chess_is_valid_uci(uci)` | IMMUTABLE |
| `chess_move_from_uci(uci)` | IMMUTABLE |
| `chess_move_to_uci(move)` | IMMUTABLE |
| `chess_move_to_san(position, move)` | IMMUTABLE |
| `chess_move_from_square(move)` | IMMUTABLE |
| `chess_move_to_square(move)` | IMMUTABLE |
| `chess_move_promotion(move)` | IMMUTABLE |

## Game functions

| Function | Volatility |
|----------|------------|
| `chess_new_game()` | IMMUTABLE |
| `chess_play(game, uci)` | IMMUTABLE |
| `chess_from_pgn(text)` | IMMUTABLE |
| `chess_to_pgn(game)` | IMMUTABLE |
| `chess_game_fen(game)` | IMMUTABLE |
| `chess_game_ply(game)` | IMMUTABLE |
| `chess_game_status(game)` | IMMUTABLE |
| `chess_game_moves(game)` | IMMUTABLE |
| `chess_game_positions(game)` | IMMUTABLE |

## Example queries

```sql
SELECT chess_to_fen(chess_start_position());

SELECT chess_to_fen(chess_apply_san(chess_start_position(), 'e4'));

SELECT chess_game_ply(chess_from_pgn('[Event "?"]\n\n1. e4 e5 *'));

SELECT count(*) FROM chess_legal_moves(chess_start_position());
```

## Notes

- `chess_is_valid_fen` and `chess_from_fen` apply strict semantic validation (kings, pawn placement, check state, castling/EP consistency).
- Illegal moves error out when applied through the SQL helpers.
