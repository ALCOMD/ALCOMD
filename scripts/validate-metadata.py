from __future__ import annotations

import json
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def load_toml(relative: str) -> dict:
    with (ROOT / relative).open("rb") as file:
        return tomllib.load(file)


def load_json(relative: str) -> dict:
    with (ROOT / relative).open("r", encoding="utf-8") as file:
        return json.load(file)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def main() -> int:
    product = load_toml("alcomd.product.toml")
    require(product["product"]["family_name"] == "ALCOMD", "Unexpected product family")
    require(product["product"]["technical_name"] == "alcomd", "Unexpected technical name")
    require(
        product["identity"]["bundle_identifier"] == "com.cqmhv.alcomd",
        "Unexpected bundle identifier",
    )

    parity = load_toml("feature-parity.toml")
    require(parity["schema"] == 1, "Unsupported feature parity schema")
    feature_ids = [item["id"] for item in parity["feature"]]
    require(len(feature_ids) == len(set(feature_ids)), "Duplicate feature IDs")

    for relative in [
        "extensions/first-party/alcomd-extension-mcp/alcomd-extension.toml",
        "extensions/first-party/alcomd-extension-discord/alcomd-extension.toml",
    ]:
        manifest = load_toml(relative)
        require(manifest["schema"] == 1, f"Unsupported extension schema: {relative}")
        require(
            manifest["id"].startswith("com.cqmhv.alcomd.extension."),
            f"Unexpected first-party extension ID: {relative}",
        )

    for relative in [
        "apps/alcomd-gui/src-tauri/tauri.conf.json",
        "apps/alcomd-gui/src-tauri/capabilities/main.json",
        "specs/rpc/system-hello.request.schema.json",
        "specs/rpc/system-hello.response.schema.json",
        "specs/extensions/manifest-v1.schema.json",
        "migrations/v3/schemas/migration-bundle-v1.schema.json",
    ]:
        load_json(relative)

    print("Metadata validation passed.")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (KeyError, OSError, TypeError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"Metadata validation failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
