$ErrorActionPreference = "Stop"
Set-Location (Join-Path $PSScriptRoot "..")
cargo test -p gambit-db --test perft -- --nocapture @args
