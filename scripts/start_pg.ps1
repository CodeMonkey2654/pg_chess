param(
    [switch]$Reset
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$PgVersion = "pg18"
$PgPort = 28818
$PgUser = $env:USERNAME
$PgUri = "postgres://${PgUser}@127.0.0.1:${PgPort}/postgres"
$PgConfig = Join-Path $env:USERPROFILE ".pgrx\18.4\bin\pg_config.exe"
$Psql = Join-Path $env:USERPROFILE ".pgrx\18.4\bin\psql.exe"

Write-Host "==> Starting pgrx PostgreSQL ($PgVersion on port $PgPort)" -ForegroundColor Cyan
cargo pgrx start $PgVersion

Write-Host "==> Installing pg_chess extension" -ForegroundColor Cyan
cargo pgrx install -p pg_chess --pg-config $PgConfig --release

if ($Reset) {
    Write-Host "==> Resetting gambit schema + pg_chess (v0.1.0 dev)" -ForegroundColor Yellow
    & $Psql $PgUri -v ON_ERROR_STOP=1 -c @"
DROP SCHEMA IF EXISTS gambit CASCADE;
DROP EXTENSION IF EXISTS pg_chess CASCADE;
DROP TYPE IF EXISTS chess_move_class CASCADE;
DROP TYPE IF EXISTS chess_eval_source CASCADE;
DROP TYPE IF EXISTS chess_analysis_status CASCADE;
DROP TYPE IF EXISTS chess_position CASCADE;
DROP TYPE IF EXISTS chess_move CASCADE;
DROP FUNCTION IF EXISTS chess_accuracy_from_classes(text[]) CASCADE;
DROP FUNCTION IF EXISTS chess_classify_cp_loss(integer) CASCADE;
DROP FUNCTION IF EXISTS chess_eval_to_cp(integer, integer) CASCADE;
"@
}

Write-Host "==> Enabling pg_chess" -ForegroundColor Cyan
& $Psql $PgUri -v ON_ERROR_STOP=1 -c "CREATE EXTENSION IF NOT EXISTS pg_chess;"

# Reinstalling the DLL does not refresh extension SQL on existing clusters.
# Dev-only: ensure analysis C functions match the installed library.
Write-Host "==> Syncing pg_chess analysis functions" -ForegroundColor Cyan
& $Psql $PgUri -v ON_ERROR_STOP=1 -c @"
CREATE OR REPLACE FUNCTION chess_classify_cp_loss(cp_loss INT DEFAULT 0)
RETURNS TEXT IMMUTABLE STRICT PARALLEL SAFE LANGUAGE c
AS 'pg_chess', 'chess_classify_cp_loss_wrapper';

CREATE OR REPLACE FUNCTION chess_accuracy_from_classes(classes TEXT[])
RETURNS real IMMUTABLE STRICT PARALLEL SAFE LANGUAGE c
AS 'pg_chess', 'chess_accuracy_from_classes_wrapper';

CREATE OR REPLACE FUNCTION chess_eval_to_cp(cp INT, mate_plies INT)
RETURNS INT IMMUTABLE PARALLEL SAFE LANGUAGE c
AS 'pg_chess', 'chess_eval_to_cp_wrapper';
"@

Write-Host "==> Applying gambit schema" -ForegroundColor Cyan
cargo run -p gambit-ingest --release -- migrate --pg-uri $PgUri

$env:DATABASE_URL = $PgUri
Write-Host ""
Write-Host "PostgreSQL is ready." -ForegroundColor Green
Write-Host "  DATABASE_URL=$PgUri" -ForegroundColor Cyan
if (-not $Reset) {
    Write-Host "  Tip: use -Reset for a clean v0.1.0 schema (drops gambit + pg_chess)" -ForegroundColor Gray
}
