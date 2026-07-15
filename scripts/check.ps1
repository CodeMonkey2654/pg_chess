$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

function Invoke-CheckStep {
    param([string]$Name, [scriptblock]$Command)
    Write-Host "==> $Name" -ForegroundColor Cyan
    & $Command
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

Invoke-CheckStep "cargo fmt --check" { cargo fmt --all --check }
Invoke-CheckStep "cargo clippy" { cargo clippy --workspace --all-targets -- -D warnings }
Invoke-CheckStep "cargo test" { cargo test --workspace }
Invoke-CheckStep "perft integration tests" { cargo test -p gambit-db --test perft -- --nocapture }

Write-Host "All checks passed." -ForegroundColor Green
