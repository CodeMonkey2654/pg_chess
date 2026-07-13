CREATE EXTENSION pg_chess;

-- btree: GROUP BY, ORDER BY, DISTINCT on chess_position
CREATE TEMP TABLE t_positions (p chess_position);
INSERT INTO t_positions VALUES
    (chess_start_position()),
    (chess_from_fen('4k3/8/8/8/8/8/8/4K3 w - - 0 1')),
    (chess_from_fen('4k3/8/8/8/8/8/8/4K3 w - - 5 9'));
CREATE INDEX ON t_positions USING btree (p);

SELECT count(DISTINCT p) FROM t_positions;
SELECT count(*) FROM (SELECT p FROM t_positions ORDER BY p) ordered_positions;

-- GIN: placement containment queries
CREATE TEMP TABLE t_placements (ply int, placement text[]);
INSERT INTO t_placements
SELECT p.ply, chess_placement(p.position)
FROM chess_game_positions(
    chess_play(chess_play(chess_new_game(), 'e2e4'), 'e7e5')
) p;
CREATE INDEX ON t_placements USING gin (placement);
SELECT count(*) FROM t_placements WHERE placement @> ARRAY['Pe4','pe5'];

-- hash lookup via chess_game_hash
CREATE TEMP TABLE t_game_hashes (hash bigint);
INSERT INTO t_game_hashes
SELECT chess_game_hash(chess_play(chess_new_game(), 'e2e4'));
CREATE INDEX ON t_game_hashes (hash);
SELECT count(*) FROM t_game_hashes
WHERE hash = chess_game_hash(chess_play(chess_new_game(), 'e2e4'));
