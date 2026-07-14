# Gambit Studio

WASM chess database browser that consumes the Gambit SDK stack: `gambit-db`, `gambit-ingest`, and PostgreSQL with `pg_chess`.

## Architecture

- **gambit-studio-ui** — Leptos WASM frontend (board replay via `gambit-db-wasm`, REST client)
- **gambit-studio-server** — Axum API (queries, benchmarks, background Lichess ingest jobs)
- **gambit-ingest** — download + ingest Lichess `.pgn.zst` shards with per-file tracking

## Prerequisites

1. PostgreSQL with `pg_chess` extension (use `scripts/start_pg.ps1`)
2. [Trunk](https://trunkrs.dev/) for WASM dev builds: `cargo install trunk`
3. WASM target: `rustup target add wasm32-unknown-unknown`

## Quick start

```powershell
# Terminal 1: PostgreSQL + schema
.\scripts\start_pg.ps1

# Terminal 2: API server
$env:DATABASE_URL = "postgres://$env:USERNAME@127.0.0.1:28818/postgres"
$env:GAMBIT_CACHE_DIR = ".cache/lichess"
cargo run -p gambit-studio-server --release

# Terminal 3: WASM UI
cd crates\gambit-studio-ui
trunk serve --port 8081
```

Open http://127.0.0.1:8081 — the UI calls the API at http://127.0.0.1:8080.

Or use the combined helper (starts PG, then API in a new window, then trunk):

```powershell
.\scripts\start_studio.ps1
```

## Loading a full Lichess year

From the UI **Dashboard**:

1. Set source name (e.g. `lichess_standard_2024`) and year (`2024`)
2. Click **Sync catalog** — registers 12 monthly shards in `gambit.filesets`
3. Click **Load full year** — downloads and ingests all shards sequentially

Or via CLI:

```powershell
cargo run -p gambit-ingest --release -- sync-catalog `
  --pg-uri $env:DATABASE_URL `
  --source lichess_standard_2024 `
  --year 2024

cargo run -p gambit-ingest --release -- load-fileset `
  --pg-uri $env:DATABASE_URL `
  --source lichess_standard_2024 `
  --year 2024 `
  --cache-dir .cache/lichess
```

## Tracking tables

Migration `003_fileset_tracking.sql` adds:

| Table | Purpose |
|-------|---------|
| `gambit.filesets` | One row per Lichess shard: URL, status, download/ingest timestamps, game counts |
| `gambit.ingest_runs` | Per-shard performance metrics (games/min, positions/sec) |

Game rows also store `pgn_sha256` and `pgn_byte_offset` linking back to their source shard.

## Data source

Monthly rated standard games from the [Lichess open database](https://database.lichess.org/). Each month is one `.pgn.zst` file (~30GB compressed for recent months). Cached downloads land in `GAMBIT_CACHE_DIR` (default `.cache/lichess`).

## UI pages

| Page | Features |
|------|----------|
| Dashboard | Source stats, 12-shard grid, sync/load controls, job progress, games/min |
| Games | Player search, game list, visual board with move animations, interactive legal moves, FEN replay via WASM engine |
| Benchmarks | Timed query suite (game count, player search, position hash, opening stats) |

## API endpoints

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/health` | DB connectivity |
| GET | `/api/sources` | List sources with counts |
| GET | `/api/filesets?source_id=` | Shard status grid |
| POST | `/api/filesets/sync` | Sync Lichess catalog for a year |
| POST | `/api/filesets/load-year` | Background full-year ingest |
| GET | `/api/jobs/{id}` | Poll ingest job progress |
| GET | `/api/games` | Search games |
| GET | `/api/games/{id}` | Game detail + plies |
| POST | `/api/bench/queries` | Run benchmark suite |

## Performance notes

- Ingest uses the fast path: no `pgn_text`, deferred chess types, COPY pipeline
- Roadmap target: ≥100,000 games/min (shown on dashboard)
- Full 2024 year (~4M+ games) requires substantial disk for DB and cache; monitor shard progress in the UI

## Attribution

Chess games from [database.lichess.org](https://database.lichess.org/) — use and share results per Lichess database terms.
