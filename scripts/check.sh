#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

skip_frontend=false
skip_gui_rust=false

for argument in "$@"; do
    case "$argument" in
        --skip-frontend) skip_frontend=true ;;
        --skip-gui-rust) skip_gui_rust=true ;;
        *)
            echo "Unknown argument: $argument" >&2
            exit 2
            ;;
    esac
done

cargo xtask check
cargo fmt --all -- --check
cargo clippy --workspace --exclude alcomd-gui --all-targets -- -D warnings
cargo test --workspace --exclude alcomd-gui
cargo fmt --manifest-path extensions/first-party/alcomd-extension-discord/backend/Cargo.toml -- --check
cargo clippy --manifest-path extensions/first-party/alcomd-extension-discord/backend/Cargo.toml --all-targets -- -D warnings

if [[ "$skip_gui_rust" == false ]]; then
    cargo check -p alcomd-gui
fi

if [[ "$skip_frontend" == false ]]; then
    npm run check
    npm run build
fi

python3 ./scripts/validate-metadata.py
