#!/usr/bin/env python3
"""Fail closed when the workspace MSRV lane or manifest topology drifts."""

from __future__ import annotations

import argparse
import shlex
import sys
import tomllib
from pathlib import Path, PurePosixPath
from typing import Any

try:
    import yaml
except ImportError:  # Fail closed rather than interpreting security topology as text.
    yaml = None


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


if yaml is not None:
    class UniqueBaseLoader(yaml.BaseLoader):
        """Load scalar text without YAML 1.1 coercion and reject duplicate keys."""


    def _construct_unique_mapping(loader: Any, node: Any, deep: bool = False):
        mapping: dict[str, Any] = {}
        for key_node, value_node in node.value:
            if not isinstance(key_node, yaml.nodes.ScalarNode):
                raise MsrvCheckError("CI workflow mapping keys must be scalar strings")
            key = loader.construct_object(key_node, deep=deep)
            if key in mapping:
                line = key_node.start_mark.line + 1
                raise MsrvCheckError(
                    f"CI workflow:{line}: duplicate YAML key {key!r}"
                )
            mapping[key] = loader.construct_object(value_node, deep=deep)
        return mapping


    UniqueBaseLoader.add_constructor(
        yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, _construct_unique_mapping
    )
else:
    UniqueBaseLoader = None


def _reject_unsupported_yaml(value: Any, path: str = "workflow") -> None:
    if isinstance(value, dict):
        for key, child in value.items():
            if not isinstance(key, str):
                raise MsrvCheckError(f"{path}: YAML mapping keys must be strings")
            if key == "<<":
                raise MsrvCheckError(f"{path}: YAML merges are forbidden")
            _reject_unsupported_yaml(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _reject_unsupported_yaml(child, f"{path}[{index}]")
    elif not isinstance(value, str):
        raise MsrvCheckError(
            f"{path}: unsupported YAML value type {type(value).__name__}"
        )


def parse_workflow(source: str) -> dict[str, Any]:
    """Parse one alias-free YAML document with duplicate-safe string scalars."""
    if yaml is None or UniqueBaseLoader is None:
        raise MsrvCheckError("structured CI validation requires PyYAML")
    try:
        for token in yaml.scan(source):
            if isinstance(token, (yaml.tokens.AnchorToken, yaml.tokens.AliasToken)):
                raise MsrvCheckError("CI workflow YAML anchors and aliases are forbidden")
            if isinstance(token, yaml.tokens.TagToken):
                raise MsrvCheckError("CI workflow explicit YAML tags are forbidden")
        document = yaml.load(source, Loader=UniqueBaseLoader)
    except MsrvCheckError:
        raise
    except yaml.YAMLError as error:
        raise MsrvCheckError(f"cannot parse CI workflow YAML: {error}") from error
    if not isinstance(document, dict):
        raise MsrvCheckError("CI workflow must be one YAML mapping document")
    _reject_unsupported_yaml(document)
    return document


def validate_unambiguous_yaml(source: str) -> None:
    """Compatibility entry point for callers that only require strict parsing."""
    parse_workflow(source)


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


def _exact_mapping(
    value: Any, label: str, expected_keys: set[str]
) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise MsrvCheckError(f"{label} must be a YAML mapping")
    actual = set(value)
    if actual != expected_keys:
        missing = sorted(expected_keys - actual)
        unknown = sorted(actual - expected_keys)
        raise MsrvCheckError(
            f"{label} keys must be exactly the reviewed allowlist; "
            f"missing={missing}, unknown={unknown}"
        )
    return value


def _exact_value(actual: Any, expected: Any, label: str) -> None:
    if actual != expected:
        raise MsrvCheckError(f"{label} must remain exactly {expected!r}")


def check_workflow_source(source: str) -> None:
    document = parse_workflow(source)
    root = _exact_mapping(
        document, "CI workflow", {"name", "on", "concurrency", "env", "jobs"}
    )
    _exact_value(root["name"], "CI", "CI workflow name")
    _exact_value(
        root["on"],
        {"push": {"branches": ["master", "main"]}, "pull_request": ""},
        "CI workflow triggers",
    )
    _exact_value(
        root["concurrency"],
        {
            "group": "ci-${{ github.workflow }}-${{ github.ref }}",
            "cancel-in-progress": "true",
        },
        "CI workflow concurrency",
    )
    _exact_value(
        root["env"],
        {
            "CARGO_INCREMENTAL": "0",
            "CARGO_PROFILE_DEV_DEBUG": "0",
            "CARGO_PROFILE_TEST_DEBUG": "0",
        },
        "CI workflow environment",
    )

    jobs = root["jobs"]
    if not isinstance(jobs, dict):
        raise MsrvCheckError("CI jobs must be a YAML mapping")
    if "static_docs" not in jobs or not isinstance(jobs["static_docs"], dict):
        raise MsrvCheckError("CI must define the static_docs dependency job")
    if "msrv" not in jobs:
        raise MsrvCheckError("CI must define exactly one 'msrv' job")
    job = _exact_mapping(
        jobs["msrv"],
        "msrv job",
        {"name", "needs", "runs-on", "timeout-minutes", "env", "steps"},
    )
    _exact_value(
        job["name"], "Rust 1.85 minimum supported version", "msrv job name"
    )
    _exact_value(job["needs"], "static_docs", "msrv job dependency")
    _exact_value(job["runs-on"], "ubuntu-latest", "msrv job runner")
    _exact_value(job["timeout-minutes"], "35", "msrv job timeout")
    _exact_value(
        job["env"],
        {"FORGE_CI_CACHE_CONTEXT": "disabled-msrv-1.85.1"},
        "msrv job environment",
    )

    steps = job["steps"]
    expected_steps: list[tuple[str, dict[str, Any]]] = [
        ("Checkout", {"name": "Checkout", "uses": CHECKOUT_ACTION}),
        (
            "Install exact MSRV toolchain",
            {
                "name": "Install exact MSRV toolchain",
                "uses": TOOLCHAIN_ACTION,
                "with": {"toolchain": TOOLCHAIN},
            },
        ),
        (
            "Verify MSRV lane contract",
            {
                "name": "Verify MSRV lane contract",
                "timeout-minutes": "6",
                "run": CHECK_COMMAND,
            },
        ),
        (
            "Check complete workspace at MSRV",
            {
                "name": "Check complete workspace at MSRV",
                "timeout-minutes": "31",
                "run": CARGO_COMMAND,
            },
        ),
        (
            "Upload MSRV timing reports",
            {
                "name": "Upload MSRV timing reports",
                "if": "always()",
                "uses": UPLOAD_ACTION,
                "with": {
                    "name": "ci-timing-msrv",
                    "path": "target/ci-timing/msrv-*.json",
                    "if-no-files-found": "warn",
                    "retention-days": "14",
                },
            },
        ),
    ]
    if not isinstance(steps, list):
        raise MsrvCheckError("msrv steps must be an exact ordered YAML list")
    names = [step.get("name") if isinstance(step, dict) else None for step in steps]
    expected_names = [name for name, _ in expected_steps]
    if names != expected_names:
        raise MsrvCheckError(
            "msrv job step topology must be the reviewed exact named sequence; "
            f"expected={expected_names}, actual={names}"
        )
    for step, (name, expected) in zip(steps, expected_steps, strict=True):
        assert isinstance(step, dict)
        _exact_mapping(step, f"msrv step {name!r}", set(expected))
        if step != expected:
            raise MsrvCheckError(
                f"msrv step {name!r} fields must match the reviewed exact values"
            )

    cargo = steps[3]
    argv = shlex.split(cargo["run"])
    required = {"--locked", "--workspace", "--all-targets", "--all-features"}
    if not required.issubset(argv) or argv.count(f"+{TOOLCHAIN}") != 1:
        raise MsrvCheckError(
            "msrv Cargo command omits a locked workspace target/feature dimension"
        )


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
