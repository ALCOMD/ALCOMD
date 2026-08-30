#!/usr/bin/env bash
set -euo pipefail

fixture_root="$(mktemp -d "${TMPDIR:-/tmp}/alcomd-m7-delete-mount.XXXXXX")"
payload_root="$fixture_root/payload"
nested_mount="$payload_root/mounted"
image_path="$fixture_root/nested.dmg"
mkdir -p "$nested_mount"

cleanup() {
    if [[ "$(uname -s)" == "Linux" ]]; then
        if mountpoint -q "$nested_mount"; then
            sudo umount "$nested_mount"
        fi
    elif mount | grep -Fq " on $nested_mount "; then
        hdiutil detach -quiet "$nested_mount"
    fi
    rm -rf -- "$fixture_root"
}
trap cleanup EXIT

if [[ "$(uname -s)" == "Linux" ]]; then
    external_root="$fixture_root/external"
    mkdir -p "$external_root"
    printf 'preserve me' > "$external_root/sentinel.txt"
    sudo mount --bind "$external_root" "$nested_mount"
else
    hdiutil create -quiet -size 16m -fs APFS -volname ALCOMD_M7_DELETE "$image_path"
    hdiutil attach -quiet -nobrowse -mountpoint "$nested_mount" "$image_path"
    printf 'preserve me' > "$nested_mount/sentinel.txt"
fi

ALCOMD_TEST_PROJECT_DELETE_MOUNT_ROOT="$payload_root" \
ALCOMD_TEST_PROJECT_DELETE_MOUNT_SENTINEL="$nested_mount/sentinel.txt" \
    cargo test --locked -p alcomd-platform \
        tests::real_nested_mount_is_rejected_and_its_sentinel_survives \
        -- --ignored --exact --nocapture
