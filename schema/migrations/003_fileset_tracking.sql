-- Fileset and ingest run tracking for Lichess shard loads.

CREATE TABLE gambit.filesets (
    id                    bigserial PRIMARY KEY,
    source_id             int NOT NULL REFERENCES gambit.sources (id),
    remote_url            text NOT NULL UNIQUE,
    filename              text NOT NULL,
    period_label          text NOT NULL,
    byte_size             bigint,
    sha256                bytea,
    status                text NOT NULL DEFAULT 'pending'
        CHECK (status IN (
            'pending', 'downloading', 'downloaded',
            'ingesting', 'complete', 'failed'
        )),
    download_started_at   timestamptz,
    download_completed_at timestamptz,
    ingest_started_at     timestamptz,
    ingest_completed_at   timestamptz,
    games_loaded          bigint NOT NULL DEFAULT 0,
    games_errors          bigint NOT NULL DEFAULT 0,
    positions_loaded      bigint NOT NULL DEFAULT 0,
    plies_loaded          bigint NOT NULL DEFAULT 0,
    error_message         text
);

CREATE INDEX filesets_source_id_idx ON gambit.filesets (source_id);
CREATE INDEX filesets_status_idx ON gambit.filesets (status);
CREATE INDEX filesets_period_label_idx ON gambit.filesets (period_label);

CREATE TABLE gambit.ingest_runs (
    id                bigserial PRIMARY KEY,
    fileset_id        bigint REFERENCES gambit.filesets (id),
    source_id         int NOT NULL REFERENCES gambit.sources (id),
    started_at        timestamptz NOT NULL DEFAULT now(),
    finished_at       timestamptz,
    workers           int,
    batch_games       int,
    games_per_min     numeric,
    positions_per_sec numeric,
    wall_seconds      numeric
);

CREATE INDEX ingest_runs_fileset_id_idx ON gambit.ingest_runs (fileset_id);
CREATE INDEX ingest_runs_source_id_idx ON gambit.ingest_runs (source_id);
