-- Read performance: cached source rollups and position game counts MV.

CREATE TABLE IF NOT EXISTS gambit.source_rollups (
    source_id   int PRIMARY KEY REFERENCES gambit.sources (id),
    games       bigint NOT NULL DEFAULT 0,
    positions   bigint NOT NULL DEFAULT 0,
    plies       bigint NOT NULL DEFAULT 0,
    updated_at  timestamptz NOT NULL DEFAULT now()
);

-- Backfill from existing data.
INSERT INTO gambit.source_rollups (source_id, games, positions, plies)
SELECT s.id,
       COALESCE(g.cnt, 0),
       COALESCE(p.cnt, 0),
       COALESCE(pl.cnt, 0)
FROM gambit.sources s
LEFT JOIN (
    SELECT source_id, count(*)::bigint AS cnt FROM gambit.games GROUP BY source_id
) g ON g.source_id = s.id
LEFT JOIN (
    SELECT source_id, count(*)::bigint AS cnt FROM gambit.positions GROUP BY source_id
) p ON p.source_id = s.id
LEFT JOIN (
    SELECT source_id, count(*)::bigint AS cnt FROM gambit.plies GROUP BY source_id
) pl ON pl.source_id = s.id
ON CONFLICT (source_id) DO UPDATE SET
    games = EXCLUDED.games,
    positions = EXCLUDED.positions,
    plies = EXCLUDED.plies,
    updated_at = now();

CREATE MATERIALIZED VIEW IF NOT EXISTS gambit.position_game_counts AS
SELECT hash, count(DISTINCT game_id)::bigint AS game_count
FROM gambit.positions
GROUP BY hash
WITH NO DATA;

CREATE UNIQUE INDEX IF NOT EXISTS position_game_counts_hash_idx
    ON gambit.position_game_counts (hash);

-- Increment source rollups after ingest batch (called from Rust).
CREATE OR REPLACE FUNCTION gambit.increment_source_rollups(
    p_source_id int,
    p_games bigint,
    p_positions bigint,
    p_plies bigint
)
RETURNS void
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO gambit.source_rollups (source_id, games, positions, plies)
    VALUES (p_source_id, p_games, p_positions, p_plies)
    ON CONFLICT (source_id) DO UPDATE SET
        games = gambit.source_rollups.games + EXCLUDED.games,
        positions = gambit.source_rollups.positions + EXCLUDED.positions,
        plies = gambit.source_rollups.plies + EXCLUDED.plies,
        updated_at = now();
END;
$$;

-- Refresh explorer stats MVs concurrently (requires unique indexes).
CREATE OR REPLACE FUNCTION gambit.refresh_explorer_stats()
RETURNS void
LANGUAGE plpgsql
AS $$
BEGIN
    REFRESH MATERIALIZED VIEW CONCURRENTLY gambit.opening_moves;
    REFRESH MATERIALIZED VIEW CONCURRENTLY gambit.position_game_counts;
END;
$$;
