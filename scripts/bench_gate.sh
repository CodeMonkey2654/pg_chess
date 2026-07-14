#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

REGRESSION_BUDGET_PERCENT=5.0
failures=()

run_gate() {
  local package="$1"
  local bench="$2"
  local baseline_dir="$3"

  if [ ! -d "$baseline_dir" ]; then
    echo "Skipping $package: no baselines at $baseline_dir"
    return 0
  fi

  for bench_dir in "$baseline_dir"/*/; do
    bench_name="$(basename "$bench_dir")"
    bench_criterion="target/criterion/$bench_name"
    rm -rf "$bench_criterion/phase2" "$bench_criterion/base"
    cp -r "$bench_dir" "$bench_criterion/phase2"
    cp -r "$bench_dir" "$bench_criterion/base"
  done

  echo "==> cargo bench -p $package (regression gate, ${REGRESSION_BUDGET_PERCENT}% budget)"
  set +e
  output="$(cargo bench -p "$package" --bench "$bench" -- --baseline phase2 --noplot 2>&1)"
  status=$?
  set -e
  echo "$output"

  if [ "$status" -ne 0 ]; then
    echo "cargo bench -p $package failed with exit code $status" >&2
    exit "$status"
  fi

  local current_bench=""
  while IFS= read -r line; do
    if [[ "$line" =~ ^[[:space:]]*([^[:space:]]+)[[:space:]]+time: ]]; then
      current_bench="${BASH_REMATCH[1]}"
    fi
    if [[ "$line" =~ change:[[:space:]]*\[([+-][0-9.]+)% ]]; then
      lower="${BASH_REMATCH[1]}"
      if awk -v lower="$lower" -v budget="$REGRESSION_BUDGET_PERCENT" 'BEGIN { exit !(lower > budget) }'; then
        failures+=("$package/${current_bench:-unknown}: lower bound +${lower}% (budget ${REGRESSION_BUDGET_PERCENT}%)")
      fi
    fi
  done <<< "$output"
}

run_gate "gambit-db" "movegen" "crates/gambit-db/benches/baselines"
run_gate "gambit-analysis" "search" "crates/gambit-analysis/benches/baselines"

if [ "${#failures[@]}" -gt 0 ]; then
  echo "Benchmark regression exceeded budget:" >&2
  printf '  %s\n' "${failures[@]}" >&2
  exit 1
fi

echo "Benchmark gate passed."
