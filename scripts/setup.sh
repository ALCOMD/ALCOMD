#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

for command in rustup cargo node npm git; do
    command -v "$command" >/dev/null || {
        echo "Required command '$command' was not found." >&2
        exit 1
    }
done

node_major="$(node --version | sed -E 's/^v([0-9]+).*/\1/')"
if [[ "$node_major" != "24" ]]; then
    echo "Node.js 24 LTS is required; found major version $node_major." >&2
    exit 1
fi

rustup show
npm install

echo "Setup complete. Run ./scripts/check.sh --skip-gui-rust"
