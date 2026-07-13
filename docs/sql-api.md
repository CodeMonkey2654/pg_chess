# SQL API

The public SQL interface is small and centered around a few helpers.

## Position functions

- `chess_start_position() -> chess_position`
- `chess_from_fen(fen text) -> chess_position`
- `chess_to_fen(position chess_position) -> text`
- `chess_side_to_move(position chess_position) -> text`
- `chess_is_valid_fen(fen text) -> boolean`
- `chess_in_check(position chess_position) -> boolean`
- `chess_is_checkmate(position chess_position) -> boolean`
- `chess_is_stalemate(position chess_position) -> boolean`
- `chess_apply_move(position chess_position, uci text) -> chess_position`
- `chess_legal_move_count(position chess_position) -> integer`
- `chess_legal_moves(position chess_position) -> setof record`
- `chess_placement(position chess_position) -> text[]`

## Move functions

- `chess_is_valid_uci(uci text) -> boolean`
- `chess_move_from_uci(uci text) -> chess_move`
- `chess_move_to_uci(move chess_move) -> text`
- `chess_move_from_square(move chess_move) -> text`
- `chess_move_to_square(move chess_move) -> text`
- `chess_move_promotion(move chess_move) -> text`

## Game functions

- `chess_new_game() -> chess_game`
- `chess_play(game chess_game, uci text) -> chess_game`
- `chess_game_fen(game chess_game) -> text`
- `chess_game_ply(game chess_game) -> integer`
- `chess_game_status(game chess_game) -> text`
- `chess_game_moves(game chess_game) -> setof record`
- `chess_game_positions(game chess_game) -> setof record`

## Example queries

```sql
SELECT chess_to_fen(chess_start_position());

SELECT chess_to_fen(chess_apply_move(chess_start_position(), 'e2e4'));

SELECT count(*) FROM chess_legal_moves(chess_start_position());

SELECT chess_game_fen(chess_play(chess_play(chess_new_game(), 'e2e4'), 'e7e5'));
```

## Notes

- `chess_is_valid_uci` and `chess_is_valid_fen` only check that the notation is well-formed.
- Illegal moves error out when applied through the SQL helpers.
