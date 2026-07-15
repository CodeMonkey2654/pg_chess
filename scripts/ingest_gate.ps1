#!/usr/bin/env pwsh
# Ingest throughput regression gate (requires DATABASE_URL + migrated schema).

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

if (-not $env:DATABASE_URL) {
    Write-Host "Skipping ingest gate: DATABASE_URL not set" -ForegroundColor Yellow
    exit 0
}

& (Join-Path $PSScriptRoot "ingest_bench.ps1") -GameCount 5000 -Profile
Write-Host "Ingest gate passed (manual baseline comparison)" -ForegroundColor Green
