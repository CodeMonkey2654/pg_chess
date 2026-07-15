param(
    [int]$Year = 2024,
    [string]$Source = "lichess_standard_2024",
    [int]$Workers = 0,
    [int]$BatchGames = 5000,
    [int]$ShardConcurrency = 0
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$Root = $PWD
$LogDir = Join-Path $Root ".logs"
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

if ($Workers -le 0) {
    $Workers = [Environment]::ProcessorCount
}
if ($ShardConcurrency -le 0) {
    $ShardConcurrency = 1
}

Write-Host "==> Stopping existing Gambit processes" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "stop_gambit.ps1")

Write-Host "==> Resetting PostgreSQL + gambit schema" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "start_pg.ps1") -Reset

. (Join-Path $PSScriptRoot "gambit_pg_env.ps1")
$CacheDir = Join-Path $Root ".cache\lichess"
New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null

Write-Host "==> Building release binaries" -ForegroundColor Cyan
cargo build --release -p gambit-ingest -p gambit-ingest-worker 2>&1 | Out-Null

$WorkerExe = Join-Path $Root "target\release\gambit-ingest-worker.exe"
$IngestExe = Join-Path $Root "target\release\gambit-ingest.exe"

$workerOut = Join-Path $LogDir "overnight-worker.out.log"
$workerErr = Join-Path $LogDir "overnight-worker.err.log"
$ingestOut = Join-Path $LogDir "overnight-ingest.out.log"
$ingestErr = Join-Path $LogDir "overnight-ingest.err.log"

Write-Host "==> Starting ingest worker on :8082" -ForegroundColor Cyan
$workerCmd = @"
Set-Location '$Root'
`$env:DATABASE_URL = '$env:DATABASE_URL'
`$env:GAMBIT_CACHE_DIR = '$CacheDir'
`$env:INGEST_ADDR = '127.0.0.1:8082'
`$env:RUST_LOG = 'info'
& '$WorkerExe' *> '$workerOut' 2> '$workerErr'
"@
Start-Process powershell -WindowStyle Hidden -ArgumentList @("-NoProfile", "-Command", $workerCmd) | Out-Null

Write-Host "  waiting for worker..." -ForegroundColor Gray
$ready = $false
for ($i = 0; $i -lt 30; $i++) {
    try {
        $tcp = Get-NetTCPConnection -LocalPort 8082 -State Listen -ErrorAction Stop
        if ($tcp) { $ready = $true; break }
    } catch {}
    Start-Sleep -Seconds 1
}
if (-not $ready) {
    throw "ingest worker did not start — see $workerErr"
}

Write-Host "==> Starting full-year ingest ($Source $Year)" -ForegroundColor Cyan
Write-Host "  workers=$Workers shard_concurrency=$ShardConcurrency batch_games=$BatchGames" -ForegroundColor Gray
Write-Host "  logs: $ingestOut" -ForegroundColor Gray

$ingestCmd = @"
Set-Location '$Root'
`$env:DATABASE_URL = '$env:DATABASE_URL'
`$env:GAMBIT_CACHE_DIR = '$CacheDir'
`$env:INGEST_ADDR = '127.0.0.1:8082'
`$env:RUST_LOG = 'info'
& '$IngestExe' load-fileset `
  --ingest-addr http://127.0.0.1:8082 `
  --source '$Source' `
  --year $Year `
  --cache-dir '$CacheDir' `
  --workers $Workers `
  --batch-games $BatchGames `
  *> '$ingestOut' 2> '$ingestErr'
"@

Start-Process powershell -WindowStyle Hidden -ArgumentList @("-NoProfile", "-Command", $ingestCmd) | Out-Null

Write-Host ""
Write-Host "Overnight ingest is running in the background." -ForegroundColor Green
Write-Host "  Worker log: $workerOut" -ForegroundColor Cyan
Write-Host "  Ingest log: $ingestOut" -ForegroundColor Cyan
Write-Host "  Cache:      $CacheDir" -ForegroundColor Cyan
Write-Host ""
Write-Host "Monitor progress:" -ForegroundColor Yellow
Write-Host "  Get-Content '$ingestOut' -Tail 20 -Wait" -ForegroundColor Gray
Write-Host "  psql `$env:DATABASE_URL -c `"SELECT period_label, status, games_loaded FROM gambit.filesets ORDER BY period_label`"" -ForegroundColor Gray
Write-Host ""
Write-Host "Backup when done:" -ForegroundColor Yellow
Write-Host "  .\scripts\pg_dump.ps1" -ForegroundColor Gray
