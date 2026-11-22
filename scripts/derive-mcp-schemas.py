"""Derive a named MCP tool catalogue from the approved native RPC contracts."""
import argparse
import copy
import json
from pathlib import Path
import re


ROOT = Path(__file__).resolve().parents[1]
RPC = ROOT / "specs/rpc"
MCP = ROOT / "specs/mcp"


def snake(name):
    return re.sub(r"[A-Z]", lambda m: "_" + m[0].lower(), name)


class Definitions:
    def __init__(self):
        self.schemas = {}
        self.definitions = {}
        self.references = {}
        self.identifiers = {}
        for path in (ROOT / "specs").rglob("*.schema.json"):
            schema = json.loads(path.read_text(encoding="utf-8"))
            if "$id" in schema:
                self.identifiers[schema["$id"]] = path

    def read(self, path, pointer):
        if path not in self.schemas:
            self.schemas[path] = json.loads(path.read_text(encoding="utf-8"))
        value = self.schemas[path]
        for key in pointer.lstrip("/").split("/") if pointer else ():
            key = key.replace("~1", "/").replace("~0", "~")
            value = value[int(key)] if isinstance(value, list) else value[key]
        return value

    def reference(self, path, link):
        file, _, pointer = link.partition("#")
        if file in self.identifiers:
            path = self.identifiers[file]
        elif "://" in file:
            raise ValueError(f"unregistered schema identifier {file}")
        elif file:
            path = (path.parent / file).resolve()
        if not path.is_relative_to(ROOT / "specs"):
            raise ValueError(f"nonlocal schema reference {link}")
        target = (path, pointer)
        if target not in self.references:
            name = path.name.split(".schema.json")[0] + "." + pointer.strip("/").replace("$defs/", "").replace("/", ".")
            if name in self.definitions:
                raise ValueError(f"duplicate bundled schema name {name}")
            self.references[target] = name
            self.definitions[name] = self.convert(self.read(path, pointer), path)
        return {"$ref": "#/$defs/" + self.references[target]}

    def convert(self, schema, path):
        if isinstance(schema, list):
            return [self.convert(item, path) for item in schema]
        if not isinstance(schema, dict):
            return schema
        result = {}
        for key, value in schema.items():
            if key == "$ref":
                result.update(self.reference(path, value))
            elif key == "properties":
                result[key] = {snake(field): self.convert(item, path) for field, item in value.items()}
            elif key == "required":
                result[key] = [snake(field) for field in value]
            elif key not in ("$id", "$schema", "$defs") and not key.startswith("x-alcomd-"):
                result[key] = self.convert(value, path)
        return result


def catalogue(bindings):
    methods = json.loads((RPC / "native-sdk-v1.methods.json").read_text(encoding="utf-8"))["methods"]
    definitions = Definitions()
    definitions.definitions["toolError"] = json.loads(
        (MCP / "environment-v1.schema.json").read_text(encoding="utf-8"))["$defs"]["toolError"]
    tools = []
    for entry in bindings["tools"]:
        method = methods[entry["method"]]
        request = definitions.reference(RPC / "native-sdk-v1.methods.json", method["requestSchema"])
        inputs = copy.deepcopy(definitions.definitions[request["$ref"].removeprefix("#/$defs/")])
        while "$ref" in inputs:
            inputs = copy.deepcopy(definitions.definitions[inputs["$ref"].removeprefix("#/$defs/")])
        if entry["effect"] == "apply":
            inputs.setdefault("properties", {})["confirm"] = {"const": True}
            inputs.setdefault("required", []).append("confirm")
        data = definitions.reference(RPC / "native-sdk-v1.methods.json", method["resultSchema"])
        if "notificationSchema" in entry:
            notification = definitions.reference(RPC / "native-sdk-v1.methods.json", entry["notificationSchema"])
            data = {"type": "object", "additionalProperties": False, "required": ["ready"],
                    "properties": {"ready": data, "notification": notification}}
        tools.append({
            "name": entry["name"], "title": entry["title"], "description": entry["description"],
            **({"annotations": entry["annotations"]} if "annotations" in entry else {}),
            "inputSchema": inputs, "outputSchema": {
                "type": "object", "additionalProperties": False,
                "properties": {"data": data, "error": {"$ref": "#/$defs/toolError"}},
                "oneOf": [{"required": ["data"]}, {"required": ["error"]}],
            }, "effect": entry["effect"], "requiredPermissions": entry["permissions"],
        })
    for tool in tools:
        for key in ("inputSchema", "outputSchema"):
            schema = tool[key]
            selected = {}
            def visit(value):
                if isinstance(value, dict):
                    if "$ref" in value:
                        name = value["$ref"].removeprefix("#/$defs/")
                        if name not in selected:
                            selected[name] = definitions.definitions[name]
                            visit(selected[name])
                    for child in value.values():
                        visit(child)
                elif isinstance(value, list):
                    for child in value:
                        visit(child)
            visit(schema)
            schema["$defs"] = selected
    return {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$id": bindings["schemaId"], "title": bindings["title"],
        "x-alcomd-tools": tools,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("bindings", type=Path)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    bindings = json.loads(args.bindings.read_text(encoding="utf-8"))
    path = MCP / bindings["schemaFile"]
    content = json.dumps(catalogue(bindings), ensure_ascii=False, indent=4) + "\n"
    if args.check:
        if path.read_text(encoding="utf-8") != content:
            parser.exit(1, f"Regenerate {path.name} from {args.bindings.name}\n")
    else:
        path.write_bytes(content.encode())


if __name__ == "__main__":
    main()
