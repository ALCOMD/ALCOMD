#!/usr/bin/env bash

set -euo pipefail

readonly GLIBC_LIMIT="2.35"
readonly RELEASE_DIRECTORY="target/release"
readonly EXPECTED_BINARIES=(
    alcomd
    alcomd-api
    alcomd-bootstrap
    alcomd-cli
    alcomd-extension-host
    alcomd-gui
    alcomd-mcp
    alcomd-migrate-v3
    alcomd-updater
)

command -v file >/dev/null || {
    echo "Required command 'file' was not found." >&2
    exit 1
}
command -v readelf >/dev/null || {
    echo "Required command 'readelf' was not found." >&2
    exit 1
}

version_greater_than() {
    [[ "$(printf '%s\n%s\n' "$1" "$2" | sort -V | tail -n 1)" == "$1" && "$1" != "$2" ]]
}

highest="0.0"
for binary_name in "${EXPECTED_BINARIES[@]}"; do
    binary_path="$RELEASE_DIRECTORY/$binary_name"
    if [[ ! -f "$binary_path" ]]; then
        echo "Expected M0 executable '$binary_path' is missing." >&2
        exit 1
    fi
    if ! file "$binary_path" | grep -q 'ELF'; then
        echo "Expected M0 executable '$binary_path' is not an ELF file." >&2
        exit 1
    fi

    while IFS= read -r version; do
        if version_greater_than "$version" "$highest"; then
            highest="$version"
        fi
    done < <(
        readelf --version-info "$binary_path" |
            grep -oE 'GLIBC_[0-9]+(\.[0-9]+)+' |
            sed 's/^GLIBC_//' |
            sort -Vu
    )
done

echo "Highest required GLIBC symbol version: GLIBC_$highest"
if version_greater_than "$highest" "$GLIBC_LIMIT"; then
    echo "GLIBC_$highest exceeds the approved GLIBC_$GLIBC_LIMIT limit." >&2
    exit 1
fi
