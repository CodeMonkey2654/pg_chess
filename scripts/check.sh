#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

echo "==> cargo fmt --check"
cargo fmt --all --check

echo "==> cargo clippy"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> cargo test"
cargo test --workspace

echo "==> perft integration tests"
cargo test -p gambit-db --test perft -- --nocapture

echo "All checks passed."
