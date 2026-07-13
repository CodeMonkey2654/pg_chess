$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

Write-Host "==> cargo fmt --check" -ForegroundColor Cyan
cargo fmt --all --check

Write-Host "==> cargo clippy" -ForegroundColor Cyan
cargo clippy --workspace --all-targets -- -D warnings

Write-Host "==> cargo test" -ForegroundColor Cyan
cargo test --workspace

Write-Host "==> perft integration tests" -ForegroundColor Cyan
cargo test -p gambit-db --test perft -- --nocapture

Write-Host "All checks passed." -ForegroundColor Green
