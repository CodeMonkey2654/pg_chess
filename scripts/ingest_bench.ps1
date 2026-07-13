$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$FixtureDir = Join-Path $PWD "tests\fixtures\pgn"
$BenchFixture = Join-Path $FixtureDir "bench_10k.pgn"

if (-not (Test-Path $BenchFixture)) {
    Write-Host "Generating 10k-game benchmark fixture..." -ForegroundColor Cyan
    $Sample = Get-Content (Join-Path $FixtureDir "sample.pgn") -Raw
    $sb = New-Object System.Text.StringBuilder
    for ($i = 0; $i -lt 10000; $i++) {
        [void]$sb.AppendLine("[Event ""Bench $i""]")
        [void]$sb.AppendLine("[White ""W$i""]")
        [void]$sb.AppendLine("[Black ""B$i""]")
        [void]$sb.AppendLine("[Result ""1-0""]")
        [void]$sb.AppendLine("")
        [void]$sb.AppendLine("1. e4 e5 2. Nf3 Nc6 3. Bc4 1-0")
        [void]$sb.AppendLine("")
    }
    Set-Content -Path $BenchFixture -Value $sb.ToString() -NoNewline
}

Write-Host "==> Parse benchmark (no DB)" -ForegroundColor Cyan
$Workers = [Environment]::ProcessorCount
cargo run -p gambit-ingest --release -- bench-parse --workers $Workers $BenchFixture

if ($env:DATABASE_URL) {
    Write-Host "==> Ingest benchmark (with DB)" -ForegroundColor Cyan
    cargo run -p gambit-ingest --release -- migrate --pg-uri $env:DATABASE_URL
    $Source = "bench_" + (Get-Date -Format "yyyyMMdd_HHmmss")
    Measure-Command {
        cargo run -p gambit-ingest --release -- import `
            --pg-uri $env:DATABASE_URL `
            --source $Source `
            --workers $Workers `
            --batch-games 5000 `
            $BenchFixture
    } | ForEach-Object { Write-Host "Wall time: $($_.TotalSeconds.ToString('F2'))s" -ForegroundColor Green }
} else {
    Write-Host "Set DATABASE_URL to run full ingest benchmark against PostgreSQL." -ForegroundColor Yellow
}
