#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

BASELINE_DIR="crates/gambit-db/benches/baselines"
CRITERION_DIR="target/criterion"
REGRESSION_BUDGET_PERCENT=5.0

for bench_dir in "$BASELINE_DIR"/*/; do
  bench_name="$(basename "$bench_dir")"
  bench_criterion="$CRITERION_DIR/$bench_name"
  rm -rf "$bench_criterion/phase2" "$bench_criterion/base"
  cp -r "$bench_dir" "$bench_criterion/phase2"
  cp -r "$bench_dir" "$bench_criterion/base"
done

echo "==> cargo bench (regression gate, ${REGRESSION_BUDGET_PERCENT}% budget)"
set +e
output="$(cargo bench -p gambit-db --bench movegen -- --baseline phase2 --noplot 2>&1)"
status=$?
set -e
echo "$output"

if [ "$status" -ne 0 ]; then
  echo "cargo bench failed with exit code $status" >&2
  exit "$status"
fi

current_bench=""
failures=()
while IFS= read -r line; do
  if [[ "$line" =~ ^[[:space:]]*([^[:space:]]+)[[:space:]]+time: ]]; then
    current_bench="${BASH_REMATCH[1]}"
  fi
  if [[ "$line" =~ change:[[:space:]]*\[([+-][0-9.]+)% ]]; then
    lower="${BASH_REMATCH[1]}"
    if awk -v lower="$lower" -v budget="$REGRESSION_BUDGET_PERCENT" 'BEGIN { exit !(lower > budget) }'; then
      failures+=("${current_bench:-unknown}: lower bound +${lower}% (budget ${REGRESSION_BUDGET_PERCENT}%)")
    fi
  fi
done <<< "$output"

if [ "${#failures[@]}" -gt 0 ]; then
  echo "Benchmark regression exceeded budget:" >&2
  printf '  %s\n' "${failures[@]}" >&2
  exit 1
fi

echo "Benchmark gate passed."
