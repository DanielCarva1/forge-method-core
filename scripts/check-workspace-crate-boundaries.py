#!/usr/bin/env python3
"""Fail-closed audit of the complete Cargo workspace architecture boundary.

The audit reads manifests and typed YAML only. A clean report is descriptive: it
cannot select a host, admit a candidate, sign, trust, install, activate, mutate
Forge state, transfer private broker keys, or grant core authority.
"""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
from typing import Any, NoReturn

try:
    import yaml
except ImportError:
    yaml = None

ROOT = Path(__file__).resolve().parents[1]
POLICY_PATH = ROOT / "contracts/policies/workspace-crate-boundary-v0.yaml"
RUST_CORE_PATH = ROOT / "contracts/architecture/rust-core.yaml"
BOUNDARIES_PATH = ROOT / "contracts/architecture/crate-boundaries.yaml"
EXPECTED_CRATE_COUNT = 23
REQUIRED_SELECTED_HOST = "none"
AUTHORITY_CRATES = {"forge-core-authority", "forge-core-kernel"}


def fail(message: str) -> NoReturn:
    raise SystemExit(f"workspace crate boundary audit failed: {message}")


if yaml is not None:

    class UniqueSafeLoader(yaml.SafeLoader):
        """Safe YAML loader that rejects duplicate mapping keys."""

    def construct_unique_mapping(
        loader: Any, node: Any, deep: bool = False
    ) -> dict[Any, Any]:
        value: dict[Any, Any] = {}
        for key_node, item_node in node.value:
            key = loader.construct_object(key_node, deep=deep)
            if not isinstance(key, str):
                fail(f"YAML has non-string mapping key at line {key_node.start_mark.line + 1}")
            if key in value:
                fail(f"YAML repeats key {key!r} at line {key_node.start_mark.line + 1}")
            value[key] = loader.construct_object(item_node, deep=deep)
        return value

    UniqueSafeLoader.add_constructor(
        yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG, construct_unique_mapping
    )
else:
    UniqueSafeLoader = None


def require_mapping(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        fail(f"{label} must be a string-keyed mapping")
    return value


def require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        fail(f"{label} must be a list")
    return value


def require_exact_keys(value: dict[str, Any], expected: set[str], label: str) -> None:
    actual = set(value)
    if actual != expected:
        fail(f"{label} has unexpected shape; missing={sorted(expected - actual)}, extra={sorted(actual - expected)}")


def load_yaml(path: Path, label: str) -> dict[str, Any]:
    if yaml is None or UniqueSafeLoader is None:
        fail("PyYAML is required to validate typed workspace architecture contracts")
    try:
        value = yaml.load(path.read_text(encoding="utf-8"), Loader=UniqueSafeLoader)
    except OSError as error:
        fail(f"cannot read {path.relative_to(ROOT)}: {error}")
    except yaml.YAMLError as error:
        fail(f"cannot parse {path.relative_to(ROOT)}: {error}")
    return require_mapping(value, label)


def require_nonempty_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        fail(f"{label} must be a non-empty string")
    return value


def checked_names(crates: list[Any], label: str, fields: set[str]) -> set[str]:
    names: set[str] = set()
    for index, value in enumerate(crates):
        crate = require_mapping(value, f"{label}[{index}]")
        require_exact_keys(crate, fields, f"{label}[{index}]")
        name = require_nonempty_string(crate["name"], f"{label}[{index}].name")
        if name in names:
            fail(f"{label} repeats crate {name!r}")
        names.add(name)
    if len(names) != EXPECTED_CRATE_COUNT:
        fail(f"{label} declares {len(names)} crates; obsolete partial/eight-crate claims are forbidden and exactly {EXPECTED_CRATE_COUNT} are required")
    return names


def load_policy() -> tuple[dict[str, Any], set[str], dict[str, set[str]], dict[str, str], set[tuple[str, str]]]:
    policy = load_yaml(POLICY_PATH, "workspace boundary policy")
    require_exact_keys(policy, {"schema_version", "policy", "status", "selected_host", "workspace_crate_boundary"}, "workspace boundary policy")
    if policy["schema_version"] != "0.1":
        fail("workspace boundary policy schema_version must be '0.1'")
    if policy["policy"] != "workspace_crate_boundary" or policy["status"] != "active":
        fail("workspace boundary policy identity must be active workspace_crate_boundary")
    if policy["selected_host"] != REQUIRED_SELECTED_HOST:
        fail("candidate workspace boundary policy must keep selected_host as 'none'")
    boundary = require_mapping(policy["workspace_crate_boundary"], "workspace_crate_boundary")
    require_exact_keys(boundary, {"crates", "reviewed_authority_edges"}, "workspace_crate_boundary")
    crates = require_list(boundary["crates"], "workspace_crate_boundary.crates")
    names = checked_names(crates, "workspace_crate_boundary.crates", {"name", "manifest_path", "depends_on"})
    dependencies_by_name: dict[str, set[str]] = {}
    manifests: dict[str, str] = {}
    for index, value in enumerate(crates):
        crate = require_mapping(value, f"declared crate {index}")
        name = crate["name"]
        manifest_path = require_nonempty_string(crate["manifest_path"], f"declared crate {name}.manifest_path")
        path = Path(manifest_path)
        if path.is_absolute() or ".." in path.parts or path.suffix != ".toml":
            fail(f"declared crate {name} manifest_path escapes the repository")
        depends_on = require_list(crate["depends_on"], f"declared crate {name}.depends_on")
        if not all(isinstance(dependency, str) and dependency for dependency in depends_on):
            fail(f"declared crate {name} has invalid depends_on values")
        dependency_set = set(depends_on)
        if len(dependency_set) != len(depends_on) or name in dependency_set or dependency_set - names:
            fail(f"declared crate {name} has invalid local dependencies")
        dependencies_by_name[name] = dependency_set
        manifests[name] = manifest_path
    reviewed: set[tuple[str, str]] = set()
    for index, value in enumerate(require_list(boundary["reviewed_authority_edges"], "reviewed_authority_edges")):
        edge = require_mapping(value, f"reviewed_authority_edges[{index}]")
        require_exact_keys(edge, {"from", "to"}, f"reviewed_authority_edges[{index}]")
        source = require_nonempty_string(edge["from"], f"reviewed_authority_edges[{index}].from")
        target = require_nonempty_string(edge["to"], f"reviewed_authority_edges[{index}].to")
        if source not in names or target not in names or (source, target) in reviewed:
            fail("reviewed_authority_edges must be unique edges between declared crates")
        reviewed.add((source, target))
    return policy, names, dependencies_by_name, manifests, reviewed


def load_architecture() -> tuple[dict[str, tuple[str, str, str]], set[str]]:
    document = load_yaml(RUST_CORE_PATH, "rust core architecture")
    require_exact_keys(document, {"schema_version", "architecture", "selected_host", "authority_model", "crates"}, "rust core architecture")
    if document["schema_version"] != "0.1" or document["architecture"] != "rust_core" or document["selected_host"] != REQUIRED_SELECTED_HOST:
        fail("rust core architecture must be schema 0.1, rust_core, and hostless")
    model = require_mapping(document["authority_model"], "rust core authority_model")
    require_exact_keys(model, {"host_client_authority", "core_authority", "candidate_documents_are_inert"}, "rust core authority_model")
    if model["candidate_documents_are_inert"] is not True:
        fail("rust core architecture must keep candidate documents inert")
    for key in ("host_client_authority", "core_authority"):
        require_nonempty_string(model[key], f"rust core authority_model.{key}")
    crates = require_list(document["crates"], "rust core crates")
    names = checked_names(crates, "rust core crates", {"name", "owns", "does_not_own", "authority_boundary"})
    result: dict[str, tuple[str, str, str]] = {}
    for crate in crates:
        name = crate["name"]
        owns = require_nonempty_string(crate["owns"], f"rust core crate {name}.owns")
        does_not_own = require_nonempty_string(crate["does_not_own"], f"rust core crate {name}.does_not_own")
        role = crate["authority_boundary"]
        if role not in {"core", "host_client", "compatibility"}:
            fail(f"rust core crate {name} has invalid authority_boundary")
        result[name] = (owns, does_not_own, role)
    return result, names


def load_boundaries() -> tuple[dict[str, tuple[str, str, str]], set[str], dict[str, set[str]], dict[str, str], set[tuple[str, str]]]:
    document = load_yaml(BOUNDARIES_PATH, "crate boundaries")
    require_exact_keys(document, {"schema_version", "contract", "selected_host", "authority_boundary", "crates"}, "crate boundaries")
    if document["schema_version"] != "0.1" or document["contract"] != "crate_boundaries" or document["selected_host"] != REQUIRED_SELECTED_HOST:
        fail("crate boundaries must be schema 0.1, crate_boundaries, and hostless")
    authority = require_mapping(document["authority_boundary"], "crate boundaries authority_boundary")
    require_exact_keys(authority, {"host_client_crates", "compatibility_client_crates", "core_authority_crates", "candidate_documents_are_inert"}, "crate boundaries authority_boundary")
    if authority["candidate_documents_are_inert"] is not True:
        fail("crate boundaries must keep candidate documents inert")
    if set(require_list(authority["core_authority_crates"], "core_authority_crates")) != AUTHORITY_CRATES:
        fail("crate boundaries must enumerate the reviewed core authority crates exactly")
    crates = require_list(document["crates"], "crate boundaries crates")
    names = checked_names(crates, "crate boundaries crates", {"name", "manifest_path", "depends_on", "owns", "does_not_own", "authority_boundary", "reviewed_authority_dependencies"})
    detail: dict[str, tuple[str, str, str]] = {}
    deps_by_name: dict[str, set[str]] = {}
    manifests: dict[str, str] = {}
    reviewed: set[tuple[str, str]] = set()
    for crate in crates:
        name = crate["name"]
        detail[name] = (require_nonempty_string(crate["owns"], f"crate boundary {name}.owns"), require_nonempty_string(crate["does_not_own"], f"crate boundary {name}.does_not_own"), crate["authority_boundary"])
        manifests[name] = require_nonempty_string(crate["manifest_path"], f"crate boundary {name}.manifest_path")
        deps = require_list(crate["depends_on"], f"crate boundary {name}.depends_on")
        reviewed_deps = require_list(crate["reviewed_authority_dependencies"], f"crate boundary {name}.reviewed_authority_dependencies")
        if any(not isinstance(dep, str) or dep not in names for dep in deps + reviewed_deps):
            fail(f"crate boundary {name} has unknown dependency")
        if len(set(deps)) != len(deps) or len(set(reviewed_deps)) != len(reviewed_deps):
            fail(f"crate boundary {name} repeats a dependency")
        deps_by_name[name] = set(deps)
        reviewed.update((name, dependency) for dependency in reviewed_deps)
    return detail, names, deps_by_name, manifests, reviewed


def cargo_metadata() -> dict[str, Any]:
    completed = subprocess.run(["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"], cwd=ROOT, text=True, capture_output=True, check=False)
    if completed.returncode != 0:
        fail(f"read-only cargo metadata failed: {completed.stderr.strip()}")
    try:
        return require_mapping(json.loads(completed.stdout), "cargo metadata")
    except json.JSONDecodeError as error:
        fail(f"cargo metadata emitted invalid JSON: {error}")


def discovered_workspace(metadata: dict[str, Any]) -> tuple[set[str], dict[str, set[str]], dict[str, str]]:
    packages = require_list(metadata.get("packages"), "cargo metadata.packages")
    member_ids = set(require_list(metadata.get("workspace_members"), "cargo metadata.workspace_members"))
    members = [require_mapping(item, "cargo metadata package") for item in packages if require_mapping(item, "cargo metadata package").get("id") in member_ids]
    names: set[str] = set()
    manifests: dict[str, str] = {}
    for package in members:
        name = require_nonempty_string(package.get("name"), "workspace package name")
        manifest = Path(require_nonempty_string(package.get("manifest_path"), f"manifest for {name}"))
        if name in names or not manifest.is_file() or manifest.is_symlink():
            fail(f"invalid or duplicate workspace manifest for {name}")
        try:
            manifests[name] = manifest.relative_to(ROOT).as_posix()
        except ValueError:
            fail(f"workspace manifest escapes repository root: {manifest}")
        names.add(name)
    graph: dict[str, set[str]] = {}
    for package in members:
        graph[package["name"]] = {dependency["name"] for dependency in require_list(package.get("dependencies"), f"dependencies for {package['name']}") if require_mapping(dependency, "cargo metadata dependency").get("name") in names}
    return names, graph, manifests


def compare(label: str, declared: Any, discovered: Any) -> None:
    if declared != discovered:
        if isinstance(declared, set) and isinstance(discovered, set):
            fail(f"{label} mismatch; missing={sorted(discovered - declared)}, extra={sorted(declared - discovered)}")
        fail(f"{label} mismatch; declared={declared!r}, discovered={discovered!r}")


def main() -> int:
    _, policy_names, policy_graph, policy_manifests, policy_reviewed = load_policy()
    architecture, architecture_names = load_architecture()
    boundaries, boundary_names, boundary_graph, boundary_manifests, boundary_reviewed = load_boundaries()
    compare("architecture crate declarations", architecture_names, policy_names)
    compare("boundary crate declarations", boundary_names, policy_names)
    compare("architecture ownership declarations", architecture, boundaries)
    compare("boundary manifest declarations", boundary_manifests, policy_manifests)
    compare("boundary dependency graph", boundary_graph, policy_graph)
    compare("reviewed authority edges", boundary_reviewed, policy_reviewed)
    expected_reviewed = {(source, target) for source, dependencies in policy_graph.items() for target in dependencies if target in AUTHORITY_CRATES}
    compare("unreviewed authority edges", policy_reviewed, expected_reviewed)
    metadata = cargo_metadata()
    discovered_names, discovered_graph, discovered_manifests = discovered_workspace(metadata)
    if len(discovered_names) != EXPECTED_CRATE_COUNT:
        fail(f"workspace must contain exactly {EXPECTED_CRATE_COUNT} crates, found {len(discovered_names)}")
    compare("crate declarations", policy_names, discovered_names)
    compare("manifest declarations", policy_manifests, discovered_manifests)
    compare("dependency graph", policy_graph, discovered_graph)
    print("Workspace crate boundary audit: clean")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, TypeError) as error:
        fail(str(error))
