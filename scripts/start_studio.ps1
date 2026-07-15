param(
    [int]$Year = 2024,
    [string]$Source = "lichess_standard_2024"
)

$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")

$Root = $PWD
$LogDir = Join-Path $Root ".logs"
New-Item -ItemType Directory -Force -Path $LogDir | Out-Null

function Start-BackgroundService {
    param(
        [string]$Name,
        [string]$Exe,
        [hashtable]$Env = @{},
        [string[]]$Args = @()
    )
    $logOut = Join-Path $LogDir "$Name.out.log"
    $logErr = Join-Path $LogDir "$Name.err.log"
    $envBlock = ($Env.GetEnumerator() | ForEach-Object { "`$env:$($_.Key)='$($_.Value)'" }) -join "; "
    $argList = ($Args | ForEach-Object { "'$_'" }) -join ", "
    $cmd = "cd '$Root'; $envBlock; & '$Exe' $argList *> '$logOut' 2> '$logErr'"
    Start-Process powershell -WindowStyle Hidden -ArgumentList @("-NoProfile", "-Command", $cmd) | Out-Null
    Write-Host "  started $Name (logs: $logOut)" -ForegroundColor Gray
}

Write-Host "==> Starting PostgreSQL + schema" -ForegroundColor Cyan
& (Join-Path $PSScriptRoot "start_pg.ps1")

$PgUri = $env:DATABASE_URL
$CacheDir = Join-Path $Root ".cache/lichess"
New-Item -ItemType Directory -Force -Path $CacheDir | Out-Null

Write-Host "==> Building release binaries (if needed)" -ForegroundColor Cyan
cargo build --release -p gambit-ingest-worker -p gambit-studio-server 2>&1 | Out-Null

$WorkerExe = Join-Path $Root "target\release\gambit-ingest-worker.exe"
$ServerExe = Join-Path $Root "target\release\gambit-studio-server.exe"

Write-Host "==> Starting ingest worker on :8082" -ForegroundColor Cyan
Start-BackgroundService -Name "ingest-worker" -Exe $WorkerExe -Env @{
    DATABASE_URL = $PgUri
    GAMBIT_CACHE_DIR = $CacheDir
    INGEST_ADDR = "127.0.0.1:8082"
}

Start-Sleep -Seconds 2

Write-Host "==> Starting Gambit Studio gRPC API on :8080" -ForegroundColor Cyan
Start-BackgroundService -Name "studio-server" -Exe $ServerExe -Env @{
    DATABASE_URL = $PgUri
    INGEST_ADDR = "http://127.0.0.1:8082"
}

Write-Host "==> Waiting for API health" -ForegroundColor Cyan
$healthy = $false
for ($i = 0; $i -lt 30; $i++) {
    try {
        $r = Invoke-WebRequest -Uri "http://127.0.0.1:8080/grpc.health.v1.Health/Check" -Method POST `
            -ContentType "application/grpc-web+proto" -Body ([byte[]]@()) -TimeoutSec 2 -ErrorAction Stop
        $healthy = $true
        break
    } catch {
        Start-Sleep -Seconds 1
    }
}
if (-not $healthy) {
    Write-Host "  API not responding yet — check .logs/studio-server.*.log" -ForegroundColor Yellow
}

Write-Host "==> Starting WASM UI on :8081 (trunk)" -ForegroundColor Cyan
if (-not (Get-Command trunk -ErrorAction SilentlyContinue)) {
    Write-Host "  trunk not found — install with: cargo install trunk" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "PostgreSQL + API are up without UI." -ForegroundColor Green
    Write-Host "  DATABASE_URL=$PgUri" -ForegroundColor Cyan
    Write-Host "  API: http://127.0.0.1:8080" -ForegroundColor Cyan
    exit 0
}

Remove-Item Env:NO_COLOR -ErrorAction SilentlyContinue
$env:NO_COLOR = $null
Push-Location (Join-Path $Root "crates\gambit-studio-ui")
try {
    Write-Host ""
    Write-Host "Gambit Studio is starting." -ForegroundColor Green
    Write-Host "  UI:   http://127.0.0.1:8081" -ForegroundColor Cyan
    Write-Host "  API:  http://127.0.0.1:8080" -ForegroundColor Cyan
    Write-Host "  Logs: $LogDir" -ForegroundColor Gray
    Write-Host ""
    trunk serve --address 127.0.0.1 --port 8081
} finally {
    Pop-Location
}
