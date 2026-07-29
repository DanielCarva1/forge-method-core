#!/usr/bin/env python3
"""Translate a closed Codex journey observation into the public Forge protocol.

This cooperative adapter does not prove that Codex performed an action. Forge
therefore receives no native proof claim and caps positive results at
``partially_supported``.
"""

import json
import re
import sys
from pathlib import Path


PROTOCOL_VERSION = "solo_host_conformance_protocol_v1"
OBSERVATION_VERSION = "forge_codex_host_observation_v1"
ADAPTER_ID = "forge.codex.cooperative"
ADAPTER_VERSION = "1.0.0"
SAFE_TOKEN = re.compile(r"^[A-Za-z0-9_.:-]{1,128}$")
GAP_KINDS = {
    "missing_host_api",
    "platform_boundary_unavailable",
    "canonical_root_unavailable",
    "invocation_unavailable",
    "evidence_unavailable",
    "isolation_unavailable",
    "promotion_unavailable",
    "recovery_unavailable",
    "native_authenticity_unavailable",
    "adapter_failure",
}


class InvalidInput(Exception):
    """A closed input failed validation."""


def require_object(value, label):
    if type(value) is not dict:
        raise InvalidInput(f"{label}_must_be_object")
    return value


def require_exact_keys(value, expected, label):
    actual = set(require_object(value, label))
    if actual != set(expected):
        raise InvalidInput(f"{label}_fields_invalid")


def require_safe_token(value, label):
    if type(value) is not str or SAFE_TOKEN.fullmatch(value) is None:
        raise InvalidInput(f"{label}_must_be_safe_token")
    return value


def load_json_file(path):
    try:
        if path.is_symlink() or not path.is_file():
            raise InvalidInput("observation_file_must_be_regular")
        if path.stat().st_size > 1024 * 1024:
            raise InvalidInput("observation_file_too_large")
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        raise InvalidInput("observation_file_invalid") from error


def validate_request(request):
    require_exact_keys(
        request,
        {
            "schema_version",
            "bindings",
            "accepted_native_proof_schemes",
            "cases",
        },
        "request",
    )
    if request["schema_version"] != PROTOCOL_VERSION:
        raise InvalidInput("request_schema_version_unsupported")
    bindings = require_object(request["bindings"], "request_bindings")
    declared = require_object(
        bindings.get("declared"),
        "request_declared_bindings",
    )
    if declared.get("adapter_id") != ADAPTER_ID:
        raise InvalidInput("request_adapter_id_unsupported")
    if declared.get("adapter_version") != ADAPTER_VERSION:
        raise InvalidInput("request_adapter_version_unsupported")
    schemes = request["accepted_native_proof_schemes"]
    if type(schemes) is not list:
        raise InvalidInput("accepted_native_proof_schemes_must_be_array")
    cases = request["cases"]
    if type(cases) is not list or len(cases) != 8:
        raise InvalidInput("request_cases_invalid")
    seen = set()
    for case in cases:
        require_exact_keys(
            case,
            {"case_id", "capability", "description", "required_assertions"},
            "request_case",
        )
        case_id = require_safe_token(case["case_id"], "request_case_id")
        if case_id in seen:
            raise InvalidInput("request_case_id_duplicate")
        seen.add(case_id)
        assertions = case["required_assertions"]
        if type(assertions) is not list or not assertions:
            raise InvalidInput("request_assertions_invalid")
        if len(set(assertions)) != len(assertions):
            raise InvalidInput("request_assertion_duplicate")
        for assertion in assertions:
            require_safe_token(assertion, "request_assertion")
    return cases


def validate_observation(observation, request_cases):
    require_exact_keys(
        observation,
        {"schema_version", "evidence_mode", "cases"},
        "observation",
    )
    if observation["schema_version"] != OBSERVATION_VERSION:
        raise InvalidInput("observation_schema_version_unsupported")
    if observation["evidence_mode"] != "cooperative_same_owner":
        raise InvalidInput("observation_evidence_mode_unsupported")
    cases = observation["cases"]
    if type(cases) is not list or len(cases) != len(request_cases):
        raise InvalidInput("observation_cases_invalid")
    by_id = {}
    for case in cases:
        require_exact_keys(
            case,
            {"case_id", "assertions", "gaps", "fact_codes"},
            "observation_case",
        )
        case_id = require_safe_token(case["case_id"], "observation_case_id")
        if case_id in by_id:
            raise InvalidInput("observation_case_id_duplicate")
        assertions = require_object(
            case["assertions"],
            "observation_assertions",
        )
        for name, passed in assertions.items():
            require_safe_token(name, "observation_assertion")
            if type(passed) is not bool:
                raise InvalidInput("observation_assertion_must_be_boolean")
        gaps = case["gaps"]
        if type(gaps) is not list:
            raise InvalidInput("observation_gaps_must_be_array")
        gap_keys = set()
        normalized_gaps = []
        for gap in gaps:
            require_exact_keys(gap, {"kind", "code"}, "observation_gap")
            if gap["kind"] not in GAP_KINDS:
                raise InvalidInput("observation_gap_kind_invalid")
            code = require_safe_token(gap["code"], "observation_gap_code")
            key = (gap["kind"], code)
            if key in gap_keys:
                raise InvalidInput("observation_gap_duplicate")
            gap_keys.add(key)
            normalized_gaps.append({"kind": gap["kind"], "code": code})
        fact_codes = case["fact_codes"]
        if (
            type(fact_codes) is not list
            or not fact_codes
            or len(fact_codes) > 32
        ):
            raise InvalidInput("observation_fact_codes_invalid")
        normalized_facts = [
            require_safe_token(fact, "observation_fact_code")
            for fact in fact_codes
        ]
        if len(set(normalized_facts)) != len(normalized_facts):
            raise InvalidInput("observation_fact_code_duplicate")
        if any(passed is False for passed in assertions.values()) and not gaps:
            raise InvalidInput("failed_assertion_requires_typed_gap")
        by_id[case_id] = {
            "assertions": assertions,
            "gaps": normalized_gaps,
            "fact_codes": normalized_facts,
        }

    request_ids = {case["case_id"] for case in request_cases}
    if set(by_id) != request_ids:
        raise InvalidInput("observation_case_set_mismatch")
    for request_case in request_cases:
        observation_case = by_id[request_case["case_id"]]
        if set(observation_case["assertions"]) != set(
            request_case["required_assertions"]
        ):
            raise InvalidInput("observation_assertion_set_mismatch")
    return by_id


def build_response(request, request_cases, observations):
    cases = []
    for request_case in request_cases:
        case_id = request_case["case_id"]
        observation = observations[case_id]
        cases.append(
            {
                "case_id": case_id,
                "assertions": {
                    assertion: {
                        "passed": observation["assertions"][assertion],
                        "native_proof_claim": None,
                    }
                    for assertion in request_case["required_assertions"]
                },
                "gaps": observation["gaps"],
                "evidence": {
                    "fact_codes": observation["fact_codes"],
                },
            }
        )
    return {
        "schema_version": request["schema_version"],
        "bindings": request["bindings"],
        "cases": cases,
    }


def parse_args():
    if len(sys.argv) != 3 or sys.argv[1] != "--observation-file":
        raise InvalidInput("adapter_arguments_invalid")
    return Path(sys.argv[2])


def main():
    try:
        observation_path = parse_args()
        request = json.load(sys.stdin)
        request_cases = validate_request(request)
        observation = load_json_file(observation_path)
        observations = validate_observation(observation, request_cases)
        json.dump(
            build_response(request, request_cases, observations),
            sys.stdout,
            sort_keys=True,
            separators=(",", ":"),
        )
    except (
        InvalidInput,
        json.JSONDecodeError,
        UnicodeError,
        OSError,
        TypeError,
        ValueError,
    ):
        sys.stderr.write("codex_adapter_input_rejected\n")
        raise SystemExit(2)


if __name__ == "__main__":
    main()
