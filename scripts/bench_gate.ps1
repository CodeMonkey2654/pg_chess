$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$regressionBudgetPercent = 5.0
$packages = @(
    @{ Package = "gambit-db"; Bench = "movegen"; Baselines = "crates/gambit-db/benches/baselines" },
    @{ Package = "gambit-analysis"; Bench = "search"; Baselines = "crates/gambit-analysis/benches/baselines" }
)

$allFailures = @()

foreach ($cfg in $packages) {
    $baselineDir = $cfg.Baselines
    if (-not (Test-Path $baselineDir)) {
        Write-Host "Skipping $($cfg.Package): no baselines at $baselineDir" -ForegroundColor Yellow
        continue
    }

    $criterionDir = Join-Path "target" "criterion"
    Get-ChildItem $baselineDir -Directory | ForEach-Object {
        $benchDir = Join-Path $criterionDir $_.Name
        $destPhase2 = Join-Path $benchDir "phase2"
        $destBase = Join-Path $benchDir "base"
        if (Test-Path $destPhase2) { Remove-Item -Recurse -Force $destPhase2 }
        if (Test-Path $destBase) { Remove-Item -Recurse -Force $destBase }
        Copy-Item -Recurse $_.FullName $destPhase2
        Copy-Item -Recurse $_.FullName $destBase
    }

    Write-Host "==> cargo bench -p $($cfg.Package) (regression gate, ${regressionBudgetPercent}% budget)" -ForegroundColor Cyan
    $prevEap = $ErrorActionPreference
    $ErrorActionPreference = "Continue"
    $outFile = "bench_gate_$($cfg.Package).out"
    cargo bench -p $cfg.Package --bench $cfg.Bench -- --baseline phase2 --noplot 2>&1 | Tee-Object -FilePath $outFile
    $benchExit = $LASTEXITCODE
    $ErrorActionPreference = $prevEap

    if ($benchExit -ne 0) {
        Write-Error "cargo bench -p $($cfg.Package) failed with exit code $benchExit"
    }

    $lines = Get-Content $outFile
    $currentBench = $null

    foreach ($line in $lines) {
        if ($line -match '^\s*(\S+)\s+time:') {
            $currentBench = $Matches[1]
        }
        if ($line -match 'change:\s*\[([+-][0-9.]+)%') {
            $lower = [double]$Matches[1]
            if ($lower -gt $regressionBudgetPercent) {
                $name = if ($currentBench) { $currentBench } else { "unknown" }
                $allFailures += "$($cfg.Package)/${name}: lower bound +${lower}% (budget ${regressionBudgetPercent}%)"
            }
        }
    }
}

if ($allFailures.Count -gt 0) {
    Write-Error ("Benchmark regression exceeded budget:`n" + ($allFailures -join "`n"))
}

Write-Host "Benchmark gate passed." -ForegroundColor Green
