import json
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


HERE = Path(__file__).resolve().parent
ADAPTER = HERE.parent / "adapter.py"
CORPUS = (
    HERE.parents[2] / "solo-host-conformance-v1" / "corpus.json"
)


def request_document():
    corpus = json.loads(CORPUS.read_text(encoding="utf-8"))
    return {
        "schema_version": "solo_host_conformance_protocol_v1",
        "bindings": {
            "declared": {
                "host_id": "openai.codex",
                "host_version": "0.144.6",
                "adapter_id": "forge.codex.cooperative",
                "adapter_version": "1.0.0",
                "platform_label": "windows-10.0.26200",
                "environment_label": "codex-desktop-wsl-ubuntu-24.04",
            },
            "observed": {},
            "corpus_sha256": "sha256:" + ("0" * 64),
        },
        "accepted_native_proof_schemes": [],
        "cases": corpus["cases"],
    }


def observation_document(request):
    return {
        "schema_version": "forge_codex_host_observation_v1",
        "evidence_mode": "cooperative_same_owner",
        "cases": [
            {
                "case_id": case["case_id"],
                "assertions": {
                    assertion: True
                    for assertion in case["required_assertions"]
                },
                "gaps": [],
                "fact_codes": ["codex_cooperative_observation"],
            }
            for case in request["cases"]
        ],
    }


def run_adapter(request, observation):
    with tempfile.TemporaryDirectory() as temp:
        observation_path = Path(temp) / "observation.json"
        observation_path.write_text(
            json.dumps(observation, separators=(",", ":")),
            encoding="utf-8",
        )
        return subprocess.run(
            [
                sys.executable,
                str(ADAPTER),
                "--observation-file",
                str(observation_path),
            ],
            input=json.dumps(request, separators=(",", ":")),
            text=True,
            capture_output=True,
            check=False,
        )


class CodexAdapterTests(unittest.TestCase):
    def test_copies_bindings_and_translates_closed_observations(self):
        request = request_document()
        completed = run_adapter(request, observation_document(request))
        self.assertEqual(completed.returncode, 0, completed.stderr)
        response = json.loads(completed.stdout)
        self.assertEqual(response["bindings"], request["bindings"])
        self.assertEqual(
            [case["case_id"] for case in response["cases"]],
            [case["case_id"] for case in request["cases"]],
        )
        self.assertTrue(
            all(
                assertion["native_proof_claim"] is None
                for case in response["cases"]
                for assertion in case["assertions"].values()
            )
        )

    def test_rejects_unknown_fields_without_emitting_partial_json(self):
        request = request_document()
        observation = observation_document(request)
        observation["raw_chat"] = "forbidden"
        completed = run_adapter(request, observation)
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(completed.stdout, "")

    def test_rejects_missing_cases_and_assertions(self):
        request = request_document()
        missing_case = observation_document(request)
        missing_case["cases"].pop()
        self.assertNotEqual(run_adapter(request, missing_case).returncode, 0)

        missing_assertion = observation_document(request)
        missing_assertion["cases"][0]["assertions"].pop(
            next(iter(missing_assertion["cases"][0]["assertions"]))
        )
        self.assertNotEqual(
            run_adapter(request, missing_assertion).returncode,
            0,
        )

    def test_failed_assertion_requires_a_typed_gap(self):
        request = request_document()
        observation = observation_document(request)
        first_assertion = next(iter(observation["cases"][0]["assertions"]))
        observation["cases"][0]["assertions"][first_assertion] = False
        completed = run_adapter(request, observation)
        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(completed.stdout, "")

    def test_accepts_and_preserves_failed_assertion_with_typed_gap(self):
        request = request_document()
        observation = observation_document(request)
        first_case = observation["cases"][0]
        first_assertion = next(iter(first_case["assertions"]))
        first_case["assertions"][first_assertion] = False
        first_case["gaps"] = [
            {
                "kind": "invocation_unavailable",
                "code": "activation_negative_check_not_exercised",
            }
        ]

        completed = run_adapter(request, observation)

        self.assertEqual(completed.returncode, 0, completed.stderr)
        response = json.loads(completed.stdout)
        response_case = response["cases"][0]
        self.assertFalse(response_case["assertions"][first_assertion]["passed"])
        self.assertEqual(response_case["gaps"], first_case["gaps"])

    def test_rejects_wrong_adapter_version(self):
        request = request_document()
        request["bindings"]["declared"]["adapter_version"] = "1.0.1"

        completed = run_adapter(request, observation_document(request))

        self.assertNotEqual(completed.returncode, 0)
        self.assertEqual(completed.stdout, "")


if __name__ == "__main__":
    unittest.main()
