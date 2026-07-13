#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
FIXTURE="$ROOT/tests/fixtures/pgn/bench_10k.pgn"
WORKERS="${WORKERS:-$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)}"

if [[ ! -f "$FIXTURE" ]]; then
  echo "Generating 10k-game benchmark fixture..."
  python3 - <<'PY' "$FIXTURE"
import pathlib, sys
out = pathlib.Path(sys.argv[1])
body = []
for i in range(10000):
    body.append(f'[Event "Bench {i}"]\n[White "W{i}"]\n[Black "B{i}"]\n[Result "1-0"]\n\n1. e4 e5 2. Nf3 Nc6 3. Bc4 1-0\n')
out.write_text("\n".join(body))
PY
fi

echo "==> Parse benchmark (no DB)"
cargo run -p gambit-ingest --release -- bench-parse --workers "$WORKERS" "$FIXTURE"

if [[ -n "${DATABASE_URL:-}" ]]; then
  echo "==> Ingest benchmark (with DB)"
  cargo run -p gambit-ingest --release -- migrate --pg-uri "$DATABASE_URL"
  SOURCE="bench_$(date +%Y%m%d_%H%M%S)"
  time cargo run -p gambit-ingest --release -- import \
    --pg-uri "$DATABASE_URL" \
    --source "$SOURCE" \
    --workers "$WORKERS" \
    --batch-games 5000 \
    "$FIXTURE"
else
  echo "Set DATABASE_URL to run full ingest benchmark against PostgreSQL."
fi
