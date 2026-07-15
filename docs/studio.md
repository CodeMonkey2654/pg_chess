# Gambit Studio

WASM chess database browser that consumes the Gambit SDK stack: `gambit-db`, `gambit-ingest`, and PostgreSQL with `pg_chess`.

## Architecture

- **gambit-studio-ui** — Leptos WASM frontend (board replay via `gambit-db-wasm`, grpc-web client)
- **gambit-studio-server** — gRPC API on `:8080` (tonic + grpc-web for browser; query RPCs + ingest proxy)
- **gambit-ingest-worker** — gRPC ingest worker on `:8082` (Lichess sync/load, job streaming)
- **gambit-ingest** — download + ingest library; CLI `migrate`/`import` use direct Postgres, `sync-catalog`/`load-fileset` call the worker

## Prerequisites

1. PostgreSQL with `pg_chess` extension (use `scripts/start_pg.ps1`)
2. [Trunk](https://trunkrs.dev/) for WASM dev builds: `cargo install trunk`
3. WASM target: `rustup target add wasm32-unknown-unknown`

## Quick start

```powershell
# Terminal 1: PostgreSQL + schema
.\scripts\start_pg.ps1

# Terminal 2: Ingest worker
$env:DATABASE_URL = "postgres://$env:USERNAME@127.0.0.1:28818/postgres"
$env:GAMBIT_CACHE_DIR = ".cache/lichess"
cargo run -p gambit-ingest-worker --release

# Terminal 3: Studio gRPC API
$env:INGEST_ADDR = "http://127.0.0.1:8082"
cargo run -p gambit-studio-server --release

# Terminal 4: WASM UI
cd crates\gambit-studio-ui
trunk serve --port 8081
```

Open http://127.0.0.1:8081 — the UI calls the API at http://127.0.0.1:8080 via grpc-web.

**Note:** If `trunk serve` fails with a port conflict, stop any old trunk process on `:8081`. `NO_COLOR` which breaks some trunk versions — `start_studio.ps1` clears it automatically.

Or use the combined helper (starts PG, ingest worker, API, then trunk):

```powershell
.\scripts\start_studio.ps1
```

See [roadmap.md](roadmap.md) for tech debt, simplification, and feature plans.

## Loading a full Lichess year

From the UI **Dashboard**:

1. Set source name (e.g. `lichess_standard_2024`) and year (`2024`)
2. Click **Sync catalog** — registers 12 monthly shards in `gambit.filesets`
3. Click **Load full year** — downloads and ingests all shards sequentially

Or via CLI (requires ingest worker on `:8082`):

```powershell
cargo run -p gambit-ingest --release -- sync-catalog `
  --source lichess_standard_2024 `
  --year 2024

cargo run -p gambit-ingest --release -- load-fileset `
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
| Games | Player search (source-scoped), paginated list, board replay with step forward/back |
| Explorer | Opening stats at current position, games at position, jump to replay |
| Benchmarks | 14-query timed suite with descriptions |

## gRPC services

Protobuf definitions live in `crates/gambit-proto/proto/gambit/v1/`.

### StudioService (`gambit-studio-server` :8080, grpc-web for browser)

| RPC | Description |
|-----|-------------|
| `Health` | DB connectivity |
| `ListSources` | Fast source list (no counts) |
| `GetSourceSummary` | Detailed counts for one source |
| `ListFilesets` | Shard status grid |
| `SearchGames` | Paginated game search |
| `GetGame` | Game detail + plies + start FEN |
| `GamesByPosition` | Paginated games at position |
| `HashFromFen` | FEN → Zobrist hash |
| `LookupPosition` | Position lookup hits |
| `OpeningStats` | Opening move stats |
| `RunBench` | Run benchmark suite |
| `SyncCatalog` | Sync Lichess catalog (proxied to worker) |
| `LoadYear` | Background full-year ingest (proxied) |
| `GetJob` / `GetActiveJob` | Job status (proxied) |
| `WatchJob` | Server-streaming job progress (proxied) |

### IngestService (`gambit-ingest-worker` :8082)

| RPC | Description |
|-----|-------------|
| `SyncCatalog` | Register Lichess shards in `gambit.filesets` |
| `LoadYear` | Download + ingest all shards for a year |
| `LoadFileset` | CLI full-year or single-shard load |
| `GetJob` / `GetActiveJob` | Job status |
| `WatchJob` | Server-streaming progress updates |

## Performance notes

- Ingest uses the fast path: no `pgn_text`, deferred chess types, COPY pipeline
- Roadmap target: ≥100,000 games/min (shown on dashboard)
- Full 2024 year (~4M+ games) requires substantial disk for DB and cache; monitor shard progress in the UI

## Attribution

Chess games from [database.lichess.org](https://database.lichess.org/) — use and share results per Lichess database terms.
