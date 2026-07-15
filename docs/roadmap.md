# Gambit roadmap

Living plan for tech debt, simplification, features, and how this stack generalizes beyond chess.

**Stack status (local):** run `.\scripts\start_studio.ps1` then open http://127.0.0.1:8081. Postgres `:28818`, API `:8080`, ingest worker `:8082`.

---

## Where we are

The hybrid analysis pipeline is end-to-end for a single game:

- **Extension** — `pg_chess` analysis types + SQL functions (`chess_move_class`, `chess_classify_cp_loss`, …)
- **Schema** — `005_analysis.sql` / `006_staging_analysis_text.sql` columns on `plies`/`games`, staging merge
- **Compute** — `gambit-ingest analyze-game` with native GambitEvaluator (corpus + Syzygy + search)
- **Studio** — eval bar, move colors, accuracy, Analyze button; gRPC `AnalyzeGame`

Ingest, explorer, and Lichess shard loading were already working. Analysis batch jobs, `position_evals` cache, Syzygy, and e2e CI are not.

---

## 1. Tech debt (fix first)


### Analysis write path

Ingest and analysis both use COPY → staging → merge. Batch analyze runs parallel engine workers (`buffer_unordered`) with bulk COPY flush.

### Ingest parallelism

| Layer | Mechanism |
|-------|-----------|
| Shard stream parse | Rayon batches (`workers × 64` chunks) via `parse_chunks_parallel` |
| COPY format | Rayon parallel row formatting in `db/copy.rs` |
| Multi-shard year load | `shard_concurrency` (default `min(cpus, 4)`) with one DB session per shard |
| Sequential shard load | Background prefetch of next shard download |

**Tunable:** `ImportOptions.workers`, `batch_games`, `shard_concurrency`.

### Dead / unwired code

| Item | Location |
|------|----------|
| `DownloadPool` never wired | `gambit-ingest/src/lichess/download_pool.rs` |
| `position_evals` + `upsert_position_eval` unused | `schema/migrations/005_analysis.sql` |
| `EvalSource::Syzygy` unused | `gambit-analysis/src/evaluator.rs` |
| Duplicate progress helpers | `gambit-studio-server/src/progress.rs`, `gambit-ingest-worker/src/progress.rs` |
| `AnalyzeGame` blocks gRPC thread | `gambit-studio-server/src/grpc.rs` |

### Jobs & worker state

Ingest jobs live in memory (`gambit-ingest-worker/src/jobs.rs`). Restart loses progress (partial recovery from `gambit.filesets` only).

**Next steps:** persist job rows in `gambit.ingest_runs` or a dedicated `gambit.jobs` table; unify analysis jobs under the same model.

### CI & testing gaps

| Gap | Notes |
|-----|-------|
| Playwright e2e not in CI | `tests/e2e/` exists, not wired in `.github/workflows/ci.yml` |
| `api_integration` soft-skips | `gambit-studio-server/tests/api_integration.rs` can pass with no DB |
| No gambit migration test in postgres job | `cargo pgrx test` only covers extension |
| No ingest throughput gate | `scripts/ingest_gate.ps1` exits 0 when `DATABASE_URL` unset |
| No `trunk build` in CI | wasm `cargo check` only |

### Dev ergonomics

- `start_pg.ps1` hardcodes `~/.pgrx/18.4/bin/...` paths.
- Trunk fails when `NO_COLOR` is set (Cursor terminals); `start_studio.ps1` now clears it and binds `127.0.0.1` only (corp DNS suffixes caused port conflicts).
- `gambit-studio-ui/dist/` should not be committed — use trunk build in deploy only.

---

## 2. Simplification opportunities

### Fewer moving parts

Today: **4 processes** (Postgres, ingest-worker, studio-server, trunk) plus **2 ingest entry points** (direct Postgres CLI vs gRPC worker).

| Option | Trade-off |
|--------|-----------|
| **A. Fold worker into studio-server** | One API process; simpler local dev; larger binary |
| **B. Keep worker, drop studio proxy** | UI talks to worker for ingest, server for queries; two grpc-web origins |
| **C. Status quo** | Clear read/write split; more terminals |

Recommendation: **A for local/small deploy**, keep worker crate as a library module inside studio-server. CLI keeps direct Postgres for `migrate`/`import`/`analyze-*`.

### Unify shard loading

Full-year loop is duplicated in `gambit-ingest/src/lib.rs` and `gambit-ingest-worker/src/load_job.rs`.

**Next step:** single `load_fileset_year()` in `gambit-ingest`, called by worker and CLI.

### Single enum source of truth

Rust `gambit-analysis::classify` → `pg_chess` SQL wrappers → staging `text` + cast on merge. Staging text is correct for COPY; duplication is in bootstrap SQL and migration history.

**Next step:** generate extension SQL from a shared const table or test that asserts Rust labels == PG enum labels.

### UI stack

Custom grpc-web encoder (`gambit-studio-ui/src/grpc_web.rs`) and local FEN board parser (`board/fen.rs`) alongside `gambit-db-wasm`.

**Next step:** use `gambit-db-wasm` for all board state; evaluate `tonic-web` or `grpc-web` generated client to drop hand-rolled proto framing.

### Crate consolidation (optional)

| Thin crate | Could merge into |
|------------|------------------|
| `gambit-ingest-worker` | `gambit-studio-server` (ingest gRPC module) |
| `gambit-db-wasm` | stay — WASM boundary is real |
| `gambit-proto` | stay — shared contracts |

---

## 3. Features to bring

### Analysis (near term)

| Feature | Effort | Notes |
|---------|--------|-------|
| **Batch analyze UI** | M | Wire `analyze-batch` as background job; poll like ingest |
| **Analysis job streaming** | M | Reuse `WatchJob` proto; move off blocking `AnalyzeGame` |
| **Best-move overlay** | S | Data already in `plies.best_move` |
| **Eval graph** | M | Plot `eval_after` by ply in replay |
| **Engine/depth picker** | S | Pass options to analyze RPC |
| **COPY staging for analysis** | S | Throughput for batch |

### Explorer & cache (medium term)

| Feature | Effort | Notes |
|---------|--------|-------|
| **`position_evals` read path** | Done | `GetPositionEval` RPC + cache upsert on miss |
| **Live position analyze button** | M | Explorer + replay share cache |
| **Syzygy in `GambitEvaluator`** | M | `gambit-db/tablebase` feature; endgame routing |
| **Corpus book in Explorer** | S | Surface `opening_moves` + `.gbook` export in UI |

### Ingest scale (medium term)

| Feature | Effort | Notes |
|---------|--------|-------|
| **Wire `DownloadPool`** | M | Prefetch next shard while ingesting current |
| **Parallel shard ingest** | L | Multiple workers, partition by `source_id` |
| **Real ingest gate in CI** | M | `ingest_gate.ps1` with fixture + min games/min |
| **Position dedup** | L | Phase 6 in `docs/ingest.md` |

### Native engine (long term)

LMR, null-move, SEE, tapered eval — native search in `gambit-analysis`. Corpus + Syzygy routing in `GambitEvaluator` (`gambit-ingest/src/analyze/gambit_eval.rs`).

### Quality & ops

| Feature | Effort |
|---------|--------|
| Playwright e2e in CI (dashboard, replay, analyze) | S |
| Read/write pool split for Postgres | M |
| Incremental opening stats refresh | M |
| DB-backed job history | M |

---

## 4. How this generalizes

Gambit is a template for **typed domain data in Postgres + bulk warehouse + interactive studio**, not only chess.

### Layered pattern

```
┌──────────────────────────────────────────────────────────────┐
│  Domain extension (pg_chess)                                 │
│  Opaque types + immutable parallel-safe functions            │
│  chess_position, chess_move_class, chess_classify_cp_loss()  │
└────────────────────────────┬─────────────────────────────────┘
                             │ types referenced by
┌────────────────────────────▼─────────────────────────────────┐
│  App schema (schema/migrations/)                             │
│  gambit.* facts, partitions, rollups, filesets, staging      │
└────────────────────────────┬─────────────────────────────────┘
                             │
       ┌─────────────────────┼─────────────────────┐
       ▼                     ▼                     ▼
  Bulk ingest           Staging merge         Online API + UI
  COPY → staging_*      merge_*, rollup_*     gRPC + WASM
       │                     │
       └──────────┬──────────┘
                  ▼
        Offline compute → hydrate caches
        (stats MV, .gbook, position_evals, batch analysis)
        Hot path stays in RAM — never per-node SQL in search
```

## Suggested execution order

### Phase A — Stabilize (next)

- [ ] Analysis staging COPY
- [ ] CI: e2e smoke, gambit migrations in postgres job
- [ ] Fold ingest-worker into studio-server (optional simplification)

### Phase B — Simplify (1 week)

- [ ] Single `load_fileset_year` implementation
- [ ] Fold ingest-worker into studio-server (or document why not)
- [ ] Deduplicate progress/format helpers

### Phase C — Complete analysis loop (2–3 weeks)

- [ ] Background analysis jobs + UI batch trigger
- [ ] `position_evals` cache read/write
- [ ] Explorer live analyze
- [ ] Syzygy routing in endgames
- [ ] Best-move overlay + eval graph in replay

### Phase D — Scale ingest (ongoing)

- [ ] Wire `DownloadPool`
- [ ] Parallel shard workers
- [ ] Automated ingest gate (100k games/min target)
- [ ] Incremental opening stats
---

## Quick reference

| Doc | Topic |
|-----|-------|
| [architecture.md](architecture.md) | Crate map |
| [ingest.md](ingest.md) | Bulk load, scale tiers |
| [analysis.md](analysis.md) | Engine + Postgres hybrid |
| [studio.md](studio.md) | Local dev, UI pages, gRPC |
| [data-types.md](data-types.md) | SQL types |

| Script | Purpose |
|--------|---------|
| `scripts/start_pg.ps1` | Postgres + extension + migrations |
| `scripts/start_studio.ps1` | Full local stack |
| `scripts/ingest_gate.ps1` | Throughput baseline (manual today) |
| `scripts/check.ps1` | fmt + clippy + test |
