-- Apply gambit schema (relative to this file)
\ir ../../../schema/migrations/001_core.sql

INSERT INTO gambit.sources (name) VALUES ('pg_regress_test')
ON CONFLICT (name) DO NOTHING;

SELECT gambit.ensure_source_partitions(
    (SELECT id FROM gambit.sources WHERE name = 'pg_regress_test'),
    'pg_regress_test'
);

WITH game AS (
    INSERT INTO gambit.games (source_id, white, black, result, ply_count)
    VALUES (
        (SELECT id FROM gambit.sources WHERE name = 'pg_regress_test'),
        'Player A', 'Player B', '1-0', 1
    )
    RETURNING id, source_id
),
start_pos AS (
    SELECT chess_start_position() AS position,
           chess_game_hash(chess_new_game()) AS hash,
           chess_to_fen(chess_start_position()) AS fen
),
after_e4 AS (
    SELECT chess_apply_move(chess_start_position(), 'e2e4') AS position,
           chess_game_hash(chess_play(chess_new_game(), 'e2e4')) AS hash,
           chess_to_fen(chess_apply_move(chess_start_position(), 'e2e4')) AS fen
)
INSERT INTO gambit.positions (game_id, source_id, ply, position, hash, fen)
SELECT game.id, game.source_id, 0, start_pos.position, start_pos.hash, start_pos.fen FROM game, start_pos
UNION ALL
SELECT game.id, game.source_id, 1, after_e4.position, after_e4.hash, after_e4.fen FROM game, after_e4;

WITH game AS (
    SELECT id, source_id FROM gambit.games
    WHERE white = 'Player A' AND black = 'Player B'
    LIMIT 1
)
INSERT INTO gambit.plies (game_id, source_id, ply, move, san, uci)
SELECT game.id, game.source_id, 1,
       chess_move_from_uci('e2e4'),
       'e4',
       'e2e4'
FROM game;

SELECT count(*) = 2 AS position_rows
FROM gambit.positions;

SELECT count(*) > 0 AS hash_lookup_works
FROM gambit.positions
WHERE hash = chess_game_hash(chess_play(chess_new_game(), 'e2e4'));

SELECT count(*) >= 1 AS partition_exists
FROM pg_class c
JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = 'gambit'
  AND c.relname = 'positions_pg_regress_test';

REFRESH MATERIALIZED VIEW gambit.opening_moves;

SELECT count(*) >= 1 AS opening_stats_populated
FROM gambit.opening_moves
WHERE move_uci = 'e2e4';
