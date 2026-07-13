#!/usr/bin/env bash
set -euo pipefail

if [[ -z "${PGURI:-}" ]]; then
  echo "Usage: PGURI=postgres://... $0" >&2
  exit 1
fi

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
MIGRATIONS="$ROOT/schema/migrations"

echo "Applying gambit schema migrations..."
for file in "$MIGRATIONS"/*.sql; do
  echo "  -> $(basename "$file")"
  psql "$PGURI" -v ON_ERROR_STOP=1 -f "$file"
done
echo "Schema migrations complete."
