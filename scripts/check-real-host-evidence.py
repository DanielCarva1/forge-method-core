#!/usr/bin/env python3
"""Validate only the structure and byte integrity of a P7F real-host evidence bundle.

A successful check is not certification of a production host, actor independence,
publication, or P7F passage. It only establishes that the supplied, bounded files
match the non-authoritative bundle contract and their declared SHA-256 bindings.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import stat
from typing import Any

try:
    import yaml
except ImportError:  # JSON remains dependency-free in release archives.
    yaml = None


SCHEMA_VERSION = "forge_real_host_evidence_bundle_v0"
COMMAND_LOG_SCHEMA_VERSION = "forge_real_host_command_log_v0"
AUTHORITY = "non_authoritative_structural_content_integrity_evidence"
MAX_BUNDLE_BYTES = 1024 * 1024
MAX_ARTIFACT_BYTES = 16 * 1024 * 1024
MAX_TOTAL_ARTIFACT_BYTES = 64 * 1024 * 1024
MAX_ARTIFACTS = 256
MAX_NESTING_DEPTH = 32
MAX_CONTAINER_ITEMS = 100_000
MAX_TEXT_BYTES = 64 * 1024
MAX_ARGV_ITEMS = 128
MAX_ARGV_BYTES = 64 * 1024
SHA256 = re.compile(r"^[0-9a-f]{64}$")
SCENARIO_IDS = [
    "clean_host_journey",
    "concurrent_conflict",
    "replacement_session_resume",
]
GOVERNED_LINK_FIELDS = [
    "claim_ref",
    "gate_result_ref",
    "verified_principal_ref",
    "admission_ref",
    "pre_effect_wal_ref",
    "effect_ref",
    "receipt_ref",
]
DISCLAIMER = (
    "This result validates only structure and content integrity; it does not certify "
    "a production host, actor independence, publication, or P7F passage."
)


class EvidenceCheckError(ValueError):
    """The evidence bundle is malformed, unsafe, or content-integrity-invalid."""


if yaml is not None:
    class UniqueKeyLoader(yaml.SafeLoader):
        """Safe YAML loader that also denies duplicate mapping keys."""


    def _construct_unique_mapping(loader: Any, node: Any, deep: bool = False):
        mapping: dict[Any, Any] = {}
        for key_node, value_node in node.value:
            key = loader.construct_object(key_node, deep=deep)
            try:
                duplicate = key in mapping
            except TypeError as error:
                raise EvidenceCheckError("mapping key must be a scalar") from error
            if duplicate:
                raise EvidenceCheckError(f"duplicate mapping key: {key!r}")
            mapping[key] = loader.construct_object(value_node, deep=deep)
        return mapping


    UniqueKeyLoader.add_constructor(
        yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, _construct_unique_mapping
    )
else:
    UniqueKeyLoader = None


def _walk_bounded(value: Any, path: str = "$", depth: int = 0) -> int:
    if depth > MAX_NESTING_DEPTH:
        raise EvidenceCheckError(f"{path}: nesting exceeds {MAX_NESTING_DEPTH}")
    if isinstance(value, str):
        if len(value.encode("utf-8")) > MAX_TEXT_BYTES:
            raise EvidenceCheckError(f"{path}: text exceeds {MAX_TEXT_BYTES} bytes")
        return 1
    if value is None or isinstance(value, (bool, int, float)):
        return 1
    if isinstance(value, list):
        count = 1
        for index, item in enumerate(value):
            count += _walk_bounded(item, f"{path}[{index}]", depth + 1)
            if count > MAX_CONTAINER_ITEMS:
                raise EvidenceCheckError("parsed document exceeds container item limit")
        return count
    if isinstance(value, dict):
        count = 1
        for key, item in value.items():
            if not isinstance(key, str):
                raise EvidenceCheckError(f"{path}: mapping keys must be strings")
            if key == "<<":
                raise EvidenceCheckError(f"{path}: YAML merge keys are forbidden")
            count += _walk_bounded(key, f"{path}.<key>", depth + 1)
            count += _walk_bounded(item, f"{path}.{key}", depth + 1)
            if count > MAX_CONTAINER_ITEMS:
                raise EvidenceCheckError("parsed document exceeds container item limit")
        return count
    raise EvidenceCheckError(f"{path}: unsupported YAML/JSON value type {type(value).__name__}")


def load_alias_free_document(raw: bytes, label: str) -> Any:
    try:
        text = raw.decode("utf-8")
    except UnicodeDecodeError as error:
        raise EvidenceCheckError(f"{label}: document is not UTF-8") from error

    def unique_json_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
        result: dict[str, Any] = {}
        for key, value in pairs:
            if key in result:
                raise EvidenceCheckError(f"{label}: duplicate mapping key: {key!r}")
            result[key] = value
        return result

    if text.lstrip().startswith(("{", "[")):
        try:
            value = json.loads(text, object_pairs_hook=unique_json_object)
        except EvidenceCheckError:
            raise
        except json.JSONDecodeError as error:
            raise EvidenceCheckError(f"{label}: invalid JSON: {error}") from error
    else:
        if yaml is None:
            raise EvidenceCheckError(
                f"{label}: YAML input requires PyYAML; use JSON for dependency-free release verification"
            )
        try:
            for token in yaml.scan(text):
                if isinstance(token, (yaml.tokens.AnchorToken, yaml.tokens.AliasToken)):
                    raise EvidenceCheckError(f"{label}: YAML anchors and aliases are forbidden")
                if isinstance(token, yaml.tokens.TagToken):
                    raise EvidenceCheckError(f"{label}: explicit YAML tags are forbidden")
            value = yaml.load(text, Loader=UniqueKeyLoader)
        except EvidenceCheckError:
            raise
        except yaml.YAMLError as error:
            raise EvidenceCheckError(f"{label}: invalid YAML: {error}") from error
    _walk_bounded(value)
    return value


def read_bounded_regular(path: Path, limit: int, label: str) -> bytes:
    try:
        metadata = path.lstat()
    except OSError as error:
        raise EvidenceCheckError(f"{label}: cannot stat {path}: {error}") from error
    if stat.S_ISLNK(metadata.st_mode) or not stat.S_ISREG(metadata.st_mode):
        raise EvidenceCheckError(f"{label}: must be a regular non-symlink file: {path}")
    if metadata.st_size <= 0 or metadata.st_size > limit:
        raise EvidenceCheckError(
            f"{label}: byte size {metadata.st_size} is outside 1..{limit}: {path}"
        )
    try:
        raw = path.read_bytes()
    except OSError as error:
        raise EvidenceCheckError(f"{label}: cannot read {path}: {error}") from error
    if len(raw) != metadata.st_size:
        raise EvidenceCheckError(f"{label}: file changed while being read: {path}")
    return raw


def exact_object(
    value: Any,
    path: str,
    required: set[str],
    optional: set[str] | None = None,
) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise EvidenceCheckError(f"{path}: must be an object")
    optional = optional or set()
    keys = set(value)
    missing = required - keys
    unknown = keys - required - optional
    if missing or unknown:
        raise EvidenceCheckError(
            f"{path}: schema mismatch; missing={sorted(missing)}, unknown={sorted(unknown)}"
        )
    return value


def nonempty_string(value: Any, path: str) -> str:
    if not isinstance(value, str) or not value.strip() or "\x00" in value:
        raise EvidenceCheckError(f"{path}: must be a non-empty NUL-free string")
    return value


def integer(value: Any, path: str, minimum: int | None = None) -> int:
    if isinstance(value, bool) or not isinstance(value, int):
        raise EvidenceCheckError(f"{path}: must be an integer")
    if minimum is not None and value < minimum:
        raise EvidenceCheckError(f"{path}: must be >= {minimum}")
    return value


def string_list(value: Any, path: str, *, nonempty: bool = True) -> list[str]:
    if not isinstance(value, list) or (nonempty and not value):
        raise EvidenceCheckError(f"{path}: must be {'a non-empty' if nonempty else 'an'} array")
    result: list[str] = []
    for index, item in enumerate(value):
        result.append(nonempty_string(item, f"{path}[{index}]"))
    return result


def canonical_relative_path(value: Any, path: str) -> str:
    text = nonempty_string(value, path)
    if "\\" in text or re.match(r"^[A-Za-z]:", text):
        raise EvidenceCheckError(f"{path}: must use canonical relative POSIX syntax")
    candidate = PurePosixPath(text)
    if candidate.is_absolute() or any(part in {"", ".", ".."} for part in text.split("/")):
        raise EvidenceCheckError(f"{path}: must be a traversal-free relative path")
    if candidate.as_posix() != text:
        raise EvidenceCheckError(f"{path}: path is not canonical")
    return text


def resolve_artifact_path(root: Path, relative: str, label: str) -> Path:
    current = root
    for part in PurePosixPath(relative).parts:
        current = current / part
        try:
            mode = current.lstat().st_mode
        except OSError as error:
            raise EvidenceCheckError(f"{label}: cannot stat {current}: {error}") from error
        if stat.S_ISLNK(mode):
            raise EvidenceCheckError(f"{label}: symlink path components are forbidden: {current}")
    return current


def validate_ref(value: Any, path: str, artifacts: dict[str, dict[str, Any]], used: set[str]) -> str:
    reference = nonempty_string(value, path)
    if reference not in artifacts:
        raise EvidenceCheckError(f"{path}: unknown artifact reference {reference!r}")
    used.add(reference)
    return reference


def validate_command_log(
    raw: bytes,
    artifact_id: str,
    scenario: dict[str, Any],
    artifacts: dict[str, dict[str, Any]],
    used: set[str],
) -> None:
    document = load_alias_free_document(raw, f"command log {artifact_id}")
    root = exact_object(
        document,
        f"command log {artifact_id}",
        {"schema_version", "scenario_id", "entries"},
    )
    if root["schema_version"] != COMMAND_LOG_SCHEMA_VERSION:
        raise EvidenceCheckError(f"command log {artifact_id}: unsupported schema_version")
    if root["scenario_id"] != scenario["scenario_id"]:
        raise EvidenceCheckError(f"command log {artifact_id}: scenario_id mismatch")
    entries = root["entries"]
    if not isinstance(entries, list) or not entries:
        raise EvidenceCheckError(f"command log {artifact_id}.entries: must be a non-empty array")
    represented_sessions: set[str] = set()
    allowed_sessions = set(scenario["session_ids"])
    for index, value in enumerate(entries):
        path = f"command log {artifact_id}.entries[{index}]"
        entry = exact_object(
            value,
            path,
            {
                "sequence",
                "session_id",
                "argv",
                "working_directory",
                "exit_code",
                "stdout_ref",
                "stderr_ref",
            },
        )
        if integer(entry["sequence"], f"{path}.sequence", 1) != index + 1:
            raise EvidenceCheckError(f"{path}.sequence: must be exact contiguous log order")
        session_id = nonempty_string(entry["session_id"], f"{path}.session_id")
        if session_id not in allowed_sessions:
            raise EvidenceCheckError(f"{path}.session_id: not declared by the scenario")
        represented_sessions.add(session_id)
        argv = string_list(entry["argv"], f"{path}.argv")
        if len(argv) > MAX_ARGV_ITEMS:
            raise EvidenceCheckError(f"{path}.argv: exceeds {MAX_ARGV_ITEMS} items")
        argv_bytes = sum(len(item.encode("utf-8")) for item in argv)
        if argv_bytes > MAX_ARGV_BYTES or any("\n" in item or "\r" in item for item in argv):
            raise EvidenceCheckError(
                f"{path}.argv: must be an exact bounded argument vector, not shell text"
            )
        nonempty_string(entry["working_directory"], f"{path}.working_directory")
        integer(entry["exit_code"], f"{path}.exit_code")
        validate_ref(entry["stdout_ref"], f"{path}.stdout_ref", artifacts, used)
        validate_ref(entry["stderr_ref"], f"{path}.stderr_ref", artifacts, used)
    if represented_sessions != allowed_sessions:
        raise EvidenceCheckError(
            f"command log {artifact_id}: every declared session must have an argv entry"
        )


def check(bundle_path: Path) -> None:
    raw_bundle = read_bounded_regular(bundle_path, MAX_BUNDLE_BYTES, "evidence bundle")
    document = load_alias_free_document(raw_bundle, "evidence bundle")
    root = exact_object(
        document,
        "$",
        {
            "schema_version",
            "authority",
            "bundle_id",
            "release_identity",
            "artifacts",
            "scenarios",
            "governed_writes",
            "ungoverned_writes",
            "residual_limitations",
            "independent_review",
        },
    )
    if root["schema_version"] != SCHEMA_VERSION:
        raise EvidenceCheckError("$.schema_version: unsupported schema")
    if root["authority"] != AUTHORITY:
        raise EvidenceCheckError("$.authority: must preserve the non-authoritative boundary")
    nonempty_string(root["bundle_id"], "$.bundle_id")

    artifact_rows = root["artifacts"]
    if not isinstance(artifact_rows, list) or not artifact_rows or len(artifact_rows) > MAX_ARTIFACTS:
        raise EvidenceCheckError(f"$.artifacts: must contain 1..{MAX_ARTIFACTS} rows")
    artifacts: dict[str, dict[str, Any]] = {}
    artifact_bytes: dict[str, bytes] = {}
    seen_paths: set[str] = set()
    total_size = 0
    root_dir = bundle_path.parent
    for index, value in enumerate(artifact_rows):
        path = f"$.artifacts[{index}]"
        row = exact_object(value, path, {"id", "path", "sha256", "size_bytes", "media_type"})
        artifact_id = nonempty_string(row["id"], f"{path}.id")
        if artifact_id in artifacts:
            raise EvidenceCheckError(f"{path}.id: duplicate artifact id {artifact_id!r}")
        relative = canonical_relative_path(row["path"], f"{path}.path")
        if relative in seen_paths:
            raise EvidenceCheckError(f"{path}.path: duplicate artifact path {relative!r}")
        seen_paths.add(relative)
        expected_digest = nonempty_string(row["sha256"], f"{path}.sha256")
        if not SHA256.fullmatch(expected_digest):
            raise EvidenceCheckError(f"{path}.sha256: must be lowercase SHA-256")
        expected_size = integer(row["size_bytes"], f"{path}.size_bytes", 1)
        if expected_size > MAX_ARTIFACT_BYTES:
            raise EvidenceCheckError(f"{path}.size_bytes: exceeds per-artifact byte limit")
        nonempty_string(row["media_type"], f"{path}.media_type")
        actual_path = resolve_artifact_path(root_dir, relative, f"artifact {artifact_id}")
        content = read_bounded_regular(actual_path, MAX_ARTIFACT_BYTES, f"artifact {artifact_id}")
        if len(content) != expected_size:
            raise EvidenceCheckError(f"{path}: size mismatch")
        if hashlib.sha256(content).hexdigest() != expected_digest:
            raise EvidenceCheckError(f"{path}: SHA-256 mismatch")
        total_size += len(content)
        if total_size > MAX_TOTAL_ARTIFACT_BYTES:
            raise EvidenceCheckError("$.artifacts: total artifact bytes exceed limit")
        artifacts[artifact_id] = row
        artifact_bytes[artifact_id] = content

    used: set[str] = set()
    release = exact_object(
        root["release_identity"],
        "$.release_identity",
        {
            "release_id",
            "product",
            "version",
            "platform",
            "source_revision",
            "archive_ref",
            "release_manifest_ref",
            "executable_ref",
        },
    )
    for field in ["release_id", "version", "platform", "source_revision"]:
        nonempty_string(release[field], f"$.release_identity.{field}")
    if release["product"] != "forge-method-core":
        raise EvidenceCheckError("$.release_identity.product: must be forge-method-core")
    release_refs = [
        validate_ref(release[field], f"$.release_identity.{field}", artifacts, used)
        for field in ["archive_ref", "release_manifest_ref", "executable_ref"]
    ]
    if len(set(release_refs)) != len(release_refs):
        raise EvidenceCheckError("$.release_identity: archive, manifest, and executable refs must differ")

    scenarios = root["scenarios"]
    if not isinstance(scenarios, list) or len(scenarios) != len(SCENARIO_IDS):
        raise EvidenceCheckError("$.scenarios: exact three-scenario sequence is required")
    all_sessions: set[str] = set()
    command_logs: list[tuple[str, dict[str, Any]]] = []
    for index, value in enumerate(scenarios):
        path = f"$.scenarios[{index}]"
        scenario = exact_object(
            value,
            path,
            {
                "ordinal",
                "scenario_id",
                "session_ids",
                "transcript_ref",
                "command_log_ref",
                "evidence_refs",
                "observation",
            },
        )
        if integer(scenario["ordinal"], f"{path}.ordinal", 1) != index + 1:
            raise EvidenceCheckError(f"{path}.ordinal: scenarios must be ordered 1..3")
        if scenario["scenario_id"] != SCENARIO_IDS[index]:
            raise EvidenceCheckError(f"{path}.scenario_id: scenario order is fixed")
        sessions = string_list(scenario["session_ids"], f"{path}.session_ids")
        if len(set(sessions)) != len(sessions) or all_sessions.intersection(sessions):
            raise EvidenceCheckError(f"{path}.session_ids: session ids must be globally distinct")
        if scenario["scenario_id"] in {"concurrent_conflict", "replacement_session_resume"} and len(sessions) < 2:
            raise EvidenceCheckError(f"{path}.session_ids: this scenario requires at least two sessions")
        all_sessions.update(sessions)
        validate_ref(scenario["transcript_ref"], f"{path}.transcript_ref", artifacts, used)
        command_ref = validate_ref(
            scenario["command_log_ref"], f"{path}.command_log_ref", artifacts, used
        )
        evidence_refs = string_list(scenario["evidence_refs"], f"{path}.evidence_refs")
        if len(set(evidence_refs)) != len(evidence_refs):
            raise EvidenceCheckError(f"{path}.evidence_refs: duplicate references are forbidden")
        for ref_index, reference in enumerate(evidence_refs):
            validate_ref(reference, f"{path}.evidence_refs[{ref_index}]", artifacts, used)
        nonempty_string(scenario["observation"], f"{path}.observation")
        command_logs.append((command_ref, scenario))

    writes = root["governed_writes"]
    if not isinstance(writes, list) or not writes:
        raise EvidenceCheckError("$.governed_writes: at least one claimed governed write is required")
    write_ids: set[str] = set()
    for index, value in enumerate(writes):
        path = f"$.governed_writes[{index}]"
        row = exact_object(
            value,
            path,
            {"write_id", "scenario_id", "target"} | set(GOVERNED_LINK_FIELDS),
        )
        write_id = nonempty_string(row["write_id"], f"{path}.write_id")
        if write_id in write_ids:
            raise EvidenceCheckError(f"{path}.write_id: duplicate write id")
        write_ids.add(write_id)
        if row["scenario_id"] not in SCENARIO_IDS:
            raise EvidenceCheckError(f"{path}.scenario_id: unknown scenario")
        nonempty_string(row["target"], f"{path}.target")
        link_refs = [
            validate_ref(row[field], f"{path}.{field}", artifacts, used)
            for field in GOVERNED_LINK_FIELDS
        ]
        if len(set(link_refs)) != len(link_refs):
            raise EvidenceCheckError(f"{path}: governed-write evidence links must be distinct")

    disclosure = exact_object(
        root["ungoverned_writes"],
        "$.ungoverned_writes",
        {"statement", "observed", "entries"},
    )
    nonempty_string(disclosure["statement"], "$.ungoverned_writes.statement")
    if not isinstance(disclosure["observed"], bool):
        raise EvidenceCheckError("$.ungoverned_writes.observed: must be boolean")
    entries = disclosure["entries"]
    if not isinstance(entries, list):
        raise EvidenceCheckError("$.ungoverned_writes.entries: must be an array")
    if disclosure["observed"] != bool(entries):
        raise EvidenceCheckError("$.ungoverned_writes: observed must exactly match whether entries exist")
    for index, value in enumerate(entries):
        path = f"$.ungoverned_writes.entries[{index}]"
        entry = exact_object(
            value, path, {"scenario_id", "target", "method", "reason", "evidence_ref"}
        )
        if entry["scenario_id"] not in SCENARIO_IDS:
            raise EvidenceCheckError(f"{path}.scenario_id: unknown scenario")
        for field in ["target", "method", "reason"]:
            nonempty_string(entry[field], f"{path}.{field}")
        validate_ref(entry["evidence_ref"], f"{path}.evidence_ref", artifacts, used)

    limitations = root["residual_limitations"]
    if not isinstance(limitations, list) or not limitations:
        raise EvidenceCheckError("$.residual_limitations: explicit non-empty disclosure is required")
    limitation_ids: set[str] = set()
    for index, value in enumerate(limitations):
        path = f"$.residual_limitations[{index}]"
        limitation = exact_object(value, path, {"limitation_id", "statement", "impact"})
        limitation_id = nonempty_string(limitation["limitation_id"], f"{path}.limitation_id")
        if limitation_id in limitation_ids:
            raise EvidenceCheckError(f"{path}.limitation_id: duplicate id")
        limitation_ids.add(limitation_id)
        nonempty_string(limitation["statement"], f"{path}.statement")
        nonempty_string(limitation["impact"], f"{path}.impact")

    review = exact_object(
        root["independent_review"],
        "$.independent_review",
        {
            "reviewer_id",
            "reviewed_at_utc",
            "disposition",
            "independence_statement",
            "limitations",
            "review_record_ref",
        },
    )
    for field in ["reviewer_id", "reviewed_at_utc", "independence_statement"]:
        nonempty_string(review[field], f"$.independent_review.{field}")
    if review["disposition"] not in {"reviewed", "qualified", "changes_requested"}:
        raise EvidenceCheckError("$.independent_review.disposition: unsupported disposition")
    string_list(review["limitations"], "$.independent_review.limitations")
    validate_ref(
        review["review_record_ref"], "$.independent_review.review_record_ref", artifacts, used
    )

    seen_command_refs: set[str] = set()
    for command_ref, scenario in command_logs:
        if command_ref in seen_command_refs:
            raise EvidenceCheckError("scenario command_log_ref values must be distinct")
        seen_command_refs.add(command_ref)
        validate_command_log(artifact_bytes[command_ref], command_ref, scenario, artifacts, used)

    unused = set(artifacts) - used
    if unused:
        raise EvidenceCheckError(f"$.artifacts: unreferenced artifacts are forbidden: {sorted(unused)}")

    print(
        f"structurally/content-integrity valid: {bundle_path} "
        f"({len(artifacts)} artifacts, {len(writes)} governed-write claims)"
    )
    print(DISCLAIMER)


def parser() -> argparse.ArgumentParser:
    result = argparse.ArgumentParser(description=__doc__)
    result.add_argument("bundle", type=Path, help="alias-free YAML or JSON evidence bundle")
    return result


if __name__ == "__main__":
    try:
        check(parser().parse_args().bundle)
    except (OSError, EvidenceCheckError) as error:
        raise SystemExit(f"real-host evidence verification failed: {error}\n{DISCLAIMER}") from error
