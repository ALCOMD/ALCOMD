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

management_fixture="crates/alcomd-testing/fixtures/m8/management-guest-source/Cargo.toml"
cargo fmt --manifest-path "$management_fixture" -- --check
cargo clippy --locked --manifest-path "$management_fixture" --all-targets -- -D warnings
cargo test --locked --manifest-path "$management_fixture"

npm run check
npm run build
npm run test --workspace @alcomd/sdk
invoke_metadata_validator
"${REPOSITORY_PYTHON[@]}" ./scripts/check-native-sdk.py

assert_lock_file_snapshot "$lock_snapshot"
