param(
    [Parameter(Mandatory = $true)]
    [string]$File,
    [switch]$Clean
)

. (Join-Path $PSScriptRoot "gambit_pg_env.ps1")

if (-not (Test-Path $File)) {
    throw "Backup file not found: $File"
}

$extra = @()
if ($Clean) { $extra += "--clean" }

Write-Host "Restoring $File into $env:DATABASE_URL" -ForegroundColor Cyan
& pg_restore -d $env:DATABASE_URL @extra $File
exit $LASTEXITCODE
