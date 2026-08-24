#!/usr/bin/env bash
set -euo pipefail

platform="${1:?platform is required}"
engine="${2:?WebView engine is required}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

cargo build --locked --release -p alcomd-gui --example m7_isolation_probe --features tauri/custom-protocol
arguments=(
    --executable target/release/examples/m7_isolation_probe
    --platform "$platform"
    --engine "$engine"
    --output "target/m7-webview-evidence/$platform.json"
)
if [[ "$platform" == "linux" ]]; then
    arguments+=(--xvfb)
fi
python scripts/run-m7-webview-probe.py "${arguments[@]}"
