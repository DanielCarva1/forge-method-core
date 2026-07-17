#!/usr/bin/env python3
"""Verify the exact, closed release executable graph and Cargo lock binding."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import re
import shlex
import stat
from pathlib import Path, PurePosixPath
from typing import NamedTuple


class ReleaseLockError(RuntimeError):
    """The release command graph is absent, ambiguous, unlocked, or ungoverned."""


class Invocation(NamedTuple):
    line: int
    tool: str
    subcommand: str
    command: str


class Step(NamedTuple):
    job: str
    name: str
    line: int
    run: str | None
    uses: str | None


class Job(NamedTuple):
    key: str
    name: str
    needs: tuple[str, ...]
    uses: str | None
    steps: tuple[Step, ...]


# This checker has no permissive production mode. A workflow edit requires review
# of the byte commitment, complete semantic manifest, and independently modeled
# graph edges. Authorizing candidate byte/graph digests cannot bypass the fixed
# manifest governed below.
EXPECTED_WORKFLOW_SHA256 = "aa7d30cbb4b54a992661c4683d4aa9ba618a1caad877a9dcdc2ef3eb6e52e654"
EXPECTED_GRAPH_SHA256 = "a9647fae81a3ca378706fdfe226908eaf34d4ce41b85317096ee036e575dde4d"
EXPECTED_CARGO_STEPS = {
    ("build", "Install cross"): ("cargo", "install", "cross", "--version", "0.2.5", "--locked", "--quiet"),
    ("build", "Build (Linux cross)"): ("cross", "build", "--locked", "--release", "--target", "${{", "matrix.target", "}}", "-p", "forge-core-cli"),
    ("build", "Build (native)"): ("cargo", "build", "--locked", "--release", "--target", "${{", "matrix.target", "}}", "-p", "forge-core-cli"),
    ("release", "Install cargo-cyclonedx"): ("cargo", "install", "cargo-cyclonedx", "--version", "0.5.9", "--locked", "--quiet"),
}
SBOM_STEP = ("release", "Generate and validate mandatory CycloneDX SBOM")
SBOM_WRAPPER_ARGV = (
    "python", "scripts/run-release-locked-sbom.py", "--lockfile", "Cargo.lock", "--",
    "--format", "json", "--manifest-path", "crates/forge-core-cli/Cargo.toml",
    "--override-filename", "forge-core-$VERSION.cdx",
)

# Every repository executable reached from a release `run` step is closed by
# path and content. The checker module is the running trust root, so it is not
# self-hashed. Imported/transitively executed support and fixture files are
# included, not merely the scripts directly named in YAML.
GOVERNED_FILE_SHA256 = {
    "scripts/run-release-locked-sbom.py": "c2d5d5461346988c83fa542d1ac4743c321bb4b0505983a6efb6285187c9eba8",
    "scripts/test-release-locking.py": "cdd8a29ea1e9b5e79a6494508add01c8b2312e858d31fd0dbdb8f61a7a1a3e80",
    "scripts/test-release-archive.py": "bf8b2ff42e91664e55dda3b623b26fe8b47f1ee524b9ca793899881081975ab2",
    "scripts/build-release-archive.py": "c5dbb723e768fec1469fd0928b138eacf4bc4d6e64e13d567c786bdb82eea593",
    "scripts/check-release-archive.py": "d61fdab452cd673a6b6fa676fe2100e4ee81a68e56e9bd0a6c4942dfd2d19ef4",
    "scripts/smoke-release-install.py": "6fa71b14db1fb27c69f7393ee81f4f1a19456fb5ce852310f3b38cae62cc3013",
    "distribution/forge": "6b151926a6b69e514d6542ff93974c1251f9a597a246ab49d8c04649f8a5f25b",
    "distribution/forge.cmd": "408e1172fcfda87b70956ddefda9798c6246419ad099763c22401638021bce38",
    "contracts/fixtures/release-lock/manifest-drift/Cargo.toml": "8ff62e94d1327c44671f0572c032cec8d770615c8356a64ec8be16751d878352",
    "contracts/fixtures/release-lock/manifest-drift/Cargo.lock": "8aac6f6c147c6e9099790e083f623e37e8016cbda16d778c9a22c1799fca46b0",
    "contracts/fixtures/release-lock/manifest-drift/src/main.rs": "536e506bb90914c243a12b397b9a998f85ae2cbd9ba02dfd03a9e155ca5ca0f4",
    "contracts/fixtures/release-lock/workflow-semantic-manifest.json": "4bf79afdf1fe16eee13b174337dd5b9d3d917142eb08e67f5c4b014f5b663eeb",
}

# Only these reviewed release payloads use Git's `text:auto` checkout policy.
# Their commitments are SHA-256 over canonical Git text (LF) bytes. All other
# governed files, especially workflow/security executables and the semantic
# manifest, retain exact materialized-byte authentication.
GIT_TEXT_GOVERNED_FILES = frozenset({
    "distribution/forge",
    "distribution/forge.cmd",
})
DIRECT_LOCAL_SCRIPTS = {
    ("metadata", "Test release lock enforcement"): {"scripts/test-release-locking.py"},
    ("metadata", "Test deterministic archive tooling"): {"scripts/test-release-archive.py"},
    ("build", "Build and verify deterministic release archive"): {
        "scripts/build-release-archive.py", "scripts/check-release-archive.py"
    },
    ("build", "Smoke extracted native release install"): {"scripts/smoke-release-install.py"},
    ("release", "Re-verify archive manifests and checksums"): {"scripts/check-release-archive.py"},
    SBOM_STEP: {"scripts/run-release-locked-sbom.py"},
}

# Reviewed transitive local executable/read graph. Contents are committed above;
# these edges state why each file is reachable. External tools are closed by the
# exact run/uses graph and immutable action revisions.
TRANSITIVE_LOCAL_GRAPH = {
    "scripts/test-release-locking.py": {
        "scripts/run-release-locked-sbom.py",
        "contracts/fixtures/release-lock/manifest-drift/Cargo.toml",
        "contracts/fixtures/release-lock/manifest-drift/Cargo.lock",
        "contracts/fixtures/release-lock/manifest-drift/src/main.rs",
        "contracts/fixtures/release-lock/workflow-semantic-manifest.json",
    },
    "scripts/test-release-archive.py": {
        "scripts/build-release-archive.py",
        "scripts/check-release-archive.py",
        "distribution/forge",
    },
    "scripts/build-release-archive.py": {"distribution/forge", "distribution/forge.cmd"},
    "scripts/smoke-release-install.py": {
        "scripts/check-release-archive.py",
        "distribution/forge",
        "distribution/forge.cmd",
    },
    "scripts/run-release-locked-sbom.py": set(),
    "scripts/check-release-archive.py": set(),
}

ANCHOR_OR_ALIAS = re.compile(r"(?:^|[\s:[{,])(?:&|\*)[A-Za-z0-9_-]+(?=$|[\s,\]}#])")
LOCAL_EXECUTABLE = re.compile(
    r"(?<![A-Za-z0-9_./-])((?:\./)?(?:[A-Za-z0-9_.-]+/)*[A-Za-z0-9_.-]+\.(?:py|sh|bash))(?![A-Za-z0-9_./-])"
)
JOB_HEADER = re.compile(r"^  ([A-Za-z0-9_-]+):\s*(?:#.*)?$")
STEP_START = re.compile(r"^      - (.*)$")
FIELD = re.compile(r"^        ([A-Za-z_][A-Za-z0-9_-]*):(?:\s*(.*))?$")
GLOBAL_VALUE_OPTIONS = {"--color", "--config", "--target-dir", "--lockfile-path", "-C", "-Z"}
GLOBAL_FLAG_OPTIONS = {
    "--locked", "--offline", "--frozen", "--quiet", "--verbose", "-q", "-v",
    "--help", "--version", "--list",
}


SEMANTIC_MANIFEST = "contracts/fixtures/release-lock/workflow-semantic-manifest.json"
PLAIN_KEY = re.compile(r"^([A-Za-z_][A-Za-z0-9_-]*):(?:\s*(.*))?$")
SEQUENCE_KEY = re.compile(r"^-\s+([A-Za-z_][A-Za-z0-9_-]*):(?:\s*(.*))?$")


def _strip_yaml_comment(text: str) -> str:
    """Strip a YAML comment without treating quoted # characters as comments."""
    single = False
    double = False
    index = 0
    while index < len(text):
        char = text[index]
        if char == "'" and not double:
            if single and index + 1 < len(text) and text[index + 1] == "'":
                index += 2
                continue
            single = not single
        elif char == '"' and not single:
            escaped = index > 0 and text[index - 1] == "\\"
            if not escaped:
                double = not double
        elif char == "#" and not single and not double and (
            index == 0 or text[index - 1].isspace()
        ):
            return text[:index].rstrip()
        index += 1
    return text.rstrip()


def _validate_unambiguous_yaml(source: str) -> None:
    """Reject duplicate mapping keys throughout the supported workflow YAML."""
    if "\t" in source:
        raise ReleaseLockError("release workflow may not contain tabs")
    seen: dict[tuple[object, int], set[str]] = {}
    active: dict[int, object] = {}
    sequence_numbers: dict[tuple[object, int], int] = {}
    block_indent: int | None = None

    for number, raw in enumerate(source.splitlines(), 1):
        indent = len(raw) - len(raw.lstrip(" "))
        if block_indent is not None:
            if not raw.strip() or indent > block_indent:
                continue
            block_indent = None
        content = _strip_yaml_comment(raw[indent:])
        if not content:
            continue
        if "<<:" in content or ANCHOR_OR_ALIAS.search(content):
            raise ReleaseLockError(
                f"workflow:{number}: YAML anchors, aliases, and merges are unsupported"
            )
        for level in [level for level in active if level >= indent]:
            del active[level]
        parent = active[max(active)] if active else ("document",)

        sequence = SEQUENCE_KEY.fullmatch(content)
        plain = PLAIN_KEY.fullmatch(content)
        if sequence is not None:
            counter_key = (parent, indent)
            item_number = sequence_numbers.get(counter_key, 0) + 1
            sequence_numbers[counter_key] = item_number
            item = (parent, "item", indent, item_number)
            active[indent] = item
            mapping = (item, indent)
            key, value = sequence.group(1), sequence.group(2) or ""
        elif plain is not None:
            mapping = (parent, indent)
            key, value = plain.group(1), plain.group(2) or ""
        elif content.startswith("-"):
            # Scalar sequence entries have no keys but still establish unique items.
            counter_key = (parent, indent)
            item_number = sequence_numbers.get(counter_key, 0) + 1
            sequence_numbers[counter_key] = item_number
            active[indent] = (parent, "item", indent, item_number)
            continue
        else:
            continue

        keys = seen.setdefault(mapping, set())
        if key in keys:
            raise ReleaseLockError(f"workflow:{number}: duplicate YAML mapping key {key!r}")
        keys.add(key)
        stripped_value = value.strip()
        if stripped_value.startswith("{") and stripped_value != "{}" and not stripped_value.startswith("${{"):
            raise ReleaseLockError(
                f"workflow:{number}: non-empty flow mappings are unsupported and ambiguous"
            )
        if stripped_value in {"|", "|-", "|+", ">", ">-", ">+"}:
            block_indent = indent
        elif not stripped_value:
            active[indent] = (mapping, key)


def workflow_semantic_manifest(source: str) -> dict[str, object]:
    """Return the complete reviewed YAML/shell token manifest.

    This is deliberately an exact closed shape, not a heuristic shell AST. Every
    non-comment YAML token and every byte of block-scalar command bodies is part
    of the immutable manifest.
    """
    _validate_unambiguous_yaml(source)
    entries: list[str] = []
    block_indent: int | None = None
    for raw in source.splitlines():
        indent = len(raw) - len(raw.lstrip(" "))
        if block_indent is not None:
            if not raw.strip() or indent > block_indent:
                entries.append(raw)
                continue
            block_indent = None
        semantic = _strip_yaml_comment(raw)
        if not semantic.strip():
            continue
        entries.append(semantic)
        value = semantic.strip()
        if re.search(r":\s*[|>]([-+])?\s*$", value):
            block_indent = indent
    return {"format": "forge-release-workflow-semantic-manifest-v1", "entries": entries}


def _check_full_semantic_manifest(source: str, root_fd: int) -> None:
    manifest_bytes = _read_relative_regular(
        root_fd, SEMANTIC_MANIFEST, "release semantic manifest"
    )
    actual_hash = _digest(manifest_bytes)
    expected_hash = GOVERNED_FILE_SHA256[SEMANTIC_MANIFEST]
    if actual_hash != expected_hash:
        raise ReleaseLockError(
            f"immutable release semantic manifest drifted ({actual_hash})"
        )
    try:
        # The bytes authenticated above are the only bytes parsed and used.
        expected = json.loads(manifest_bytes)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise ReleaseLockError(f"invalid immutable release semantic manifest: {error}") from error
    actual = workflow_semantic_manifest(source)
    if actual != expected:
        raise ReleaseLockError(
            "release workflow semantics differ from the immutable full manifest; "
            "all job, step, action-input, environment, runner, matrix, shell, "
            "permission, output, container, service, concurrency, timeout, "
            "default, condition, and checkout-ref fields are closed"
        )


def _digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def _scalar(value: str, location: str) -> str:
    value = value.strip()
    if not value:
        return ""
    if value.startswith("'"):
        if len(value) < 2 or not value.endswith("'"):
            raise ReleaseLockError(f"{location}: unterminated single-quoted YAML scalar")
        return value[1:-1].replace("''", "'")
    if value.startswith('"'):
        if len(value) < 2 or not value.endswith('"'):
            raise ReleaseLockError(f"{location}: unterminated double-quoted YAML scalar")
        try:
            # JSON strings are the intentionally supported, auditable subset of
            # YAML double-quoted scalar escapes.
            import json
            parsed = json.loads(value)
        except (ValueError, TypeError) as error:
            raise ReleaseLockError(f"{location}: unsupported quoted YAML scalar") from error
        if not isinstance(parsed, str):
            raise ReleaseLockError(f"{location}: scalar must be text")
        return parsed
    comment = re.search(r"\s+#", value)
    if comment is not None:
        value = value[:comment.start()].rstrip()
    if value.startswith(("*", "&", "!", "[", "{")):
        raise ReleaseLockError(f"{location}: unsupported YAML scalar syntax")
    return value


def _block_value(
    lines: list[str], start: int, end: int, field_indent: int, style: str, location: str
) -> tuple[str, set[int]]:
    indexes: list[int] = []
    index = start
    while index < end:
        line = lines[index]
        indent = len(line) - len(line.lstrip(" "))
        if line.strip() and indent <= field_indent:
            break
        indexes.append(index)
        index += 1
    nonempty = [len(lines[i]) - len(lines[i].lstrip(" ")) for i in indexes if lines[i].strip()]
    if not nonempty or min(nonempty) <= field_indent:
        raise ReleaseLockError(f"{location}: empty or malformed run block")
    content_indent = min(nonempty)
    content = [lines[i][content_indent:] if lines[i].strip() else "" for i in indexes]
    if style == "|":
        return "\n".join(content) + "\n", set(indexes)
    # Bounded folded scalars: ordinary lines and blank paragraph separators.
    # More-indented folding has surprising YAML semantics and is rejected.
    if any(lines[i].strip() and (len(lines[i]) - len(lines[i].lstrip(" "))) != content_indent for i in indexes):
        raise ReleaseLockError(f"{location}: more-indented folded run blocks are unsupported")
    paragraphs: list[str] = []
    current: list[str] = []
    for line in content:
        if line:
            current.append(line)
        else:
            if current:
                paragraphs.append(" ".join(current))
                current = []
            paragraphs.append("")
    if current:
        paragraphs.append(" ".join(current))
    return "\n".join(paragraphs).rstrip("\n") + "\n", set(indexes)


def parse_workflow(source: str) -> list[Step]:
    """Parse the bounded GitHub workflow subset; reject YAML indirection."""
    _validate_unambiguous_yaml(source)
    if "\t" in source:
        raise ReleaseLockError("release workflow may not contain tabs")
    lines = source.splitlines()
    if not any(line == "jobs:" for line in lines):
        raise ReleaseLockError("release workflow has no jobs mapping")

    steps: list[Step] = []
    block_lines: set[int] = set()
    job: str | None = None
    in_steps = False
    index = 0
    while index < len(lines):
        line = lines[index]
        job_match = JOB_HEADER.match(line)
        if job_match:
            job = job_match.group(1)
            in_steps = False
            index += 1
            continue
        if job is not None and line == "    steps:":
            in_steps = True
            index += 1
            continue
        if not in_steps:
            index += 1
            continue
        start_match = STEP_START.match(line)
        if start_match is None:
            if line.strip() and len(line) - len(line.lstrip(" ")) <= 4:
                in_steps = False
            index += 1
            continue

        start = index
        end = start + 1
        while end < len(lines):
            candidate = lines[end]
            indent = len(candidate) - len(candidate.lstrip(" "))
            if STEP_START.match(candidate) or (candidate.strip() and indent <= 4):
                break
            end += 1

        fields: dict[str, tuple[str, int]] = {}
        first = start_match.group(1)
        first_field = re.fullmatch(r"([A-Za-z_][A-Za-z0-9_-]*):(?:\s*(.*))?", first)
        candidates: list[tuple[str, str, int]] = []
        if first_field:
            candidates.append((first_field.group(1), first_field.group(2) or "", start))
        elif first.strip():
            raise ReleaseLockError(f"workflow:{start + 1}: unsupported step syntax")
        for field_index in range(start + 1, end):
            match = FIELD.match(lines[field_index])
            if match:
                candidates.append((match.group(1), match.group(2) or "", field_index))

        for key, value, field_index in candidates:
            if key not in {"name", "run", "uses"}:
                continue
            if key in fields:
                raise ReleaseLockError(f"workflow:{field_index + 1}: duplicate step key {key!r}")
            location = f"workflow:{field_index + 1}"
            if key == "run" and value in {"|", "|-", "|+", ">", ">-", ">+"}:
                parsed, occupied = _block_value(
                    lines, field_index + 1, end, 8, value[0], location
                )
                block_lines.update(occupied)
            else:
                parsed = _scalar(value, location)
            fields[key] = (parsed, field_index + 1)

        if "name" not in fields or not fields["name"][0]:
            raise ReleaseLockError(f"workflow:{start + 1}: every release step needs an exact name")
        if ("run" in fields) == ("uses" in fields):
            raise ReleaseLockError(
                f"workflow:{start + 1}: step must have exactly one of run or uses"
            )
        if job is None:
            raise ReleaseLockError(f"workflow:{start + 1}: step is outside a job")
        steps.append(
            Step(job, fields["name"][0], start + 1,
                 fields.get("run", (None, 0))[0], fields.get("uses", (None, 0))[0])
        )
        index = end

    for line_index, line in enumerate(lines):
        if line_index in block_lines:
            continue
        stripped = re.sub(r"\s+#.*$", "", line)
        if "<<:" in stripped or ANCHOR_OR_ALIAS.search(stripped):
            raise ReleaseLockError(
                f"workflow:{line_index + 1}: YAML anchors, aliases, and merges are unsupported"
            )
    identities = [(step.job, step.name) for step in steps]
    if len(identities) != len(set(identities)):
        raise ReleaseLockError("release workflow contains duplicate named steps in a job")
    if not steps:
        raise ReleaseLockError("release workflow contains no steps")
    return steps


def parse_graph(source: str) -> tuple[Job, ...]:
    """Model every job, dependency edge, job-level use, and named step body."""
    lines = source.splitlines()
    try:
        jobs_line = lines.index("jobs:")
    except ValueError as error:
        raise ReleaseLockError("release workflow has no jobs mapping") from error
    headers = [
        (index, match.group(1))
        for index in range(jobs_line + 1, len(lines))
        if (match := JOB_HEADER.match(lines[index])) is not None
    ]
    if not headers:
        raise ReleaseLockError("release workflow has no jobs")
    parsed_steps = parse_workflow(source)
    jobs: list[Job] = []
    for position, (start, key) in enumerate(headers):
        end = headers[position + 1][0] if position + 1 < len(headers) else len(lines)
        fields: dict[str, str] = {}
        for index in range(start + 1, end):
            match = re.match(r"^    ([A-Za-z_][A-Za-z0-9_-]*):(?:\s*(.*))?$", lines[index])
            if match is None or match.group(1) not in {"name", "needs", "uses"}:
                continue
            field = match.group(1)
            if field in fields:
                raise ReleaseLockError(f"workflow:{index + 1}: duplicate job field {field!r}")
            value = match.group(2) or ""
            fields[field] = value.strip() if field == "needs" else _scalar(value, f"workflow:{index + 1}")
        name = fields.get("name", "")
        if not name:
            raise ReleaseLockError(f"workflow:{start + 1}: every release job needs an exact name")
        needs_source = fields.get("needs", "")
        if needs_source.startswith("["):
            if not needs_source.endswith("]"):
                raise ReleaseLockError(f"workflow:{start + 1}: unsupported multiline needs")
            needs = tuple(
                item.strip() for item in needs_source[1:-1].split(",") if item.strip()
            )
        else:
            needs = (needs_source,) if needs_source else ()
        if len(needs) != len(set(needs)):
            raise ReleaseLockError(f"workflow:{start + 1}: duplicate dependency edge")
        steps = tuple(step for step in parsed_steps if step.job == key)
        job_uses = fields.get("uses") or None
        if bool(steps) == bool(job_uses):
            raise ReleaseLockError(
                f"workflow:{start + 1}: job must have exactly one of steps or job-level uses"
            )
        jobs.append(Job(key, name, needs, job_uses, steps))
    keys = [job.key for job in jobs]
    if len(keys) != len(set(keys)):
        raise ReleaseLockError("release workflow contains duplicate jobs")
    known = set(keys)
    for job in jobs:
        unknown = set(job.needs) - known
        if unknown:
            raise ReleaseLockError(f"job {job.key!r} needs unknown jobs {sorted(unknown)!r}")
    return tuple(jobs)


def graph_digest(source: str) -> str:
    """Commit the complete workflow semantics plus the independently modeled edges."""
    jobs = parse_graph(source)
    payload = {
        "semantic_manifest": workflow_semantic_manifest(source),
        "modeled_jobs": [
            {
                "job": job.key,
                "name": job.name,
                "needs": list(job.needs),
                "uses": job.uses,
                "steps": [
                    {"name": step.name, "run": step.run, "uses": step.uses}
                    for step in job.steps
                ],
            }
            for job in jobs
        ],
    }
    return _digest(json.dumps(payload, sort_keys=True, separators=(",", ":")).encode())


def _segments(body: str) -> list[list[str]]:
    if "<<" in body or "`" in body or "$(" in body:
        raise ReleaseLockError("Cargo-bearing shell uses unsupported expansion or heredoc")
    logical = re.sub(r"\\\r?\n[ \t]*", " ", body).replace("\n", " ; ")
    lexer = shlex.shlex(logical, posix=True, punctuation_chars=";&|()<>")
    lexer.whitespace_split = True
    lexer.commenters = "#"
    try:
        tokens = list(lexer)
    except ValueError as error:
        raise ReleaseLockError(f"ambiguous shell syntax: {error}") from error
    result: list[list[str]] = []
    current: list[str] = []
    for token in tokens:
        if token in {";", "&&", "||", "|", "&", "\n"}:
            if current:
                result.append(current)
                current = []
            continue
        if token in {"(", ")", "<", ">", "<<", ">>"}:
            raise ReleaseLockError(f"unsupported shell operator in Cargo-bearing run: {token}")
        current.append(token)
    if current:
        result.append(current)
    return result


def _cargo_subcommand(arguments: list[str]) -> tuple[str, int]:
    index = 0
    if index < len(arguments) and arguments[index].startswith("+"):
        if arguments[index] == "+":
            raise ReleaseLockError("empty Cargo toolchain selector")
        index += 1
    while index < len(arguments):
        token = arguments[index]
        if token == "--":
            raise ReleaseLockError("Cargo subcommand is absent before --")
        if token in GLOBAL_FLAG_OPTIONS or token.startswith("--config="):
            index += 1
            continue
        if token in GLOBAL_VALUE_OPTIONS:
            if index + 1 >= len(arguments) or arguments[index + 1] == "--":
                raise ReleaseLockError(f"Cargo global option {token} is missing its value")
            index += 2
            continue
        if token.startswith("-"):
            raise ReleaseLockError(f"unsupported Cargo global option before subcommand: {token}")
        return token, index
    raise ReleaseLockError("Cargo invocation has no subcommand")


def _without_heredocs(body: str) -> str:
    """Remove literal heredoc payloads while preserving their shell command line."""
    result: list[str] = []
    delimiter: str | None = None
    for line in body.splitlines():
        if delimiter is not None:
            if line == delimiter:
                delimiter = None
            continue
        match = re.search(r"<<-?\s*(['\"]?)([A-Za-z_][A-Za-z0-9_]*)\1\s*$", line)
        if match is not None:
            delimiter = match.group(2)
            result.append(line[: match.start()].rstrip())
        else:
            result.append(line)
    if delimiter is not None:
        raise ReleaseLockError("unterminated shell heredoc")
    return "\n".join(result)


def find_invocations(body: str, line: int = 1) -> list[Invocation]:
    """Scan one run body; reject Cargo wrappers and executable indirection."""
    shell = _without_heredocs(body)
    candidate = re.compile(r"(?<![A-Za-z0-9_.-])(?:cargo(?:-[A-Za-z0-9_-]+)?|cross)(?![A-Za-z0-9_.-])", re.I)
    if candidate.search(shell) is None and not re.search(
        r"(?:^|[;&|]\s*)(?:source|\.)\s+", shell, re.M
    ):
        return []

    invocations: list[Invocation] = []
    for original_words in _segments(shell):
        words = list(original_words)
        if not words:
            continue
        segment_has_candidate = any(candidate.search(word) for word in words)
        if words[0] in {"source", "."}:
            raise ReleaseLockError("source and dot commands are forbidden in release runs")
        if words[0] in {"alias", "function"} or any(word in {"alias", "function"} for word in words):
            if segment_has_candidate:
                raise ReleaseLockError("shell aliases and functions are forbidden in Cargo-bearing runs")
            continue
        assignments = []
        while words and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*=.*", words[0]):
            assignments.append(words.pop(0))
        if assignments and (
            any(candidate.search(item) for item in assignments) or not words
        ):
            raise ReleaseLockError("shell assignment may not define or hide a release executable")
        if not words:
            continue
        executable = words[0]
        basename = PurePosixPath(executable.replace("\\", "/")).name
        if basename in {"env", "eval", "exec", "time", "command"}:
            if segment_has_candidate:
                raise ReleaseLockError(f"{basename} may not wrap a Cargo release executable")
            continue
        if basename in {"sh", "bash"} and "-c" in words[1:]:
            if segment_has_candidate:
                raise ReleaseLockError(f"{basename} -c may not hide a Cargo release executable")
            continue
        if executable.startswith("$") or executable.startswith("${"):
            if segment_has_candidate:
                raise ReleaseLockError("variable-selected release executables are forbidden")
            continue
        if basename.startswith("cargo-"):
            raise ReleaseLockError("direct Cargo plugin execution is forbidden")
        if basename not in {"cargo", "cross"}:
            if segment_has_candidate:
                raise ReleaseLockError(
                    f"Cargo release executable is indirect behind {executable!r}"
                )
            continue
        if executable != basename:
            raise ReleaseLockError(f"path-qualified {basename} execution is forbidden")
        subcommand, _ = _cargo_subcommand(words[1:])
        if subcommand == "cyclonedx":
            raise ReleaseLockError("direct Cargo cyclonedx plugin execution is forbidden")
        separator = words.index("--") if "--" in words else len(words)
        if "--locked" not in words[1:separator]:
            raise ReleaseLockError(
                f"{basename} {subcommand} must include --locked before Cargo's -- separator"
            )
        invocations.append(Invocation(line, basename, subcommand, shlex.join(words)))
    return invocations


def _script_command(body: str, script: str) -> tuple[str, ...]:
    lines = body.splitlines()
    for index, line in enumerate(lines):
        if script not in line:
            continue
        command = line.strip()
        while command.endswith("\\"):
            command = command[:-1] + " "
            index += 1
            if index >= len(lines):
                raise ReleaseLockError(f"unterminated command for {script}")
            command += lines[index].strip()
        try:
            return tuple(shlex.split(command, posix=True))
        except ValueError as error:
            raise ReleaseLockError(f"ambiguous command for {script}: {error}") from error
    raise ReleaseLockError(f"missing exact local command for {script}")


def _require_descriptor_primitives() -> None:
    if not hasattr(os, "O_NOFOLLOW") or not Path("/proc/self/fd").is_dir():
        raise ReleaseLockError(
            "release governance requires Linux O_NOFOLLOW and /proc/self/fd"
        )


def _open_absolute_directory(path: Path, label: str) -> int:
    """Bind an absolute directory through no-follow opens of every component."""
    _require_descriptor_primitives()
    absolute = Path(os.path.abspath(path))
    descriptor = os.open("/", os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for part in absolute.parts[1:]:
            if part in {"", ".", ".."}:
                raise ReleaseLockError(f"{label} has an unsafe path component: {absolute}")
            next_fd = os.open(
                part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=descriptor
            )
            os.close(descriptor)
            descriptor = next_fd
        return descriptor
    except BaseException as error:
        os.close(descriptor)
        if isinstance(error, ReleaseLockError):
            raise
        if isinstance(error, OSError):
            raise ReleaseLockError(
                f"cannot bind {label} without following symlinks: {absolute}: {error}"
            ) from error
        raise


def _relative_parts(relative: str) -> tuple[str, ...]:
    pure = PurePosixPath(relative)
    if pure.is_absolute() or not pure.parts or any(
        part in {"", ".", ".."} for part in pure.parts
    ):
        raise ReleaseLockError(f"unsafe governed relative path: {relative!r}")
    return pure.parts


def _read_relative_regular(root_fd: int, relative: str, label: str) -> bytes:
    """Read one leaf through retained no-follow parent descriptors and bind identity."""
    parts = _relative_parts(relative)
    parent_fd = os.dup(root_fd)
    try:
        for part in parts[:-1]:
            next_fd = os.open(
                part, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd
            )
            os.close(parent_fd)
            parent_fd = next_fd
        fd = os.open(
            parts[-1],
            os.O_RDONLY | os.O_NOFOLLOW | getattr(os, "O_BINARY", 0),
            dir_fd=parent_fd,
        )
        try:
            before = os.fstat(fd)
            if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1:
                raise ReleaseLockError(f"{label} is not a safe regular file: {relative}")
            chunks: list[bytes] = []
            while chunk := os.read(fd, 1024 * 1024):
                chunks.append(chunk)
            after = os.fstat(fd)
            before_identity = (
                before.st_dev, before.st_ino, before.st_mode, before.st_nlink,
                before.st_size, before.st_mtime_ns, before.st_ctime_ns,
            )
            after_identity = (
                after.st_dev, after.st_ino, after.st_mode, after.st_nlink,
                after.st_size, after.st_mtime_ns, after.st_ctime_ns,
            )
            if before_identity != after_identity:
                raise ReleaseLockError(f"{label} changed while read: {relative}")
            return b"".join(chunks)
        finally:
            os.close(fd)
    except ReleaseLockError:
        raise
    except OSError as error:
        raise ReleaseLockError(f"cannot read {label} {relative}: {error}") from error
    finally:
        os.close(parent_fd)


def _read_absolute_regular(path: Path, label: str) -> bytes:
    absolute = Path(os.path.abspath(path))
    parent_fd = _open_absolute_directory(absolute.parent, f"{label} parent")
    try:
        return _read_relative_regular(parent_fd, absolute.name, label)
    finally:
        os.close(parent_fd)


def _canonical_governed_bytes(relative: str, data: bytes) -> bytes:
    """Return reviewed Git text bytes; reject every representation but LF/CRLF."""
    if relative not in GIT_TEXT_GOVERNED_FILES or b"\r" not in data:
        return data
    crlf_count = data.count(b"\r\n")
    if crlf_count != data.count(b"\r") or crlf_count != data.count(b"\n"):
        raise ReleaseLockError(
            f"governed Git text has mixed or lone CR line endings: {relative}"
        )
    return data.replace(b"\r\n", b"\n")


def _check_governed_files(root_fd: int) -> None:
    governed = set(GOVERNED_FILE_SHA256)
    graph_files = set(TRANSITIVE_LOCAL_GRAPH)
    graph_files.update(path for targets in TRANSITIVE_LOCAL_GRAPH.values() for path in targets)
    if not graph_files <= governed:
        raise ReleaseLockError(
            f"transitive local graph contains uncommitted files: {sorted(graph_files - governed)}"
        )
    for relative, expected in GOVERNED_FILE_SHA256.items():
        # The semantic manifest was already authenticated and consumed from one
        # descriptor read; never reopen it as a later authorization decision.
        if relative == SEMANTIC_MANIFEST:
            continue
        materialized = _read_relative_regular(
            root_fd, relative, "governed release file"
        )
        actual = _digest(_canonical_governed_bytes(relative, materialized))
        if actual != expected:
            raise ReleaseLockError(
                f"governed release file content drifted: {relative} ({actual})"
            )


def _check_graph_security(source: str, jobs: tuple[Job, ...]) -> None:
    by_key = {job.key: job for job in jobs}
    metadata = by_key.get("metadata")
    if metadata is None or metadata.needs:
        raise ReleaseLockError("metadata must be the root release identity gate")
    gate = next(
        (step for step in metadata.steps if step.name == "Bind executing workflow to checked-out release commit"),
        None,
    )
    if gate is None or gate.run is None:
        raise ReleaseLockError("immutable executing-workflow identity gate is missing")
    binding = "EXECUTING_WORKFLOW_SHA: ${{ github.workflow_sha }}"
    if source.count(binding) != 1:
        raise ReleaseLockError("workflow identity gate must consume exact github.workflow_sha")
    if "test \"$EXECUTING_WORKFLOW_SHA\" = \"$checked_out_sha\"" not in gate.run:
        raise ReleaseLockError("workflow identity gate does not compare workflow SHA to checkout HEAD")

    checkout_use = "uses: actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5"
    metadata_ref = "ref: ${{ github.event_name == 'workflow_dispatch' && inputs.tag || github.ref }}"
    immutable_ref = "ref: ${{ needs.metadata.outputs.commit_sha }}"
    ref_keys = re.findall(r"^\s+ref:\s*.+$", source, re.MULTILINE)
    if (
        source.count(checkout_use) != 3
        or ref_keys.count("          " + metadata_ref) != 1
        or ref_keys.count("          " + immutable_ref) != 2
        or len(ref_keys) != 3
    ):
        raise ReleaseLockError(
            "checkout refs are not closed: metadata selects the requested tag and "
            "every artifact job must checkout exactly needs.metadata.outputs.commit_sha"
        )

    def reaches_metadata(key: str, visiting: set[str]) -> bool:
        if key == "metadata":
            return True
        if key in visiting:
            raise ReleaseLockError(f"dependency cycle reaches {key!r}")
        return any(
            reaches_metadata(parent, visiting | {key})
            for parent in by_key[key].needs
        )

    for job in jobs:
        if job.key != "metadata" and not reaches_metadata(job.key, set()):
            raise ReleaseLockError(
                f"artifact-capable job {job.key!r} does not depend on metadata identity gate"
            )
        if job.uses is not None and job.uses.startswith(("./", "../")):
            raise ReleaseLockError(f"local job-level reusable workflow is forbidden: {job.uses}")
        uses_values = [step.uses for step in job.steps if step.uses is not None]
        if job.uses is not None:
            uses_values.append(job.uses)
        for uses in uses_values:
            if uses.startswith(("./", "../")):
                raise ReleaseLockError(f"ungoverned local action/workflow is forbidden: {uses}")
            if re.fullmatch(r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+(?:/.+)?@[0-9a-f]{40}", uses) is None:
                raise ReleaseLockError(f"remote action/workflow is not immutable: {uses}")


def _check_source_bound(
    source: str,
    *,
    root_fd: int,
    sbom_runner: Path | None,
    expected_workflow_sha256: str,
    expected_graph_sha256: str,
) -> list[Invocation]:
    try:
        raw = source.encode("utf-8")
    except UnicodeEncodeError as error:
        raise ReleaseLockError(f"workflow is not UTF-8: {error}") from error
    # Authenticate the exact candidate bytes before any YAML/shell parsing.
    actual_workflow_hash = _digest(raw)
    if actual_workflow_hash != expected_workflow_sha256:
        raise ReleaseLockError(
            "release workflow is outside the exact reviewed source identity "
            f"({actual_workflow_hash}); checker plus commitments are the review trust root"
        )
    _check_full_semantic_manifest(source, root_fd)
    jobs = parse_graph(source)
    steps = [step for job in jobs for step in job.steps]
    by_identity = {(step.job, step.name): step for step in steps}
    _check_graph_security(source, jobs)

    actual_direct: dict[tuple[str, str], set[str]] = {}
    scanned: list[Invocation] = []
    for step in steps:
        if step.run is None:
            continue
        scripts = {path.removeprefix("./") for path in LOCAL_EXECUTABLE.findall(step.run)}
        if scripts:
            actual_direct[(step.job, step.name)] = scripts
        scanned.extend(find_invocations(step.run, step.line))
    if actual_direct != DIRECT_LOCAL_SCRIPTS:
        raise ReleaseLockError(
            f"local release script graph drifted; expected={DIRECT_LOCAL_SCRIPTS!r}, actual={actual_direct!r}"
        )

    actual_graph_hash = graph_digest(source)
    if actual_graph_hash != expected_graph_sha256:
        raise ReleaseLockError(
            f"release job/step graph is outside the reviewed closed manifest ({actual_graph_hash})"
        )

    if sbom_runner is not None:
        runner_hash = _digest(_read_absolute_regular(sbom_runner, "SBOM runner"))
        if runner_hash != GOVERNED_FILE_SHA256["scripts/run-release-locked-sbom.py"]:
            raise ReleaseLockError("SBOM runner is outside the governed content contract")
    _check_governed_files(root_fd)

    invocations: list[Invocation] = []
    for identity, expected_argv in EXPECTED_CARGO_STEPS.items():
        step = by_identity.get(identity)
        if step is None or step.run is None:
            raise ReleaseLockError(f"missing exact Cargo step {identity!r}")
        try:
            actual_argv = tuple(shlex.split(step.run.strip(), posix=True))
        except ValueError as error:
            raise ReleaseLockError(f"ambiguous Cargo step {identity!r}: {error}") from error
        if actual_argv != expected_argv:
            raise ReleaseLockError(
                f"Cargo argv drifted in {identity!r}: expected={expected_argv!r}, actual={actual_argv!r}"
            )
        parsed = find_invocations(step.run, step.line)
        if len(parsed) != 1:
            raise ReleaseLockError(f"Cargo step {identity!r} must execute exactly one Cargo command")
        invocations.extend(parsed)

    sbom = by_identity.get(SBOM_STEP)
    if sbom is None or sbom.run is None:
        raise ReleaseLockError("missing exact named SBOM execution step")
    if _script_command(sbom.run, "scripts/run-release-locked-sbom.py") != SBOM_WRAPPER_ARGV:
        raise ReleaseLockError("SBOM step does not execute the exact governed wrapper argv")
    invocations.append(
        Invocation(sbom.line, "cargo", "metadata", "cargo metadata --locked (exact governed cargo-cyclonedx shim)")
    )
    if [(item.tool, item.subcommand, item.command) for item in scanned] != [
        (item.tool, item.subcommand, item.command) for item in invocations[:-1]
    ]:
        raise ReleaseLockError("whole-workflow Cargo scan differs from exact five-invocation model")
    return invocations


def check_source(
    source: str,
    *,
    repo_root: Path,
    sbom_runner: Path | None = None,
    expected_workflow_sha256: str = EXPECTED_WORKFLOW_SHA256,
    expected_graph_sha256: str = EXPECTED_GRAPH_SHA256,
) -> list[Invocation]:
    root_fd = _open_absolute_directory(repo_root, "canonical repository root")
    try:
        return _check_source_bound(
            source, root_fd=root_fd, sbom_runner=sbom_runner,
            expected_workflow_sha256=expected_workflow_sha256,
            expected_graph_sha256=expected_graph_sha256,
        )
    finally:
        os.close(root_fd)


def check(
    workflow: Path, sbom_runner: Path | None = None, repo_root: Path | None = None
) -> list[Invocation]:
    root = Path(os.path.abspath(
        repo_root or Path(os.path.abspath(__file__)).parents[1]
    ))
    canonical = root / ".github/workflows/release.yml"
    candidate = Path(os.path.abspath(workflow))
    if candidate != canonical:
        raise ReleaseLockError(
            f"checker is bound to canonical repository workflow {canonical}, got {candidate}"
        )
    root_fd = _open_absolute_directory(root, "canonical repository root")
    try:
        raw = _read_relative_regular(
            root_fd, ".github/workflows/release.yml", "canonical release workflow"
        )
        # Do not decode or otherwise consume canonical bytes until committed.
        actual = _digest(raw)
        if actual != EXPECTED_WORKFLOW_SHA256:
            raise ReleaseLockError(
                "release workflow is outside the exact reviewed source identity "
                f"({actual}); checker plus commitments are the review trust root"
            )
        try:
            source = raw.decode("utf-8")
        except UnicodeDecodeError as error:
            raise ReleaseLockError(f"cannot read UTF-8 workflow {candidate}: {error}") from error
        return _check_source_bound(
            source, root_fd=root_fd, sbom_runner=sbom_runner,
            expected_workflow_sha256=EXPECTED_WORKFLOW_SHA256,
            expected_graph_sha256=EXPECTED_GRAPH_SHA256,
        )
    finally:
        os.close(root_fd)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "workflow", nargs="?", type=Path,
        default=Path(__file__).resolve().parents[1] / ".github/workflows/release.yml",
    )
    args = parser.parse_args()
    invocations = check(args.workflow)
    print(f"Release Cargo lock audit: {len(invocations)} exact locked invocation(s)")
    for item in invocations:
        print(f"  {args.workflow}:{item.line}: {item.command}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except ReleaseLockError as error:
        raise SystemExit(f"release Cargo lock audit failed: {error}") from error
