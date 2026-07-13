#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
cargo pgrx test pg18 -p pg_chess
