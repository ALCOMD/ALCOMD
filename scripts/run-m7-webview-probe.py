#!/usr/bin/env python3
"""Run and validate the test-only M7 real-WebView isolation probe."""

from __future__ import annotations

import argparse
import json
import subprocess
from pathlib import Path


BOOLEAN_FIELDS = (
    "processEnteredMain",
    "webviewCreated",
    "extensionDocumentLoaded",
    "cspApplied",
    "bridgeEstablished",
    "tauriGlobalPresent",
    "tauriInternalsPresent",
    "rawInvokeReachable",
    "eventTransportReachable",
    "channelTransportReachable",
    "parentDomReachable",
    "openerReachable",
    "topNavigationSucceeded",
    "networkSucceeded",
    "filesystemAuthority",
    "daemonSocketAuthority",
)
STRING_FIELDS = (
    "platform",
    "webviewEngine",
    "candidateMode",
    "physicalOrigin",
    "logicalOriginBinding",
    "result",
)
ALLOWED_RESULTS = {"pass", "isolation_failed", "harness_unavailable"}


def unavailable(platform: str, engine: str, reason: str) -> dict[str, object]:
    evidence: dict[str, object] = {
        "schema": 2,
        "platform": platform,
        "webviewEngine": engine,
        "candidateMode": "sandboxed_cross_origin_iframe",
        "physicalOrigin": "",
        "logicalOriginBinding": "",
        "result": "harness_unavailable",
        "failedChecks": reason,
    }
    evidence.update({field: False for field in BOOLEAN_FIELDS})
    return evidence


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--executable", type=Path, required=True)
    parser.add_argument("--platform", required=True)
    parser.add_argument("--engine", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--xvfb", action="store_true")
    args = parser.parse_args()

    source = args.executable.with_name("m7-webview-probe-result.json")
    source.unlink(missing_ok=True)
    command = [str(args.executable)]
    if args.xvfb:
        command = ["xvfb-run", "-a", *command]

    process_error = ""
    return_code: int | None = None
    try:
        completed = subprocess.run(command, check=False, timeout=55)
        return_code = completed.returncode
    except subprocess.TimeoutExpired:
        process_error = "probe-timeout"
    except OSError as error:
        process_error = f"probe-launch-failed:{error.__class__.__name__}"

    if source.is_file():
        try:
            evidence = json.loads(source.read_text(encoding="utf-8"))
        except (json.JSONDecodeError, OSError) as error:
            evidence = unavailable(args.platform, args.engine, f"malformed-evidence:{error.__class__.__name__}")
    else:
        reason = process_error or f"pre-main-exit:{return_code}"
        evidence = unavailable(args.platform, args.engine, reason)

    errors: list[str] = []
    if not isinstance(evidence, dict):
        evidence = unavailable(args.platform, args.engine, "evidence-not-object")
        errors.append("evidence-not-object")
    for field in BOOLEAN_FIELDS:
        if not isinstance(evidence.get(field), bool):
            errors.append(f"invalid-{field}")
    for field in STRING_FIELDS:
        if not isinstance(evidence.get(field), str):
            errors.append(f"invalid-{field}")
    if evidence.get("platform") != args.platform:
        errors.append("wrong-platform")
    if evidence.get("webviewEngine") != args.engine:
        errors.append("wrong-engine")
    if evidence.get("result") == "pass" and evidence.get("engineDetected") is not True:
        errors.append("engine-not-detected")
    if evidence.get("candidateMode") != "sandboxed_cross_origin_iframe":
        errors.append("wrong-candidate")
    if evidence.get("result") not in ALLOWED_RESULTS:
        errors.append("invalid-result")
    if return_code != 0:
        errors.append(f"process-exit-{return_code}")
    if evidence.get("result") != "pass":
        errors.append(f"probe-{evidence.get('result', 'missing-result')}")

    if errors:
        evidence["validationErrors"] = errors
    evidence["virtualDisplay"] = args.xvfb
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    print(json.dumps(evidence, indent=2, sort_keys=True))
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
