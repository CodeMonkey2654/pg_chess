-- First refresh of explorer MVs must be non-concurrent when not yet populated.

CREATE OR REPLACE FUNCTION gambit.refresh_explorer_stats()
RETURNS void
LANGUAGE plpgsql
AS $$
DECLARE
    opening_populated boolean;
    counts_populated boolean;
BEGIN
    SELECT ispopulated INTO opening_populated
    FROM pg_matviews
    WHERE schemaname = 'gambit' AND matviewname = 'opening_moves';

    IF opening_populated THEN
        REFRESH MATERIALIZED VIEW CONCURRENTLY gambit.opening_moves;
    ELSE
        REFRESH MATERIALIZED VIEW gambit.opening_moves;
    END IF;

    SELECT ispopulated INTO counts_populated
    FROM pg_matviews
    WHERE schemaname = 'gambit' AND matviewname = 'position_game_counts';

    IF counts_populated THEN
        REFRESH MATERIALIZED VIEW CONCURRENTLY gambit.position_game_counts;
    ELSE
        REFRESH MATERIALIZED VIEW gambit.position_game_counts;
    END IF;
END;
$$;
