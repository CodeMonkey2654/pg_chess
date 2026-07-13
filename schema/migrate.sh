#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${PGURI:-}" ]]; then
  echo "Usage: PGURI=postgres://... $0" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
psql "$PGURI" -v ON_ERROR_STOP=1 -f "$ROOT/schema/migrations/001_core.sql"
echo "Schema migration complete."
