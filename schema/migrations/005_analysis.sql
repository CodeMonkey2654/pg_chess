-- Analysis columns on partitioned plies/games + hash-keyed position eval cache.
-- Requires: CREATE EXTENSION pg_chess (chess_move, chess_move_class, chess_eval_source, etc.)

-- Types and pure functions live in the pg_chess extension:
--   chess_move_class, chess_eval_source, chess_analysis_status
--   chess_classify_cp_loss(), chess_accuracy_from_classes(), chess_eval_to_cp()

-- ---------------------------------------------------------------------------
-- Extend plies in-place (propagates to all partitions)
-- ---------------------------------------------------------------------------

ALTER TABLE gambit.plies
    ADD COLUMN IF NOT EXISTS eval_before    smallint,
    ADD COLUMN IF NOT EXISTS eval_after     smallint,
    ADD COLUMN IF NOT EXISTS best_move      chess_move,
    ADD COLUMN IF NOT EXISTS cp_loss        smallint,
    ADD COLUMN IF NOT EXISTS move_class     chess_move_class,
    ADD COLUMN IF NOT EXISTS eval_depth     smallint,
    ADD COLUMN IF NOT EXISTS eval_source    chess_eval_source;

CREATE INDEX IF NOT EXISTS plies_analyzed_game_idx
    ON gambit.plies (source_id, game_id)
    WHERE move_class IS NOT NULL;

-- ---------------------------------------------------------------------------
-- Extend games with rollup columns
-- ---------------------------------------------------------------------------

ALTER TABLE gambit.games
    ADD COLUMN IF NOT EXISTS analysis_status  chess_analysis_status NOT NULL DEFAULT 'none',
    ADD COLUMN IF NOT EXISTS accuracy_white   real,
    ADD COLUMN IF NOT EXISTS accuracy_black   real,
    ADD COLUMN IF NOT EXISTS blunders_white   smallint,
    ADD COLUMN IF NOT EXISTS blunders_black   smallint,
    ADD COLUMN IF NOT EXISTS analyzed_at      timestamptz,
    ADD COLUMN IF NOT EXISTS analysis_version int;

CREATE INDEX IF NOT EXISTS games_analysis_pending_idx
    ON gambit.games (source_id)
    WHERE analysis_status IN ('none', 'failed');

CREATE INDEX IF NOT EXISTS games_analyzed_idx
    ON gambit.games (source_id, analyzed_at DESC NULLS LAST)
    WHERE analysis_status = 'complete';

-- ---------------------------------------------------------------------------
-- Hash-keyed position eval cache (transposition dedup for Explorer)
-- ---------------------------------------------------------------------------

CREATE TABLE IF NOT EXISTS gambit.position_evals (
    hash          bigint NOT NULL,
    profile_id    smallint NOT NULL,
    eval_cp       smallint NOT NULL,
    best_move     chess_move NOT NULL,
    mate_plies    smallint,
    depth         smallint NOT NULL,
    pv            chess_move[],
    source        chess_eval_source NOT NULL,
    computed_at   timestamptz NOT NULL DEFAULT now(),
    PRIMARY KEY (hash, profile_id)
);

CREATE INDEX IF NOT EXISTS position_evals_stale_idx
    ON gambit.position_evals (computed_at);

-- ---------------------------------------------------------------------------
-- Staging for bulk ply analysis writes (mirrors ingest COPY pattern)
-- ---------------------------------------------------------------------------

CREATE UNLOGGED TABLE IF NOT EXISTS gambit.staging_ply_analysis (
    game_id       bigint NOT NULL,
    ply           int NOT NULL,
    eval_before   smallint,
    eval_after    smallint,
    best_move     text NOT NULL,
    cp_loss       smallint,
    move_class    text,
    eval_depth    smallint,
    eval_source   text
);

-- ---------------------------------------------------------------------------
-- Bulk merge staging → plies
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION gambit.merge_ply_analysis(p_source_id int)
RETURNS bigint
LANGUAGE plpgsql
AS $$
DECLARE
    updated bigint;
BEGIN
    UPDATE gambit.plies pl
    SET
        eval_before  = s.eval_before,
        eval_after   = s.eval_after,
        best_move    = s.best_move::chess_move,
        cp_loss      = s.cp_loss,
        move_class   = s.move_class::chess_move_class,
        eval_depth   = s.eval_depth,
        eval_source  = s.eval_source::chess_eval_source
    FROM gambit.staging_ply_analysis s
    WHERE pl.source_id = p_source_id
      AND pl.game_id = s.game_id
      AND pl.ply = s.ply;

    GET DIAGNOSTICS updated = ROW_COUNT;
    TRUNCATE gambit.staging_ply_analysis;
    RETURN updated;
END;
$$;

-- ---------------------------------------------------------------------------
-- Rollup game accuracy from plies (uses pg_chess extension functions)
-- ---------------------------------------------------------------------------

CREATE OR REPLACE FUNCTION gambit.rollup_game_analysis(p_game_id bigint)
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
    v_white_labels text[];
    v_black_labels text[];
    v_blunders_w   smallint;
    v_blunders_b   smallint;
BEGIN
    SELECT
        array_agg(move_class::text ORDER BY ply) FILTER (WHERE ply % 2 = 1),
        array_agg(move_class::text ORDER BY ply) FILTER (WHERE ply % 2 = 0),
        count(*) FILTER (WHERE ply % 2 = 1 AND move_class = 'blunder')::smallint,
        count(*) FILTER (WHERE ply % 2 = 0 AND move_class = 'blunder')::smallint
    INTO v_white_labels, v_black_labels, v_blunders_w, v_blunders_b
    FROM gambit.plies
    WHERE game_id = p_game_id
      AND move_class IS NOT NULL;

    UPDATE gambit.games
    SET
        analysis_status  = 'complete',
        accuracy_white   = chess_accuracy_from_classes(v_white_labels),
        accuracy_black   = chess_accuracy_from_classes(v_black_labels),
        blunders_white   = COALESCE(v_blunders_w, 0),
        blunders_black   = COALESCE(v_blunders_b, 0),
        analyzed_at      = now()
    WHERE id = p_game_id;
END;
$$;

CREATE OR REPLACE FUNCTION gambit.upsert_position_eval(
    p_hash        bigint,
    p_profile_id  smallint,
    p_eval_cp     smallint,
    p_best_move   text,
    p_mate_plies  smallint,
    p_depth       smallint,
    p_pv          text[],
    p_source      chess_eval_source
)
RETURNS void
LANGUAGE plpgsql
AS $$
BEGIN
    INSERT INTO gambit.position_evals (
        hash, profile_id, eval_cp, best_move, mate_plies, depth, pv, source
    )
    VALUES (
        p_hash, p_profile_id, p_eval_cp, p_best_move::chess_move, p_mate_plies,
        p_depth,
        (SELECT array_agg(m::chess_move) FROM unnest(p_pv) AS m),
        p_source
    )
    ON CONFLICT (hash, profile_id) DO UPDATE SET
        eval_cp     = EXCLUDED.eval_cp,
        best_move   = EXCLUDED.best_move,
        mate_plies  = EXCLUDED.mate_plies,
        depth       = EXCLUDED.depth,
        pv          = EXCLUDED.pv,
        source      = EXCLUDED.source,
        computed_at = now();
END;
$$;
