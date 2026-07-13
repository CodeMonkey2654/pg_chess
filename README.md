# pg_chess / gambit-db

Chess positions, moves, and games for PostgreSQL — powered by the **gambit-db** Rust engine.

## Workspace

| Crate | Role |
|-------|------|
| [`gambit-db`](crates/gambit-db/) | Core chess engine (`cargo add gambit-db`) |
| [`gambit-ingest`](crates/gambit-ingest/) | Bulk PGN ingest CLI (`gambit-ingest import`) |
| [`pg_chess`](crates/pg_chess/) | PostgreSQL extension (`CREATE EXTENSION pg_chess`) |

SQL types and functions use chess-native naming: `chess_position`, `chess_to_fen()`, `chess_legal_moves()`, etc.

## Quick start (PostgreSQL)

```bash
cargo install --locked cargo-pgrx
cargo pgrx init --pg18 download
cargo pgrx run pg18 -p pg_chess
```

```sql
SELECT chess_to_fen(chess_start_position());
SELECT chess_to_fen(chess_apply_move(chess_start_position(), 'e2e4'));
SELECT count(*) FROM chess_legal_moves(chess_start_position());
```

## Development

```powershell
# Start PostgreSQL + schema (sets $env:DATABASE_URL)
.\scripts\start_pg.ps1

# Full quality gates (fmt, clippy, tests, perft)
.\scripts\check.ps1

# Individual tasks
.\scripts\perft.ps1
.\scripts\bench.ps1
.\scripts\pgrx_test.ps1
.\scripts\ingest_bench.ps1
```

```bash
# Linux / CI
bash scripts/check.sh
```

### Rust library only

```bash
cargo test -p gambit-db
cargo test -p gambit-db --test perft
cargo bench -p gambit-db
```

## Status

Production foundation: workspace split, perft correctness suite, criterion benchmarks with CI regression gates, SAN/PGN, strict FEN, bulk ingest (`gambit-ingest`), Python bindings (`gambit-py`), and UCI wrapper (`gambit-uci`). See `docs/` for architecture, SQL API, [ingest guide](docs/ingest.md), and the [roadmap](roadmap.md).

### Benchmark regression budget

After performance changes, run `.\scripts\bench_gate.ps1` (or `bash scripts/bench_gate.sh`). CI fails if any benchmark's lower-bound change vs the committed `phase2` baseline exceeds 5%.
