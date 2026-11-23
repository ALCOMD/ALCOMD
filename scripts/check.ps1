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

$managementFixture = "crates/alcomd-testing/fixtures/m8/management-guest-source/Cargo.toml"
cargo fmt --manifest-path $managementFixture -- --check
cargo clippy --locked --manifest-path $managementFixture --all-targets -- -D warnings
cargo test --locked --manifest-path $managementFixture

npm run check
npm run build
npm run test --workspace @alcomd/sdk
Invoke-MetadataValidator
& $script:RepositoryPython.Executable @($script:RepositoryPython.PrefixArguments) ./scripts/check-native-sdk.py

Assert-LockFileSnapshot $lockSnapshot
