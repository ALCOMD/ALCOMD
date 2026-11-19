$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))
. (Join-Path $PSScriptRoot "common.ps1")

Assert-RepositoryToolchain
$lockSnapshot = Get-LockFileSnapshot

cargo test --locked --workspace
cargo build --locked --package alcomd --package alcomd-cli
.\scripts\test-m1-auto-start.ps1
npm run check

Assert-LockFileSnapshot $lockSnapshot
