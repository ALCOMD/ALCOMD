$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))
. (Join-Path $PSScriptRoot "common.ps1")

Assert-RepositoryToolchain
$lockSnapshot = Get-LockFileSnapshot

cargo test --locked --workspace
cargo test --locked --manifest-path extensions/first-party/alcomd-extension-discord/backend/Cargo.toml
npm run check

Assert-LockFileSnapshot $lockSnapshot
