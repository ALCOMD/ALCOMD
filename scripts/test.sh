#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
source ./scripts/common.sh

assert_repository_toolchain
lock_snapshot="$(get_lock_file_snapshot)"

cargo test --locked --workspace
cargo test --locked --manifest-path extensions/first-party/alcomd-extension-discord/backend/Cargo.toml
npm run check

assert_lock_file_snapshot "$lock_snapshot"
