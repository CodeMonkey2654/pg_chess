$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
Write-Host "Running cargo pgrx test for pg_chess..." -ForegroundColor Cyan
cargo pgrx test pg18 -p pg_chess
