param(
    [int]$GameCount = 50000,
    [int]$Workers = 0,
    [int]$BatchGames = 5000,
    [switch]$Profile
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if ($Workers -le 0) {
    $Workers = [Environment]::ProcessorCount
}

$PgUser = $env:USERNAME
$PgPort = 28818
$env:DATABASE_URL = "postgres://${PgUser}@127.0.0.1:${PgPort}/postgres"
$Psql = Join-Path $env:USERPROFILE ".pgrx\18.4\bin\psql.exe"

$FixtureDir = Join-Path $PWD "tests\fixtures\pgn"
$BenchFixture = Join-Path $FixtureDir "bench_${GameCount}.pgn"

function Write-Metric($Label, $Value) {
    Write-Host ("  {0,-22} {1}" -f $Label, $Value)
}

Write-Host "========================================" -ForegroundColor Cyan
Write-Host " Gambit Ingest Load Test" -ForegroundColor Cyan
Write-Host "========================================" -ForegroundColor Cyan
Write-Host ""
Write-Metric "Target games" $GameCount
Write-Metric "Workers" $Workers
Write-Metric "Batch size" $BatchGames
Write-Metric "Profile" $Profile.IsPresent
Write-Metric "DATABASE_URL" $env:DATABASE_URL
Write-Host ""

# Ensure PG is running
$pgStatus = cargo pgrx status pg18 2>&1 | Out-String
if ($pgStatus -notmatch "running") {
    Write-Host "Starting PostgreSQL..." -ForegroundColor Yellow
    cargo pgrx start pg18
}

# Generate fixture if missing
if (-not (Test-Path $BenchFixture)) {
    Write-Host "Generating $GameCount-game fixture (~$([math]::Round($GameCount * 0.00015, 1)) MB estimated)..." -ForegroundColor Cyan
    $genStart = Get-Date
    $sw = [System.IO.StreamWriter]::new($BenchFixture, $false, [System.Text.UTF8Encoding]::new($false))
    $sw.NewLine = "`n"
    for ($i = 0; $i -lt $GameCount; $i++) {
        $sw.WriteLine("[Event ""Bench $i""]")
        $sw.WriteLine("[White ""W$i""]")
        $sw.WriteLine("[Black ""B$i""]")
        $sw.WriteLine("[Result ""1-0""]")
        $sw.WriteLine("")
        $sw.WriteLine("1. e4 e5 2. Nf3 Nc6 3. Bc4 1-0")
        $sw.WriteLine("")
    }
    $sw.Close()
    $genElapsed = (Get-Date) - $genStart
    $sizeMb = (Get-Item $BenchFixture).Length / 1MB
    Write-Host "Fixture written: $([math]::Round($sizeMb, 2)) MB in $($genElapsed.TotalSeconds.ToString('F1'))s" -ForegroundColor Green
} else {
    $sizeMb = (Get-Item $BenchFixture).Length / 1MB
    Write-Host "Using existing fixture: $BenchFixture ($([math]::Round($sizeMb, 2)) MB)" -ForegroundColor Green
}

Write-Host ""
Write-Host "==> Phase 1: Parse benchmark (CPU only, no DB)" -ForegroundColor Cyan
$parseStart = Get-Date
cargo run -p gambit-ingest --release -- bench-parse --workers $Workers $BenchFixture
$parseElapsed = (Get-Date) - $parseStart

Write-Host ""
Write-Host "==> Phase 2: Schema + DB ingest" -ForegroundColor Cyan
$prevEap = $ErrorActionPreference
$ErrorActionPreference = "Continue"
cargo run -p gambit-ingest --release -- migrate --pg-uri $env:DATABASE_URL 2>&1 | Out-Null

$Source = "loadtest_" + (Get-Date -Format "yyyyMMdd_HHmmss")
$ingestStart = Get-Date
$importArgs = @(
    "run", "-p", "gambit-ingest", "--release", "--", "import",
    "--pg-uri", $env:DATABASE_URL,
    "--source", $Source,
    "--workers", $Workers,
    "--batch-games", $BatchGames
)
if ($Profile) { $importArgs += "--profile" }
$importArgs += $BenchFixture
cargo @importArgs
if ($LASTEXITCODE -ne 0) { throw "Ingest failed with exit code $LASTEXITCODE" }
$ErrorActionPreference = $prevEap
$ingestElapsed = (Get-Date) - $ingestStart

Write-Host ""
Write-Host "==> Phase 3: Post-load DB metrics" -ForegroundColor Cyan
$dbGames = & $Psql $env:DATABASE_URL -t -A -c "SELECT count(*) FROM gambit.games WHERE source_id = (SELECT id FROM gambit.sources WHERE name = '$Source');"
$dbPositions = & $Psql $env:DATABASE_URL -t -A -c "SELECT count(*) FROM gambit.positions WHERE source_id = (SELECT id FROM gambit.sources WHERE name = '$Source');"
$dbPlies = & $Psql $env:DATABASE_URL -t -A -c "SELECT count(*) FROM gambit.plies WHERE source_id = (SELECT id FROM gambit.sources WHERE name = '$Source');"
$dbSize = & $Psql $env:DATABASE_URL -t -A -c "SELECT pg_size_pretty(pg_total_relation_size('gambit.positions'));"
$positionsPerGame = if ([int]$dbGames -gt 0) { [math]::Round([int]$dbPositions / [int]$dbGames, 1) } else { 0 }

$ingestSecs = $ingestElapsed.TotalSeconds
$positionsPerSec = if ($ingestSecs -gt 0) { [math]::Round([int]$dbPositions / $ingestSecs, 0) } else { 0 }
$gamesPerMin = if ($ingestSecs -gt 0) { [math]::Round([int]$dbGames / $ingestSecs * 60, 0) } else { 0 }

Write-Host ""
Write-Host "========================================" -ForegroundColor Green
Write-Host " Load Test Results ($Source)" -ForegroundColor Green
Write-Host "========================================" -ForegroundColor Green
Write-Host ""
Write-Host "Database counts:" -ForegroundColor Yellow
Write-Metric "Games in DB" $dbGames
Write-Metric "Positions in DB" $dbPositions
Write-Metric "Plies in DB" $dbPlies
Write-Metric "Positions / game" $positionsPerGame
Write-Metric "positions table size" $dbSize
Write-Host ""
Write-Host "Timings:" -ForegroundColor Yellow
Write-Metric "Parse phase" ("{0:F2}s" -f $parseElapsed.TotalSeconds)
Write-Metric "Ingest phase" ("{0:F2}s" -f $ingestSecs)
Write-Metric "Total wall time" ("{0:F2}s" -f ($parseElapsed.TotalSeconds + $ingestSecs))
Write-Host ""
Write-Host "Ingest throughput:" -ForegroundColor Yellow
Write-Metric "Games/min" $gamesPerMin
Write-Metric "Positions/sec" $positionsPerSec
Write-Metric "Plies/sec" $(if ($ingestSecs -gt 0) { [math]::Round([int]$dbPlies / $ingestSecs, 0) } else { 0 })
Write-Host ""
Write-Host "Roadmap target: >=100,000 games/min" -ForegroundColor DarkGray
Write-Host ""
