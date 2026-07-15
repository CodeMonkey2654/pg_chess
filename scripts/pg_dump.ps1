param(
    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$Args
)

. (Join-Path $PSScriptRoot "gambit_pg_env.ps1")

$outDir = Join-Path (Join-Path $PSScriptRoot "..") "backups"
New-Item -ItemType Directory -Force -Path $outDir | Out-Null

if ($Args.Count -eq 0) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $outFile = Join-Path $outDir "gambit-$stamp.dump"
    Write-Host "Dumping to $outFile" -ForegroundColor Cyan
    & pg_dump -Fc -f $outFile $env:DATABASE_URL
    Write-Host "Done: $outFile" -ForegroundColor Green
    exit $LASTEXITCODE
}

& pg_dump @Args
exit $LASTEXITCODE
