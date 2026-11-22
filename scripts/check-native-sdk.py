"""Verify the native SDK catalogue, its TypeScript binding and official GUI coverage."""

import json
import re
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
RPC = ROOT / "specs/rpc"


def check_reference(reference):
    file_name, _, pointer = reference.partition("#")
    path = RPC / file_name
    if path.parent != RPC or not file_name.endswith(".schema.json"):
        raise ValueError(f"unsupported native SDK Schema reference {reference}")
    value = json.loads(path.read_text(encoding="utf-8"))
    for segment in pointer.split("/")[1:]:
        value = value[int(segment)] if isinstance(value, list) else value[segment.replace("~1", "/").replace("~0", "~")]
    if not isinstance(value, dict):
        raise ValueError(f"expected a Schema object at {reference}")
    return value


def check_native_sdk():
    catalogue = json.loads((RPC / "native-sdk-v1.methods.json").read_text(encoding="utf-8"))
    methods = catalogue["methods"]
    assert catalogue["rpcVersion"] == 1
    allowed_error_codes = set(check_reference(catalogue["errors"])["properties"]["code"]["enum"])
    request_errors = catalogue["requestErrorCodes"]
    capability_error = catalogue["capabilityErrorCode"]
    profiles = catalogue["errorProfiles"]
    assert list(profiles) == sorted(profiles), "native SDK error profiles must have stable order"
    assert capability_error == "capability_required" and capability_error in allowed_error_codes
    for name, codes in [("requestErrorCodes", request_errors), *profiles.items()]:
        assert codes and codes == sorted(set(codes)), f"{name} must contain unique sorted codes"
        unexpected = set(codes) - allowed_error_codes
        assert not unexpected, f"{name} has errors outside the public enum: {sorted(unexpected)}"
    used_profiles = set()
    assert list(methods) == sorted(methods), "native SDK methods must have stable order"

    protocol = "\n".join(
        path.read_text(encoding="utf-8")
        for path in (ROOT / "crates/alcomd-protocol/src").glob("*.rs")
    )
    constants = dict(re.findall(r'const\s+([A-Z_0-9]+)\s*:\s*&str\s*=\s*"([^"]+)"', protocol))
    capabilities = {value for name, value in constants.items() if name.startswith("CAPABILITY_")}
    declared_methods = {value for name, value in constants.items() if name.startswith("METHOD_")}
    declared_types = set(re.findall(r"\bpub\s+(?:struct|enum|type)\s+(\w+)", protocol))
    declared_types.add("EmptyParams")
    assert declared_methods.issuperset(methods), "catalogue contains an undeclared RPC method"

    rust = "\n".join(path.read_text(encoding="utf-8") for path in (ROOT / "crates/alcomd-client/src").glob("*.rs"))
    wrappers = {}
    for entry in re.finditer(r"    pub async fn (\w+)\(.*?(?=\n    (?:pub |async |fn )|\Z)", rust, re.S):
        method_call = re.search(r"\.call\(\s*(?:alcomd_protocol::)?(METHOD_\w+),", entry[0])
        if method_call:
            wrappers[entry[1]] = constants[method_call[1]]
    wrappers["EventSubscription::connect"] = constants["METHOD_EVENTS_SUBSCRIBE"]
    service_methods = set(json.loads((RPC.parent / "first-release-contracts.json").read_text(encoding="utf-8"))["serviceMethods"])
    for wire, entry in methods.items():
        assert entry["kind"] in ("read", "write", "stream"), wire
        assert entry["kind"] != "stream" or wire == "events.subscribe", wire
        assert wire not in service_methods, f"{wire} is reserved for the protocol adapter"
        assert wrappers[entry["rustMethod"]] == wire, wire
        assert capabilities.issuperset(entry["capabilities"]), wire
        error_profiles = entry["errorProfiles"]
        assert error_profiles == sorted(set(error_profiles)), f"{wire} has duplicate or unordered error profiles"
        assert profiles.keys() >= set(error_profiles), f"{wire} references an unknown error profile"
        used_profiles.update(error_profiles)
        for role in ("request", "result"):
            assert entry[f"{role}Type"] in declared_types, wire
            check_reference(entry[f"{role}Schema"])
    assert used_profiles == profiles.keys(), "native SDK error catalogue contains unused profiles"

    gui = "\n".join(
        path.read_text(encoding="utf-8")
        for path in (ROOT / "apps/alcomd-gui/src-tauri/src").glob("*.rs")
    )
    gui_calls = set(re.findall(r"\b(?:client|connection|rpc)\s*\.\s*(\w+)\s*\(", gui))
    gui_methods = {wrappers[call] for call in gui_calls.intersection(wrappers)}
    if "EventSubscription::connect(" in gui:
        gui_methods.add("events.subscribe")
    uncovered = gui_methods.difference(methods)
    assert not uncovered, f"missing official GUI methods in the native SDK: {sorted(uncovered)}"

    ts = (ROOT / "packages/alcomd-sdk/src/sdk-methods.ts").read_text(encoding="utf-8")
    ts_kinds = dict(re.findall(r'^    "([^"]+)": "(read|write|stream)",$', ts, re.M))
    ts_types = re.findall(r'^    "([^"]+)": \{ params: .+; result: .+ \};$', ts, re.M)
    assert ts_kinds == {wire: entry["kind"] for wire, entry in methods.items()}
    assert ts_types == list(methods), "TypeScript bindings drifted from the public catalogue"
    print(
        f"Native SDK contract passed ({len(methods)} methods, {len(gui_methods)} GUI methods, "
        f"{len(profiles)} error profiles)."
    )


if __name__ == "__main__":
    check_native_sdk()
