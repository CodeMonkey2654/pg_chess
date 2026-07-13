$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$baselineDir = Join-Path (Join-Path (Join-Path "crates" "gambit-db") "benches") "baselines"
$criterionDir = Join-Path "target" "criterion"
$regressionBudgetPercent = 5.0

Get-ChildItem $baselineDir -Directory | ForEach-Object {
    $benchDir = Join-Path $criterionDir $_.Name
    $destPhase2 = Join-Path $benchDir "phase2"
    $destBase = Join-Path $benchDir "base"
    if (Test-Path $destPhase2) { Remove-Item -Recurse -Force $destPhase2 }
    if (Test-Path $destBase) { Remove-Item -Recurse -Force $destBase }
    Copy-Item -Recurse $_.FullName $destPhase2
    Copy-Item -Recurse $_.FullName $destBase
}

Write-Host "==> cargo bench (regression gate, ${regressionBudgetPercent}% budget)" -ForegroundColor Cyan
$prevEap = $ErrorActionPreference
$ErrorActionPreference = "Continue"
cargo bench -p gambit-db --bench movegen -- --baseline phase2 --noplot 2>&1 | Tee-Object -FilePath bench_gate.out
$benchExit = $LASTEXITCODE
$ErrorActionPreference = $prevEap

if ($benchExit -ne 0) {
    Write-Error "cargo bench failed with exit code $benchExit"
}

$lines = Get-Content bench_gate.out
$currentBench = $null
$failures = @()

foreach ($line in $lines) {
    if ($line -match '^\s*(\S+)\s+time:') {
        $currentBench = $Matches[1]
    }
    if ($line -match 'change:\s*\[([+-][0-9.]+)%') {
        $lower = [double]$Matches[1]
        if ($lower -gt $regressionBudgetPercent) {
            $name = if ($currentBench) { $currentBench } else { "unknown" }
            $failures += "${name}: lower bound +${lower}% (budget ${regressionBudgetPercent}%)"
        }
    }
}

if ($failures.Count -gt 0) {
    Write-Error ("Benchmark regression exceeded budget:`n" + ($failures -join "`n"))
}

Write-Host "Benchmark gate passed." -ForegroundColor Green
