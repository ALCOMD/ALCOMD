"""Project the named MCP tools' closed schemas from the current native contracts."""

import argparse
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RPC = ROOT / "specs/rpc"
CATALOG = ROOT / "specs/mcp/project-catalog-v1.schema.json"
MAX_SAFE = 9_007_199_254_740_991


def snake(name):
    return re.sub(r"[A-Z]", lambda match: "_" + match.group().lower(), name)


class Schemas:
    def __init__(self):
        self.files = {p.name: json.loads(p.read_text(encoding="utf-8")) for p in RPC.glob("*.schema.json")}
        self.ids = {schema["$id"]: name for name, schema in self.files.items() if "$id" in schema}

    def resolve(self, ref, current):
        origin, _, pointer = ref.partition("#")
        filename = self.ids.get(origin, origin) if origin else current
        value = self.files[filename]
        for part in pointer.split("/")[1:]:
            key = part.replace("~1", "/").replace("~0", "~")
            value = value[int(key)] if isinstance(value, list) else value[key]
        return filename, pointer, value

    def project(self, root, *, prefix=""):
        definitions = {}
        labels = {}

        def convert(value, filename):
            if isinstance(value, list):
                return [convert(item, filename) for item in value]
            if not isinstance(value, dict):
                return value
            output = {}
            for key, node in value.items():
                if key == "$ref":
                    target, pointer, schema = self.resolve(node, filename)
                    identity = (target, pointer)
                    if identity not in labels:
                        label = prefix + pointer.removeprefix("/$defs/").replace("/", "-")
                        if label in labels.values():
                            label = prefix + target.split(".")[0] + "-" + label.removeprefix(prefix)
                        labels[identity] = label
                        definitions[label] = convert(schema, target)
                    output[key] = "#/$defs/" + labels[identity]
                elif key == "properties":
                    output[key] = {snake(field): convert(spec, filename) for field, spec in node.items()}
                elif key == "required":
                    output[key] = [snake(field) for field in node]
                elif key == "maximum":
                    output[key] = min(MAX_SAFE, node)
                else:
                    output[key] = convert(node, filename)
            return output

        filename, _, root_schema = self.resolve(root, None)
        return convert(root_schema, filename), definitions


def generated():
    catalog = json.loads(CATALOG.read_text(encoding="utf-8"))
    methods = json.loads((RPC / "native-sdk-v1.methods.json").read_text(encoding="utf-8"))["methods"]
    base = json.loads((ROOT / "specs/mcp/package-removal-v1.schema.json").read_text(encoding="utf-8"))
    error = base["$defs"]["toolError"]
    error["properties"]["operation_id"] = base["$defs"]["id"]
    schemas = Schemas()
    for tool in catalog["x-alcomd-tools"]:
        native = methods[tool["x-alcomd-method"]]
        tool["annotations"] = {
            "readOnlyHint": tool["effect"] == "read",
            "destructiveHint": tool["x-alcomd-method"] in {
                "projects.unregister", "projects.applyDeleteDirectory", "projects.applyUnityMigration",
                "packages.userPackages.remove",
            },
            "idempotentHint": True,
            "openWorldHint": False,
        }
        inputs, input_defs = schemas.project(native["requestSchema"])
        if tool["effect"] == "apply" and tool["x-alcomd-method"] not in {"projects.register", "projects.create"}:
            inputs["properties"]["confirm"] = {"const": True}
            inputs.setdefault("required", []).append("confirm")
        if tool["x-alcomd-method"] in {"projects.planUnityMigration", "projects.applyUnityMigration"}:
            inputs["properties"]["in_place"] = {"type": "boolean", "default": False}
            inputs["properties"]["copy_operation_id"] = {"$ref": "#/$defs/uuid"}
            inputs["not"] = {
                "properties": {"in_place": {"const": True}},
                "required": ["in_place", "copy_operation_id"],
            }
        if input_defs:
            inputs["$defs"] = input_defs
        data, output_defs = schemas.project(native["resultSchema"])
        if tool["x-alcomd-method"] == "projects.planUnityMigration":
            planned = next(item for item in data["oneOf"] if item["properties"]["kind"]["const"] == "planned")
            path = {"type": "string", "minLength": 1}
            planned["properties"]["destination_path"] = path
            copy, copy_defs = schemas.project(methods["projects.planCopy"]["resultSchema"], prefix="copy-")
            output_defs.update(copy_defs)
            properties = {
                "kind": {"const": "copy_required"},
                "copy_plan": copy,
                "source_path": path,
                "destination_path": path,
                "target_unity_version": {"$ref": "#/$defs/canonicalUnityVersion"},
                "target_sdk_version": {"type": "string", "minLength": 1},
                "target_installation_id": {"$ref": "#/$defs/uuid"},
            }
            data["oneOf"].append({
                "type": "object", "additionalProperties": False,
                "properties": properties, "required": list(properties),
            })
        output = {
            "type": "object",
            "additionalProperties": False,
            "properties": {"data": data, "error": error},
            "oneOf": [{"required": ["data"]}, {"required": ["error"]}],
        }
        if output_defs:
            output["$defs"] = output_defs
        tool["inputSchema"] = inputs
        tool["outputSchema"] = output

    # Keep each derived schema on its own line so the published tools remain navigable.
    prefix = json.dumps({k: v for k, v in catalog.items() if k != "x-alcomd-tools"}, indent=4)[:-2]
    tools = []
    for tool in catalog["x-alcomd-tools"]:
        fields = [f'            {json.dumps(k)}: {json.dumps(v, separators=(",", ": "), ensure_ascii=False)}' for k, v in tool.items()]
        tools.append("        {\n" + ",\n".join(fields) + "\n        }")
    return prefix + ',\n    "x-alcomd-tools": [\n' + ",\n".join(tools) + "\n    ]\n}\n"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    options = parser.parse_args()
    content = generated()
    if options.check:
        if CATALOG.read_text(encoding="utf-8") != content:
            raise SystemExit("MCP project schemas differ from their native methods")
    else:
        CATALOG.write_text(content, encoding="utf-8", newline="\n")
