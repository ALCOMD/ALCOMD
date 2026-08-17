#!/usr/bin/env bash

REPOSITORY_PYTHON=()

require_command() {
    command -v "$1" >/dev/null || {
        echo "Required command '$1' was not found." >&2
        exit 1
    }
}

resolve_repository_python() {
    if command -v python3 >/dev/null; then
        REPOSITORY_PYTHON=(python3)
    elif command -v python >/dev/null; then
        REPOSITORY_PYTHON=(python)
    else
        echo "Python 3.11 or newer was not found." >&2
        exit 1
    fi
}

assert_repository_toolchain() {
    for command_name in rustup rustc cargo rustfmt clippy-driver node npm git; do
        require_command "$command_name"
    done

    local rust_version
    rust_version="$(rustc --version | awk '{print $2}')"
    if [[ "$rust_version" != "1.97.1" ]]; then
        echo "Rust 1.97.1 is required; found $rust_version." >&2
        exit 1
    fi

    local node_major
    node_major="$(node --version | sed -E 's/^v([0-9]+).*/\1/')"
    if [[ "$node_major" != "24" ]]; then
        echo "Node.js 24 LTS is required; found major version $node_major." >&2
        exit 1
    fi

    resolve_repository_python
    "${REPOSITORY_PYTHON[@]}" -c 'import sys; raise SystemExit(0 if sys.version_info >= (3, 11) else 1)' || {
        echo "Python 3.11 or newer is required for tomllib." >&2
        exit 1
    }

    if command -v pwsh >/dev/null; then
        local powershell_major
        powershell_major="$(pwsh -NoLogo -NoProfile -Command '$PSVersionTable.PSVersion.Major')"
        if (( powershell_major < 7 )); then
            echo "PowerShell 7 or newer is required when PowerShell scripts are used." >&2
            exit 1
        fi
        echo "PowerShell $powershell_major detected."
    else
        echo "PowerShell 7 was not found; this Bash validation path does not invoke PowerShell scripts." >&2
    fi

    echo "Toolchain: Rust $rust_version; Node $(node --version); npm $(npm --version); $("${REPOSITORY_PYTHON[@]}" --version 2>&1)"
}

assert_unix_tauri_prerequisites() {
    case "$(uname -s)" in
        Linux)
            require_command cc
            require_command pkg-config
            local missing=()
            for module in gtk+-3.0 webkit2gtk-4.1 openssl ayatana-appindicator3-0.1 librsvg-2.0; do
                if ! pkg-config --exists "$module"; then
                    missing+=("$module")
                fi
            done
            if (( ${#missing[@]} > 0 )); then
                echo "Missing Linux Tauri pkg-config modules: ${missing[*]}" >&2
                echo "Ubuntu 22.04 packages include libgtk-3-dev libwebkit2gtk-4.1-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev." >&2
                exit 1
            fi
            echo "Linux Tauri prerequisites found."
            ;;
        Darwin)
            require_command xcode-select
            require_command xcrun
            if [[ "$(uname -m)" != "arm64" ]]; then
                echo "The approved macOS M0 target requires an Apple Silicon arm64 host." >&2
                exit 1
            fi
            xcode-select -p >/dev/null
            xcrun --sdk macosx --show-sdk-path >/dev/null
            echo "macOS Apple Silicon and Xcode Command Line Tools found."
            ;;
        *)
            echo "Unsupported platform for scripts/setup.sh: $(uname -s)" >&2
            exit 1
            ;;
    esac
}

get_lock_file_snapshot() {
    local lock_files=(
        Cargo.lock
        package-lock.json
        extensions/first-party/alcomd-extension-discord/backend/Cargo.lock
    )
    local lock_file
    local hashes=()
    for lock_file in "${lock_files[@]}"; do
        [[ -f "$lock_file" ]] || {
            echo "Required lock file '$lock_file' is missing." >&2
            exit 1
        }
        hashes+=("$(git hash-object -- "$lock_file")")
    done
    local IFS='|'
    printf '%s' "${hashes[*]}"
}

assert_lock_file_snapshot() {
    local before="$1"
    local after
    after="$(get_lock_file_snapshot)"
    if [[ "$after" != "$before" ]]; then
        echo "One or more of the three required lock files changed while running the command." >&2
        exit 1
    fi
}

invoke_metadata_validator() {
    if [[ ${#REPOSITORY_PYTHON[@]} -eq 0 ]]; then
        resolve_repository_python
    fi
    "${REPOSITORY_PYTHON[@]}" ./scripts/validate-metadata.py
}
