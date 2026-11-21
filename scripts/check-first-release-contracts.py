"""Check the owned contract registry, SDK schemas and component imports together."""
import json
import os
from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[1]
SPEC = ROOT / "specs"


def unique_fields(pairs):
    record = {}
    for key, value in pairs:
        if key in record:
            raise ValueError(f"duplicate JSON field {key}")
        record[key] = value
    return record


def read_json(path):
    try:
        return json.loads(path.read_text(encoding="utf-8"), object_pairs_hook=unique_fields)
    except ValueError as error:
        raise ValueError(f"{path}: {error}") from error


def unique_names(values, label):
    result = set()
    for value in values:
        if value in result:
            raise ValueError(f"duplicate {label}: {value}")
        result.add(value)
    return result


def check_first_release_versions(release):
    schemas = release["schemas"]
    config = read_json(SPEC / schemas["Config"])
    hello = read_json(SPEC / schemas["Handshake"])["properties"]["result"]["properties"]
    abi = read_json(SPEC / schemas["Extension ABI"])
    abi_fields = hello["extensionApi"]["properties"]
    if abi_fields["world"]["const"] != abi["world"]:
        raise ValueError("handshake base world disagrees with the extension ABI contract")
    exchange = read_json(SPEC / "formats/repository-configuration-v1.schema.json")
    change_set = read_json(SPEC / "rpc/m4-package-transaction.schema.json")["$defs"]["changeSet"]
    versions = {
        "registry": release["contractVersion"],
        "RPC": release["rpcVersion"],
        "Config": config["properties"]["schema"]["const"],
        "ABI": abi["abiMajor"],
        "handshake ABI": abi_fields["major"]["const"],
        "repository exchange": exchange["properties"]["version"]["const"],
        "package change set": change_set["properties"]["formatVersion"]["const"],
        **{name: hello[name]["const"] for name in ("rpcVersion", "dataSchema", "configSchema")},
    }
    for name, version in versions.items():
        if type(version) is not int or version != 1:
            raise ValueError(f"{name}: expected first-release version 1")
    for relative in ("alcomd.product.toml", "docs/baselines/source-lock.toml"):
        if not re.search(r"(?m)^schema = 1$", (ROOT / relative).read_text(encoding="utf-8")):
            raise ValueError(f"{relative}: expected schema 1")
    for path, name in [
        ("crates/alcomd-store/src/lib.rs", "CURRENT_DATA_SCHEMA"),
        ("crates/alcomd-store/src/config.rs", "SCHEMA_VERSION"),
        ("crates/alcomd-protocol/src/lib.rs", "CONFIG_SCHEMA_VERSION"),
        ("crates/alcomd-protocol/src/lib.rs", "RPC_VERSION"),
    ]:
        source = (ROOT / path).read_text(encoding="utf-8")
        if not re.search(rf"\bconst {name}: u32 = 1;", source):
            raise ValueError(f"{path}: first-release {name} must be 1")
    # These Store methods construct ALCOMD request fingerprints. Restrict the
    # check to their bodies so package manifests and input fixture versions are independent.
    for module, functions in {
        "m3.rs": ("registration_fingerprint", "credential_fingerprint"),
        "m5.rs": ("launch_fingerprint",),
        "m6.rs": ("accept_plan_on", "set_desired"),
    }.items():
        source = (ROOT / "crates/alcomd-store/src" / module).read_text(encoding="utf-8")
        for name in functions:
            body = re.search(rf"(?ms)^(?:pub\(super\) )?fn {name}\b.*?^}}", source)
            tags = re.findall(r'"version"\s*:\s*(\d+)', body[0].replace(r'\"', '"')) if body else []
            if set(tags) != {"1"}:
                raise ValueError(f"{module}:{name}: expected first-release fingerprint version 1")


def main():
    declarations = []
    for path in (ROOT / "crates/alcomd-protocol/src").rglob("*.rs"):
        declarations.extend(re.findall(
            r'(?m)^pub const (METHOD|CAPABILITY)_\w+: &str =\s*"([^"]+)";',
            path.read_text(encoding="utf-8"),
        ))
    methods = unique_names((v for k, v in declarations if k == "METHOD"), "RPC method")
    capabilities = unique_names((v for k, v in declarations if k == "CAPABILITY"), "capability")
    if not methods or not capabilities or any(not cap.endswith(".v1") for cap in capabilities):
        raise ValueError("the first-release capability registry must contain only v1 entries")
    release = read_json(SPEC / "first-release-contracts.json")
    check_first_release_versions(release)
    manifest = read_json(SPEC / release["nativeMethods"])
    expected_methods = unique_names([*manifest["methods"], *release["serviceMethods"]], "release method")
    if methods != expected_methods or capabilities != unique_names(release["capabilities"], "release capability"):
        raise ValueError("Core declarations disagree with the first-release manifest")
    for relative in [release["permissions"], *release["schemas"].values()]:
        if not (SPEC / relative).is_file():
            raise ValueError(f"missing contract: {relative}")
    matrix = (ROOT / "docs/exec-plans/M11-client-operation-matrix.md").read_text(encoding="utf-8")
    rows = re.findall(r"(?m)^\| `([^\s`]+)` \| (?:已提供|缺少入口|仅本机交互) \|", matrix)
    if unique_names(rows, "client operation matrix row") != set(manifest["methods"]):
        raise ValueError("client operation matrix disagrees with the native method catalogue")
    if manifest["rpcVersion"] != 1:
        raise ValueError("invalid SDK protocol version")
    for name, entry in manifest["methods"].items():
        required = unique_names(entry["capabilities"], f"{name} capability")
        if name not in methods or (required | set(entry.get("capabilityByKind", {}).values())) - capabilities:
            raise ValueError(f"{name}: unknown method or capability")
        for key in ("requestSchema", "resultSchema"):
            file, _, pointer = entry[key].partition("#")
            value = read_json(SPEC / "rpc" / file)
            for segment in pointer.lstrip("/").split("/") if pointer else ():
                value = value[int(segment)] if isinstance(value, list) else value[segment.replace("~1", "/").replace("~0", "~")]

    tools = []
    for path in (SPEC / "mcp").glob("*.schema.json"):
        catalog = read_json(path)
        for item in catalog.get("x-alcomd-tools", []):
            tools.append(item["name"])
    unique_names(tools, "MCP tool")
    for path in (SPEC / "extensions/wit").rglob("*.wit"):
        for version in re.findall(r'alcomd:[\w/-]+@(\d+\.\d+\.\d+(?:[-+][\w.-]+)?)', path.read_text(encoding="utf-8")):
            if version != "1.0.0":
                raise ValueError(f"{path}: invalid first-release interface {version}")
    for path in SPEC.rglob("*.json"):
        document = read_json(path)
        if isinstance(document, dict):
            for field in ("contractVersion", "schemaVersion", "formatVersion", "rpcVersion",
                          "x-alcomd-state-version", "x-alcomd-state-schema", "x-alcomd-state-schema-proposed"):
                if field in document and document[field] != 1:
                    raise ValueError(f"{path}: {field} must be 1")
    # Paths for contracts retired before first release must not return as live links.
    family = re.compile(r'(?<![\w-])(?:settings|state|mcp-management|m8-mcp-(?:oauth|clients)|backup-commands)-v(\d+)(?=[./\\#`\s)]|$)')
    capability_reference = re.compile(
        r"(?<![\w.-])(?:" + "|".join(re.escape(cap[:-3]) for cap in sorted(capabilities))
        + r")\.v(\d+)\b"
    )
    link = re.compile(r"\[[^\[\]]*\]\(([^)]+)\)")
    for directory in ("specs", "docs", "crates", "apps", "packages", "migrations", "scripts", "xtask"):
        for path in tracked_text(directory):
            source = path.read_text(encoding="utf-8")
            for pattern in (family, capability_reference):
                for match in pattern.finditer(source):
                    if match[1] != "1":
                        raise ValueError(f"{path.relative_to(ROOT)}: invalid contract reference {match[0]}")
            if path.suffix == ".md":
                for match in link.finditer(source):
                    target = match[1].strip("<>").split("#", 1)[0]
                    if not target or ":" in target or target.startswith(("/", "$")):
                        continue
                    if not (path.parent / target).exists():
                        raise ValueError(f"{path.relative_to(ROOT)}: missing linked file {target}")
    if not tools:
        raise ValueError("missing MCP tool catalog")
    print(f"First-release contracts checked: {len(methods)} RPC methods, "
          f"{len(capabilities)} capabilities, {len(tools)} MCP tools.")


def tracked_text(directory):
    excluded = {"target", "node_modules", "dist", "build", "test-results", "playwright-report", ".git", ".local"}
    for parent, children, names in os.walk(ROOT / directory):
        children[:] = [child for child in children if child not in excluded]
        for name in names:
            path = Path(parent) / name
            if path.suffix in {".rs", ".md", ".json", ".toml", ".wit", ".ts", ".tsx", ".py"}:
                yield path


if __name__ == "__main__":
    try:
        main()
    except (IndexError, KeyError, TypeError, ValueError, OSError) as error:
        sys.exit(str(error))
