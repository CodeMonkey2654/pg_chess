$ErrorActionPreference = "Stop"

param(
    [Parameter(Mandatory = $true)]
    [string]$PgUri
)

$MigrationsDir = Join-Path $PSScriptRoot "migrations"
$MigrationFiles = Get-ChildItem -Path $MigrationsDir -Filter "*.sql" | Sort-Object Name

if ($MigrationFiles.Count -eq 0) {
    throw "No migration files found in $MigrationsDir"
}

Write-Host "Applying gambit schema migrations..." -ForegroundColor Cyan
foreach ($file in $MigrationFiles) {
    Write-Host "  -> $($file.Name)" -ForegroundColor Gray
    psql $PgUri -v ON_ERROR_STOP=1 -f $file.FullName
}
Write-Host "Schema migrations complete." -ForegroundColor Green
