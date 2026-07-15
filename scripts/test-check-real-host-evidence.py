#!/usr/bin/env python3
"""Focused positive and fail-closed tests for real-host evidence checking."""

from __future__ import annotations

from contextlib import redirect_stdout
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest


SCRIPTS = Path(__file__).resolve().parent


def load_module(name: str, filename: str):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / filename)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load {filename}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


checker = load_module("forge_real_host_evidence_checker", "check-real-host-evidence.py")


class EvidenceFixture:
    def __init__(self, root: Path):
        self.root = root
        self.artifact_dir = root / "artifacts"
        self.artifact_dir.mkdir()
        self.rows: list[dict] = []
        self.contents: dict[str, bytes] = {}
        self.bundle_path = root / "bundle.yaml"

    def add(self, artifact_id: str, content: bytes | str, media_type: str = "text/plain") -> str:
        raw = content.encode("utf-8") if isinstance(content, str) else content
        path = self.artifact_dir / f"{artifact_id}.dat"
        path.write_bytes(raw)
        self.contents[artifact_id] = raw
        self.rows.append(
            {
                "id": artifact_id,
                "path": f"artifacts/{artifact_id}.dat",
                "sha256": hashlib.sha256(raw).hexdigest(),
                "size_bytes": len(raw),
                "media_type": media_type,
            }
        )
        return artifact_id

    def command_log(self, scenario_id: str, sessions: list[str]) -> str:
        log_id = f"log-{scenario_id}"
        entries = []
        for index, session in enumerate(sessions, start=1):
            stdout_ref = self.add(f"stdout-{scenario_id}-{index}", f"stdout {index}\n")
            stderr_ref = self.add(f"stderr-{scenario_id}-{index}", f"stderr {index}\n")
            entries.append(
                {
                    "sequence": index,
                    "session_id": session,
                    "argv": ["forge-core", "status", "--json", f"--session={session}"],
                    "working_directory": "/clean/consumer-project",
                    "exit_code": 0,
                    "stdout_ref": stdout_ref,
                    "stderr_ref": stderr_ref,
                }
            )
        raw = json.dumps(
            {
                "schema_version": checker.COMMAND_LOG_SCHEMA_VERSION,
                "scenario_id": scenario_id,
                "entries": entries,
            },
            sort_keys=True,
            separators=(",", ":"),
        )
        return self.add(log_id, raw, "application/json")

    def build(self) -> dict:
        archive = self.add("release-archive", b"release archive bytes\n", "application/octet-stream")
        manifest = self.add("release-manifest", '{"version":"0.9.0"}\n', "application/json")
        executable = self.add("release-executable", b"executable bytes\n", "application/octet-stream")

        scenario_data = [
            ("clean_host_journey", ["session-clean"]),
            ("concurrent_conflict", ["session-conflict-a", "session-conflict-b"]),
            ("replacement_session_resume", ["session-original", "session-replacement"]),
        ]
        scenarios = []
        for ordinal, (scenario_id, sessions) in enumerate(scenario_data, start=1):
            transcript = self.add(f"transcript-{scenario_id}", f"transcript {scenario_id}\n")
            evidence = self.add(f"scenario-evidence-{scenario_id}", f"evidence {scenario_id}\n")
            scenarios.append(
                {
                    "ordinal": ordinal,
                    "scenario_id": scenario_id,
                    "session_ids": sessions,
                    "transcript_ref": transcript,
                    "command_log_ref": self.command_log(scenario_id, sessions),
                    "evidence_refs": [evidence],
                    "observation": f"Recorded observation for {scenario_id}; not a pass verdict.",
                }
            )

        links = {}
        for field in checker.GOVERNED_LINK_FIELDS:
            links[field] = self.add(f"write-{field}", f"{field} evidence\n")
        review_record = self.add("review-record", "independent review record\n")
        bundle = {
            "schema_version": checker.SCHEMA_VERSION,
            "authority": checker.AUTHORITY,
            "bundle_id": "bundle.p7f.fixture.v0",
            "release_identity": {
                "release_id": "forge-method-core-v0.9.0-linux-x86_64",
                "product": "forge-method-core",
                "version": "0.9.0",
                "platform": "linux-x86_64",
                "source_revision": "0123456789abcdef",
                "archive_ref": archive,
                "release_manifest_ref": manifest,
                "executable_ref": executable,
            },
            "artifacts": self.rows,
            "scenarios": scenarios,
            "governed_writes": [
                {
                    "write_id": "governed-write-1",
                    "scenario_id": "clean_host_journey",
                    "target": ".forge-method/artifacts/result.yaml",
                    **links,
                }
            ],
            "ungoverned_writes": {
                "statement": "No ungoverned writes were observed; this is an explicit disclosure.",
                "observed": False,
                "entries": [],
            },
            "residual_limitations": [
                {
                    "limitation_id": "same-principal-boundary",
                    "statement": "The run did not establish hostile same-principal filesystem isolation.",
                    "impact": "A cooperating local-principal assumption remains.",
                }
            ],
            "independent_review": {
                "reviewer_id": "reviewer.fixture",
                "reviewed_at_utc": "2026-07-14T12:00:00Z",
                "disposition": "qualified",
                "independence_statement": "Reviewer reports no authorship of the captured run.",
                "limitations": ["The checker cannot verify that statement or actor independence."],
                "review_record_ref": review_record,
            },
        }
        self.write_bundle(bundle)
        return bundle

    def write_bundle(self, bundle: dict) -> None:
        self.bundle_path.write_text(json.dumps(bundle, indent=2) + "\n", encoding="utf-8")

    def rewrite_artifact(self, bundle: dict, artifact_id: str, document: dict) -> None:
        raw = json.dumps(document, sort_keys=True, separators=(",", ":")).encode("utf-8")
        row = next(row for row in bundle["artifacts"] if row["id"] == artifact_id)
        (self.root / row["path"]).write_bytes(raw)
        row["size_bytes"] = len(raw)
        row["sha256"] = hashlib.sha256(raw).hexdigest()
        self.write_bundle(bundle)


class RealHostEvidenceTests(unittest.TestCase):
    def fixture(self, directory: str) -> tuple[EvidenceFixture, dict]:
        fixture = EvidenceFixture(Path(directory))
        return fixture, fixture.build()

    def test_valid_bundle_reports_narrow_non_authoritative_result(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture, _ = self.fixture(directory)
            output = io.StringIO()
            with redirect_stdout(output):
                checker.check(fixture.bundle_path)
            rendered = output.getvalue()
            self.assertIn("structurally/content-integrity valid", rendered)
            self.assertIn("does not certify a production host", rendered)
            self.assertIn("actor independence, publication, or P7F passage", rendered)

    def test_rejects_digest_drift(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture, bundle = self.fixture(directory)
            row = next(row for row in bundle["artifacts"] if row["id"] == "write-receipt_ref")
            (fixture.root / row["path"]).write_bytes(b"tampered receipt\n")
            with self.assertRaisesRegex(checker.EvidenceCheckError, "(?:size|SHA-256) mismatch"):
                checker.check(fixture.bundle_path)

    def test_rejects_wrong_scenario_order_and_reused_session(self) -> None:
        mutations = [
            lambda bundle: bundle["scenarios"].__setitem__(
                slice(0, 2), list(reversed(bundle["scenarios"][:2]))
            ),
            lambda bundle: bundle["scenarios"][1]["session_ids"].__setitem__(
                0, bundle["scenarios"][0]["session_ids"][0]
            ),
        ]
        for mutation in mutations:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                fixture, bundle = self.fixture(directory)
                mutation(bundle)
                fixture.write_bundle(bundle)
                with self.assertRaises(checker.EvidenceCheckError):
                    checker.check(fixture.bundle_path)

    def test_rejects_missing_governed_write_link(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture, bundle = self.fixture(directory)
            del bundle["governed_writes"][0]["admission_ref"]
            fixture.write_bundle(bundle)
            with self.assertRaisesRegex(checker.EvidenceCheckError, "admission_ref"):
                checker.check(fixture.bundle_path)

    def test_rejects_shell_text_instead_of_exact_argv(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture, bundle = self.fixture(directory)
            scenario = bundle["scenarios"][0]
            log_id = scenario["command_log_ref"]
            log = json.loads(fixture.contents[log_id])
            log["entries"][0]["argv"] = "forge-core status --json"
            fixture.rewrite_artifact(bundle, log_id, log)
            with self.assertRaisesRegex(checker.EvidenceCheckError, "argv"):
                checker.check(fixture.bundle_path)

    def test_rejects_missing_mandatory_disclosures_or_review(self) -> None:
        fields = ["ungoverned_writes", "residual_limitations", "independent_review"]
        for field in fields:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                fixture, bundle = self.fixture(directory)
                del bundle[field]
                fixture.write_bundle(bundle)
                with self.assertRaisesRegex(checker.EvidenceCheckError, field):
                    checker.check(fixture.bundle_path)

    def test_rejects_yaml_aliases_duplicate_keys_and_oversize_bundle(self) -> None:
        invalid_documents = [
            "schema_version: &v forge_real_host_evidence_bundle_v0\nauthority: *v\n",
            "schema_version: one\nschema_version: two\n",
        ]
        for document in invalid_documents:
            with self.subTest(document=document), tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / "bundle.yaml"
                path.write_text(document, encoding="utf-8")
                with self.assertRaises(checker.EvidenceCheckError):
                    checker.check(path)
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "bundle.json"
            path.write_bytes(b"{}" + b" " * checker.MAX_BUNDLE_BYTES)
            with self.assertRaisesRegex(checker.EvidenceCheckError, "byte size"):
                checker.check(path)

    def test_rejects_unreferenced_artifact_and_noncanonical_path(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            fixture, bundle = self.fixture(directory)
            fixture.add("orphan", "orphan\n")
            fixture.write_bundle(bundle)
            with self.assertRaisesRegex(checker.EvidenceCheckError, "unreferenced"):
                checker.check(fixture.bundle_path)
        with tempfile.TemporaryDirectory() as directory:
            fixture, bundle = self.fixture(directory)
            bundle["artifacts"][0]["path"] = "artifacts/../escape"
            fixture.write_bundle(bundle)
            with self.assertRaisesRegex(checker.EvidenceCheckError, "traversal-free"):
                checker.check(fixture.bundle_path)


if __name__ == "__main__":
    unittest.main()
