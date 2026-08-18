#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
daemon="$(pwd)/target/debug/alcomd"
cli="$(pwd)/target/debug/alcomd-cli"
probe_root="$(mktemp -d)"
runtime="$probe_root/runtime"
out_one="$probe_root/one.out"
err_one="$probe_root/one.err"
out_two="$probe_root/two.out"
err_two="$probe_root/two.err"

regex_escape() {
    printf '%s' "$1" | sed 's/[][\.^$*+?(){}|]/\\&/g'
}

daemon_pattern="^$(regex_escape "$daemon") --runtime-dir $(regex_escape "$runtime")$"

cleanup() {
    while IFS= read -r pid; do
        if [[ -n "$pid" ]]; then
            kill "$pid" 2>/dev/null || true
        fi
    done < <(pgrep -f "$daemon_pattern" || true)
    rm -rf -- "$probe_root"
}
trap cleanup EXIT

"$cli" --runtime-dir "$runtime" --json system status >"$out_one" 2>"$err_one" &
first=$!
"$cli" --runtime-dir "$runtime" --json system status >"$out_two" 2>"$err_two" &
second=$!
wait "$first"
wait "$second"

daemon_pids="$(pgrep -f "$daemon_pattern" || true)"
daemon_count="$(printf '%s\n' "$daemon_pids" | sed '/^$/d' | wc -l | tr -d ' ')"
if [[ "$daemon_count" -ne 1 ]]; then
    echo "Expected one authoritative daemon; found $daemon_count." >&2
    exit 1
fi
grep -Fq '"state":"ready"' "$out_one"
grep -Fq '"state":"ready"' "$out_two"
if [[ -s "$err_one" || -s "$err_two" ]]; then
    echo "Concurrent CLI wrote unexpected stderr output." >&2
    exit 1
fi

echo "M1 concurrent daemon auto-start passed; authoritative daemon count: 1."
