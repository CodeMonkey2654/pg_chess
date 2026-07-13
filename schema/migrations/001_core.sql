-- Phase 3 core schema for gambit chess database ingest.
-- Requires: CREATE EXTENSION pg_chess;

CREATE SCHEMA IF NOT EXISTS gambit;

-- ---------------------------------------------------------------------------
-- Sources (import batches, e.g. lichess_2024-01)
-- ---------------------------------------------------------------------------

CREATE TABLE gambit.sources (
    id          serial PRIMARY KEY,
    name        text NOT NULL UNIQUE,
    description text,
    imported_at timestamptz NOT NULL DEFAULT now()
);

-- ---------------------------------------------------------------------------
-- Games
-- ---------------------------------------------------------------------------

CREATE TABLE gambit.games (
    id            bigserial PRIMARY KEY,
    source_id     int NOT NULL REFERENCES gambit.sources (id),
    pgn_text      text,
    pgn_sha256    bytea,
    pgn_byte_offset bigint,
    white         text,
    black         text,
    white_elo     int,
    black_elo     int,
    event         text,
    site          text,
    round         text,
    game_date     date,
    result        text NOT NULL CHECK (result IN ('1-0', '0-1', '1/2-1/2', '*')),
    eco           text,
    ply_count     int NOT NULL,
    imported_at   timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX games_source_id_idx ON gambit.games (source_id);
CREATE INDEX games_white_black_idx ON gambit.games (white, black);
CREATE INDEX games_game_date_idx ON gambit.games (game_date);
CREATE INDEX games_imported_at_brin_idx ON gambit.games USING brin (imported_at);

-- ---------------------------------------------------------------------------
-- Positions (partitioned by source_id)
-- ---------------------------------------------------------------------------

CREATE TABLE gambit.positions (
    game_id   bigint NOT NULL,
    source_id int NOT NULL,
    ply       int NOT NULL,
    position  chess_position,
    hash      bigint NOT NULL,
    fen       text NOT NULL,
    PRIMARY KEY (source_id, game_id, ply)
) PARTITION BY LIST (source_id);

CREATE TABLE gambit.positions_default PARTITION OF gambit.positions DEFAULT;

-- ---------------------------------------------------------------------------
-- Plies (partitioned by source_id)
-- ---------------------------------------------------------------------------

CREATE TABLE gambit.plies (
    game_id   bigint NOT NULL,
    source_id int NOT NULL,
    ply       int NOT NULL,
    move      chess_move,
    san       text NOT NULL,
    uci       text NOT NULL,
    PRIMARY KEY (source_id, game_id, ply)
) PARTITION BY LIST (source_id);

CREATE TABLE gambit.plies_default PARTITION OF gambit.plies DEFAULT;

-- ---------------------------------------------------------------------------
-- Staging tables (UNLOGGED, used by gambit-ingest COPY pipeline)
-- ---------------------------------------------------------------------------

CREATE UNLOGGED TABLE gambit.staging_games (
    batch_seq     int NOT NULL,
    pgn_text      text,
    pgn_sha256    bytea,
    pgn_byte_offset bigint,
    white         text,
    black         text,
    white_elo     int,
    black_elo     int,
    event         text,
    site          text,
    round         text,
    game_date     date,
    result        text NOT NULL,
    eco           text,
    ply_count     int NOT NULL
);

CREATE UNLOGGED TABLE gambit.staging_positions (
    batch_seq int NOT NULL,
    ply       int NOT NULL,
    fen       text NOT NULL,
    hash      bigint NOT NULL
);

CREATE UNLOGGED TABLE gambit.staging_plies (
    batch_seq int NOT NULL,
    ply       int NOT NULL,
    uci       text NOT NULL,
    san       text NOT NULL
);

-- ---------------------------------------------------------------------------
-- Opening move statistics (materialized view, refreshed post-ingest)
-- ---------------------------------------------------------------------------

CREATE MATERIALIZED VIEW gambit.opening_moves AS
SELECT
    prefix.hash AS prefix_hash,
    pl.uci AS move_uci,
    count(*)::bigint AS count,
    count(*) FILTER (
        WHERE g.result = '1-0'
    )::bigint AS white_wins,
    count(*) FILTER (
        WHERE g.result = '0-1'
    )::bigint AS black_wins,
    count(*) FILTER (
        WHERE g.result = '1/2-1/2'
    )::bigint AS draws
FROM gambit.plies pl
JOIN gambit.positions prefix
    ON prefix.game_id = pl.game_id
    AND prefix.source_id = pl.source_id
    AND prefix.ply = pl.ply - 1
JOIN gambit.games g ON g.id = pl.game_id
GROUP BY prefix.hash, pl.uci
WITH NO DATA;

CREATE UNIQUE INDEX opening_moves_prefix_move_idx
    ON gambit.opening_moves (prefix_hash, move_uci);

-- ---------------------------------------------------------------------------
-- Helper: create source-specific partitions
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION gambit.ensure_source_partitions(p_source_id int, p_source_name text)
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
    safe_name text;
    pos_part  text;
    ply_part  text;
BEGIN
    safe_name := regexp_replace(lower(p_source_name), '[^a-z0-9]+', '_', 'g');
    safe_name := trim(both '_' from safe_name);
    IF safe_name = '' THEN
        safe_name := 'source_' || p_source_id::text;
    END IF;

    pos_part := 'positions_' || safe_name;
    ply_part := 'plies_' || safe_name;

    IF NOT EXISTS (
        SELECT 1 FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'gambit' AND c.relname = pos_part
    ) THEN
        EXECUTE format(
            'CREATE TABLE gambit.%I PARTITION OF gambit.positions FOR VALUES IN (%s)',
            pos_part, p_source_id
        );
        EXECUTE format(
            'CREATE INDEX %I ON gambit.%I (hash)',
            pos_part || '_hash_idx', pos_part
        );
        EXECUTE format(
            'CREATE INDEX %I ON gambit.%I (game_id, ply)',
            pos_part || '_game_ply_idx', pos_part
        );
    END IF;

    IF NOT EXISTS (
        SELECT 1 FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'gambit' AND c.relname = ply_part
    ) THEN
        EXECUTE format(
            'CREATE TABLE gambit.%I PARTITION OF gambit.plies FOR VALUES IN (%s)',
            ply_part, p_source_id
        );
        EXECUTE format(
            'CREATE INDEX %I ON gambit.%I (game_id, ply)',
            ply_part || '_game_ply_idx', ply_part
        );
    END IF;
END;
$$;

-- Indexes on default partitions (fallback before source-specific partitions exist)
CREATE INDEX positions_default_hash_idx ON gambit.positions_default (hash);
CREATE INDEX positions_default_game_ply_idx ON gambit.positions_default (game_id, ply);
CREATE INDEX plies_default_game_ply_idx ON gambit.plies_default (game_id, ply);

-- ---------------------------------------------------------------------------
-- Post-ingest type materialization (chess_position / chess_move from text)
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION gambit.backfill_positions(p_source_id int)
RETURNS bigint
LANGUAGE plpgsql
AS $$
DECLARE
    updated bigint;
BEGIN
    UPDATE gambit.positions p
    SET position = p.fen::chess_position
    WHERE p.source_id = p_source_id
      AND p.position IS NULL;

    GET DIAGNOSTICS updated = ROW_COUNT;
    RETURN updated;
END;
$$;

CREATE OR REPLACE FUNCTION gambit.backfill_plies(p_source_id int)
RETURNS bigint
LANGUAGE plpgsql
AS $$
DECLARE
    updated bigint;
BEGIN
    UPDATE gambit.plies pl
    SET move = pl.uci::chess_move
    WHERE pl.source_id = p_source_id
      AND pl.move IS NULL;

    GET DIAGNOSTICS updated = ROW_COUNT;
    RETURN updated;
END;
$$;

CREATE OR REPLACE FUNCTION gambit.ensure_position_indexes(p_source_id int)
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
    src_name  text;
    safe_name text;
    pos_part  text;
BEGIN
    SELECT name INTO src_name FROM gambit.sources WHERE id = p_source_id;
    IF src_name IS NULL THEN
        RETURN;
    END IF;

    safe_name := regexp_replace(lower(src_name), '[^a-z0-9]+', '_', 'g');
    safe_name := trim(both '_' from safe_name);
    IF safe_name = '' THEN
        safe_name := 'source_' || p_source_id::text;
    END IF;

    pos_part := 'positions_' || safe_name;

    IF EXISTS (
        SELECT 1 FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE n.nspname = 'gambit' AND c.relname = pos_part
    ) THEN
        EXECUTE format(
            'CREATE INDEX IF NOT EXISTS %I ON gambit.%I (position)',
            pos_part || '_position_idx', pos_part
        );
    END IF;

    EXECUTE
        'CREATE INDEX IF NOT EXISTS positions_default_position_idx
         ON gambit.positions_default (position)';
END;
$$;
