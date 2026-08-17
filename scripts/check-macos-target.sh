#!/usr/bin/env bash

set -euo pipefail

readonly REQUIRED_ARCHITECTURE="arm64"
readonly REQUIRED_DEPLOYMENT_TARGET="11.0"
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

for command_name in file lipo otool; do
    command -v "$command_name" >/dev/null || {
        echo "Required command '$command_name' was not found." >&2
        exit 1
    }
done

for binary_name in "${EXPECTED_BINARIES[@]}"; do
    binary_path="$RELEASE_DIRECTORY/$binary_name"
    if [[ ! -f "$binary_path" ]]; then
        echo "Expected M0 executable '$binary_path' is missing." >&2
        exit 1
    fi

    file_output="$(file "$binary_path")"
    architectures="$(lipo -archs "$binary_path")"
    if [[ "$architectures" != "$REQUIRED_ARCHITECTURE" ]]; then
        echo "Expected '$binary_path' to be arm64-only; found '$architectures'." >&2
        exit 1
    fi

    deployment_target="$(otool -l "$binary_path" | awk '
        $1 == "cmd" && $2 == "LC_BUILD_VERSION" { in_build_version = 1; next }
        in_build_version && $1 == "minos" { print $2; exit }
    ')"
    if [[ -z "$deployment_target" ]]; then
        echo "Unable to determine LC_BUILD_VERSION minos for '$binary_path'." >&2
        exit 1
    fi
    if [[ "$deployment_target" != "$REQUIRED_DEPLOYMENT_TARGET" ]]; then
        echo "Expected '$binary_path' deployment target $REQUIRED_DEPLOYMENT_TARGET; found $deployment_target." >&2
        exit 1
    fi

    echo "$binary_name: architecture=$architectures; minos=$deployment_target; file=$file_output"
done
