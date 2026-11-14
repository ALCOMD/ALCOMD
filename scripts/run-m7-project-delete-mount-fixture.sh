#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/.."
test_name="project_delete::tests::real_nested_mount_is_rejected_and_its_sentinel_survives"
host_os="$(uname -s)"
if [[ "$host_os" != Linux && "$host_os" != Darwin ]]; then
    printf 'This fixture requires Linux or macOS.\n' >&2
    exit 1
fi

fixture_root="$(mktemp -d "${TMPDIR:-/tmp}/alcomd-m7-delete-mount.XXXXXX")"
payload_root="$fixture_root/payload"
nested_mount="$payload_root/mounted"
image_path="$fixture_root/nested.dmg"
mount_attached=false
mkdir -p "$nested_mount"

cleanup() {
    local status=$?
    local detach_status=0
    local attempt
    trap - EXIT
    if [[ "$mount_attached" == true ]]; then
        if [[ "$host_os" == Linux ]]; then
            sudo umount "$nested_mount" || detach_status=$?
        else
            # Retry only EBUSY, normally; never force-detach an occupied volume.
            for attempt in 1 2 3 4 5; do
                detach_status=0
                hdiutil detach "$nested_mount" || detach_status=$?
                if [[ "$detach_status" != 16 || "$attempt" == 5 ]]; then
                    break
                fi
                sleep 1
            done
        fi
        if [[ "$detach_status" != 0 ]]; then
            printf 'Unmount failed (%s); preserving mounted fixture: %s\n' \
                "$detach_status" "$fixture_root" >&2
            if [[ "$status" == 0 ]]; then
                status=$detach_status
            fi
            exit "$status"
        fi
    fi
    rm -rf -- "$fixture_root" || { if [[ "$status" == 0 ]]; then status=1; fi; }
    exit "$status"
}
trap cleanup EXIT

# Fail before mounting if the exact ignored test is absent or renamed.
cargo test --locked -p alcomd-platform --lib "$test_name" \
    -- --ignored --exact --list | tee "$fixture_root/list.log"
if [[ "$(grep -Fxc "$test_name: test" "$fixture_root/list.log")" != 1 ]]; then
    printf 'Expected exactly one nested-mount test before mounting.\n' >&2
    exit 1
fi

if [[ "$host_os" == Linux ]]; then
    external_root="$fixture_root/external"
    mkdir -p "$external_root"
    printf 'preserve me' > "$external_root/sentinel.txt"
    sudo mount --bind "$external_root" "$nested_mount"
    mount_attached=true
else
    hdiutil create -quiet -size 16m -fs APFS -volname ALCOMD_M7_DELETE "$image_path"
    hdiutil attach -quiet -nobrowse -mountpoint "$nested_mount" "$image_path"
    mount_attached=true
    printf 'preserve me' > "$nested_mount/sentinel.txt"
fi

ALCOMD_TEST_PROJECT_DELETE_MOUNT_ROOT="$payload_root" \
ALCOMD_TEST_PROJECT_DELETE_MOUNT_SENTINEL="$nested_mount/sentinel.txt" \
    cargo test --locked -p alcomd-platform --lib "$test_name" \
        -- --ignored --exact --nocapture --format pretty --color never \
        | tee "$fixture_root/test.log"
if ! grep -Fxq "test $test_name ... ok" "$fixture_root/test.log" \
    || ! grep -Fq 'test result: ok. 1 passed; 0 failed; 0 ignored;' "$fixture_root/test.log"; then
    printf 'Expected the named nested-mount test to execute and pass exactly once.\n' >&2
    exit 1
fi
