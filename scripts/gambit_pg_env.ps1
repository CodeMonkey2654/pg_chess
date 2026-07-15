# Dot-source to set Gambit Postgres env vars and put pg_dump/pg_restore on PATH.
# Usage: . .\scripts\gambit_pg_env.ps1

$PgBin = Join-Path $env:USERPROFILE ".pgrx\18.4\bin"
$PgPort = 28818
$PgUser = $env:USERNAME

if (-not (Test-Path $PgBin)) {
    throw "pgrx Postgres bin not found at $PgBin — run scripts\start_pg.ps1 first"
}

$env:DATABASE_URL = "postgres://${PgUser}@127.0.0.1:${PgPort}/postgres"
$env:PGHOST = "127.0.0.1"
$env:PGPORT = "$PgPort"
$env:PGUSER = $PgUser
$env:PGDATABASE = "postgres"

if ($env:PATH -notlike "*$PgBin*") {
    $env:PATH = "$PgBin;$env:PATH"
}
