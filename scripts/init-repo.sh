#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."

if [[ ! -d .git ]]; then
    git init -b main
fi

git status --short

if [[ "${1:-}" == "--commit" ]]; then
    git add --all
    git commit -m "chore: initialize ALCOMD v4 repository"
fi
