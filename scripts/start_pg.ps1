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

Write-Host "==> Enabling pg_chess" -ForegroundColor Cyan
& $Psql $PgUri -v ON_ERROR_STOP=1 -c "CREATE EXTENSION IF NOT EXISTS pg_chess;"

Write-Host "==> Applying gambit schema" -ForegroundColor Cyan
cargo run -p gambit-ingest --release -- migrate --pg-uri $PgUri

$env:DATABASE_URL = $PgUri
Write-Host ""
Write-Host "PostgreSQL is ready." -ForegroundColor Green
Write-Host "  DATABASE_URL=$PgUri" -ForegroundColor Cyan
Write-Host ""
Write-Host "Example import:" -ForegroundColor Yellow
Write-Host "  cargo run -p gambit-ingest --release -- import ``" -ForegroundColor Gray
Write-Host "    --pg-uri `$env:DATABASE_URL ``" -ForegroundColor Gray
Write-Host "    --source test ``" -ForegroundColor Gray
Write-Host "    tests\fixtures\pgn\sample.pgn" -ForegroundColor Gray
Write-Host ""
Write-Host "Connect with psql:" -ForegroundColor Yellow
Write-Host "  & '$Psql' `$env:DATABASE_URL" -ForegroundColor Gray
