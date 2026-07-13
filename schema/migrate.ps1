$ErrorActionPreference = "Stop"

param(
    [Parameter(Mandatory = $true)]
    [string]$PgUri
)

$Root = Split-Path -Parent $PSScriptRoot
$Migration = Join-Path $PSScriptRoot "migrations\001_core.sql"

if (-not (Test-Path $Migration)) {
    throw "Migration not found: $Migration"
}

Write-Host "Applying gambit schema migration..." -ForegroundColor Cyan
psql $PgUri -v ON_ERROR_STOP=1 -f $Migration
Write-Host "Schema migration complete." -ForegroundColor Green
