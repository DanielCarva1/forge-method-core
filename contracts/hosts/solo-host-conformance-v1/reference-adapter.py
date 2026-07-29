#!/usr/bin/env python3
"""Protocol-only reference adapter. It never proves that a real host is supported."""
import json
import sys
import time

mode = sys.argv[1] if len(sys.argv) > 1 else "reference"
if mode == "timeout":
    time.sleep(5)
    raise SystemExit(0)
if mode == "oversize":
    sys.stdout.write("x" * (4 * 1024 * 1024 + 1))
    raise SystemExit(0)

request = json.load(sys.stdin)
cases = []
for case in request["cases"]:
    passed = mode != "unsupported"
    gaps = []
    if mode == "reference":
        gaps.append({
            "kind": "native_authenticity_unavailable",
            "code": "reference_adapter_only"
        })
    elif mode == "unsupported":
        gaps.append({
            "kind": "adapter_failure",
            "code": "example_host_action_unavailable"
        })
    fact_codes = ["closed_example_observation", f"case_{case['case_id'].replace('-', '_')}"]
    if mode == "unsafe-secret":
        fact_codes.append("client_secret_value")
    if mode == "unsafe-path":
        fact_codes.append("/home/alice/private")
    cases.append({
        "case_id": case["case_id"],
        "assertions": {
            name: {"passed": passed, "native_proof_claim": None}
            for name in case["required_assertions"]
        },
        "gaps": gaps,
        "evidence": {"fact_codes": fact_codes}
    })

if mode == "reversed":
    cases.reverse()
    for case in cases:
        case["evidence"]["fact_codes"].reverse()

json.dump({
    "schema_version": request["schema_version"],
    "bindings": request["bindings"],
    "cases": cases
}, sys.stdout, sort_keys=True, separators=(",", ":"))