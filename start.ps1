$ErrorActionPreference = "Stop"

# ---- Config -----------------------------------------------------------------
$LLVM_VERSION   = "18.1.8"
$LLVM_URL       = "https://github.com/llvm/llvm-project/releases/download/llvmorg-$LLVM_VERSION/LLVM-$LLVM_VERSION-win64.exe"
$LLVM_SHA256    = "94af030060d88cc17e9f00ef1663ebdc1126b35e16bebdfa1e807984b70abd8f"  # official 18.1.8 win64
$LLVM_INSTALL   = "C:\Program Files\LLVM-18"
$LLVM_BIN       = Join-Path $LLVM_INSTALL "bin"
$PG_VERSION     = "pg18"

# ---- Helpers ----------------------------------------------------------------
function Test-LLVM18 {
    param([string]$BinPath)
    $libclang = Join-Path $BinPath "libclang.dll"
    $clangExe = Join-Path $BinPath "clang.exe"
    if ((Test-Path $libclang) -and (Test-Path $clangExe)) {
        try {
            $v = & $clangExe --version 2>$null
            if ($v -match "clang version 18\.") { return $true }
        } catch {}
    }
    return $false
}

function Install-LLVM18 {
    Write-Host "LLVM 18 not found. Downloading $LLVM_VERSION..." -ForegroundColor Cyan
    $tmp = Join-Path $env:TEMP "LLVM-$LLVM_VERSION-win64.exe"

    if (Test-Path $tmp) { Remove-Item $tmp -Force }

    # TLS 1.2 for older PowerShell hosts
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    Invoke-WebRequest -Uri $LLVM_URL -OutFile $tmp -UseBasicParsing

    Write-Host "Verifying checksum..." -ForegroundColor Cyan
    $hash = (Get-FileHash -Path $tmp -Algorithm SHA256).Hash
    if ($hash -ne $LLVM_SHA256.ToUpper()) {
        Write-Host "Checksum mismatch!" -ForegroundColor Red
        Write-Host "  expected: $($LLVM_SHA256.ToUpper())"
        Write-Host "  actual:   $hash"
        Remove-Item $tmp -Force
        exit 1
    }
    Write-Host "Checksum OK." -ForegroundColor Green

    # The LLVM installer is NSIS-based: /S = silent, /D = install dir (must be last, unquoted, no trailing slash)
    Write-Host "Installing LLVM 18 silently to $LLVM_INSTALL (admin required)..." -ForegroundColor Cyan
    $proc = Start-Process -FilePath $tmp -ArgumentList "/S", "/D=$LLVM_INSTALL" -Verb RunAs -Wait -PassThru
    if ($proc.ExitCode -ne 0) {
        Write-Host "LLVM installer exited with code $($proc.ExitCode)" -ForegroundColor Red
        exit 1
    }
    Remove-Item $tmp -Force

    if (-not (Test-LLVM18 -BinPath $LLVM_BIN)) {
        Write-Host "LLVM 18 install did not produce a valid clang 18 at $LLVM_BIN" -ForegroundColor Red
        exit 1
    }
    Write-Host "LLVM 18 installed." -ForegroundColor Green
}

function Ensure-CargoPgrx {
    if (-not (Get-Command cargo-pgrx -ErrorAction SilentlyContinue)) {
        Write-Host "Installing cargo-pgrx..." -ForegroundColor Cyan
        cargo install --locked cargo-pgrx
    } else {
        Write-Host "cargo-pgrx already installed." -ForegroundColor Green
    }
}

function Ensure-PgrxInitialized {
    $configPath = Join-Path $env:USERPROFILE ".pgrx\config.toml"
    if (-not (Test-Path $configPath)) {
        Write-Host "Initializing pgrx for $PG_VERSION..." -ForegroundColor Cyan
        cargo pgrx init --$PG_VERSION download
    } else {
        Write-Host "pgrx config already exists at $configPath" -ForegroundColor Green
    }
}

# ---- Main -------------------------------------------------------------------

# 1. Find or install LLVM 18
$candidates = @($LLVM_BIN, "C:\Program Files\LLVM\bin")
$found = $candidates | Where-Object { Test-LLVM18 -BinPath $_ } | Select-Object -First 1

if (-not $found) {
    Install-LLVM18
    $found = $LLVM_BIN
}
Write-Host "Using LLVM 18 at: $found" -ForegroundColor Green

# 2. Set LIBCLANG_PATH (session + persistent user)
$env:LIBCLANG_PATH = $found
if (-not ($env:Path -split ';' | Where-Object { $_ -eq $found })) {
    $env:Path = "$found;$env:Path"
}
[Environment]::SetEnvironmentVariable("LIBCLANG_PATH", $found, "User")

Write-Host "LIBCLANG_PATH=$env:LIBCLANG_PATH" -ForegroundColor Cyan
Write-Host "libclang.dll exists: $(Test-Path (Join-Path $found 'libclang.dll'))" -ForegroundColor Cyan
& (Join-Path $found "clang.exe") --version

# 3. Toolchain + pgrx
Ensure-CargoPgrx
Ensure-PgrxInitialized

# 4. Build + run
Write-Host "Cleaning build artifacts..." -ForegroundColor Cyan
cargo clean

Write-Host "Running pgrx extension on $PG_VERSION..." -ForegroundColor Cyan
cargo pgrx run $PG_VERSION -p pg_chess