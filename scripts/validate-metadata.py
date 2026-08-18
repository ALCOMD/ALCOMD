from __future__ import annotations

import hashlib
import json
import re
import sys
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PROJECT_LICENSE = "AGPL-3.0-only"
PROJECT_REPOSITORY = "https://github.com/ALCOMD/ALCOMD"
RUST_VERSION = "1.97.1"
COMMIT_PATTERN = re.compile(r"^[0-9a-f]{40}$")
SHA256_PATTERN = re.compile(r"^[0-9a-f]{64}$")
SHA256_FINGERPRINT_PATTERN = re.compile(r"^sha256:[0-9a-f]{64}$")


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
    require(product["product"]["display_name"] == "ALCOMD3", "Unexpected display name")
    require(product["product"]["technical_name"] == "alcomd", "Unexpected technical name")
    require(product["product"]["publisher_name"] == "CQMHV", "Unexpected publisher")
    require(product["product"]["version"] == "4.0.0-alpha.0", "Unexpected product version")
    require(
        product["identity"]["bundle_identifier"] == "com.cqmhv.alcomd",
        "Unexpected bundle identifier",
    )
    require(
        product["identity"]["windows_aumid"] == "CQMHV.ALCOMD",
        "Unexpected Windows AUMID",
    )
    require(product["identity"]["uri_scheme"] == "alcomd", "Unexpected URI scheme")
    require(
        product["identity"]["linux_desktop_id"]
        == product["identity"]["bundle_identifier"],
        "Linux desktop ID does not match the bundle identity",
    )
    require(
        product["identity"]["data_directory"] == "ALCOMD",
        "Unexpected data directory",
    )
    require(
        product["identity"]["install_directory"] == "ALCOMD",
        "Unexpected install directory",
    )
    expected_binaries = {
        "daemon": "alcomd",
        "gui": "alcomd-gui",
        "cli": "alcomd-cli",
        "mcp": "alcomd-mcp",
        "api": "alcomd-api",
        "extension_host": "alcomd-extension-host",
        "bootstrap": "alcomd-bootstrap",
        "updater": "alcomd-updater",
        "migration_v3": "alcomd-migrate-v3",
    }
    require(product["binaries"] == expected_binaries, "Unexpected product binary identity")

    tauri = load_json("apps/alcomd-gui/src-tauri/tauri.conf.json")
    require(
        tauri["productName"] == product["product"]["display_name"],
        "Tauri productName does not match the user brand",
    )
    require(
        tauri["identifier"] == product["identity"]["bundle_identifier"],
        "Tauri identifier does not match the product identity",
    )
    require(
        tauri["version"] == product["product"]["version"],
        "Tauri version does not match the product version",
    )
    require(
        tauri["mainBinaryName"] == product["binaries"]["gui"],
        "Tauri binary name does not match the product identity",
    )
    require(
        tauri["bundle"]["publisher"] == product["product"]["publisher_name"],
        "Tauri publisher does not match the product identity",
    )

    parity = load_toml("feature-parity.toml")
    require(parity["schema"] == 2, "Unsupported feature parity schema")
    parity_metadata = parity["metadata"]
    require(parity_metadata["baseline_frozen"] is True, "Baselines are not frozen")
    require(
        parity_metadata["audit_status"] in {"in_progress", "complete"},
        "Unsupported feature audit status",
    )
    require(
        parity_metadata["inventory_granularity"] in {"domain_seed", "user_entry"},
        "Unsupported feature inventory granularity",
    )
    require(isinstance(parity_metadata["m1_complete"], bool), "Invalid M-1 state")
    m1_complete = parity_metadata["m1_complete"]
    if parity_metadata["inventory_granularity"] == "domain_seed":
        require(not m1_complete, "A domain-seed inventory cannot complete M-1")
    if m1_complete:
        require(
            parity_metadata["audit_status"] == "complete",
            "Completed M-1 requires a completed feature audit",
        )
        require(
            parity_metadata["inventory_granularity"] == "user_entry",
            "Completed M-1 requires user-entry granularity",
        )
    else:
        require(
            parity_metadata["audit_status"] != "complete",
            "A completed feature audit must set m1_complete = true",
        )

    test_plan = load_toml("docs/testing/test-plan.toml")
    require(test_plan["schema"] == 1, "Unsupported test plan schema")
    test_status_values = set(test_plan["metadata"]["status_values"])
    tests_by_id = {item["id"]: item for item in test_plan["test"]}
    require(
        len(tests_by_id) == len(test_plan["test"]),
        "Duplicate test plan IDs",
    )
    for test_id, test in tests_by_id.items():
        require(test["stage"], f"Test plan has no stage: {test_id}")
        require(test["kind"], f"Test plan has no kind: {test_id}")
        require(test["platforms"], f"Test plan has no platforms: {test_id}")
        require(test["description"], f"Test plan has no description: {test_id}")
        require(
            test["status"] in test_status_values,
            f"Unsupported test plan status: {test_id}",
        )

    features = parity["feature"]
    feature_status_values = set(parity_metadata["status_values"])
    implementation_status_values = set(
        parity_metadata["implementation_status_values"]
    )
    for feature in features:
        require(feature["source"], f"Feature has no source: {feature['id']}")
        require(feature["user_entry"], f"Feature has no user entry: {feature['id']}")
        require(feature["behavior"], f"Feature has no behavior: {feature['id']}")
        require(feature["evidence"], f"Feature has no evidence: {feature['id']}")
        require(feature["coverage"], f"Feature has no coverage: {feature['id']}")
        require(
            feature["status"] in feature_status_values,
            f"Unsupported feature audit status: {feature['id']}",
        )
        require(
            feature["implementation_status"] in implementation_status_values,
            f"Unsupported implementation status: {feature['id']}",
        )
        require(isinstance(feature["tests"], list), f"Invalid tests field: {feature['id']}")
        for test_id in feature["tests"]:
            require(
                test_id in tests_by_id,
                f"Unknown test plan reference on {feature['id']}: {test_id}",
            )
        if m1_complete:
            if feature["release_class"] == "release_blocker":
                require(
                    feature["status"] == "verified",
                    f"Release blocker is not verified: {feature['id']}",
                )
                require(feature["tests"], f"Release blocker has no test plan: {feature['id']}")

    feature_ids = [item["id"] for item in features]
    require(len(feature_ids) == len(set(feature_ids)), "Duplicate feature IDs")

    migration_artifacts = load_toml("migrations/v3/artifacts.toml")
    require(migration_artifacts["schema"] == 2, "Unsupported migration artifact schema")
    require(
        isinstance(migration_artifacts["instance_snapshot_available"], bool),
        "Invalid migration instance snapshot state",
    )
    artifact_ids: list[str] = []
    valid_classifications = set(migration_artifacts["classification"])
    for artifact in migration_artifacts["artifact"]:
        artifact_ids.append(artifact["id"])
        require(artifact["kind"], f"Artifact has no kind: {artifact['id']}")
        require(artifact["location"], f"Artifact has no location: {artifact['id']}")
        require(artifact["owner"], f"Artifact has no owner: {artifact['id']}")
        require(artifact["evidence"], f"Artifact has no evidence: {artifact['id']}")
        require(artifact["residue_tests"], f"Artifact has no residue test: {artifact['id']}")
        require(
            all(test_id in tests_by_id for test_id in artifact["residue_tests"]),
            f"Artifact references an unknown residue test: {artifact['id']}",
        )
        classifications = set(artifact["classification"].split(","))
        require(
            classifications <= valid_classifications,
            f"Artifact has an invalid classification: {artifact['id']}",
        )
        require(
            artifact["template_confirmed"] is True,
            f"Artifact template lacks source confirmation: {artifact['id']}",
        )
        if artifact["confirmed"] and artifact["classification"] != "N":
            require(
                migration_artifacts["instance_snapshot_available"] is True,
                f"Artifact instance is confirmed without a snapshot: {artifact['id']}",
            )
    require(len(artifact_ids) == len(set(artifact_ids)), "Duplicate migration artifact IDs")

    source_lock = load_toml("docs/baselines/source-lock.toml")
    require(source_lock["schema"] == 5, "Unsupported source lock schema")
    require(source_lock["frozen"] is True, "Source/spec inputs are not frozen")

    for section_name in [
        "alcomd3_v3_audit_source",
        "alcomd3_v3_migration_entry_release",
        "alcomd3_v3_migration_assets",
        "vrc_get_function_behavior",
        "mcp",
    ]:
        require(
            source_lock[section_name]["status"] == "frozen",
            f"Baseline section is not frozen: {section_name}",
        )

    for section_name in [
        "alcomd3_v3_audit_source",
        "alcomd3_v3_migration_entry_release",
        "vrc_get_function_behavior",
    ]:
        require(
            COMMIT_PATTERN.fullmatch(source_lock[section_name]["commit"]) is not None,
            f"Invalid frozen commit: {section_name}",
        )

    audit_source = source_lock["alcomd3_v3_audit_source"]
    migration_entry = source_lock["alcomd3_v3_migration_entry_release"]
    require(
        audit_source["repository"]
        == migration_entry["repository"]
        == "https://github.com/ALCOMD/ALCOMD3.git",
        "Unexpected v3 baseline repository",
    )
    require(migration_entry["tag"] == "v3.4.0", "Unexpected migration tag")
    require(migration_entry["version"] == "3.4.0", "Unexpected migration version")
    require(audit_source["tag"] == migration_entry["tag"], "Audit tag mismatch")
    require(audit_source["version"] == migration_entry["version"], "Audit version mismatch")
    require(
        audit_source["commit"]
        == audit_source["tag_commit"]
        == migration_entry["commit"]
        == migration_entry["tag_commit"],
        "v3 audit source and migration entry do not resolve to one commit",
    )
    for section_name, section in [
        ("alcomd3_v3_audit_source", audit_source),
        ("vrc_get_function_behavior", source_lock["vrc_get_function_behavior"]),
    ]:
        require(section["remote_verified"] is True, f"Remote is unverified: {section_name}")
        require(section["tags_complete"] is True, f"Tags are incomplete: {section_name}")
        require(section["shallow"] is False, f"Shallow source is forbidden: {section_name}")
        require(
            section["remote_ref_at_freeze"].startswith("refs/heads/"),
            f"Invalid frozen remote ref: {section_name}",
        )
        expected_identity = (
            section["repository"]
            .removeprefix("https://github.com/")
            .removesuffix(".git")
            .lower()
        )
        require(
            section["commit_api_url"]
            == f"https://api.github.com/repos/{expected_identity}/commits/{section['commit']}",
            f"Invalid commit API evidence: {section_name}",
        )

    for field_name in ["version_manifest_blob_sha1"]:
        require(
            COMMIT_PATTERN.fullmatch(audit_source[field_name]) is not None,
            f"Invalid Git blob ID: {field_name}",
        )
    require(
        COMMIT_PATTERN.fullmatch(migration_entry["config_blob_sha1"]) is not None,
        "Invalid frozen config blob ID",
    )
    require(
        migration_entry["stable_update_api"]
        == "https://alcomd.cqmhv.com/api/v1/updates/stable.json",
        "Unexpected stable update API",
    )
    require(
        migration_entry["beta_update_api"]
        == "https://alcomd.cqmhv.com/api/v1/updates/beta.json",
        "Unexpected beta update API",
    )

    migration_assets = source_lock["alcomd3_v3_migration_assets"]
    require(
        migration_assets["repository"] == migration_entry["repository"],
        "Migration asset repository mismatch",
    )
    require(migration_assets["release_id"] > 0, "Invalid migration release ID")
    require(
        migration_assets["release_tag"] == migration_entry["tag"],
        "Migration asset release tag mismatch",
    )
    require(
        migration_assets["release_commit"] == migration_entry["commit"],
        "Migration asset release commit mismatch",
    )
    require(
        migration_assets["release_api_url"]
        == "https://api.github.com/repos/ALCOMD/ALCOMD3/releases/tags/v3.4.0",
        "Unexpected migration release API URL",
    )
    require(
        isinstance(migration_assets["release_immutable"], bool),
        "Invalid GitHub release immutable state",
    )
    require(
        COMMIT_PATTERN.fullmatch(migration_assets["updater_public_key_blob_sha1"])
        is not None,
        "Invalid updater public-key blob ID",
    )
    require(
        SHA256_FINGERPRINT_PATTERN.fullmatch(
            migration_assets["updater_public_key_fingerprint"]
        )
        is not None,
        "Invalid updater public-key fingerprint",
    )
    require(
        re.fullmatch(r"[0-9A-F]{16}", migration_assets["updater_public_key_minisign_id"])
        is not None,
        "Invalid updater minisign key ID",
    )

    updater_assets = migration_assets["updater_assets"]
    installer_assets = migration_assets["installer_assets"]
    require(
        {item["platform"] for item in updater_assets}
        == {"windows-x86_64", "darwin-aarch64", "linux-x86_64"},
        "Updater asset platform set is incomplete",
    )
    require(len(installer_assets) == 4, "Installer asset set is incomplete")
    asset_ids: list[int] = []
    asset_names: list[str] = []
    for asset in updater_assets:
        require(asset["asset_id"] > 0 and asset["asset_size"] > 0, "Invalid updater asset")
        require(
            SHA256_PATTERN.fullmatch(asset["asset_sha256"]) is not None,
            f"Invalid updater asset digest: {asset['asset_name']}",
        )
        require(
            asset["asset_url"].endswith(f"/{asset['asset_name']}"),
            f"Updater asset URL/name mismatch: {asset['asset_name']}",
        )
        require(
            asset["signature_asset_name"] == f"{asset['asset_name']}.sig",
            f"Updater signature mismatch: {asset['asset_name']}",
        )
        require(
            asset["signature_asset_id"] > 0 and asset["signature_asset_size"] > 0,
            f"Invalid updater signature asset: {asset['asset_name']}",
        )
        require(
            SHA256_PATTERN.fullmatch(asset["signature_asset_sha256"]) is not None,
            f"Invalid updater signature digest: {asset['asset_name']}",
        )
        require(
            asset["signature_asset_url"].endswith(
                f"/{asset['signature_asset_name']}"
            ),
            f"Updater signature URL/name mismatch: {asset['asset_name']}",
        )
        asset_ids.extend([asset["asset_id"], asset["signature_asset_id"]])
        asset_names.extend([asset["asset_name"], asset["signature_asset_name"]])

    for asset in installer_assets:
        require(asset["asset_id"] > 0 and asset["asset_size"] > 0, "Invalid installer asset")
        require(asset["config_id"] and asset["format"], "Installer asset lacks provenance")
        require(
            SHA256_PATTERN.fullmatch(asset["asset_sha256"]) is not None,
            f"Invalid installer asset digest: {asset['asset_name']}",
        )
        require(
            asset["asset_url"].endswith(f"/{asset['asset_name']}"),
            f"Installer asset URL/name mismatch: {asset['asset_name']}",
        )
        asset_ids.append(asset["asset_id"])
        asset_names.append(asset["asset_name"])

    require(len(asset_ids) == len(set(asset_ids)), "Duplicate release asset IDs")
    require(len(asset_names) == len(set(asset_names)), "Duplicate release asset names")
    require(
        migration_assets["release_asset_count"] == len(asset_ids),
        "Release asset count does not match frozen assets",
    )

    vrc_get_behavior = source_lock["vrc_get_function_behavior"]
    require(
        vrc_get_behavior["repository"] == "https://github.com/vrc-get/vrc-get.git",
        "Unexpected vrc-get baseline repository",
    )
    require(
        vrc_get_behavior["usage"] == "function-and-behavior-audit-only",
        "vrc-get baseline must remain audit-only",
    )
    require(
        vrc_get_behavior["scopes"]
        == ["functionality", "security", "cli", "error-handling"],
        "vrc-get audit scopes are incomplete",
    )
    require(
        vrc_get_behavior["source_reuse"] is False,
        "vrc-get source reuse must remain disabled",
    )
    require(
        vrc_get_behavior["exact_tag"] == bool(vrc_get_behavior["exact_tags"]),
        "vrc-get exact-tag state does not match its tag list",
    )

    features_by_id = {item["id"]: item for item in parity["feature"]}
    for feature_id in [
        "projects.management",
        "packages.vpm",
        "repositories.management",
        "unity.integration",
        "cli.complete",
    ]:
        require(
            "vrc-get-frozen" in features_by_id[feature_id]["source"],
            f"Missing vrc-get functional baseline source: {feature_id}",
        )

    require(
        (ROOT / "docs/baselines/vrc-get.md").is_file(),
        "Missing vrc-get functional baseline document",
    )

    provenance = load_toml("docs/baselines/asset-provenance.toml")
    require(provenance["schema"] == 1, "Unsupported asset provenance schema")
    provenance_assets = provenance["asset"]
    provenance_ids = [asset["id"] for asset in provenance_assets]
    require(len(provenance_ids) == len(set(provenance_ids)), "Duplicate provenance IDs")
    covered_asset_paths: set[str] = set()
    for asset in provenance_assets:
        require(asset["source_repository"], f"Missing provenance repository: {asset['id']}")
        require(
            COMMIT_PATTERN.fullmatch(asset["source_commit"]) is not None,
            f"Invalid provenance commit: {asset['id']}",
        )
        require(asset["source_path"], f"Missing provenance source path: {asset['id']}")
        require(
            SHA256_PATTERN.fullmatch(asset["source_sha256"]) is not None,
            f"Invalid provenance source digest: {asset['id']}",
        )
        require(
            asset["source_commit"] == audit_source["commit"],
            f"Provenance source is outside the frozen v3 commit: {asset['id']}",
        )
        require((ROOT / asset["license_file"]).is_file(), f"Missing asset license: {asset['id']}")
        for file_entry in asset["files"]:
            relative = file_entry["path"]
            require(relative not in covered_asset_paths, f"Duplicate asset provenance: {relative}")
            asset_path = ROOT / relative
            require(asset_path.is_file(), f"Provenance file does not exist: {relative}")
            digest = hashlib.sha256(asset_path.read_bytes()).hexdigest()
            require(digest == file_entry["sha256"], f"Asset digest mismatch: {relative}")
            covered_asset_paths.add(relative)

    known_icon_paths = {"apps/alcomd-gui/app-icon.png"}
    known_icon_paths.update(
        path.relative_to(ROOT).as_posix()
        for path in (ROOT / "apps/alcomd-gui/src-tauri/icons").iterdir()
        if path.is_file()
    )
    require(
        known_icon_paths <= covered_asset_paths,
        "One or more known GUI icons have no provenance entry",
    )

    mcp = source_lock["mcp"]
    require(mcp["specification"] == "2026-07-28", "Unexpected MCP specification")
    require(
        mcp["url"]
        == "https://modelcontextprotocol.io/specification/2026-07-28",
        "Unexpected MCP specification URL",
    )
    require(
        mcp["repository"]
        == "https://github.com/modelcontextprotocol/modelcontextprotocol.git",
        "Unexpected MCP specification repository",
    )
    for field_name in ["commit", "schema_blob_sha1", "conformance_tarball_sha1"]:
        require(
            COMMIT_PATTERN.fullmatch(mcp[field_name]) is not None,
            f"Invalid MCP frozen identity: {field_name}",
        )
    require(
        SHA256_PATTERN.fullmatch(mcp["schema_sha256"]) is not None,
        "Invalid MCP schema SHA-256",
    )
    require(
        mcp["schema_path"] == "schema/2026-07-28/schema.ts",
        "Unexpected MCP schema path",
    )
    require(
        mcp["conformance_package"] == "@modelcontextprotocol/conformance",
        "Unexpected MCP conformance package",
    )
    require(mcp["conformance_version"], "Missing MCP conformance version")
    require(
        re.fullmatch(r"sha512-[A-Za-z0-9+/]+={0,2}", mcp["conformance_tarball_integrity"])
        is not None,
        "Invalid MCP conformance integrity",
    )

    for extension_name in ["mcp", "discord"]:
        relative = (
            "extensions/first-party/"
            f"alcomd-extension-{extension_name}/alcomd-extension.toml"
        )
        manifest = load_toml(relative)
        require(manifest["schema"] == 1, f"Unsupported extension schema: {relative}")
        require(
            manifest["id"]
            == f"{product['identity']['bundle_identifier']}.extension.{extension_name}",
            f"Unexpected first-party extension ID: {relative}",
        )
        require(
            manifest["publisher"] == product["product"]["publisher_name"],
            f"Unexpected first-party extension publisher: {relative}",
        )
        require(
            manifest["license"] == PROJECT_LICENSE,
            f"Unexpected first-party extension license: {relative}",
        )

    cargo_workspace = load_toml("Cargo.toml")
    workspace_package = cargo_workspace["workspace"]["package"]
    require(
        workspace_package["version"] == product["product"]["version"],
        "Rust workspace version does not match the product version",
    )
    require(workspace_package["edition"] == "2024", "Unexpected Rust edition")
    require(
        workspace_package["rust-version"] == RUST_VERSION,
        "Unexpected Rust version",
    )
    require(
        workspace_package["license"] == PROJECT_LICENSE,
        "Unexpected Rust workspace license",
    )
    require(
        workspace_package["repository"] == PROJECT_REPOSITORY,
        "Unexpected Rust workspace repository",
    )
    expected_workspace_members = {
        "apps/alcomd",
        "apps/alcomd-cli",
        "apps/alcomd-mcp",
        "apps/alcomd-api",
        "apps/alcomd-extension-host",
        "apps/alcomd-bootstrap",
        "apps/alcomd-updater",
        "apps/alcomd-gui/src-tauri",
        "crates/alcomd-domain",
        "crates/alcomd-application",
        "crates/alcomd-protocol",
        "crates/alcomd-client",
        "crates/alcomd-store",
        "crates/alcomd-platform",
        "crates/alcomd-vpm",
        "crates/alcomd-extensions",
        "crates/alcomd-import",
        "crates/alcomd-testing",
        "migrations/v3/app/alcomd-migrate-v3",
        "xtask",
    }
    require(
        set(cargo_workspace["workspace"]["members"]) == expected_workspace_members,
        "Unexpected Cargo workspace members",
    )
    tokio_features = set(cargo_workspace["workspace"]["dependencies"]["tokio"]["features"])
    require(
        {"io-util", "macros", "net", "rt-multi-thread", "signal", "sync", "time"}
        <= tokio_features,
        "Tokio lacks an M1-required feature",
    )
    require("full" not in tokio_features, "Tokio full feature is not allowed")

    platform_manifest = load_toml("crates/alcomd-platform/Cargo.toml")
    unix_rustix = platform_manifest["target"]["cfg(unix)"]["dependencies"]["rustix"]
    require(unix_rustix["version"] == "=1.1.4", "Unexpected rustix version")
    require(unix_rustix["default-features"] is False, "rustix defaults must be disabled")
    require(
        set(unix_rustix["features"]) == {"std", "fs", "process"},
        "Unexpected rustix feature set",
    )
    windows_sys = platform_manifest["target"]["cfg(windows)"]["dependencies"][
        "windows-sys"
    ]
    require(windows_sys["version"] == "=0.61.2", "Unexpected windows-sys version")
    require(
        set(windows_sys["features"])
        == {
            "Win32_Foundation",
            "Win32_Security",
            "Win32_Security_Authorization",
            "Win32_System_Com",
            "Win32_System_Threading",
            "Win32_UI_Shell",
        },
        "Unexpected windows-sys feature set",
    )
    require(
        platform_manifest["lints"]["rust"]["unsafe_code"] == "deny",
        "alcomd-platform must deny unsafe_code by default",
    )
    require(
        platform_manifest["lints"]["clippy"]["undocumented_unsafe_blocks"] == "deny",
        "alcomd-platform must deny undocumented unsafe blocks",
    )

    cargo_lock = load_toml("Cargo.lock")
    locked_packages = {
        (package["name"], package["version"]) for package in cargo_lock["package"]
    }
    require(("rustix", "1.1.4") in locked_packages, "rustix 1.1.4 is not locked")
    require(
        ("linux-raw-sys", "0.12.1") in locked_packages,
        "linux-raw-sys 0.12.1 is not locked",
    )
    root_manifest = load_toml("Cargo.toml")
    rusqlite = root_manifest["workspace"]["dependencies"]["rusqlite"]
    require(rusqlite["version"] == "=0.40.1", "Unexpected rusqlite version")
    require(rusqlite["default-features"] is False, "rusqlite defaults must be disabled")
    require(set(rusqlite["features"]) == {"bundled"}, "Unexpected rusqlite feature set")
    require(("rusqlite", "0.40.1") in locked_packages, "rusqlite 0.40.1 is not locked")
    require(
        ("libsqlite3-sys", "0.38.1") in locked_packages,
        "libsqlite3-sys 0.38.1 is not locked",
    )
    require(
        sum(1 for name, _ in locked_packages if name == "rustix") == 1,
        "Multiple rustix versions are locked",
    )

    rust_toolchain = load_toml("rust-toolchain.toml")
    require(
        rust_toolchain["toolchain"]["channel"] == RUST_VERSION,
        "Unexpected rust-toolchain channel",
    )

    discord_backend = load_toml(
        "extensions/first-party/alcomd-extension-discord/backend/Cargo.toml"
    )
    require(
        discord_backend["package"]["repository"]
        == PROJECT_REPOSITORY,
        "Unexpected Discord backend repository",
    )

    npm_package_paths = [
        "package.json",
        "apps/alcomd-gui/package.json",
        "packages/alcomd-extension-sdk/package.json",
        "packages/alcomd-sdk/package.json",
        "packages/alcomd-ui/package.json",
    ]
    for relative in npm_package_paths:
        package = load_json(relative)
        require(
            package["license"] == PROJECT_LICENSE,
            f"Unexpected npm package license: {relative}",
        )

    root_package = load_json("package.json")
    require(
        root_package["name"] == f"{product['product']['technical_name']}-workspace",
        "Unexpected root npm package name",
    )
    require(
        root_package["version"] == product["product"]["version"],
        "Root npm version does not match the product version",
    )
    require(root_package["engines"]["node"] == ">=24 <25", "Unexpected Node engine")

    gui_package = load_json("apps/alcomd-gui/package.json")
    require(
        gui_package["name"] == f"@{product['product']['technical_name']}/gui",
        "Unexpected GUI npm package name",
    )
    require(
        gui_package["version"] == product["product"]["version"],
        "GUI npm version does not match the product version",
    )

    package_lock = load_json("package-lock.json")
    lock_root = package_lock["packages"][""]
    require(
        lock_root["version"] == product["product"]["version"],
        "package-lock root version does not match the product version",
    )
    require(
        lock_root["engines"]["node"] == root_package["engines"]["node"],
        "package-lock Node engine does not match package.json",
    )

    for relative in [
        "apps/alcomd-gui/src-tauri/capabilities/main.json",
        "specs/rpc/system-hello.request.schema.json",
        "specs/rpc/system-hello.response.schema.json",
        "specs/rpc/request-envelope.schema.json",
        "specs/rpc/response-envelope.schema.json",
        "specs/rpc/rpc-error.schema.json",
        "specs/rpc/system-status.request.schema.json",
        "specs/rpc/system-status.response.schema.json",
        "specs/rpc/operation.schema.json",
        "specs/rpc/event.schema.json",
        "specs/rpc/state-check.request.schema.json",
        "specs/rpc/state-check.response.schema.json",
        "specs/rpc/operations-get.request.schema.json",
        "specs/rpc/operations-get.response.schema.json",
        "specs/rpc/operations-list.request.schema.json",
        "specs/rpc/operations-list.response.schema.json",
        "specs/rpc/operations-cancel.request.schema.json",
        "specs/rpc/operations-cancel.response.schema.json",
        "specs/rpc/events-list.request.schema.json",
        "specs/rpc/events-list.response.schema.json",
        "specs/extensions/manifest-v1.schema.json",
        "migrations/v3/schemas/migration-bundle-v1.schema.json",
    ]:
        load_json(relative)

    require((ROOT / "LICENSE").is_file(), "Missing LICENSE file")

    if m1_complete:
        print("Metadata validation passed; M-1 completion contract is satisfied.")
    else:
        missing_test_plans = sum(
            1
            for feature in features
            if feature["release_class"] == "release_blocker" and not feature["tests"]
        )
        print(
            "Metadata draft validation passed; M-1 is not complete "
            f"({missing_test_plans} release blockers still lack test plans)."
        )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (KeyError, OSError, TypeError, ValueError, tomllib.TOMLDecodeError) as error:
        print(f"Metadata validation failed: {error}", file=sys.stderr)
        raise SystemExit(1) from error
