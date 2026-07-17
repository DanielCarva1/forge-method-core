#!/usr/bin/env python3
"""Verify the exact, closed release executable graph and Cargo lock binding."""

from __future__ import annotations

import argparse
import hashlib
import re
import shlex
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


# This checker deliberately has no permissive mode. Any release workflow edit must
# update this reviewed content commitment and the semantic command expectations.
EXPECTED_WORKFLOW_SHA256 = "6a9fd2484ea488fec3c9b0101c685b0ad62df3a57da19bb63594ec0ec6658ddc"
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
    "scripts/run-release-locked-sbom.py": "81f0e23ad5928426a5ac2b75f4b6a07486cfa32c3819ad8bf56c286e763cbbcc",
    "scripts/test-release-locking.py": "b7491f05744b797b5339fec768f86bfe75434db81c8f86ec8f00f693b8f0611e",
    "scripts/test-release-archive.py": "bf8b2ff42e91664e55dda3b623b26fe8b47f1ee524b9ca793899881081975ab2",
    "scripts/build-release-archive.py": "c5dbb723e768fec1469fd0928b138eacf4bc4d6e64e13d567c786bdb82eea593",
    "scripts/check-release-archive.py": "d61fdab452cd673a6b6fa676fe2100e4ee81a68e56e9bd0a6c4942dfd2d19ef4",
    "scripts/smoke-release-install.py": "6fa71b14db1fb27c69f7393ee81f4f1a19456fb5ce852310f3b38cae62cc3013",
    "distribution/forge": "6b151926a6b69e514d6542ff93974c1251f9a597a246ab49d8c04649f8a5f25b",
    "contracts/fixtures/release-lock/manifest-drift/Cargo.toml": "8ff62e94d1327c44671f0572c032cec8d770615c8356a64ec8be16751d878352",
    "contracts/fixtures/release-lock/manifest-drift/Cargo.lock": "8aac6f6c147c6e9099790e083f623e37e8016cbda16d778c9a22c1799fca46b0",
    "contracts/fixtures/release-lock/manifest-drift/src/main.rs": "536e506bb90914c243a12b397b9a998f85ae2cbd9ba02dfd03a9e155ca5ca0f4",
}
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


def find_invocations(body: str, line: int = 1) -> list[Invocation]:
    """Parse simple Cargo-bearing shell; ambiguous executable indirection rejects."""
    invocations: list[Invocation] = []
    for words in _segments(body):
        if not words:
            continue
        if words[0] in {"alias", "function"} or any(word in {"alias", "function"} for word in words):
            raise ReleaseLockError("shell aliases and functions are forbidden in Cargo-bearing runs")
        assignments = []
        while words and re.fullmatch(r"[A-Za-z_][A-Za-z0-9_]*=.*", words[0]):
            assignments.append(words.pop(0))
        if assignments:
            if any(re.search(r"cargo|cross", item, re.IGNORECASE) for item in assignments) or not words:
                raise ReleaseLockError("shell assignment may not define or hide a release executable")
        if not words:
            continue
        if words[0] == "command":
            words = words[1:]
            if not words:
                raise ReleaseLockError("command wrapper has no executable")
        executable = words[0]
        if executable.startswith("$") or executable.startswith("${"):
            raise ReleaseLockError("variable-selected release executables are forbidden")
        basename = PurePosixPath(executable.replace("\\", "/")).name
        if basename == "cargo-cyclonedx":
            raise ReleaseLockError("direct cargo-cyclonedx execution is forbidden")
        if basename not in {"cargo", "cross"}:
            continue
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


def _check_governed_files(root: Path) -> None:
    for relative, expected in GOVERNED_FILE_SHA256.items():
        path = root / relative
        try:
            if not path.is_file() or path.is_symlink():
                raise ReleaseLockError(f"governed release file is missing or unsafe: {relative}")
            actual = _digest(path.read_bytes())
        except OSError as error:
            raise ReleaseLockError(f"cannot read governed release file {relative}: {error}") from error
        if actual != expected:
            raise ReleaseLockError(
                f"governed release file content drifted: {relative} ({actual})"
            )


def check(
    workflow: Path, sbom_runner: Path | None = None, repo_root: Path | None = None
) -> list[Invocation]:
    try:
        raw = workflow.read_bytes()
        source = raw.decode("utf-8")
    except (OSError, UnicodeDecodeError) as error:
        raise ReleaseLockError(f"cannot read UTF-8 workflow {workflow}: {error}") from error
    steps = parse_workflow(source)
    by_identity = {(step.job, step.name): step for step in steps}

    # Inventory local scripts semantically from actual run bodies before checking
    # the whole-workflow commitment, so unsupported additions get a precise error.
    actual_direct: dict[tuple[str, str], set[str]] = {}
    for step in steps:
        if step.run is None:
            if step.uses is not None and step.uses.startswith(("./", "../")):
                raise ReleaseLockError(f"ungoverned local action in {step.job}/{step.name}: {step.uses}")
            continue
        scripts = {path.removeprefix("./") for path in LOCAL_EXECUTABLE.findall(step.run)}
        if scripts:
            actual_direct[(step.job, step.name)] = scripts
    if actual_direct != DIRECT_LOCAL_SCRIPTS:
        raise ReleaseLockError(
            f"local release script graph drifted; expected={DIRECT_LOCAL_SCRIPTS!r}, actual={actual_direct!r}"
        )

    actual_workflow_hash = _digest(raw)
    if actual_workflow_hash != EXPECTED_WORKFLOW_SHA256:
        raise ReleaseLockError(
            "release workflow is outside the exact reviewed command graph "
            f"({actual_workflow_hash}); review semantics and update the commitment"
        )

    root = repo_root or Path(__file__).resolve().parents[1]
    if sbom_runner is not None:
        expected_runner = root / "scripts/run-release-locked-sbom.py"
        if sbom_runner.resolve() != expected_runner.resolve():
            runner_hash = _digest(sbom_runner.read_bytes())
            if runner_hash != GOVERNED_FILE_SHA256["scripts/run-release-locked-sbom.py"]:
                raise ReleaseLockError("SBOM runner is outside the governed content contract")
    _check_governed_files(root)

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
    return invocations


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
