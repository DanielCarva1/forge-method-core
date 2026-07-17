#!/usr/bin/env python3
"""Fail closed when the workspace MSRV lane or manifest topology drifts."""

from __future__ import annotations

import argparse
import re
import shlex
import sys
import tomllib
from pathlib import Path, PurePosixPath
from typing import Any, NamedTuple


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/ci.yml"
DECLARED_MSRV = "1.85"
TOOLCHAIN = "1.85.1"
CHECKOUT_ACTION = "actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5"
TOOLCHAIN_ACTION = "dtolnay/rust-toolchain@4be7066ada62dd38de10e7b70166bc74ed198c30"
UPLOAD_ACTION = "actions/upload-artifact@ea165f8d65b6e75b540449e92b4886f43607fa02"
CHECK_COMMAND = (
    "python scripts/run-ci-tier.py --tier msrv-contract --budget-seconds 300 "
    "--report target/ci-timing/msrv-contract.json -- python scripts/test-msrv.py"
)
CARGO_COMMAND = (
    "python scripts/run-ci-tier.py --tier msrv-workspace --budget-seconds 1800 "
    "--report target/ci-timing/msrv-workspace.json -- cargo +1.85.1 check "
    "--locked --workspace --all-targets --all-features"
)


class MsrvCheckError(RuntimeError):
    """The declared MSRV or its complete CI proof has drifted."""


class Step(NamedTuple):
    name: str
    fields: dict[str, str]
    with_fields: dict[str, str]


ANCHOR_OR_ALIAS = re.compile(r"(?:^|[\s:[{,])(?:&|\*)[A-Za-z0-9_-]+(?=$|[\s,\]}#])")
PLAIN_KEY = re.compile(r"^([A-Za-z_][A-Za-z0-9_-]*):(?:\s*(.*))?$")
SEQUENCE_KEY = re.compile(r"^-\s+([A-Za-z_][A-Za-z0-9_-]*):(?:\s*(.*))?$")


def _strip_yaml_comment(text: str) -> str:
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
        elif char == '"' and not single and (index == 0 or text[index - 1] != "\\"):
            double = not double
        elif char == "#" and not single and not double and (
            index == 0 or text[index - 1].isspace()
        ):
            return text[:index].rstrip()
        index += 1
    return text.rstrip()


def validate_unambiguous_yaml(source: str) -> None:
    """Reject duplicate keys, tabs, aliases, and merges before semantic parsing."""
    if "\t" in source:
        raise MsrvCheckError("CI workflow may not contain tabs")
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
            raise MsrvCheckError(
                f"CI workflow:{number}: YAML anchors, aliases, and merges are unsupported"
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
            counter_key = (parent, indent)
            item_number = sequence_numbers.get(counter_key, 0) + 1
            sequence_numbers[counter_key] = item_number
            active[indent] = (parent, "item", indent, item_number)
            continue
        else:
            continue

        keys = seen.setdefault(mapping, set())
        if key in keys:
            raise MsrvCheckError(f"CI workflow:{number}: duplicate YAML key {key!r}")
        keys.add(key)
        if value in {"|", ">", "|-", ">-", "|+", ">+"}:
            block_indent = indent
        elif not value:
            active[indent] = (parent, key, indent)


def _load_toml(path: Path) -> dict[str, Any]:
    try:
        with path.open("rb") as stream:
            return tomllib.load(stream)
    except (OSError, tomllib.TOMLDecodeError) as error:
        raise MsrvCheckError(f"cannot parse {path}: {error}") from error


def _member_paths(manifest: dict[str, Any]) -> list[PurePosixPath]:
    workspace = manifest.get("workspace")
    if not isinstance(workspace, dict):
        raise MsrvCheckError("root Cargo.toml must define [workspace]")
    members = workspace.get("members")
    if not isinstance(members, list) or not members:
        raise MsrvCheckError("workspace.members must be a non-empty explicit array")
    normalized: list[PurePosixPath] = []
    for value in members:
        if not isinstance(value, str) or not value:
            raise MsrvCheckError("every workspace member must be a non-empty path")
        path = PurePosixPath(value)
        if path.is_absolute() or ".." in path.parts or any(char in value for char in "*?["):
            raise MsrvCheckError(f"workspace member must be an explicit local path: {value!r}")
        if path in normalized:
            raise MsrvCheckError(f"duplicate workspace member: {value!r}")
        normalized.append(path)
    return normalized


def check_manifests(root: Path = ROOT) -> list[str]:
    manifest = _load_toml(root / "Cargo.toml")
    workspace = manifest["workspace"]
    package_policy = workspace.get("package")
    if not isinstance(package_policy, dict) or package_policy.get("rust-version") != DECLARED_MSRV:
        raise MsrvCheckError(
            f"workspace.package.rust-version must remain exactly {DECLARED_MSRV!r}"
        )
    members = _member_paths(manifest)
    declared = {str(member) for member in members}
    discovered = {
        path.parent.relative_to(root).as_posix()
        for path in (root / "crates").glob("*/Cargo.toml")
        if path.is_file()
    }
    if declared != discovered:
        missing = sorted(discovered - declared)
        unknown = sorted(declared - discovered)
        raise MsrvCheckError(
            f"workspace member topology differs from crates/* manifests; omitted={missing}, unknown={unknown}"
        )

    names: list[str] = []
    for relative in members:
        path = root / relative / "Cargo.toml"
        member = _load_toml(path)
        package = member.get("package")
        if not isinstance(package, dict) or not isinstance(package.get("name"), str):
            raise MsrvCheckError(f"workspace member {relative} lacks package.name")
        rust_version = package.get("rust-version")
        if rust_version not in (None, {"workspace": True}):
            raise MsrvCheckError(
                f"{relative}/Cargo.toml must inherit the workspace MSRV or omit a package override"
            )
        features = member.get("features", {})
        if not isinstance(features, dict) or not all(isinstance(key, str) for key in features):
            raise MsrvCheckError(f"{relative}/Cargo.toml has an invalid [features] table")
        names.append(package["name"])
    if len(names) != len(set(names)):
        raise MsrvCheckError("workspace package names must be unique")
    return names


def _job_block(source: str, key: str) -> list[str]:
    lines = source.splitlines()
    starts = [index for index, line in enumerate(lines) if line == f"  {key}:"]
    if len(starts) != 1:
        raise MsrvCheckError(f"CI must define exactly one {key!r} job")
    start = starts[0]
    end = next(
        (index for index in range(start + 1, len(lines)) if re.fullmatch(r"  [A-Za-z0-9_-]+:", lines[index])),
        len(lines),
    )
    return lines[start:end]


def _scalar(block: list[str], key: str) -> str | None:
    prefix = f"    {key}:"
    values = [_strip_yaml_comment(line[len(prefix):]).strip() for line in block if line.startswith(prefix)]
    if len(values) > 1:
        raise MsrvCheckError(f"msrv job has ambiguous {key!r} fields")
    return values[0] if values else None


def _steps(block: list[str]) -> list[Step]:
    starts = [index for index, line in enumerate(block) if line.startswith("      - name:")]
    steps: list[Step] = []
    for position, start in enumerate(starts):
        end = starts[position + 1] if position + 1 < len(starts) else len(block)
        name = _strip_yaml_comment(block[start].split(":", 1)[1]).strip()
        fields: dict[str, str] = {}
        with_fields: dict[str, str] = {}
        in_with = False
        for line in block[start + 1:end]:
            if line == "        with:":
                in_with = True
                continue
            match = re.fullmatch(r"        ([A-Za-z_][A-Za-z0-9_-]*):\s*(.*?)\s*", line)
            if match:
                in_with = False
                fields[match.group(1)] = _strip_yaml_comment(match.group(2)).strip()
                continue
            match = re.fullmatch(r"          ([A-Za-z_][A-Za-z0-9_-]*):\s*(.*?)\s*", line)
            if in_with and match:
                with_fields[match.group(1)] = _strip_yaml_comment(match.group(2)).strip()
        steps.append(Step(name, fields, with_fields))
    return steps


def check_workflow_source(source: str) -> None:
    validate_unambiguous_yaml(source)
    required_triggers = (
        "on:\n  push:\n    branches: [master, main]\n  pull_request:",
    )
    if not all(fragment in source for fragment in required_triggers):
        raise MsrvCheckError("CI triggers must include every pull request and main/master push")
    block = _job_block(source, "msrv")
    if _scalar(block, "needs") != "static_docs":
        raise MsrvCheckError("msrv job must depend exactly on static_docs")
    if _scalar(block, "runs-on") != "ubuntu-latest":
        raise MsrvCheckError("msrv job must use the supported Linux host")
    if _scalar(block, "if") is not None:
        raise MsrvCheckError("msrv job may not have a conditional trigger")

    steps = _steps(block)
    if (
        '      FORGE_CI_CACHE_CONTEXT: "disabled-msrv-1.85.1"' not in block
        or any(
            "cache" in step.name.casefold()
            or "cache" in step.fields.get("uses", "").casefold()
            or "shared-key" in step.with_fields
            or "save-if" in step.with_fields
            for step in steps
        )
    ):
        raise MsrvCheckError("msrv job must disable and never restore/save Cargo caches")
    if [step.name for step in steps] != [
        "Checkout",
        "Install exact MSRV toolchain",
        "Verify MSRV lane contract",
        "Check complete workspace at MSRV",
        "Upload MSRV timing reports",
    ]:
        raise MsrvCheckError("msrv job step topology is not the reviewed exact sequence")
    checkout, install, contract, cargo, upload = steps
    if checkout.fields.get("uses") != CHECKOUT_ACTION:
        raise MsrvCheckError("msrv checkout action must use the reviewed immutable revision")
    if install.fields.get("uses") != TOOLCHAIN_ACTION or install.with_fields != {"toolchain": TOOLCHAIN}:
        raise MsrvCheckError(f"msrv setup must install exact Rust {TOOLCHAIN}")
    if contract.fields.get("run") != CHECK_COMMAND:
        raise MsrvCheckError("msrv contract test must use the reviewed bounded command")
    if cargo.fields.get("run") != CARGO_COMMAND:
        raise MsrvCheckError("msrv compilation must use the complete exact locked command")
    argv = shlex.split(cargo.fields["run"])
    required = {"--locked", "--workspace", "--all-targets", "--all-features"}
    if not required.issubset(argv) or argv.count(f"+{TOOLCHAIN}") != 1:
        raise MsrvCheckError("msrv Cargo command omits a locked workspace target/feature dimension")
    if upload.fields.get("if") != "always()" or upload.fields.get("uses") != UPLOAD_ACTION:
        raise MsrvCheckError("MSRV timing upload must run always at an immutable revision")
    if upload.with_fields != {
        "name": "ci-timing-msrv",
        "path": "target/ci-timing/msrv-*.json",
        "if-no-files-found": "warn",
        "retention-days": "14",
    }:
        raise MsrvCheckError("MSRV timing artifact retention/path topology drifted")


def check(workflow: Path = WORKFLOW, root: Path = ROOT) -> list[str]:
    packages = check_manifests(root)
    try:
        source = workflow.read_text(encoding="utf-8")
    except OSError as error:
        raise MsrvCheckError(f"cannot read {workflow}: {error}") from error
    check_workflow_source(source)
    return packages


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--workflow", type=Path)
    args = parser.parse_args(argv)
    workflow = args.workflow or args.root / ".github/workflows/ci.yml"
    try:
        packages = check(workflow, args.root)
    except MsrvCheckError as error:
        print(f"MSRV check failed: {error}", file=sys.stderr)
        return 1
    print(
        f"MSRV topology passed: Rust {TOOLCHAIN}, {len(packages)} workspace packages, "
        "all targets/features, locked Cargo, no shared cache"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
