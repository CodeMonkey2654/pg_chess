-- Query performance indexes for Studio player search and source-scoped browse.

CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE INDEX IF NOT EXISTS games_white_trgm_idx
    ON gambit.games USING gin (lower(white) gin_trgm_ops);

CREATE INDEX IF NOT EXISTS games_black_trgm_idx
    ON gambit.games USING gin (lower(black) gin_trgm_ops);

CREATE INDEX IF NOT EXISTS games_source_date_idx
    ON gambit.games (source_id, game_date DESC NULLS LAST);
