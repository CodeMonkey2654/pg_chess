param(
    [int]$Year = 2024,
    [string]$Source = "lichess_standard_2024"
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

Write-Host "==> Starting PostgreSQL + schema" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "start_pg.ps1")

Write-Host "==> Starting Gambit Studio API on :8080" -ForegroundColor Cyan
$env:GAMBIT_CACHE_DIR = Join-Path $PWD ".cache/lichess"
Start-Process powershell -ArgumentList @(
    "-NoExit",
    "-Command",
    "cd '$PWD'; `$env:DATABASE_URL='$env:DATABASE_URL'; `$env:GAMBIT_CACHE_DIR='$env:GAMBIT_CACHE_DIR'; cargo run -p gambit-studio-server --release"
)

Write-Host "==> Starting WASM UI on :8081 (requires trunk)" -ForegroundColor Cyan
Write-Host "Install trunk if needed: cargo install trunk" -ForegroundColor Yellow
Push-Location (Join-Path $PWD "crates\gambit-studio-ui")
try {
    trunk serve --port 8081
} finally {
    Pop-Location
}
