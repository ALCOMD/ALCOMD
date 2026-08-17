#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
source ./scripts/common.sh

assert_repository_toolchain
lock_snapshot="$(get_lock_file_snapshot)"

cargo run --locked --package xtask -- check
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo check --locked --package alcomd-gui

discord_manifest="extensions/first-party/alcomd-extension-discord/backend/Cargo.toml"
cargo fmt --manifest-path "$discord_manifest" -- --check
cargo clippy --locked --manifest-path "$discord_manifest" --all-targets -- -D warnings
cargo test --locked --manifest-path "$discord_manifest"

npm run check
npm run build
invoke_metadata_validator

assert_lock_file_snapshot "$lock_snapshot"
