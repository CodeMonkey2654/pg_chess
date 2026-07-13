# Bulk ingest

Load PGN databases into PostgreSQL using the `gambit` schema and `gambit-ingest` CLI.

## Prerequisites

1. PostgreSQL with the `pg_chess` extension installed:

```sql
CREATE EXTENSION pg_chess;
```

2. Apply the gambit schema:

```powershell
$env:DATABASE_URL = "postgres://$env:USERNAME@127.0.0.1:28818/postgres"
cargo run -p gambit-ingest -- migrate --pg-uri $env:DATABASE_URL
```

Or via the migration script:

```powershell
.\schema\migrate.ps1 -PgUri $env:DATABASE_URL
```

## Import a PGN file

```powershell
cargo run -p gambit-ingest --release -- import `
  --pg-uri $env:DATABASE_URL `
  --source lichess_2024-01 `
  --workers 8 `
  --batch-games 5000 `
  tests\fixtures\pgn\multi_game.pgn
```

### Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--source` | (required) | Import batch name; creates a partition |
| `--workers` | CPU count | Parallel PGN parse threads |
| `--batch-games` | 5000 | Games per COPY flush |
| `--store-pgn` | off | Store full PGN text on each game row |
| `--fail-fast` | off | Stop on first parse error |
| `--profile` | off | Print per-step timing breakdown |
| `--eager-types` | off | Cast to `chess_position`/`chess_move` during INSERT (slower) |

By default, `pgn_text` is **not** stored (best for large Lichess dumps). Metadata, plies, and positions remain fully queryable. Use `--store-pgn` for small collections or debugging.

## Parse benchmark (no database)

```powershell
cargo run -p gambit-ingest --release -- bench-parse `
  --workers 8 `
  tests\fixtures\pgn\sample.pgn
```

## Refresh opening statistics

After ingest, rebuild the `opening_moves` materialized view:

```powershell
cargo run -p gambit-ingest -- refresh-stats --pg-uri $env:DATABASE_URL
```

## Query cookbook

### Exact position lookup by Zobrist hash

```sql
SELECT g.white, g.black, p.ply, p.fen
FROM gambit.positions p
JOIN gambit.games g ON g.id = p.game_id
WHERE p.hash = 123456789012345678;
```

### Opening explorer (from materialized stats)

```sql
SELECT move_uci, count, white_wins, black_wins, draws
FROM gambit.opening_moves
WHERE prefix_hash = (
    SELECT hash FROM gambit.positions
    WHERE fen = 'rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1'
    LIMIT 1
)
ORDER BY count DESC;
```

### Games by player

```sql
SELECT white, black, result, game_date
FROM gambit.games
WHERE white ILIKE '%Carlsen%'
ORDER BY game_date DESC
LIMIT 20;
```

## Performance tuning

- **`--workers`**: Set to physical CPU cores for parse-bound workloads.
- **`--batch-games`**: Larger batches reduce transaction overhead; 2000–10000 is typical.
- **Omit `--store-pgn`** at scale — saves significant disk and COPY time.
- **Deferred types (default)**: Bulk load stores FEN/UCI text only; `chess_position` / `chess_move` columns are backfilled after all batches, skipping expensive casts during INSERT. Use `--eager-types` for the old behavior.
- **Partition per source**: Each `--source` gets dedicated `positions_*` / `plies_*` partitions with hash indexes. The `position` btree index is created after backfill.
- **UNLOGGED staging**: COPY pipeline uses unlogged staging tables for minimal WAL during bulk load.
- **`--profile`**: Per-step timing breakdown (parse, COPY, INSERT, backfill).

## Scale notes

| Tier | Recommendation |
|------|----------------|
| Dev (10k games) | Default settings, `cargo pgrx run` PG instance |
| Production (10M+) | Dedicated PG cluster, omit `pgn_text`, BRIN on `imported_at`, read replicas for search |
| Ambitious (100M+) | Consider physical position dedup catalog (Phase 6), columnar cold storage |

## Full ingest benchmark

```powershell
.\scripts\ingest_bench.ps1
```

Reports parse and ingest throughput (games/sec, positions/sec) against a generated fixture.
