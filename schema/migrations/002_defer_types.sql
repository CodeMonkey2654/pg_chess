-- Allow deferred chess type materialization during bulk ingest.
-- Safe to re-run (uses IF EXISTS / CREATE OR REPLACE).

ALTER TABLE gambit.positions ALTER COLUMN position DROP NOT NULL;
ALTER TABLE gambit.plies ALTER COLUMN move DROP NOT NULL;

-- Backfill helpers (idempotent replace)
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
