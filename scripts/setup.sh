#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
source ./scripts/common.sh

assert_repository_toolchain
assert_unix_tauri_prerequisites
lock_snapshot="$(get_lock_file_snapshot)"
npm ci
assert_lock_file_snapshot "$lock_snapshot"

echo "Setup complete. Run ./scripts/check.sh"
