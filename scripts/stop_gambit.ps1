$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

Write-Host "==> Stopping Gambit app processes" -ForegroundColor Cyan
foreach ($name in @("gambit-ingest-worker", "gambit-studio-server", "gambit-ingest", "trunk")) {
    Get-Process -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
        Write-Host "  stopping $($_.ProcessName) (pid $($_.Id))" -ForegroundColor Gray
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
}

foreach ($port in @(8080, 8081, 8082)) {
    Get-NetTCPConnection -LocalPort $port -ErrorAction SilentlyContinue |
        Select-Object -ExpandProperty OwningProcess -Unique |
        ForEach-Object {
            Write-Host "  stopping pid $_ on port $port" -ForegroundColor Gray
            Stop-Process -Id $_ -Force -ErrorAction SilentlyContinue
        }
}

Write-Host "==> Stopping pgrx PostgreSQL (pg18 / :28818)" -ForegroundColor Cyan
cargo pgrx stop pg18 2>&1 | Out-Null

Start-Sleep -Seconds 2
Write-Host "Gambit stack stopped." -ForegroundColor Green
