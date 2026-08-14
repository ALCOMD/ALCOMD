param(
    [switch]$SkipFrontend,
    [switch]$SkipGuiRust
)

$ErrorActionPreference = "Stop"
Set-Location (Resolve-Path (Join-Path $PSScriptRoot ".."))

cargo xtask check
cargo fmt --all -- --check

$excluded = @("--exclude", "alcomd-gui")
cargo clippy --workspace @excluded --all-targets -- -D warnings
cargo test --workspace @excluded
cargo fmt --manifest-path extensions/first-party/alcomd-extension-discord/backend/Cargo.toml -- --check
cargo clippy --manifest-path extensions/first-party/alcomd-extension-discord/backend/Cargo.toml --all-targets -- -D warnings

if (-not $SkipGuiRust) {
    cargo check -p alcomd-gui
}

if (-not $SkipFrontend) {
    npm run check
    npm run build
}

python .\scripts\validate-metadata.py
