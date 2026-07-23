#!/usr/bin/env python3
"""Validate release authority with the immutable protected-base checker."""

from __future__ import annotations

import argparse
import importlib.util
import os
import sys
from pathlib import Path
from types import ModuleType
from typing import Any

try:
    import yaml
except ImportError:  # Fail closed rather than interpreting security topology as text.
    yaml = None


ROOT = Path(__file__).resolve().parents[1]
WORKFLOW = ROOT / ".github/workflows/release.yml"
POLICY_WORKFLOW = ROOT / ".github/workflows/release-policy.yml"
CHECKOUT_ACTION = "actions/checkout@34e114876b0b11c390a56381ad16ebd13914f8d5"
PYYAML_VERSION = "6.0.3"
PYYAML_INSTALL_COMMAND = (
    "python -m pip install --disable-pip-version-check --no-deps "
    f"PyYAML=={PYYAML_VERSION}"
)
POLICY_COMMAND = (
    "python -I trusted/scripts/check-release-policy.py --root candidate "
    "--workflow candidate/.github/workflows/release.yml "
    "--policy-workflow candidate/.github/workflows/release-policy.yml"
)


class ReleasePolicyError(RuntimeError):
    """The independent release admission boundary has drifted or is incomplete."""


if yaml is not None:

    class UniqueBaseLoader(yaml.BaseLoader):
        """Load string scalars while rejecting duplicate mapping keys."""

    def _construct_unique_mapping(loader: Any, node: Any, deep: bool = False):
        mapping: dict[str, Any] = {}
        for key_node, value_node in node.value:
            if not isinstance(key_node, yaml.nodes.ScalarNode):
                raise ReleasePolicyError(
                    "release policy workflow mapping keys must be scalar strings"
                )
            key = loader.construct_object(key_node, deep=deep)
            if key in mapping:
                line = key_node.start_mark.line + 1
                raise ReleasePolicyError(
                    f"release policy workflow:{line}: duplicate YAML key {key!r}"
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
                raise ReleasePolicyError(f"{path}: YAML mapping keys must be strings")
            if key == "<<":
                raise ReleasePolicyError(f"{path}: YAML merges are forbidden")
            _reject_unsupported_yaml(child, f"{path}.{key}")
    elif isinstance(value, list):
        for index, child in enumerate(value):
            _reject_unsupported_yaml(child, f"{path}[{index}]")
    elif not isinstance(value, str):
        raise ReleasePolicyError(
            f"{path}: unsupported YAML value type {type(value).__name__}"
        )


def parse_workflow(source: str) -> dict[str, Any]:
    """Parse one alias-free YAML document with duplicate-safe string scalars."""
    if yaml is None or UniqueBaseLoader is None:
        raise ReleasePolicyError("structured release policy validation requires PyYAML")
    try:
        for token in yaml.scan(source):
            if isinstance(token, (yaml.tokens.AnchorToken, yaml.tokens.AliasToken)):
                raise ReleasePolicyError(
                    "release policy YAML anchors and aliases are forbidden"
                )
            if isinstance(token, yaml.tokens.TagToken):
                raise ReleasePolicyError(
                    "release policy YAML explicit tags are forbidden"
                )
        document = yaml.load(source, Loader=UniqueBaseLoader)
    except ReleasePolicyError:
        raise
    except yaml.YAMLError as error:
        raise ReleasePolicyError(
            f"cannot parse release policy workflow YAML: {error}"
        ) from error
    if not isinstance(document, dict):
        raise ReleasePolicyError("release policy workflow must be one YAML mapping")
    _reject_unsupported_yaml(document)
    return document


def _exact_mapping(value: Any, label: str, expected_keys: set[str]) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ReleasePolicyError(f"{label} must be a YAML mapping")
    actual = set(value)
    if actual != expected_keys:
        missing = sorted(expected_keys - actual)
        unknown = sorted(actual - expected_keys)
        raise ReleasePolicyError(
            f"{label} keys must be exactly the reviewed allowlist; "
            f"missing={missing}, unknown={unknown}"
        )
    return value


def _exact_value(actual: Any, expected: Any, label: str) -> None:
    if actual != expected:
        raise ReleasePolicyError(f"{label} must remain exactly {expected!r}")


def check_policy_workflow_source(source: str) -> None:
    """Require the exact read-only protected-base release policy topology."""
    document = parse_workflow(source)
    root = _exact_mapping(
        document, "release policy workflow", {"name", "on", "permissions", "jobs"}
    )
    _exact_value(root["name"], "Release Policy", "release policy workflow name")
    _exact_value(
        root["on"],
        {
            "pull_request_target": {
                "branches": ["main"],
                "types": ["opened", "reopened", "synchronize", "ready_for_review"],
            }
        },
        "release policy workflow triggers",
    )
    _exact_value(
        root["permissions"],
        {"contents": "read"},
        "release policy workflow permissions",
    )
    jobs = _exact_mapping(root["jobs"], "release policy jobs", {"enforce"})
    job = _exact_mapping(
        jobs["enforce"],
        "release policy enforce job",
        {"name", "runs-on", "timeout-minutes", "steps"},
    )
    _exact_value(
        job["name"], "Enforce trusted release policy", "release policy job name"
    )
    _exact_value(job["runs-on"], "ubuntu-latest", "release policy job runner")
    _exact_value(job["timeout-minutes"], "10", "release policy job timeout")
    expected_steps = [
        {
            "name": "Checkout trusted base policy",
            "uses": CHECKOUT_ACTION,
            "with": {
                "repository": "${{ github.repository }}",
                "ref": "${{ github.event.pull_request.base.sha }}",
                "path": "trusted",
                "persist-credentials": "false",
                "fetch-depth": "1",
            },
        },
        {
            "name": "Checkout candidate as untrusted data",
            "uses": CHECKOUT_ACTION,
            "with": {
                "repository": "${{ github.event.pull_request.head.repo.full_name }}",
                "ref": "${{ github.event.pull_request.head.sha }}",
                "path": "candidate",
                "persist-credentials": "false",
                "fetch-depth": "1",
            },
        },
        {
            "name": "Provision exact YAML parser",
            "run": PYYAML_INSTALL_COMMAND,
        },
        {
            "name": "Validate candidate with trusted base checker",
            "run": POLICY_COMMAND,
        },
    ]
    steps = job["steps"]
    if not isinstance(steps, list) or steps != expected_steps:
        raise ReleasePolicyError(
            "release policy steps must be the reviewed exact ordered sequence"
        )
    for step, expected in zip(steps, expected_steps, strict=True):
        _exact_mapping(
            step,
            f"release policy step {expected['name']!r}",
            set(expected),
        )


def _require_regular_data_file(path: Path, root: Path, label: str) -> None:
    try:
        relative = path.relative_to(root)
    except ValueError as error:
        raise ReleasePolicyError(
            f"{label} must remain inside the candidate root"
        ) from error
    current = root
    for part in relative.parts:
        current /= part
        if current.is_symlink():
            raise ReleasePolicyError(f"{label} must not be a symbolic link")
    if not path.is_file():
        raise ReleasePolicyError(f"required {label} is missing or not a regular file")


def _load_release_lock_checker() -> ModuleType:
    path = Path(__file__).resolve().with_name("check-release-locking.py")
    spec = importlib.util.spec_from_file_location(
        "forge_trusted_release_lock_checker", path
    )
    if spec is None or spec.loader is None:
        raise ReleasePolicyError(f"cannot load trusted release lock checker {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _require_trusted_checker_parity(root: Path, relative: str, label: str) -> None:
    candidate = root / relative
    trusted = Path(__file__).resolve().parents[1] / relative
    _require_regular_data_file(candidate, root, label)
    try:
        candidate_bytes = candidate.read_bytes()
        trusted_bytes = trusted.read_bytes()
    except OSError as error:
        raise ReleasePolicyError(f"cannot compare {label} with protected base: {error}") from error
    if candidate_bytes != trusted_bytes:
        raise ReleasePolicyError(f"{label} differs from the protected-base trust root")


def check(
    workflow: Path = WORKFLOW,
    root: Path = ROOT,
    policy_workflow: Path | None = None,
) -> list[Any]:
    """Validate candidate data with only protected-base executable authority."""
    root = Path(os.path.abspath(root))
    if root.is_symlink() or not root.is_dir():
        raise ReleasePolicyError(
            "candidate root must be a direct, existing directory rather than an alias"
        )
    canonical_workflow = root / ".github/workflows/release.yml"
    canonical_policy = root / ".github/workflows/release-policy.yml"
    workflow = Path(os.path.abspath(workflow))
    policy_workflow = Path(os.path.abspath(policy_workflow or canonical_policy))
    if workflow != canonical_workflow:
        raise ReleasePolicyError(
            f"release policy is bound to canonical workflow {canonical_workflow}, got {workflow}"
        )
    if policy_workflow != canonical_policy:
        raise ReleasePolicyError(
            f"release policy is bound to canonical policy workflow {canonical_policy}, "
            f"got {policy_workflow}"
        )
    _require_regular_data_file(workflow, root, "release workflow")
    _require_regular_data_file(policy_workflow, root, "release policy workflow")
    try:
        policy_source = policy_workflow.read_text(encoding="utf-8")
    except (OSError, UnicodeError) as error:
        raise ReleasePolicyError(
            f"cannot read candidate release policy data: {error}"
        ) from error
    check_policy_workflow_source(policy_source)

    lock_checker = _load_release_lock_checker()
    try:
        invocations = lock_checker.check(workflow, repo_root=root)
    except lock_checker.ReleaseLockError as error:
        raise ReleasePolicyError(
            f"trusted release executable-graph validation failed: {error}"
        ) from error
    _require_trusted_checker_parity(
        root, "scripts/check-release-policy.py", "candidate release policy checker"
    )
    _require_trusted_checker_parity(
        root, "scripts/check-release-locking.py", "candidate release lock checker"
    )
    return invocations


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--workflow", type=Path)
    parser.add_argument("--policy-workflow", type=Path)
    args = parser.parse_args(argv)
    workflow = args.workflow or args.root / ".github/workflows/release.yml"
    policy_workflow = (
        args.policy_workflow
        or args.root / ".github/workflows/release-policy.yml"
    )
    try:
        invocations = check(workflow, args.root, policy_workflow)
    except ReleasePolicyError as error:
        print(f"Release policy check failed: {error}", file=sys.stderr)
        return 1
    print(
        "Release policy passed: protected-base checker, exact read-only PR policy, "
        f"{len(invocations)} governed locked Cargo invocation(s)"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
