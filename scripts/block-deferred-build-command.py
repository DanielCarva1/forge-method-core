#!/usr/bin/env python3
"""Allow trusted compile-only feedback through a strict executable allowlist."""

from __future__ import annotations

import json
import os
import re
import shlex
import sys
import tomllib
from pathlib import Path
from typing import Any

try:
    import yaml
except ImportError:
    yaml = None


ROOT = Path(__file__).resolve().parents[1]
CAMPAIGN = ROOT / "contracts/plan/product-gap-closure-campaign-v1.yaml"
INVENTORY = ROOT / "contracts/plan/product-gap-closure-story-inventory-v1.yaml"
PLAN = ROOT / "contracts/plan/product-gap-closure-plan.yaml"

SOURCE_ITEM_IDS = {
    "C1.1", "C1.2", "C1.3", "C1.4",
    "C2.1", "C2.2", "C2.3", "C2.4",
    "C3.1",
    "C4.1", "C4.2",
    "C5.1", "C5.2", "C5.3",
    "C6.1", "C6.2",
    "C7.1", "C7.2",
}
ITEM_IDS = SOURCE_ITEM_IDS | {"C3.2", "C3.3", "C3.4"}
SOURCE_ITEM_SCHEDULE = "pre_stabilization_implementation"
SOURCE_STORY_SCHEDULE = "pre_stabilization_source"
SOURCE_CLOSED_STATUSES = {"implemented_pending_evidence", "completed"}
SOURCE_CHECKPOINT_STATES = {"source_complete", "implemented_pending_evidence"}
CAMPAIGN_STATUS_VALUES = {
    "planned",
    "in_progress",
    "blocked_external",
    "implemented_pending_evidence",
    "completed",
}
INVENTORY_STATUS_VALUES = {"planned", "in_progress", "blocked", "source_complete"}
CAMPAIGN_CHECKPOINT_KINDS = {"campaign-item-checkpoint", "c2.2-continuity-projection"}
EXPECTED_SOURCE_LEAVES = {"C1.4", "C2.4", "C3.1", "C4.2", "C5.3", "C7.2"}
EXPECTED_FORENSIC_IDS = {f"v2-{index:03d}" for index in range(1, 27)}
EXPECTED_EVIDENCE_RECORDS = {
    "C3.2.work.1": ("C3.2", "stabilization_only"),
    "P7G.1": ("C3.2", "stabilization_only"),
    "P7E.1": ("C3.3", "publication_only"),
    "P7F.1": ("C3.4", "field_or_independent_evidence_only"),
    "P7H.1": ("C3.4", "field_or_independent_evidence_only"),
}
EXPECTED_ADJUDICATED_DISPOSITIONS = {
    "GAP-002.reinitialize-as-new": "assign",
    "GAP-010.codex-conformance": "assign",
    "FRUST-001": "supporting_predecessor",
    "FRUST-002": "assign",
    "FRUST-010": "assign",
    "FRUST-020": "assign",
    "FRUST-021": "supporting_predecessor",
    "FRUST-022": "supporting_predecessor",
    "FRUST-030": "assign",
    "FRUST-031": "assign",
    "FRUST-040": "assign",
    "FRUST-041": "supporting_predecessor",
    "FRUST-050": "supporting_predecessor",
    "FRUST-051": "assign",
    "FRUST-052": "assign",
}

PLAN_PER_PART = [
    "Use compiler errors as the implementation work queue: every coherent source batch must pass a compile-only check through /usr/bin/python3 -I /mnt/d/forge-method-core/scripts/hermetic-compile-feedback.py with --locked or --frozen and an explicit -p/--package scope before work moves on. The launcher replaces inherited executable, native-tool, config, proxy, shell, Python, Git, and Rust injection variables, verifies a root-owned non-symlink Cargo/rustc/rustdoc chain, and exposes only /usr/bin for native child discovery. Cargo check still executes dependency or workspace build scripts and proc macros as necessary compile-time behavior, but it must not execute project tests or binaries.",
    "Run /usr/bin/python3 -I /mnt/d/forge-method-core/scripts/hermetic-compile-feedback.py check --locked --workspace --all-targets periodically across accumulated source batches; the same launcher with metadata --locked --no-deps --format-version 1 may inspect the workspace without executing project tests or binaries. Non-hermetic direct Cargo and unknown wrappers fail closed.",
    "Write package, contract, adversarial, fixture, and failure-injection test source with its owning story, but defer runtime tests, full suites, E2E, stress, fuzz, bench, project linked or release builds, MSRV/platform matrices, archives, hosted CI, publication, independent review, and field evidence.",
]
PLAN_CLOSURE = [
    "Close each source phase only after static authority, dependency, checkpoint, and source-inventory review plus successful compile-only compiler-feedback checks for every coherent source batch; compiler feedback is implementation hygiene, not stabilization evidence.",
    "Reserve runtime and cumulative test evidence, failure injection, project linked and release builds, native-platform and MSRV matrices, release archives, and hosted timing for C3.2; publication for C3.3; and real-host plus independent-review evidence for C3.4.",
]
INVENTORY_COMPILER_INVARIANTS = {
    "Every coherent pre_stabilization_source batch must pass a compile-only locked or frozen package-scoped check through /usr/bin/python3 -I /mnt/d/forge-method-core/scripts/hermetic-compile-feedback.py before work moves on, with periodic compile-only locked workspace checks through the same closed-environment launcher; cargo check may execute build scripts and proc macros as compile-time behavior but must not execute project tests or binaries, and non-hermetic direct Cargo fails closed.",
    "C3.2 is stabilization only; it may execute deferred runtime, full-suite, failure-injection, project-linked-build, platform, archive, and hosted gates but may not own missing source implementation.",
    "A source_complete value records source state and does not by itself claim compiler-feedback, runtime, full-suite, hosted, publication, independent-review, or field evidence passed.",
}
COMPILER_FEEDBACK_POLICY = {
    "mode": "compiler_errors_as_work_queue",
    "active_before_item": "C3.2",
    "hermetic_launcher_only": True,
    "canonical_launcher": "/usr/bin/python3 -I /mnt/d/forge-method-core/scripts/hermetic-compile-feedback.py",
    "trusted_cargo_path": "/opt/forge-method/rust-1.85.1/bin/cargo",
    "trusted_rustc_path": "/opt/forge-method/rust-1.85.1/bin/rustc",
    "trusted_rustdoc_path": "/opt/forge-method/rust-1.85.1/bin/rustdoc",
    "trusted_path": "/usr/bin",
    "trusted_executable_invariants": "Cargo, rustc, rustdoc, every executable path component, and every PATH directory are root-owned non-symlinks, are not group/world writable, and are not writable by the invoking uid.",
    "fixed_environment": {
        "HOME": "/nonexistent",
        "CARGO_HOME": "/home/user/.cargo",
        "PATH": "/usr/bin",
        "RUSTC": "/opt/forge-method/rust-1.85.1/bin/rustc",
        "RUSTDOC": "/opt/forge-method/rust-1.85.1/bin/rustdoc",
        "LANG": "C.UTF-8",
        "LC_ALL": "C.UTF-8",
    },
    "execution_boundary": "The canonical launcher rejects any mutable or symlinked Cargo/rustc/rustdoc chain, replaces the inherited environment, and exposes only root-owned /usr/bin for native child discovery. Compile-only cargo check still executes dependency or workspace build scripts and proc macros as necessary compile-time behavior, including building proc-macro artifacts; it must not execute project tests or binaries.",
    "requires_structurally_valid_policy": True,
    "coherent_batch_requirement": "Every coherent source batch passes a compile-only locked or frozen package-scoped check through the canonical hermetic launcher before work moves on.",
    "periodic_workspace_requirement": "Accumulated source batches periodically pass /usr/bin/python3 -I /mnt/d/forge-method-core/scripts/hermetic-compile-feedback.py check --locked --workspace --all-targets.",
    "allowed_command_shapes": [
        "/usr/bin/python3 -I /mnt/d/forge-method-core/scripts/hermetic-compile-feedback.py check (--locked|--frozen) (-p|--package) <workspace-package> [reviewed compile-only selectors]",
        "/usr/bin/python3 -I /mnt/d/forge-method-core/scripts/hermetic-compile-feedback.py check (--locked|--frozen) --workspace [reviewed compile-only selectors]",
        "/usr/bin/python3 -I /mnt/d/forge-method-core/scripts/hermetic-compile-feedback.py metadata (--locked|--frozen) --no-deps --format-version 1",
    ],
    "allowed_check_flags": [
        "--all-targets",
        "--all-features",
        "--features <feature-list>",
        "--no-default-features",
        "--lib",
        "--bins",
        "--examples",
        "--tests",
        "--benches",
        "--bin <name>",
        "--example <name>",
        "--test <name>",
        "--bench <name>",
    ],
    "forbidden_modifiers": [
        "+toolchain",
        "--release or --profile",
        "--target or --target-dir",
        "--config or -Z",
        "inherited executable, native tool-discovery, config, proxy, shell, Python, Git, or Rust wrapper injection",
        "non-hermetic direct Cargo, unknown launchers, shell control operators, pipes, redirects, or substitutions",
    ],
}
DEFERRED_HEAVY_POLICY = {
    "active": True,
    "authority_item": "C3.2",
    "opens_after_typed_source_closure": True,
    "scope": "Runtime tests, project linked or release builds, matrices, archives, Cargo-backed project execution, and hosted stabilization commands open only after every typed C3.2 precondition; compile-time build-script and proc-macro execution inherent in admitted cargo check remains allowed before C3.2.",
    "heavy_commands_forbidden_before_c3_2": [
        "cargo test, including --no-run and every filter or target variant",
        "cargo run, build, install, bench, nextest, fuzz, clippy, doc, package, rustc, fix, clean, update, fetch, vendor, tree, generate-lockfile, aliases, plugins, and unknown subcommands",
        "direct rustc, rustup, rustfmt, rustdoc, clippy-driver, nextest, cross, and cargo-* tools",
        "toolchain overrides, release or custom profiles, target or target-dir selection, Cargo config, unstable flags, and environment or runner injection",
        "runtime tests, full suites, E2E, stress, failure injection, benchmarks, fuzzing, and doctests",
        "project linked builds, release builds, native-platform or MSRV matrices, archives, SBOMs, and install smoke",
        "exec, builtin, command, env, time, timeout, nice, nohup, script, stdbuf, ionice, corepack, xargs, find -exec, make, just, task, mise, direnv, nix/devenv, shell or language launchers, generic child launchers, aliases, functions, path shims, and unreviewed local scripts",
        "Cargo-backed generators or checkers that execute project binaries; admitted cargo check may execute build scripts and proc macros only as inherent compile-time behavior",
        "GitHub Actions, gh workflow or run execution, act, and CI triggers",
    ],
    "publication_commands_forbidden_before_c3_3": [
        "cargo publish, login, owner, and yank",
        "git tag",
        "git push",
        "gh release",
        "release publication or public-asset mutation",
    ],
    "field_commands_forbidden_before_c3_4": [
        "real-host or production-host journeys",
        "field evidence collection",
        "independent semantic or actor-separation review",
    ],
    "c3_2_does_not_lift": [
        "publication commands",
        "field commands",
        "independent-review commands",
    ],
}

TRUSTED_COMPILE_CARGO = "/opt/forge-method/rust-1.85.1/bin/cargo"
TRUSTED_CARGO = "/home/user/.cargo/bin/cargo"
TRUSTED_PYTHON = "/usr/bin/python3"
HERMETIC_COMPILE_LAUNCHER = str(ROOT / "scripts/hermetic-compile-feedback.py")
TRUSTED_GIT = "/usr/bin/git"
TRUSTED_RG = "/usr/bin/rg"
TRUSTED_GH = "/usr/bin/gh"
TRUSTED_ACT = "/usr/bin/act"
TRUSTED_NPM = "/usr/bin/npm"
TRUSTED_PNPM = "/usr/bin/pnpm"
TRUSTED_TWINE = "/usr/bin/twine"
TRUSTED_RUSTUP = "/home/user/.cargo/bin/rustup"
TRUSTED_HEAVY_EXECUTABLES = {
    "/home/user/.cargo/bin/rustc",
    "/home/user/.cargo/bin/rustfmt",
    "/home/user/.cargo/bin/rustdoc",
    "/home/user/.cargo/bin/clippy-driver",
    "/home/user/.cargo/bin/cargo-nextest",
    "/home/user/.cargo/bin/nextest",
    "/home/user/.cargo/bin/cross",
}
SAFE_GIT_SUBCOMMANDS = {
    "status",
    "diff",
    "show",
    "log",
    "ls-files",
    "ls-tree",
    "rev-parse",
    "grep",
    "blame",
    "shortlog",
    "describe",
    "name-rev",
    "check-ignore",
    "check-attr",
    "check-ref-format",
    "merge-base",
}
GIT_EXTERNAL_DIFF_SUBCOMMANDS = {"diff", "show", "log"}
GIT_SAFE_CONFIG_PREFIX = [
    "-c", "core.fsmonitor=false",
    "-c", "core.untrackedCache=false",
    "-c", "diff.external=",
    "-c", "core.attributesFile=/dev/null",
]
REVIEWED_STATIC_SCRIPTS = {
    "scripts/check-static-structured-text.py",
    "scripts/check-doc-links.py",
    "scripts/check-public-promises.py",
    "scripts/check-msrv.py",
    "scripts/check-release-locking.py",
}
HEAVY_SCRIPTS = {
    "scripts/generate-workspace-layout.py",
    "scripts/check-test-inventory.py",
    "scripts/run-ci-tier.py",
    "scripts/build-release-archive.py",
    "scripts/test-release-archive.py",
    "scripts/smoke-release-install.py",
    "scripts/run-release-locked-sbom.py",
}
FIELD_SCRIPTS = {
    "scripts/check-real-host-evidence.py",
    "scripts/run-real-host-journey.py",
    "scripts/run-field-evidence.py",
    "scripts/run-independent-semantic-review.py",
}
PUBLICATION_CARGO_SUBCOMMANDS = {"publish", "login", "owner", "yank"}
HEAVY_CARGO_SUBCOMMANDS = {
    "test",
    "run",
    "build",
    "install",
    "bench",
    "nextest",
    "fuzz",
    "clippy",
    "doc",
    "package",
    "rustc",
    "fix",
    "clean",
    "update",
    "fetch",
    "vendor",
    "tree",
    "generate-lockfile",
    "check",
    "metadata",
}
SAFE_CHECK_SWITCHES = {
    "--all-targets",
    "--all-features",
    "--no-default-features",
    "--lib",
    "--bins",
    "--examples",
    "--tests",
    "--benches",
}
CHECK_VALUE_FLAGS = {"--features", "--bin", "--example", "--test", "--bench"}
PACKAGE_NAME = re.compile(r"[A-Za-z0-9_.-]+")
ENV_ASSIGNMENT = re.compile(r"[A-Za-z_][A-Za-z0-9_]*=.*", re.DOTALL)
SHELL_CONSTRUCTION = re.compile(
    r"(?:`|\$\(|\$\{|\$['\"]|\$[0-9@*#?$!\-]|"
    r"\$[A-Za-z_][A-Za-z0-9_]*|[<>]\(|[*?\[\]{}]|"
    r"(?:^|[\s;&|])(?:eval|source|alias|unalias|function)\b|"
    r"(?:^|[;&|]\s*)\.\s+\S)",
    re.IGNORECASE,
)
FORBIDDEN_CARGO_ENV_NAMES = {
    "CARGO_HOME",
    "RUSTUP_HOME",
    "RUSTC",
    "RUSTDOC",
    "RUSTFLAGS",
    "RUSTDOCFLAGS",
    "RUSTC_WRAPPER",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTUP_TOOLCHAIN",
    "CARGO_BUILD_RUSTC",
    "CARGO_BUILD_RUSTC_WRAPPER",
    "CARGO_BUILD_RUSTDOC",
    "CARGO_BUILD_TARGET",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_TARGET_DIR",
    "BASH_ENV",
    "ENV",
    "LD_PRELOAD",
}
FORBIDDEN_CARGO_ENV_PREFIXES = ("CARGO_", "RUSTUP_", "RUSTC_", "RUSTDOC_")
FORBIDDEN_GIT_ENV_NAMES = {
    "GIT_EXTERNAL_DIFF",
    "GIT_CONFIG",
    "GIT_CONFIG_PARAMETERS",
    "GIT_CONFIG_SYSTEM",
    "GIT_CONFIG_GLOBAL",
    "GIT_CONFIG_NOSYSTEM",
    "GIT_ATTR_NOSYSTEM",
}
FORBIDDEN_GIT_ENV_PREFIXES = ("GIT_CONFIG_KEY_", "GIT_CONFIG_VALUE_")
FORBIDDEN_PYTHON_ENV_NAMES = {
    "PYTHONPATH",
    "PYTHONHOME",
    "PYTHONUSERBASE",
    "PYTHONSTARTUP",
    "PYTHONINSPECT",
    "PYTHONBREAKPOINT",
    "PYTHONPLATLIBDIR",
    "PYTHONWARNINGS",
}
AFFIRMATIVE_HOST_DECISION_STATUS = "concluded_exact_version_affirmative"
AFFIRMATIVE_HOST_SELECTION = "selected"
if yaml is not None:

    class UniqueSafeLoader(yaml.SafeLoader):
        """Reject duplicate keys in command-gate authorities."""

    def construct_unique_mapping(
        loader: Any, node: Any, deep: bool = False
    ) -> dict[Any, Any]:
        result: dict[Any, Any] = {}
        for key_node, value_node in node.value:
            key = loader.construct_object(key_node, deep=deep)
            if not isinstance(key, (str, int, float, bool, type(None))):
                raise ValueError("non-scalar YAML mapping key")
            if key in result:
                raise ValueError(f"duplicate YAML key {key!r}")
            result[key] = loader.construct_object(value_node, deep=deep)
        return result

    UniqueSafeLoader.add_constructor(
        yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG,
        construct_unique_mapping,
    )
else:
    UniqueSafeLoader = None


def mapping(value: Any) -> dict[str, Any] | None:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        return None
    return value


def string_list(value: Any) -> list[str] | None:
    if not isinstance(value, list) or not all(isinstance(item, str) for item in value):
        return None
    return value


def indexed(values: Any) -> dict[str, dict[str, Any]] | None:
    if not isinstance(values, list):
        return None
    result: dict[str, dict[str, Any]] = {}
    for value in values:
        record = mapping(value)
        if record is None:
            return None
        record_id = record.get("id")
        if not isinstance(record_id, str) or not record_id or record_id in result:
            return None
        result[record_id] = record
    return result


def relative_regular_ref(value: Any) -> bool:
    if not isinstance(value, str) or not value:
        return False
    raw_path = value.split("#", 1)[0]
    path = Path(raw_path)
    if path.is_absolute() or ".." in path.parts:
        return False
    resolved = ROOT / path
    return resolved.is_file() and not resolved.is_symlink()


def accepted_campaign_checkpoint_kinds(campaign: dict[str, Any]) -> set[str] | None:
    resume = mapping(campaign.get("resume_authority"))
    if resume is None:
        return None
    kinds = string_list(resume.get("accepted_checkpoint_kinds"))
    if kinds is None or len(kinds) != len(set(kinds)):
        return None
    accepted = set(kinds)
    if accepted != CAMPAIGN_CHECKPOINT_KINDS:
        return None
    return accepted


def campaign_checkpoint_valid(value: Any, accepted_kinds: set[str]) -> bool:
    checkpoint = mapping(value)
    if checkpoint is None:
        return False
    required = {"kind", "state_ref", "base_commit", "updated_at"}
    if not required <= set(checkpoint):
        return False
    if checkpoint.get("kind") not in accepted_kinds:
        return False
    if not relative_regular_ref(checkpoint.get("state_ref")):
        return False
    base_commit = checkpoint.get("base_commit")
    if not isinstance(base_commit, str) or re.fullmatch(r"[0-9a-f]{40}", base_commit) is None:
        return False
    updated_at = checkpoint.get("updated_at")
    if not isinstance(updated_at, str) or re.fullmatch(r"\d{4}-\d{2}-\d{2}", updated_at) is None:
        return False
    for field in ("evidence_refs", "remaining_work"):
        if field in checkpoint and string_list(checkpoint[field]) is None:
            return False
    return True


def campaign_items_valid(
    campaign: dict[str, Any], items: dict[str, dict[str, Any]]
) -> bool:
    vocabulary = mapping(campaign.get("status_vocabulary"))
    values = string_list(vocabulary.get("values")) if vocabulary else None
    accepted_kinds = accepted_campaign_checkpoint_kinds(campaign)
    if values is None or len(values) != len(set(values)):
        return False
    if set(values) != CAMPAIGN_STATUS_VALUES or accepted_kinds is None:
        return False
    for item in items.values():
        status = item.get("status")
        owner = item.get("owner")
        checkpoint = item.get("checkpoint")
        blocked_reason = item.get("blocked_reason")
        if status not in CAMPAIGN_STATUS_VALUES:
            return False
        if owner is not None and (not isinstance(owner, str) or not owner):
            return False
        if status == "planned":
            if owner is not None or checkpoint is not None or blocked_reason is not None:
                return False
        elif status == "in_progress":
            if (
                owner is None
                or blocked_reason is not None
                or not campaign_checkpoint_valid(checkpoint, accepted_kinds)
            ):
                return False
        elif status == "blocked_external":
            reason = mapping(blocked_reason)
            if (
                owner is not None
                or reason is None
                or reason.get("kind") != "external"
                or not campaign_checkpoint_valid(checkpoint, accepted_kinds)
            ):
                return False
        elif (
            owner is not None
            or blocked_reason is not None
            or not campaign_checkpoint_valid(checkpoint, accepted_kinds)
        ):
            return False
    return True


def inventory_counts_valid(
    inventory: dict[str, Any], records: dict[str, dict[str, Any]]
) -> bool:
    exclusions = indexed(inventory.get("forensic_exclusions"))
    counts = mapping(inventory.get("counts"))
    expected_keys = {
        "current",
        "source_control_test",
        "evidence_stage",
        "forensic_exclusions",
        "canonical_reconciled",
        "adjudicated_current_obligations",
        "source_complete_true",
        "by_status",
    }
    if exclusions is None or counts is None or set(counts) != expected_keys:
        return False
    actual_by_status = {status: 0 for status in INVENTORY_STATUS_VALUES}
    for record in records.values():
        status = record.get("status")
        if status not in INVENTORY_STATUS_VALUES:
            return False
        actual_by_status[status] += 1
    declared_by_status = mapping(counts.get("by_status"))
    actual = {
        "current": len(records),
        "source_control_test": sum(
            record.get("schedule_class") == SOURCE_STORY_SCHEDULE
            for record in records.values()
        ),
        "evidence_stage": sum(
            record.get("schedule_class") != SOURCE_STORY_SCHEDULE
            for record in records.values()
        ),
        "forensic_exclusions": len(exclusions),
        "canonical_reconciled": sum(
            record.get("disposition") == "canonical_story"
            for record in records.values()
        ),
        "adjudicated_current_obligations": sum(
            record.get("disposition") != "canonical_story"
            for record in records.values()
        ),
        "source_complete_true": sum(
            record.get("source_complete") is True for record in records.values()
        ),
    }
    return (
        declared_by_status is not None
        and set(declared_by_status) == INVENTORY_STATUS_VALUES
        and declared_by_status == actual_by_status
        and all(counts.get(key) == value for key, value in actual.items())
    )


def cross_authority_projection_valid(
    campaign: dict[str, Any],
    inventory: dict[str, Any],
    items: dict[str, dict[str, Any]],
    records: dict[str, dict[str, Any]],
) -> bool:
    story_meta = mapping(campaign.get("story_inventory"))
    exclusions = indexed(inventory.get("forensic_exclusions"))
    if story_meta is None or exclusions is None:
        return False
    source_records = {
        record_id: record
        for record_id, record in records.items()
        if record.get("schedule_class") == SOURCE_STORY_SCHEDULE
    }
    if (
        story_meta.get("authority") != INVENTORY.relative_to(ROOT).as_posix()
        or story_meta.get("schema_version") != "1.0"
        or story_meta.get("current_record_count") != len(records)
        or story_meta.get("source_story_count") != len(source_records)
        or story_meta.get("evidence_story_count") != len(records) - len(source_records)
        or story_meta.get("forensic_exclusion_count") != len(exclusions)
        or story_meta.get("records_embedded") is not False
    ):
        return False
    if sum(record.get("disposition") == "canonical_story" for record in records.values()) != 66:
        return False
    adjudicated = {
        record_id: record.get("disposition")
        for record_id, record in records.items()
        if record.get("disposition") != "canonical_story"
    }
    if adjudicated != EXPECTED_ADJUDICATED_DISPOSITIONS:
        return False

    records_by_item: dict[str, list[dict[str, Any]]] = {
        item_id: [] for item_id in SOURCE_ITEM_IDS
    }
    for record_id, record in source_records.items():
        campaign_item = record.get("campaign_item")
        if record_id == "FRUST-001":
            if campaign_item is not None:
                return False
            continue
        if campaign_item not in SOURCE_ITEM_IDS:
            return False
        records_by_item[campaign_item].append(record)
        owner = record.get("owner")
        checkpoint = mapping(record.get("checkpoint"))
        if owner is not None:
            if checkpoint is None or checkpoint.get("participant") != owner:
                return False
            item = items[campaign_item]
            if item.get("status") != "in_progress" or item.get("owner") != owner:
                return False

    if any(not item_records for item_records in records_by_item.values()):
        return False
    for item_id, item_records in records_by_item.items():
        item = items[item_id]
        record_owners = {
            record.get("owner") for record in item_records if record.get("owner") is not None
        }
        if item.get("status") == "in_progress":
            if record_owners != {item.get("owner")}:
                return False
        elif record_owners:
            return False
        if item.get("status") in SOURCE_CLOSED_STATUSES and any(
            record.get("status") != "source_complete"
            or record.get("source_complete") is not True
            or record.get("owner") is not None
            or record.get("remaining_source_work") != []
            for record in item_records
        ):
            return False
    return True


def story_checkpoint_valid(value: Any, disposition: Any) -> bool:
    checkpoint = mapping(value)
    if checkpoint is None:
        return False
    required = {
        "kind",
        "state",
        "authority_ref",
        "authority_anchor",
        "participant",
        "generation",
    }
    if set(checkpoint) != required:
        return False
    if not isinstance(checkpoint.get("kind"), str) or not checkpoint["kind"]:
        return False
    state = checkpoint.get("state")
    if state not in SOURCE_CHECKPOINT_STATES:
        return False
    if disposition == "supporting_predecessor" and state != "source_complete":
        return False
    if not relative_regular_ref(checkpoint.get("authority_ref")):
        return False
    if not isinstance(checkpoint.get("authority_anchor"), str) or not checkpoint["authority_anchor"]:
        return False
    participant = checkpoint.get("participant")
    if participant is not None and (not isinstance(participant, str) or not participant):
        return False
    generation = checkpoint.get("generation")
    if generation is not None and (
        not isinstance(generation, int) or isinstance(generation, bool) or generation < 0
    ):
        return False
    return True


def load_yaml(path: Path) -> dict[str, Any] | None:
    if yaml is None or UniqueSafeLoader is None:
        return None
    try:
        if not path.is_file() or path.is_symlink():
            return None
        value = yaml.load(path.read_text(encoding="utf-8"), Loader=UniqueSafeLoader)
    except (OSError, UnicodeError, ValueError, yaml.YAMLError):
        return None
    return mapping(value)


def load_authorities() -> dict[str, dict[str, Any]] | None:
    campaign = load_yaml(CAMPAIGN)
    inventory = load_yaml(INVENTORY)
    plan = load_yaml(PLAN)
    if campaign is None or inventory is None or plan is None:
        return None
    return {"campaign": campaign, "inventory": inventory, "plan": plan}


def dependency_closure(
    by_id: dict[str, dict[str, Any]], item_id: str
) -> set[str] | None:
    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(current: str) -> bool:
        if current in visited:
            return True
        if current in visiting or current not in by_id:
            return False
        dependencies = string_list(by_id[current].get("depends_on"))
        if dependencies is None or len(dependencies) != len(set(dependencies)):
            return False
        visiting.add(current)
        for dependency in dependencies:
            if dependency not in by_id or dependency == current or not visit(dependency):
                return False
        visiting.remove(current)
        visited.add(current)
        return True

    if not visit(item_id):
        return None
    return visited - {item_id}


def combined_source_graph_valid(
    items: dict[str, dict[str, Any]], records: dict[str, dict[str, Any]]
) -> bool:
    """Validate campaign-item and source-story authority as one dependency graph."""
    source_items = {item_id: items[item_id] for item_id in SOURCE_ITEM_IDS if item_id in items}
    if set(source_items) != SOURCE_ITEM_IDS:
        return False
    source_records = {
        record_id: record
        for record_id, record in records.items()
        if record.get("schedule_class") == SOURCE_STORY_SCHEDULE
    }
    owner_by_story: dict[str, str | None] = {}
    graph: dict[str, set[str]] = {
        **{f"item:{item_id}": set() for item_id in source_items},
        **{f"story:{record_id}": set() for record_id in source_records},
    }

    for item_id, item in source_items.items():
        dependencies = string_list(item.get("depends_on"))
        if dependencies is None or len(dependencies) != len(set(dependencies)):
            return False
        if any(dependency not in source_items for dependency in dependencies):
            return False
        graph[f"item:{item_id}"].update(f"item:{dependency}" for dependency in dependencies)

    for record_id, record in source_records.items():
        owner = record.get("campaign_item")
        if record_id == "FRUST-001":
            if owner is not None:
                return False
        elif owner not in source_items:
            return False
        owner_by_story[record_id] = owner
        if owner is not None:
            graph[f"item:{owner}"].add(f"story:{record_id}")

    for record_id, record in source_records.items():
        dependencies = string_list(record.get("dependencies"))
        if dependencies is None or len(dependencies) != len(set(dependencies)):
            return False
        owner = owner_by_story[record_id]
        for dependency in dependencies:
            target_item: str | None
            if dependency in source_records:
                graph[f"story:{record_id}"].add(f"story:{dependency}")
                target_item = owner_by_story[dependency]
            elif dependency in source_items:
                graph[f"story:{record_id}"].add(f"item:{dependency}")
                target_item = dependency
            else:
                return False
            if owner is not None and target_item is not None:
                target_closure = dependency_closure(source_items, target_item)
                if target_closure is None or owner in target_closure:
                    return False

    visiting: set[str] = set()
    visited: set[str] = set()

    def visit(node: str) -> bool:
        if node in visited:
            return True
        if node in visiting:
            return False
        visiting.add(node)
        if any(not visit(dependency) for dependency in graph[node]):
            return False
        visiting.remove(node)
        visited.add(node)
        return True

    return all(visit(node) for node in graph)


def selected_host_valid(plan: dict[str, Any]) -> bool:
    phases = plan.get("phases")
    if not isinstance(phases, list):
        return False
    phase = next(
        (
            value
            for value in phases
            if isinstance(value, dict)
            and value.get("id") == "C1-first-use-authority-vertical-slice"
        ),
        None,
    )
    phase_map = mapping(phase)
    if phase_map is None or not isinstance(phase_map.get("sequence"), list):
        return False
    c11 = next(
        (
            value
            for value in phase_map["sequence"]
            if isinstance(value, dict) and value.get("id") == "C1.1"
        ),
        None,
    )
    c11_map = mapping(c11)
    if c11_map is None:
        return False
    screening = mapping(c11_map.get("screening_checkpoint"))
    selected = mapping(screening.get("selected_reference_host")) if screening else None
    required_fields = {
        "kind",
        "exact_version",
        "decision_ref",
        "decision_id",
        "decision_status",
        "selection_binding",
    }
    if selected is None or set(selected) != required_fields:
        return False
    kind = selected.get("kind")
    version = selected.get("exact_version")
    decision_ref = selected.get("decision_ref")
    decision_id = selected.get("decision_id")
    decision_status = selected.get("decision_status")
    selection_binding = selected.get("selection_binding")
    if not isinstance(kind, str) or not kind or kind == "none":
        return False
    if not isinstance(version, str) or re.fullmatch(
        r"v?[0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z][0-9A-Za-z.-]*)?"
        r"(?:\+[0-9A-Za-z][0-9A-Za-z.-]*)?",
        version,
    ) is None:
        return False
    if (
        not isinstance(decision_id, str)
        or not decision_id
        or decision_status != AFFIRMATIVE_HOST_DECISION_STATUS
        or not isinstance(selection_binding, str)
        or not selection_binding
        or not relative_regular_ref(decision_ref)
    ):
        return False
    decision = load_yaml(ROOT / decision_ref.split("#", 1)[0])
    if decision is None:
        return False
    subject = mapping(decision.get("exact_local_subject"))
    resolution = mapping(decision.get("resolution"))
    if subject is None or resolution is None:
        return False
    subject_version = subject.get("version", subject.get("binary_version"))
    return (
        decision.get("artifact_kind") == "host-capability-decision"
        and decision.get("decision_id") == decision_id
        and decision.get("status") == decision_status
        and subject_version == version.lstrip("v")
        and resolution.get("selected_C1_reference_host") == AFFIRMATIVE_HOST_SELECTION
        and resolution.get("selection_binding") == selection_binding
    )


def compiler_feedback_policy_valid(
    campaign: dict[str, Any], inventory: dict[str, Any], plan: dict[str, Any]
) -> bool:
    if (
        campaign.get("schema_version") != "0.1"
        or campaign.get("artifact_kind") != "canonical-campaign-manifest"
        or inventory.get("schema_version") != "1.0"
        or inventory.get("artifact_kind") != "product-gap-closure-story-inventory"
        or plan.get("schema_version") != "0.1"
        or plan.get("artifact_kind") != "product-gap-closure-plan"
    ):
        return False

    execution = mapping(campaign.get("execution_policy"))
    if execution is None:
        return False
    compiler_feedback = mapping(execution.get("compiler_feedback"))
    deferred_heavy = mapping(execution.get("deferred_heavy_stabilization"))
    if compiler_feedback != COMPILER_FEEDBACK_POLICY or deferred_heavy != DEFERRED_HEAVY_POLICY:
        return False
    if "no_build_override" in execution:
        return False

    scheduling = mapping(campaign.get("scheduling_vocabulary"))
    definitions = mapping(scheduling.get("definitions")) if scheduling else None
    if definitions is None:
        return False
    if definitions.get("pre_stabilization_implementation") != (
        "All source, fixture, checker, and product-surface implementation uses continuous compile-only "
        "compiler feedback while runtime and heavy gates remain deferred; cargo check may execute build "
        "scripts and proc macros as compile-time behavior but must not execute project tests or binaries."
    ):
        return False
    if definitions.get("stabilization_gate") != (
        "The sole campaign item allowed to execute deferred runtime, project-linked-build, matrix, "
        "archive, and hosted gates after every source item and source story closes."
    ):
        return False

    verification = mapping(plan.get("verification_strategy"))
    if verification is None:
        return False
    if verification.get("per_large_work_part") != PLAN_PER_PART:
        return False
    if verification.get("closure_gates") != PLAN_CLOSURE:
        return False

    invariants = string_list(inventory.get("scheduling_invariants"))
    if invariants is None or not INVENTORY_COMPILER_INVARIANTS <= set(invariants):
        return False

    items = indexed(campaign.get("items"))
    if items is None or set(items) != ITEM_IDS or not campaign_items_valid(campaign, items):
        return False
    source_items = {
        item_id
        for item_id, item in items.items()
        if item.get("schedule_class") == SOURCE_ITEM_SCHEDULE
    }
    if source_items != SOURCE_ITEM_IDS:
        return False
    if items["C3.2"].get("schedule_class") != "stabilization_gate":
        return False
    if {
        item_id
        for item_id, item in items.items()
        if item.get("schedule_class") == "post_stabilization_evidence"
    } != {"C3.3", "C3.4"}:
        return False
    for item_id in SOURCE_ITEM_IDS:
        dependencies = string_list(items[item_id].get("depends_on"))
        if dependencies is None or any(
            dependency not in SOURCE_ITEM_IDS for dependency in dependencies
        ):
            return False
    if set(string_list(items["C3.2"].get("depends_on")) or []) != EXPECTED_SOURCE_LEAVES:
        return False
    if dependency_closure(items, "C3.2") != SOURCE_ITEM_IDS:
        return False

    records = indexed(inventory.get("current_records"))
    if records is None or len(records) != 81 or not inventory_counts_valid(inventory, records):
        return False
    if sum(
        record.get("schedule_class") == SOURCE_STORY_SCHEDULE
        for record in records.values()
    ) != 76:
        return False
    source_records = {
        record_id: record
        for record_id, record in records.items()
        if record.get("schedule_class") == SOURCE_STORY_SCHEDULE
    }
    for record in source_records.values():
        status = record.get("status")
        source_complete = record.get("source_complete")
        remaining = string_list(record.get("remaining_source_work"))
        owner = record.get("owner")
        checkpoint = record.get("checkpoint")
        if status not in {"planned", "in_progress", "blocked", "source_complete"}:
            return False
        if remaining is None or not isinstance(source_complete, bool):
            return False
        if status == "planned" and (
            owner is not None or checkpoint is not None or source_complete or not remaining
        ):
            return False
        if status in {"in_progress", "blocked"} and (
            checkpoint is None or source_complete or not remaining
        ):
            return False
        if status == "source_complete" and (
            owner is not None or checkpoint is None or not source_complete or remaining
        ):
            return False
        dependencies = string_list(record.get("dependencies"))
        if dependencies is None or len(dependencies) != len(set(dependencies)):
            return False
        if any(
            dependency not in records and dependency not in SOURCE_ITEM_IDS
            for dependency in dependencies
        ):
            return False
    if not combined_source_graph_valid(items, records):
        return False
    if not cross_authority_projection_valid(campaign, inventory, items, records):
        return False
    return True


def implementation_compiler_feedback_open(
    authorities: dict[str, dict[str, Any]] | None = None,
) -> bool:
    documents = authorities if authorities is not None else load_authorities()
    if documents is None or set(documents) != {"campaign", "inventory", "plan"}:
        return False
    campaign = mapping(documents.get("campaign"))
    inventory = mapping(documents.get("inventory"))
    plan = mapping(documents.get("plan"))
    if campaign is None or inventory is None or plan is None:
        return False
    if not compiler_feedback_policy_valid(campaign, inventory, plan):
        return False
    items = indexed(campaign.get("items"))
    stabilization = mapping(campaign.get("stabilization"))
    if items is None or stabilization is None:
        return False
    for item_id in ("C3.2", "C3.3", "C3.4"):
        item = items[item_id]
        if (
            item.get("status") != "planned"
            or item.get("owner") is not None
            or item.get("checkpoint") is not None
        ):
            return False
    return stabilization.get("status") == "planned"


def source_authorities_ready(
    campaign: dict[str, Any], inventory: dict[str, Any], plan: dict[str, Any]
) -> tuple[bool, dict[str, dict[str, Any]] | None]:
    if not compiler_feedback_policy_valid(campaign, inventory, plan):
        return False, None
    items = indexed(campaign.get("items"))
    records = indexed(inventory.get("current_records"))
    exclusions = indexed(inventory.get("forensic_exclusions"))
    if items is None or set(items) != ITEM_IDS or records is None or exclusions is None:
        return False, items
    if len(records) != 81 or len(exclusions) != 26 or set(exclusions) != EXPECTED_FORENSIC_IDS:
        return False, items
    if any(
        exclusion.get("disposition") != "forensic_reference_only"
        or exclusion.get("kind") != "legacy_v2_story"
        for exclusion in exclusions.values()
    ):
        return False, items
    if not inventory_counts_valid(inventory, records):
        return False, items
    accepted_checkpoint_kinds = accepted_campaign_checkpoint_kinds(campaign)
    if accepted_checkpoint_kinds is None:
        return False, items

    scope = mapping(campaign.get("campaign_scope"))
    if scope is None:
        return False, items
    active = string_list(scope.get("active_item_ids"))
    deferred = string_list(scope.get("deferred_item_ids"))
    if active is None or deferred is None or len(active) != len(set(active)):
        return False, items
    if set(active) != ITEM_IDS or deferred:
        return False, items

    for item_id in SOURCE_ITEM_IDS:
        if items[item_id].get("status") not in SOURCE_CLOSED_STATUSES:
            return False, items
        if items[item_id].get("owner") is not None:
            return False, items
        if not campaign_checkpoint_valid(
            items[item_id].get("checkpoint"), accepted_checkpoint_kinds
        ):
            return False, items

    if string_list(items["C3.3"].get("depends_on")) != ["C3.2"]:
        return False, items
    c34_dependencies = string_list(items["C3.4"].get("depends_on"))
    if c34_dependencies is None or set(c34_dependencies) != {"C1.4", "C3.3"}:
        return False, items

    source_records = {
        record_id: record
        for record_id, record in records.items()
        if record.get("schedule_class") == SOURCE_STORY_SCHEDULE
    }
    evidence_records = {
        record_id: record
        for record_id, record in records.items()
        if record.get("schedule_class") != SOURCE_STORY_SCHEDULE
    }
    if len(source_records) != 76 or set(evidence_records) != set(EXPECTED_EVIDENCE_RECORDS):
        return False, items
    for record_id, (campaign_item, schedule_class) in EXPECTED_EVIDENCE_RECORDS.items():
        record = evidence_records[record_id]
        if (
            record.get("campaign_item") != campaign_item
            or record.get("schedule_class") != schedule_class
            or record.get("status") != "planned"
            or record.get("source_complete") is not True
            or record.get("owner") is not None
            or record.get("remaining_source_work") != []
            or record.get("checkpoint") is not None
        ):
            return False, items
    adjudicated = {
        record_id: record.get("disposition")
        for record_id, record in records.items()
        if record.get("disposition") != "canonical_story"
    }
    if adjudicated != EXPECTED_ADJUDICATED_DISPOSITIONS:
        return False, items
    if sum(
        record.get("disposition") == "canonical_story" for record in records.values()
    ) != 66:
        return False, items
    owned_source_items = {
        record.get("campaign_item")
        for record in source_records.values()
        if record.get("campaign_item") is not None
    }
    if owned_source_items != SOURCE_ITEM_IDS:
        return False, items
    for record_id, record in source_records.items():
        if record.get("status") != "source_complete" or record.get("source_complete") is not True:
            return False, items
        if record.get("owner") is not None or record.get("remaining_source_work") != []:
            return False, items
        campaign_item = record.get("campaign_item")
        if record_id == "FRUST-001":
            if campaign_item is not None:
                return False, items
        elif campaign_item not in SOURCE_ITEM_IDS:
            return False, items
        disposition = record.get("disposition")
        if disposition not in {"canonical_story", "assign", "supporting_predecessor"}:
            return False, items
        if not story_checkpoint_valid(record.get("checkpoint"), disposition):
            return False, items
        dependencies = string_list(record.get("dependencies"))
        if dependencies is None or len(dependencies) != len(set(dependencies)):
            return False, items
        for dependency in dependencies:
            if dependency in records:
                if records[dependency].get("schedule_class") != SOURCE_STORY_SCHEDULE:
                    return False, items
            elif dependency in items:
                if dependency not in SOURCE_ITEM_IDS:
                    return False, items
            else:
                return False, items
    if not combined_source_graph_valid(items, records):
        return False, items

    story_meta = mapping(campaign.get("story_inventory"))
    if story_meta is None:
        return False, items
    if (
        story_meta.get("authority") != INVENTORY.relative_to(ROOT).as_posix()
        or story_meta.get("schema_version") != "1.0"
        or story_meta.get("current_record_count") != 81
        or story_meta.get("source_story_count") != 76
        or story_meta.get("evidence_story_count") != 5
        or story_meta.get("forensic_exclusion_count") != 26
        or story_meta.get("records_embedded") is not False
    ):
        return False, items

    stabilization = mapping(campaign.get("stabilization"))
    if stabilization is None:
        return False, items
    if (
        stabilization.get("item_id") != "C3.2"
        or stabilization.get("lifts_stabilization_commands_only") is not True
        or stabilization.get("does_not_lift_publication_or_field_commands") is not True
    ):
        return False, items

    return selected_host_valid(plan), items


def stage_permissions(
    authorities: dict[str, dict[str, Any]] | None = None,
) -> dict[str, bool]:
    documents = authorities if authorities is not None else load_authorities()
    closed = {"stabilization": False, "publication": False, "field": False}
    if documents is None or set(documents) != {"campaign", "inventory", "plan"}:
        return closed
    campaign = mapping(documents.get("campaign"))
    inventory = mapping(documents.get("inventory"))
    plan = mapping(documents.get("plan"))
    if campaign is None or inventory is None or plan is None:
        return closed

    ready, items = source_authorities_ready(campaign, inventory, plan)
    if not ready or items is None:
        return closed
    stabilization = mapping(campaign.get("stabilization"))
    accepted_checkpoint_kinds = accepted_campaign_checkpoint_kinds(campaign)
    if stabilization is None or accepted_checkpoint_kinds is None:
        return closed

    c32 = items["C3.2"]
    c33 = items["C3.3"]
    c34 = items["C3.4"]
    c32_status = c32.get("status")
    closed["stabilization"] = (
        c32_status == "in_progress"
        and stabilization.get("status") == "in_progress"
        and isinstance(c32.get("owner"), str)
        and bool(c32["owner"])
        and campaign_checkpoint_valid(c32.get("checkpoint"), accepted_checkpoint_kinds)
    )
    c32_completed = (
        c32_status == "completed"
        and stabilization.get("status") == "completed"
        and c32.get("owner") is None
        and campaign_checkpoint_valid(c32.get("checkpoint"), accepted_checkpoint_kinds)
    )
    closed["publication"] = (
        c32_completed
        and c33.get("status") == "in_progress"
        and isinstance(c33.get("owner"), str)
        and bool(c33["owner"])
        and campaign_checkpoint_valid(c33.get("checkpoint"), accepted_checkpoint_kinds)
    )
    c33_completed = (
        c32_completed
        and c33.get("status") == "completed"
        and c33.get("owner") is None
        and campaign_checkpoint_valid(c33.get("checkpoint"), accepted_checkpoint_kinds)
    )
    closed["field"] = (
        c33_completed
        and c34.get("status") == "in_progress"
        and isinstance(c34.get("owner"), str)
        and bool(c34["owner"])
        and campaign_checkpoint_valid(c34.get("checkpoint"), accepted_checkpoint_kinds)
    )
    return closed


def parse_shell_command(command: str) -> tuple[list[str] | None, str | None]:
    if not command.strip():
        return None, "empty shell command cannot be classified safely"
    if "\x00" in command or "\n" in command or "\r" in command:
        return None, "multi-command or NUL shell input is blocked by fail-closed enforcement"
    if SHELL_CONSTRUCTION.search(command):
        return None, "shell substitution, expansion, eval, or source is blocked by fail-closed enforcement"
    try:
        lexer = shlex.shlex(command, posix=True, punctuation_chars=";&|<>")
        lexer.whitespace_split = True
        lexer.commenters = ""
        tokens = list(lexer)
    except ValueError:
        return None, "shell command cannot be classified safely"
    if not tokens:
        return None, "empty shell command cannot be classified safely"
    if any(token and set(token) <= set(";&|<>") for token in tokens):
        return None, "shell control operators, pipes, and redirects are blocked by fail-closed enforcement"
    if ENV_ASSIGNMENT.fullmatch(tokens[0]):
        return None, "environment-prefixed command execution is blocked by fail-closed enforcement"
    return tokens, None


def workspace_package_names() -> set[str] | None:
    root_manifest = ROOT / "Cargo.toml"
    try:
        if not root_manifest.is_file() or root_manifest.is_symlink():
            return None
        root_document = tomllib.loads(root_manifest.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, tomllib.TOMLDecodeError):
        return None
    workspace = mapping(root_document.get("workspace"))
    members = string_list(workspace.get("members")) if workspace else None
    if members is None or not members or len(members) != len(set(members)):
        return None
    names: set[str] = set()
    for member in members:
        member_path = Path(member)
        if member_path.is_absolute() or ".." in member_path.parts:
            return None
        manifest = ROOT / member_path / "Cargo.toml"
        try:
            if not manifest.is_file() or manifest.is_symlink():
                return None
            document = tomllib.loads(manifest.read_text(encoding="utf-8"))
        except (OSError, UnicodeError, tomllib.TOMLDecodeError):
            return None
        package = mapping(document.get("package"))
        name = package.get("name") if package else None
        if (
            not isinstance(name, str)
            or PACKAGE_NAME.fullmatch(name) is None
            or name.startswith("-")
            or name in names
        ):
            return None
        names.add(name)
    return names


def package_value_valid(value: str) -> bool:
    if not value or value.startswith("-") or PACKAGE_NAME.fullmatch(value) is None:
        return False
    names = workspace_package_names()
    return names is not None and value in names


def cargo_check_valid(argv: list[str]) -> bool:
    if len(argv) < 4 or argv[0] != TRUSTED_COMPILE_CARGO or argv[1] != "check":
        return False
    lock_modes = 0
    workspace_scopes = 0
    package_scopes = 0
    index = 2
    while index < len(argv):
        token = argv[index]
        if token in {"--locked", "--frozen"}:
            lock_modes += 1
            index += 1
            continue
        if token == "--workspace":
            workspace_scopes += 1
            index += 1
            continue
        if token in {"-p", "--package"}:
            if index + 1 >= len(argv) or not package_value_valid(argv[index + 1]):
                return False
            package_scopes += 1
            index += 2
            continue
        if token.startswith("--package="):
            if not package_value_valid(token.split("=", 1)[1]):
                return False
            package_scopes += 1
            index += 1
            continue
        if token in SAFE_CHECK_SWITCHES:
            index += 1
            continue
        if token in CHECK_VALUE_FLAGS:
            if index + 1 >= len(argv) or not argv[index + 1] or argv[index + 1].startswith("-"):
                return False
            index += 2
            continue
        if token.startswith("--features="):
            if not token.split("=", 1)[1]:
                return False
            index += 1
            continue
        return False
    if lock_modes != 1:
        return False
    return (workspace_scopes == 1 and package_scopes == 0) or (
        workspace_scopes == 0 and package_scopes == 1
    )


def cargo_metadata_valid(argv: list[str]) -> bool:
    if len(argv) < 5 or argv[0] != TRUSTED_COMPILE_CARGO or argv[1] != "metadata":
        return False
    lock_modes = 0
    no_deps = 0
    format_version = 0
    index = 2
    while index < len(argv):
        token = argv[index]
        if token in {"--locked", "--frozen"}:
            lock_modes += 1
            index += 1
            continue
        if token == "--no-deps":
            no_deps += 1
            index += 1
            continue
        if token == "--format-version":
            if index + 1 >= len(argv) or argv[index + 1] != "1":
                return False
            format_version += 1
            index += 2
            continue
        if token == "--format-version=1":
            format_version += 1
            index += 1
            continue
        return False
    return lock_modes == 1 and no_deps == 1 and format_version == 1


def hermetic_compile_launcher_valid(argv: list[str]) -> bool:
    if len(argv) < 4 or argv[:3] != [TRUSTED_PYTHON, "-I", HERMETIC_COMPILE_LAUNCHER]:
        return False
    cargo_argv = [TRUSTED_COMPILE_CARGO, *argv[3:]]
    return cargo_check_valid(cargo_argv) or cargo_metadata_valid(cargo_argv)


def repository_script(token: str) -> str | None:
    try:
        path = Path(token)
        if not path.is_absolute() or path.as_posix() != token:
            return None
        relative = path.relative_to(ROOT).as_posix()
    except (OSError, RuntimeError, ValueError):
        return None
    if relative not in REVIEWED_STATIC_SCRIPTS | HEAVY_SCRIPTS | FIELD_SCRIPTS:
        return None
    if not path.is_file() or path.is_symlink():
        return None
    return relative


def strict_git_parts(argv: list[str]) -> tuple[str, list[str]] | None:
    prefix = [TRUSTED_GIT, "--no-pager", *GIT_SAFE_CONFIG_PREFIX]
    if len(argv) <= len(prefix) or argv[: len(prefix)] != prefix:
        return None
    index = len(prefix)
    if index < len(argv) and argv[index] == "-C":
        if index + 1 >= len(argv) or Path(argv[index + 1]) != ROOT:
            return None
        index += 2
    if index >= len(argv):
        return None
    return argv[index].lower(), argv[index + 1 :]


def git_signature_format_requested(arguments: list[str]) -> bool:
    for index, token in enumerate(arguments):
        if token == "--show-signature":
            return True
        if token in {"--pretty", "--format"}:
            if index + 1 < len(arguments) and "%G" in arguments[index + 1]:
                return True
        elif token.startswith(("--pretty=", "--format=")) and "%G" in token.split("=", 1)[1]:
            return True
    return False


def read_only_git_valid(argv: list[str]) -> bool:
    parts = strict_git_parts(argv)
    if parts is None:
        return False
    subcommand, arguments = parts
    if subcommand not in SAFE_GIT_SUBCOMMANDS or git_signature_format_requested(arguments):
        return False
    if any(
        token in {
            "--ext-diff",
            "--textconv",
            "--open-files-in-pager",
            "--paginate",
            "--config-env",
            "-c",
        }
        or token.startswith(
            (
                "--output=",
                "--open-files-in-pager=",
                "--config-env=",
                "--exec-path=",
                "--git-dir=",
                "--work-tree=",
                "--namespace=",
            )
        )
        or token in {"--output", "--exec-path", "--git-dir", "--work-tree", "--namespace"}
        or (subcommand == "grep" and token.startswith("-") and not token.startswith("--") and "O" in token[1:])
        for token in arguments
    ):
        return False
    if subcommand in GIT_EXTERNAL_DIFF_SUBCOMMANDS and (
        "--no-ext-diff" not in arguments or "--no-textconv" not in arguments
    ):
        return False
    return True


def gh_subcommand(argv: list[str]) -> str | None:
    if not argv or argv[0] != TRUSTED_GH:
        return None
    index = 1
    options_with_values = {"-R", "--repo", "--hostname"}
    while index < len(argv):
        token = argv[index]
        if token in options_with_values:
            index += 2
            continue
        if token.startswith(("--repo=", "--hostname=")):
            index += 1
            continue
        if token.startswith("-"):
            index += 1
            continue
        return token.lower()
    return None


def cargo_environment_override() -> str | None:
    for name, value in sorted(os.environ.items()):
        if not value:
            continue
        if (
            name in FORBIDDEN_CARGO_ENV_NAMES
            or name.startswith(FORBIDDEN_CARGO_ENV_PREFIXES)
            or name.startswith("BASH_FUNC_")
        ):
            return name
    return None


def git_environment_override() -> str | None:
    for name, value in sorted(os.environ.items()):
        if value and (
            name in FORBIDDEN_GIT_ENV_NAMES
            or name == "GIT_CONFIG_COUNT"
            or name.startswith(FORBIDDEN_GIT_ENV_PREFIXES)
        ):
            return name
    return None


def python_environment_override() -> str | None:
    for name, value in sorted(os.environ.items()):
        if value and name in FORBIDDEN_PYTHON_ENV_NAMES:
            return name
    return None


def rustup_valid(argv: list[str]) -> bool:
    if len(argv) < 2 or argv[0] != TRUSTED_RUSTUP:
        return False
    subcommand = argv[1]
    arguments = argv[2:]
    toolchain = re.compile(r"[A-Za-z0-9_.-]+(?:-[A-Za-z0-9_.-]+)*")
    target = re.compile(r"[A-Za-z0-9_]+(?:-[A-Za-z0-9_]+){2,3}")
    component = {"rustc", "cargo", "rust-std", "rust-docs", "rustfmt", "clippy", "rust-src"}
    if subcommand == "show":
        return arguments in ([], ["active-toolchain"])
    if subcommand == "toolchain":
        if arguments == ["list"]:
            return True
        if len(arguments) < 2 or arguments[0] not in {"install", "uninstall"}:
            return False
        if toolchain.fullmatch(arguments[1]) is None:
            return False
        if arguments[0] == "uninstall":
            return len(arguments) == 2
        index = 2
        seen_profile = False
        while index < len(arguments):
            token = arguments[index]
            if token == "--profile" and not seen_profile and index + 1 < len(arguments):
                if arguments[index + 1] not in {"minimal", "default", "complete"}:
                    return False
                seen_profile = True
                index += 2
                continue
            if token in {"--component", "--target"} and index + 1 < len(arguments):
                value = arguments[index + 1]
                if token == "--component" and value not in component:
                    return False
                if token == "--target" and target.fullmatch(value) is None:
                    return False
                index += 2
                continue
            return False
        return True
    if subcommand in {"target", "component"}:
        if arguments == ["list"]:
            return True
        if len(arguments) not in {2, 4} or arguments[0] not in {"add", "remove"}:
            return False
        value = arguments[1]
        if subcommand == "target" and target.fullmatch(value) is None:
            return False
        if subcommand == "component" and value not in component:
            return False
        return len(arguments) == 2 or (
            arguments[2] == "--toolchain" and toolchain.fullmatch(arguments[3]) is not None
        )
    return False


def classify_argv(argv: list[str], command: str) -> set[str]:
    del command
    executable = argv[0]

    if executable == TRUSTED_CARGO:
        if len(argv) < 2:
            return {"unknown"}
        subcommand_index = 1
        if argv[subcommand_index].startswith("+"):
            subcommand_index += 1
        if subcommand_index >= len(argv):
            return {"unknown"}
        subcommand = argv[subcommand_index].lower()
        if subcommand in PUBLICATION_CARGO_SUBCOMMANDS:
            return {"publication"}
        if subcommand in HEAVY_CARGO_SUBCOMMANDS:
            return {"heavy"}
        return {"unknown"}

    if executable == TRUSTED_GIT:
        if read_only_git_valid(argv):
            return {"read_only"}
        parts = strict_git_parts(argv)
        if parts is not None and parts[0] in {"push", "tag"}:
            return {"publication"}
        return {"unknown"}

    if executable == TRUSTED_RG:
        if len(argv) < 3 or argv[1] != "--no-config":
            return {"unknown"}
        if any(
            token in {"--pre", "--search-zip", "--hostname-bin"}
            or token.startswith(("--pre=", "--search-zip=", "--hostname-bin="))
            or (token.startswith("-") and not token.startswith("--") and "z" in token[1:])
            for token in argv[2:]
        ):
            return {"unknown"}
        return {"read_only"}

    if executable == TRUSTED_PYTHON:
        if hermetic_compile_launcher_valid(argv):
            return {"compiler_feedback"}
        if len(argv) != 3 or argv[1] != "-I":
            return {"unknown"}
        script = repository_script(argv[2])
        if script in REVIEWED_STATIC_SCRIPTS:
            return {"reviewed_static"}
        if script in HEAVY_SCRIPTS:
            return {"heavy"}
        if script in FIELD_SCRIPTS:
            return {"field"}
        return {"unknown"}

    if executable == TRUSTED_RUSTUP:
        return {"heavy"} if rustup_valid(argv) else {"unknown"}
    if executable in TRUSTED_HEAVY_EXECUTABLES:
        return {"heavy"}
    if executable == TRUSTED_ACT:
        return {"hosted"}
    if executable == TRUSTED_GH:
        subcommand = gh_subcommand(argv)
        if subcommand == "release":
            return {"publication"}
        if subcommand in {"workflow", "run"}:
            return {"hosted"}
        return {"unknown"}
    if executable in {TRUSTED_NPM, TRUSTED_PNPM}:
        if len(argv) >= 2 and argv[1].lower() == "publish":
            return {"publication"}
        return {"unknown"}
    if executable == TRUSTED_TWINE:
        if len(argv) >= 2 and argv[1].lower() == "upload":
            return {"publication"}
        return {"unknown"}

    return {"unknown"}


def classify_command(command: str) -> set[str]:
    argv, error = parse_shell_command(command)
    if error is not None or argv is None:
        if error and "control operators" in error:
            return {"indirection"}
        if error and any(
            marker in error
            for marker in ("substitution", "environment-prefixed", "multi-command")
        ):
            return {"indirection"}
        return {"unclassifiable"}
    return classify_argv(argv, command)


def blocked_reason(
    command: str, authorities: dict[str, dict[str, Any]] | None = None
) -> str | None:
    argv, parse_error = parse_shell_command(command)
    if parse_error is not None or argv is None:
        return parse_error or "shell command cannot be classified safely"
    categories = classify_argv(argv, command)
    if "unknown" in categories:
        return (
            "executable or launcher is not on the strict positive allowlist; use exact trusted paths "
            "for compile-only Cargo, read-only Git/ripgrep, or isolated reviewed repository scripts"
        )
    if argv[0] in {TRUSTED_CARGO, TRUSTED_RUSTUP} and (
        override := cargo_environment_override()
    ):
        return (
            f"trusted Cargo/rustup is blocked because inherited environment variable {override} "
            "can override executable, toolchain, target, wrapper, runner, flags, or tool configuration"
        )
    if argv[0] == TRUSTED_GIT and (override := git_environment_override()):
        return f"trusted Git is blocked because inherited environment variable {override} can inject config, diff, or pager execution"
    if (
        argv[0] == TRUSTED_PYTHON
        and "compiler_feedback" not in categories
        and (override := python_environment_override())
    ):
        return f"reviewed Python is blocked because inherited environment variable {override} can inject import or startup behavior"
    if categories <= {"read_only", "reviewed_static"}:
        return None

    try:
        permissions = stage_permissions(authorities)
        compiler_feedback_open = implementation_compiler_feedback_open(authorities)
    except (AttributeError, KeyError, OSError, TypeError, ValueError):
        permissions = {"stabilization": False, "publication": False, "field": False}
        compiler_feedback_open = False

    if "compiler_feedback" in categories:
        if compiler_feedback_open or permissions["stabilization"]:
            return None
        return (
            "compile-only locked/frozen check or bounded metadata through the canonical hermetic launcher opens only "
            "during structurally valid pre-C3.2 implementation or fully opened C3.2 stabilization"
        )
    if "publication" in categories and not permissions["publication"]:
        return "publication commands open only during C3.3 after completed C3.2 stabilization"
    if "field" in categories and not permissions["field"]:
        return (
            "real-host, field, and independent-review commands open only during C3.4 "
            "after completed C3.3 publication"
        )
    if ({"heavy", "hosted"} & categories) and not permissions["stabilization"]:
        return (
            "runtime Rust, project linked-build, Cargo-backed project execution, and hosted commands open only "
            "during C3.2 after every typed source precondition; only cargo-check build scripts and proc macros "
            "are admitted as inherent compile-time behavior before C3.2"
        )
    return None


def deny(reason: str) -> None:
    output: dict[str, Any] = {
        "hookSpecificOutput": {
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": reason,
        }
    }
    print(json.dumps(output, separators=(",", ":")))


def main() -> int:
    try:
        payload = json.load(sys.stdin)
    except (json.JSONDecodeError, OSError) as error:
        deny(f"cannot validate Bash command fail-closed: {error}")
        return 0
    if not isinstance(payload, dict):
        deny("hook payload must be a JSON object")
        return 0
    if payload.get("tool_name") != "Bash":
        return 0
    tool_input = payload.get("tool_input")
    if not isinstance(tool_input, dict) or not isinstance(tool_input.get("command"), str):
        deny("Bash command payload is missing or malformed")
        return 0
    if reason := blocked_reason(tool_input["command"]):
        deny(reason)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
