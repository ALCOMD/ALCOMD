$ErrorActionPreference = "Stop"
Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))

cargo test --workspace --exclude alcomd-gui
npm run check
