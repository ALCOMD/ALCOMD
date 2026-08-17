$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true
Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))
. (Join-Path $PSScriptRoot "common.ps1")

Assert-RepositoryToolchain
$lockSnapshot = Get-LockFileSnapshot

cargo run --locked --package xtask -- check
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo check --locked --package alcomd-gui

$discordManifest = "extensions/first-party/alcomd-extension-discord/backend/Cargo.toml"
cargo fmt --manifest-path $discordManifest -- --check
cargo clippy --locked --manifest-path $discordManifest --all-targets -- -D warnings
cargo test --locked --manifest-path $discordManifest

npm run check
npm run build
Invoke-MetadataValidator

Assert-LockFileSnapshot $lockSnapshot
