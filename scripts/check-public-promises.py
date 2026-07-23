#!/usr/bin/env python3
"""Fail-closed static audit for public identity, payload, and command promises."""

from __future__ import annotations

from collections import Counter
import html
import importlib.util
import json
from pathlib import Path
import re
import tomllib
from typing import Any, NoReturn

try:
    import yaml
except ImportError:  # Fail closed rather than interpreting typed authority as text.
    yaml = None

ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = ROOT / "contracts/plan/product-gap-closure-story-inventory-v1.yaml"
CAMPAIGN_PATH = ROOT / "contracts/plan/product-gap-closure-campaign-v1.yaml"
BACKLOG_PATH = ROOT / "contracts/backlog/rust-only-core-backlog.yaml"
GAP_REGISTER_PATH = ROOT / "docs/product-gap-register.md"

SOURCE_STORY_SCHEDULE = "pre_stabilization_source"
SOURCE_ITEM_SCHEDULE = "pre_stabilization_implementation"
SOURCE_DISPOSITIONS = {"canonical_story", "assign", "supporting_predecessor"}
FORBIDDEN_SOURCE_DEPENDENCIES = {"C3.2", "C3.3", "C3.4"}
CAMPAIGN_WIDE_SUPPORTING_PREDECESSOR = "FRUST-001"

EXPECTED_PUBLIC_OBLIGATION_MAPPINGS: dict[str, tuple[str | None, str]] = {
    "GAP-002.reinitialize-as-new": ("C2.3", "assign"),
    "GAP-010.codex-conformance": ("C4.1", "assign"),
    "FRUST-001": (None, "supporting_predecessor"),
    "FRUST-002": ("C3.1", "assign"),
    "FRUST-010": ("C1.1", "assign"),
    "FRUST-011": ("C1.1", "canonical_story"),
    "FRUST-020": ("C5.2", "assign"),
    "FRUST-021": ("C5.3", "supporting_predecessor"),
    "FRUST-022": ("C5.2", "supporting_predecessor"),
    "FRUST-030": ("C5.2", "assign"),
    "FRUST-031": ("C5.2", "assign"),
    "FRUST-040": ("C5.3", "assign"),
    "FRUST-041": ("C5.3", "supporting_predecessor"),
    "FRUST-050": ("C2.3", "supporting_predecessor"),
    "FRUST-051": ("C2.3", "assign"),
    "FRUST-052": ("C2.3", "assign"),
    "FRUST-060": ("C4.1", "canonical_story"),
    "FRUST-061": ("C2.3", "canonical_story"),
}

HOST_CAPABILITY_RESULT_FIELDS = {
    "host_kind",
    "exact_host_version",
    "adapter_id",
    "exact_adapter_version",
    "source_commit",
    "conformance_state",
    "case_inventory_digest",
    "result_bundle_digest",
    "residual_limitations",
}
HOST_CAPABILITY_PROJECTION_FIELDS = {
    "installability",
    "read_only_mcp",
    "human_origin_assurance",
    "governed_mutation",
    "field_evidence",
}
HOST_CONFORMANCE_STATES = {
    "unsupported",
    "candidate",
    "conformant_source",
    "conformant_released",
    "field_verified",
}
BACKUP_RESTORE_COMMAND_KEYS = {
    ("forge-core", "backup", "create"),
    ("forge-core", "backup", "verify"),
    ("forge-core", "restore", "preflight"),
    ("forge-core", "restore", "apply"),
}


def fail(message: str) -> NoReturn:
    raise SystemExit(f"public promise audit failed: {message}")


if yaml is not None:

    class UniqueSafeLoader(yaml.SafeLoader):
        """Parse ordinary YAML while rejecting duplicate mapping keys."""

    def construct_unique_mapping(
        loader: Any, node: Any, deep: bool = False
    ) -> dict[Any, Any]:
        mapping: dict[Any, Any] = {}
        for key_node, value_node in node.value:
            key = loader.construct_object(key_node, deep=deep)
            if not isinstance(key, (str, int, float, bool, type(None))):
                fail(
                    "YAML mapping key at line "
                    f"{key_node.start_mark.line + 1} is not scalar"
                )
            if key in mapping:
                fail(
                    f"duplicate YAML key {key!r} at line "
                    f"{key_node.start_mark.line + 1}"
                )
            mapping[key] = loader.construct_object(value_node, deep=deep)
        return mapping

    UniqueSafeLoader.add_constructor(
        yaml.resolver.BaseResolver.DEFAULT_MAPPING_TAG,
        construct_unique_mapping,
    )
else:
    UniqueSafeLoader = None


def relative(path: Path) -> str:
    return path.relative_to(ROOT).as_posix()


def load_yaml(path: Path) -> Any:
    if yaml is None or UniqueSafeLoader is None:
        fail("PyYAML is required for public promise ownership validation")
    try:
        return yaml.load(path.read_text(encoding="utf-8"), Loader=UniqueSafeLoader)
    except SystemExit:
        raise
    except yaml.YAMLError as error:
        fail(f"{relative(path)} is invalid YAML: {error}")


def unique_json_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    value: dict[str, Any] = {}
    for key, item in pairs:
        if key in value:
            fail(f"duplicate JSON key {key!r}")
        value[key] = item
    return value


def load_structured(path: Path) -> Any:
    if path.suffix.lower() == ".json":
        try:
            return json.loads(
                path.read_text(encoding="utf-8"), object_pairs_hook=unique_json_object
            )
        except json.JSONDecodeError as error:
            fail(
                f"{relative(path)}:{error.lineno}:{error.colno} is invalid JSON: "
                f"{error.msg}"
            )
    return load_yaml(path)


def require_mapping(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict) or not all(isinstance(key, str) for key in value):
        fail(f"{label} must be a string-keyed mapping")
    return value


def require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list):
        fail(f"{label} must be a list")
    return value


def indexed_records(
    values: Any, label: str, *, id_field: str = "id"
) -> dict[str, dict[str, Any]]:
    records: dict[str, dict[str, Any]] = {}
    for index, value in enumerate(require_list(values, label)):
        record = require_mapping(value, f"{label}[{index}]")
        record_id = record.get(id_field)
        if not isinstance(record_id, str) or not record_id:
            fail(f"{label}[{index}].{id_field} must be a non-empty string")
        if record_id in records:
            fail(f"{label} contains duplicate {id_field} {record_id!r}")
        records[record_id] = record
    return records


def command_key(command: str) -> tuple[str, ...]:
    tokens = command.strip().split()
    key: list[str] = []
    for token in tokens:
        if token == "\\" or token.startswith("-") or token.startswith("[") or token.startswith("<"):
            break
        if any(marker in token for marker in ("|", "(", ")")):
            break
        key.append(token)
    return tuple(key)


def canonical_command_keys() -> set[tuple[str, ...]]:
    generated = (ROOT / "docs/generated/command-surface.md").read_text(encoding="utf-8")
    commands = re.findall(r"<code>(forge-core .*?)</code>", generated)
    keys = {command_key(html.unescape(command)) for command in commands}
    if not keys:
        fail("generated command surface contains no forge-core usages")
    return keys


def documented_command_keys() -> list[tuple[Path, int, tuple[str, ...]]]:
    found: list[tuple[Path, int, tuple[str, ...]]] = []
    roots = [ROOT / "README.md", ROOT / "CONTRIBUTING.md", ROOT / "docs"]
    files: list[Path] = []
    for item in roots:
        files.extend(item.rglob("*.md") if item.is_dir() else [item])
    for path in files:
        if path.name == "command-surface.md" and path.parent.name == "generated":
            continue
        for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
            stripped = line.strip().lstrip("$>").strip().strip("`")
            if stripped.startswith("forge-core "):
                key = command_key(stripped)
                if len(key) >= 2:
                    found.append((path, number, key))
    return found


def current_frust_story_ids(backlog: dict[str, Any]) -> set[str]:
    story_ids: list[str] = []
    for epic_index, epic_value in enumerate(require_list(backlog.get("epics"), "backlog.epics")):
        epic = require_mapping(epic_value, f"backlog.epics[{epic_index}]")
        for story_index, story_value in enumerate(
            require_list(epic.get("stories"), f"backlog.epics[{epic_index}].stories")
        ):
            story = require_mapping(
                story_value, f"backlog.epics[{epic_index}].stories[{story_index}]"
            )
            story_id = story.get("id")
            if isinstance(story_id, str) and re.fullmatch(r"FRUST-\d{3}", story_id):
                story_ids.append(story_id)
    counts = Counter(story_ids)
    duplicates = sorted(story_id for story_id, count in counts.items() if count != 1)
    if duplicates:
        fail(f"current Rust backlog contains duplicate story ids: {duplicates}")
    if not story_ids:
        fail("current Rust backlog contains no FRUST story ids")
    return set(story_ids)


def gap_classifications(register: str) -> dict[str, str]:
    registered = set(re.findall(r"\*\*(GAP-\d{3})\*\*", register))
    classifications: dict[str, str] = {}
    row_pattern = re.compile(
        r"^\|\s*\*\*(GAP-\d{3})\*\*\s*\|[^|]*\|\s*`([^`]+)`\s*\|"
    )
    for line in register.splitlines():
        match = row_pattern.match(line)
        if not match:
            continue
        gap_id, classification = match.groups()
        if gap_id in classifications:
            fail(f"product gap register contains duplicate table row {gap_id}")
        classifications[gap_id] = classification
    if registered != set(classifications):
        fail(
            "product gap register classifications are incomplete; "
            f"missing={sorted(registered - set(classifications))}, "
            f"unknown={sorted(set(classifications) - registered)}"
        )
    return classifications


def campaign_phase(item_id: str) -> str:
    match = re.fullmatch(r"(C[1-7])\.\d", item_id)
    if not match:
        fail(f"invalid campaign item id {item_id!r}")
    return match.group(1)


def validate_source_record_state(record_id: str, record: dict[str, Any]) -> None:
    source_complete = record.get("source_complete")
    remaining = record.get("remaining_source_work")
    owner = record.get("owner")
    status = record.get("status")
    disposition = record.get("disposition")
    checkpoint = record.get("checkpoint")

    if not isinstance(source_complete, bool):
        fail(f"inventory record {record_id} source_complete must be boolean")
    remaining_list = require_list(
        remaining, f"inventory record {record_id}.remaining_source_work"
    )
    if owner is not None and not isinstance(owner, str):
        fail(f"inventory record {record_id} owner must be a string or null")

    if source_complete:
        if remaining_list:
            fail(f"source-complete inventory record {record_id} retains source work")
        if owner is not None:
            fail(f"source-complete inventory record {record_id} retains owner {owner!r}")
        if status != "source_complete":
            fail(
                f"source-complete inventory record {record_id} uses status {status!r}"
            )
        checkpoint_map = require_mapping(
            checkpoint, f"source-complete inventory record {record_id}.checkpoint"
        )
        checkpoint_state = checkpoint_map.get("state")
        if checkpoint_state not in {"source_complete", "implemented_pending_evidence"}:
            fail(
                f"source-complete inventory record {record_id} lacks a static "
                "source-completion checkpoint"
            )
        if (
            disposition == "supporting_predecessor"
            and checkpoint_state != "source_complete"
        ):
            fail(
                f"supporting predecessor {record_id} must use a source_complete "
                "checkpoint"
            )
    else:
        if not remaining_list:
            fail(f"incomplete inventory record {record_id} has no remaining source work")
        if status == "source_complete":
            fail(f"incomplete inventory record {record_id} uses source_complete status")

    if disposition == "supporting_predecessor" and not source_complete:
        fail(f"supporting predecessor {record_id} is not source-complete")


def check_public_obligation_ownership() -> dict[str, dict[str, Any]]:
    inventory = require_mapping(load_yaml(INVENTORY_PATH), "story inventory")
    campaign = require_mapping(load_yaml(CAMPAIGN_PATH), "campaign manifest")
    backlog = require_mapping(load_yaml(BACKLOG_PATH), "current Rust backlog")
    register = GAP_REGISTER_PATH.read_text(encoding="utf-8")

    if inventory.get("artifact_kind") != "product-gap-closure-story-inventory":
        fail("story inventory has the wrong artifact_kind")
    if campaign.get("artifact_kind") != "canonical-campaign-manifest":
        fail("campaign manifest has the wrong artifact_kind")

    records = indexed_records(inventory.get("current_records"), "inventory.current_records")
    items = indexed_records(campaign.get("items"), "campaign.items")
    inventory_meta = require_mapping(campaign.get("story_inventory"), "campaign.story_inventory")
    if inventory_meta.get("authority") != relative(INVENTORY_PATH):
        fail("campaign manifest does not name the canonical story inventory authority")

    declared_counts = {
        "current_record_count": len(records),
        "source_story_count": sum(
            record.get("schedule_class") == SOURCE_STORY_SCHEDULE
            for record in records.values()
        ),
        "evidence_story_count": sum(
            record.get("schedule_class") != SOURCE_STORY_SCHEDULE
            for record in records.values()
        ),
        "forensic_exclusion_count": len(
            require_list(inventory.get("forensic_exclusions"), "inventory.forensic_exclusions")
        ),
    }
    for field, actual in declared_counts.items():
        if inventory_meta.get(field) != actual:
            fail(
                f"campaign story_inventory {field}={inventory_meta.get(field)!r} "
                f"does not match inventory value {actual}"
            )

    source_item_ids = {
        item_id
        for item_id, item in items.items()
        if item.get("schedule_class") == SOURCE_ITEM_SCHEDULE
    }
    for item_id in sorted(source_item_ids):
        dependencies = require_list(
            items[item_id].get("depends_on"), f"campaign item {item_id}.depends_on"
        )
        for dependency in dependencies:
            if dependency not in items:
                fail(f"source campaign item {item_id} has unknown dependency {dependency!r}")
            if items[dependency].get("schedule_class") != SOURCE_ITEM_SCHEDULE:
                fail(
                    f"source campaign item {item_id} depends on non-source item "
                    f"{dependency}"
                )

    stabilization = require_mapping(campaign.get("stabilization"), "campaign.stabilization")
    if stabilization.get("item_id") != "C3.2":
        fail("campaign stabilization item must remain C3.2")
    opens_when = require_mapping(
        stabilization.get("opens_when"), "campaign.stabilization.opens_when"
    )
    requirements = indexed_records(
        opens_when.get("requirements"), "campaign.stabilization.opens_when.requirements"
    )
    source_item_gate = requirements.get("all-source-items-complete")
    source_story_gate = requirements.get("all-source-stories-complete")
    if source_item_gate is None or source_story_gate is None:
        fail("C3.2 opening gate omits source-item or source-story completeness")
    gated_item_ids = set(
        require_list(
            source_item_gate.get("item_ids"),
            "campaign.stabilization.all-source-items-complete.item_ids",
        )
    )
    if gated_item_ids != source_item_ids:
        fail(
            "C3.2 source-item gate does not cover the exact source campaign; "
            f"missing={sorted(source_item_ids - gated_item_ids)}, "
            f"unknown={sorted(gated_item_ids - source_item_ids)}"
        )
    if source_item_gate.get("required_schedule_class") != SOURCE_ITEM_SCHEDULE:
        fail("C3.2 source-item gate uses the wrong schedule class")
    if set(source_item_gate.get("allowed_statuses", [])) != {
        "implemented_pending_evidence",
        "completed",
    } or source_item_gate.get("required_owner", "missing") is not None:
        fail("C3.2 source-item gate must require closed source status and no owner")

    source_selector = require_mapping(
        source_story_gate.get("source_story_selector"),
        "campaign.stabilization.all-source-stories-complete.source_story_selector",
    )
    source_requirements = require_mapping(
        source_story_gate.get("require_for_each_source_story"),
        "campaign.stabilization.all-source-stories-complete.require_for_each_source_story",
    )
    if (
        source_story_gate.get("authority")
        != f"{relative(INVENTORY_PATH)}#current_records"
        or source_story_gate.get("expected_current_record_count") != len(records)
        or source_story_gate.get("expected_source_story_count")
        != declared_counts["source_story_count"]
        or source_selector.get("schedule_class") != SOURCE_STORY_SCHEDULE
        or source_requirements.get("status") != "source_complete"
        or source_requirements.get("source_complete") is not True
        or source_requirements.get("owner", "missing") is not None
        or source_requirements.get("remaining_source_work") != []
    ):
        fail("C3.2 source-story gate does not require every source obligation closed")

    backlog_frust = current_frust_story_ids(backlog)
    expected_frust = {
        obligation_id
        for obligation_id in EXPECTED_PUBLIC_OBLIGATION_MAPPINGS
        if re.fullmatch(r"FRUST-\d{3}", obligation_id)
    }
    if backlog_frust != expected_frust:
        fail(
            "public obligation mapping does not cover the current FRUST backlog; "
            f"unmapped={sorted(backlog_frust - expected_frust)}, "
            f"stale={sorted(expected_frust - backlog_frust)}"
        )
    inventory_frust = {
        record_id for record_id in records if re.fullmatch(r"FRUST-\d{3}", record_id)
    }
    if inventory_frust != backlog_frust:
        fail(
            "story inventory/current FRUST backlog mismatch; "
            f"missing={sorted(backlog_frust - inventory_frust)}, "
            f"unknown={sorted(inventory_frust - backlog_frust)}"
        )

    null_source_owners: list[str] = []
    records_by_item: dict[str, list[str]] = {}
    for record_id, record in records.items():
        if record.get("schedule_class") != SOURCE_STORY_SCHEDULE:
            continue
        validate_source_record_state(record_id, record)
        disposition = record.get("disposition")
        if disposition not in SOURCE_DISPOSITIONS:
            fail(
                f"source record {record_id} has invalid disposition {disposition!r}"
            )
        dependencies = require_list(
            record.get("dependencies"), f"inventory record {record_id}.dependencies"
        )
        forbidden = sorted(
            dependency
            for dependency in dependencies
            if dependency in FORBIDDEN_SOURCE_DEPENDENCIES
        )
        if forbidden:
            fail(
                f"pre-stabilization source record {record_id} depends on evidence "
                f"items {forbidden}"
            )
        for dependency in dependencies:
            if dependency in records:
                dependency_schedule = records[dependency].get("schedule_class")
                expected_schedule = SOURCE_STORY_SCHEDULE
            elif dependency in items:
                dependency_schedule = items[dependency].get("schedule_class")
                expected_schedule = SOURCE_ITEM_SCHEDULE
            else:
                fail(
                    f"pre-stabilization source record {record_id} has unknown "
                    f"dependency {dependency!r}"
                )
            if dependency_schedule != expected_schedule:
                fail(
                    f"pre-stabilization source record {record_id} depends on "
                    f"non-source obligation {dependency}"
                )

        campaign_item = record.get("campaign_item")
        if campaign_item is None:
            null_source_owners.append(record_id)
            continue
        if not isinstance(campaign_item, str) or campaign_item not in items:
            fail(
                f"inventory record {record_id} has unknown campaign owner "
                f"{campaign_item!r}"
            )
        if items[campaign_item].get("schedule_class") != SOURCE_ITEM_SCHEDULE:
            fail(
                f"source record {record_id} is owned by non-source campaign item "
                f"{campaign_item}"
            )
        records_by_item.setdefault(campaign_item, []).append(record_id)

    if null_source_owners != [CAMPAIGN_WIDE_SUPPORTING_PREDECESSOR]:
        fail(
            "the only campaign-wide source predecessor without a C-item owner must be "
            f"{CAMPAIGN_WIDE_SUPPORTING_PREDECESSOR}; found={null_source_owners}"
        )

    for item_id in sorted(source_item_ids):
        if items[item_id].get("status") not in {
            "implemented_pending_evidence",
            "completed",
        }:
            continue
        owned_records = [records[record_id] for record_id in records_by_item.get(item_id, [])]
        if not owned_records or not all(
            record.get("source_complete") is True for record in owned_records
        ):
            fail(
                f"campaign item {item_id} claims source implementation while its "
                "inventory obligations remain incomplete"
            )

    for obligation_id, (expected_item, expected_disposition) in (
        EXPECTED_PUBLIC_OBLIGATION_MAPPINGS.items()
    ):
        record = records.get(obligation_id)
        if record is None:
            fail(f"story inventory omits public source obligation {obligation_id}")
        if record.get("schedule_class") != SOURCE_STORY_SCHEDULE:
            fail(f"public source obligation {obligation_id} is not pre-stabilization")
        if record.get("campaign_item") != expected_item:
            fail(
                f"public source obligation {obligation_id} must be owned by "
                f"{expected_item!r}, found {record.get('campaign_item')!r}"
            )
        if record.get("disposition") != expected_disposition:
            fail(
                f"public source obligation {obligation_id} must use disposition "
                f"{expected_disposition!r}, found {record.get('disposition')!r}"
            )

    predecessor = records[CAMPAIGN_WIDE_SUPPORTING_PREDECESSOR]
    consumers = set(
        require_list(
            predecessor.get("reference_consumers"),
            f"inventory record {CAMPAIGN_WIDE_SUPPORTING_PREDECESSOR}.reference_consumers",
        )
    )
    if consumers != set(items):
        fail(
            f"{CAMPAIGN_WIDE_SUPPORTING_PREDECESSOR} must support the whole campaign; "
            f"missing={sorted(set(items) - consumers)}, "
            f"unknown={sorted(consumers - set(items))}"
        )

    classifications = gap_classifications(register)
    gap_items: dict[str, list[str]] = {gap_id: [] for gap_id in classifications}
    for item_id, item in items.items():
        for gap_ref in require_list(item.get("gap_refs"), f"campaign item {item_id}.gap_refs"):
            if gap_ref not in gap_items:
                fail(f"campaign item {item_id} references unknown product gap {gap_ref!r}")
            gap_items[gap_ref].append(item_id)

    for gap_id, owning_items in gap_items.items():
        if not owning_items:
            fail(f"accepted product gap {gap_id} has no campaign owner")
        phases = {campaign_phase(item_id) for item_id in owning_items}
        if len(phases) != 1:
            fail(
                f"accepted product gap {gap_id} has multiple campaign phase owners: "
                f"{sorted(phases)}"
            )
        if classifications[gap_id] == "external_evidence_pending":
            continue
        source_items = [
            item_id
            for item_id in owning_items
            if items[item_id].get("schedule_class") == SOURCE_ITEM_SCHEDULE
        ]
        if not source_items:
            fail(f"accepted source gap {gap_id} has no pre-stabilization owner")
        missing_inventory = sorted(
            item_id for item_id in source_items if item_id not in records_by_item
        )
        if missing_inventory:
            fail(
                f"accepted source gap {gap_id} has campaign owners without inventory "
                f"dispositions: {missing_inventory}"
            )

    return records


def capability_matrix_entries(value: Any) -> list[dict[str, Any]]:
    candidates: list[list[dict[str, Any]]] = []

    def visit(current: Any) -> None:
        if isinstance(current, list):
            if current and all(
                isinstance(entry, dict) and isinstance(entry.get("host_kind"), str)
                for entry in current
            ):
                candidates.append(current)
            for entry in current:
                visit(entry)
        elif isinstance(current, dict):
            for entry in current.values():
                visit(entry)

    visit(value)
    if not candidates:
        return []
    longest = max(len(candidate) for candidate in candidates)
    largest = [candidate for candidate in candidates if len(candidate) == longest]
    if len(largest) != 1:
        fail("supported-host capability artifact has ambiguous host entry lists")
    return largest[0]


def nonempty_field(entry: dict[str, Any], field: str, label: str) -> Any:
    if field not in entry:
        fail(f"{label} omits required field {field!r}")
    value = entry[field]
    if value is None or value == "" or value == [] or value == {}:
        fail(f"{label} has empty required field {field!r}")
    return value


def exact_version(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        fail(f"{label} must be a non-empty exact version string")
    normalized = value.strip().lower()
    inexact = {"*", "any", "latest", "current", "unversioned", "unknown", "all"}
    if normalized in inexact or any(marker in normalized for marker in (">", "<", "^", "~")):
        fail(f"{label} is not an exact version: {value!r}")
    return value.strip()


def digest_value(value: Any, label: str) -> None:
    if not isinstance(value, str) or not re.fullmatch(
        r"(?:sha256:)?[0-9a-f]{64}", value
    ):
        fail(f"{label} must be a lowercase sha256 digest")


def validate_installability(value: Any, label: str) -> None:
    allowed_states = {
        "supported",
        "unsupported",
        "candidate",
        "not_installable",
        "source_only",
    }
    if isinstance(value, str):
        if value not in allowed_states:
            fail(
                f"{label} must be an explicit installability state, not recognition "
                f"metadata: {value!r}"
            )
        return
    installability = require_mapping(value, label)
    state = installability.get("state")
    if state not in allowed_states:
        fail(f"{label}.state must be one of {sorted(allowed_states)}, found {state!r}")
    if state in {"supported", "candidate", "source_only"} and not any(
        installability.get(field)
        for field in ("asset_refs", "installer_ref", "source_ref", "package_ref")
    ):
        fail(f"{label} lacks an installation asset or source reference")


def check_c4_2_support_projection(records: dict[str, dict[str, Any]]) -> None:
    record = records.get("C4.2.work.1")
    if record is None:
        fail("story inventory omits C4.2 supported-host capability matrix obligation")
    if record.get("source_complete") is not True:
        return

    checkpoint = require_mapping(record.get("checkpoint"), "C4.2.work.1.checkpoint")
    authority_ref = checkpoint.get("authority_ref")
    if not isinstance(authority_ref, str) or not authority_ref:
        fail("source-complete C4.2 lacks a typed capability-matrix authority_ref")
    authority_name = authority_ref.split("#", 1)[0]
    authority_relative = Path(authority_name)
    if authority_relative.is_absolute() or ".." in authority_relative.parts:
        fail("C4.2 capability matrix authority_ref must stay inside the repository")
    authority_path = ROOT / authority_relative
    forbidden_authorities = {
        INVENTORY_PATH,
        CAMPAIGN_PATH,
        ROOT / "contracts/plan/product-gap-closure-plan.yaml",
        BACKLOG_PATH,
        ROOT / "contracts/spec/host-origin-broker-conformance-v0.yaml",
    }
    if authority_path in forbidden_authorities:
        fail(
            "C4.2 cannot use a plan, backlog, inventory, or schema as its completed "
            "supported-host matrix"
        )
    if authority_path.suffix.lower() not in {".yaml", ".yml", ".json"}:
        fail("C4.2 capability matrix must be typed YAML or JSON authority")
    if not authority_path.is_file() or authority_path.is_symlink():
        fail(f"C4.2 capability matrix is not a regular file: {authority_ref!r}")

    matrix = load_structured(authority_path)
    entries = capability_matrix_entries(matrix)
    if not entries:
        document = require_mapping(matrix, "C4.2 candidate capability matrix")
        candidate = require_mapping(
            document.get("host_support_matrix"),
            "C4.2 candidate capability matrix.host_support_matrix",
        )
        serialization_boundary = require_mapping(
            candidate.get("serialization_boundary"),
            "C4.2 candidate capability matrix.serialization_boundary",
        )
        authority_flags = {
            "grants_support_authority",
            "grants_release_authority",
            "grants_install_authority",
            "grants_mutation_authority",
            "grants_signing_authority",
            "grants_trust_authority",
            "grants_private_key_authority",
            "grants_host_selection_authority",
        }
        if (
            document.get("schema_version") != "0.1"
            or candidate.get("authority") != "candidate_only"
            or candidate.get("selected_host") is not None
            or candidate.get("records") != []
            or set(serialization_boundary) != authority_flags
            or any(serialization_boundary.values())
        ):
            fail(
                "empty C4.2 matrix must remain a closed candidate-only projection "
                "with selected_host null and no authority grants"
            )
        codex = records.get("GAP-010.codex-conformance")
        codex_checkpoint = require_mapping(
            codex.get("checkpoint") if codex is not None else None,
            "GAP-010.codex-conformance.checkpoint",
        )
        if (
            codex is None
            or codex.get("status") != "blocked"
            or codex.get("source_complete") is not False
            or codex_checkpoint.get("state") != "blocked_on_exact_host_execution"
        ):
            fail(
                "empty C4.2 matrix is valid only while exact-host conformance remains "
                "explicitly blocked and unsupported"
            )
        return

    seen_coordinates: set[tuple[str, str]] = set()
    codex_result: dict[str, Any] | None = None
    for index, entry in enumerate(entries):
        label = f"{relative(authority_path)} host entry {index}"
        missing = sorted(HOST_CAPABILITY_RESULT_FIELDS - set(entry))
        if missing:
            fail(f"{label} omits exact conformance result fields: {missing}")
        projection_missing = sorted(HOST_CAPABILITY_PROJECTION_FIELDS - set(entry))
        if projection_missing:
            fail(f"{label} omits support projection fields: {projection_missing}")
        recognition_fields = {
            "manifest_recognition", "manifest_recognized"
        } & set(entry)
        if not recognition_fields:
            fail(f"{label} omits explicit manifest recognition state")
        recognition_field = sorted(recognition_fields)[0]
        if entry[recognition_field] is None or entry[recognition_field] == "":
            fail(f"{label}.{recognition_field} must state recognition explicitly")
        for projection_field in (
            "read_only_mcp",
            "human_origin_assurance",
            "governed_mutation",
        ):
            if entry[projection_field] in (None, "", [], {}):
                fail(f"{label}.{projection_field} must state capability explicitly")
        if entry["field_evidence"] is None:
            fail(f"{label}.field_evidence must state evidence or explicit absence")

        host_kind = nonempty_field(entry, "host_kind", label)
        if not isinstance(host_kind, str):
            fail(f"{label}.host_kind must be a string")
        host_version = exact_version(
            nonempty_field(entry, "exact_host_version", label),
            f"{label}.exact_host_version",
        )
        exact_version(
            nonempty_field(entry, "exact_adapter_version", label),
            f"{label}.exact_adapter_version",
        )
        adapter_id = nonempty_field(entry, "adapter_id", label)
        if not isinstance(adapter_id, str):
            fail(f"{label}.adapter_id must be a string")
        source_commit = nonempty_field(entry, "source_commit", label)
        if not isinstance(source_commit, str) or not re.fullmatch(
            r"[0-9a-f]{40}", source_commit
        ):
            fail(f"{label}.source_commit must be 40 lowercase hexadecimal characters")

        conformance_state = nonempty_field(entry, "conformance_state", label)
        if conformance_state not in HOST_CONFORMANCE_STATES:
            fail(
                f"{label}.conformance_state must be one of "
                f"{sorted(HOST_CONFORMANCE_STATES)}, found {conformance_state!r}"
            )
        digest_value(
            nonempty_field(entry, "case_inventory_digest", label),
            f"{label}.case_inventory_digest",
        )
        digest_value(
            nonempty_field(entry, "result_bundle_digest", label),
            f"{label}.result_bundle_digest",
        )
        nonempty_field(entry, "residual_limitations", label)
        validate_installability(entry["installability"], f"{label}.installability")

        coordinate = (host_kind.lower(), host_version)
        if coordinate in seen_coordinates:
            fail(f"C4.2 capability matrix duplicates exact host coordinate {coordinate}")
        seen_coordinates.add(coordinate)
        if coordinate == ("codex", "0.143.0"):
            codex_result = entry

    if codex_result is None:
        fail("C4.2 capability matrix omits the adjudicated Codex 0.143.0 result")
    if codex_result.get("conformance_state") != "unsupported":
        fail("Codex 0.143.0 must remain unsupported until a new exact-version result exists")
    failed_capabilities = codex_result.get("failed_capabilities")
    if (
        not isinstance(failed_capabilities, list)
        or not failed_capabilities
        or not all(isinstance(value, str) and value for value in failed_capabilities)
    ):
        fail("unsupported Codex 0.143.0 result must name failed capabilities")


def check_backup_restore_projection(
    records: dict[str, dict[str, Any]], canonical: set[tuple[str, ...]]
) -> None:
    continuity_records = [
        record
        for record in records.values()
        if record.get("campaign_item") == "C2.2"
        and record.get("schedule_class") == SOURCE_STORY_SCHEDULE
    ]
    if not continuity_records:
        fail("story inventory contains no C2.2 backup/restore source records")
    source_complete = all(record.get("source_complete") is True for record in continuity_records)

    generated_projection = BACKUP_RESTORE_COMMAND_KEYS & canonical
    if not source_complete:
        if generated_projection:
            fail(
                "generated command surface publicly projects backup/restore before "
                "the complete C2.2 source inventory is source-complete: "
                f"{sorted(' '.join(key) for key in generated_projection)}"
            )
        return

    missing_generated = sorted(BACKUP_RESTORE_COMMAND_KEYS - canonical)
    if missing_generated:
        fail(
            "source-complete C2.2 is missing generated public command projection: "
            f"{[' '.join(key) for key in missing_generated]}"
        )

    command_source = (
        ROOT / "crates/forge-core-command-surface/src/lib.rs"
    ).read_text(encoding="utf-8")
    source_commands = {
        command_key(command)
        for command in re.findall(r'"\s*(forge-core [^"\n]+)"', command_source)
    }
    missing_source = sorted(BACKUP_RESTORE_COMMAND_KEYS - source_commands)
    if missing_source:
        fail(
            "source-complete C2.2 lacks canonical command metadata: "
            f"{[' '.join(key) for key in missing_source]}"
        )

    public_paths = [
        ROOT / "docs/getting-started.md",
        ROOT / "docs/operator-guide.md",
        GAP_REGISTER_PATH,
    ]
    public_text = "\n".join(path.read_text(encoding="utf-8") for path in public_paths)
    normalized = " ".join(public_text.lower().split())
    stale_claims = [
        "restore_verified_backup is deferred",
        "deferred until the complete-state verified restore protocol ships",
        "does not yet ship the required complete-state backup/verify/restore",
        "complete-state backup/verify/restore or durable reinitialize plan/apply operation",
        "complete-state backup, verification, restore, and durable reinitialize-as-new apply are not product-owned",
    ]
    present_stale_claims = [claim for claim in stale_claims if claim in normalized]
    if present_stale_claims:
        fail(
            "source-complete C2.2 conflicts with stale public backup/restore claims: "
            f"{present_stale_claims}"
        )
    missing_documented = sorted(
        " ".join(key)
        for key in BACKUP_RESTORE_COMMAND_KEYS
        if " ".join(key) not in public_text
    )
    if missing_documented:
        fail(
            "source-complete C2.2 lacks public backup/restore command documentation: "
            f"{missing_documented}"
        )


def check_payload() -> None:
    manifest = ROOT / "distribution/release-payload.txt"
    entries = [
        line.strip()
        for line in manifest.read_text(encoding="utf-8").splitlines()
        if line.strip() and not line.lstrip().startswith("#")
    ]
    if len(entries) != len(set(entries)):
        fail("distribution/release-payload.txt contains duplicate entries")
    for entry in entries:
        path = ROOT / entry
        if not path.is_file() or path.is_symlink():
            fail(f"payload entry is not a regular in-repository file: {entry}")
    required = {
        "skill/start-forge/SKILL.md",
        "docs/getting-started.md",
        "docs/operator-guide.md",
        "docs/real-host-proof.md",
        "contracts/spec/real-host-evidence-bundle-v0.yaml",
        "contracts/spec/domain-pack-rebase-v0.yaml",
        "docs/product-gap-register.md",
        "contracts/spec/C1.1-codex-host-capability-decision.yaml",
        "contracts/spec/C1.1-pi-host-capability-decision.yaml",
        "contracts/spec/C1.1-opencode-host-capability-decision.yaml",
        "contracts/spec/host-origin-broker-conformance-v0.yaml",
        "contracts/spec/workflow-host-origin-adapter-v0.yaml",
        "contracts/spec/workflow-action-origin-broker-v0.yaml",
        "contracts/plan/product-gap-closure-plan.yaml",
        "scripts/check-real-host-evidence.py",
    }
    missing = required - set(entries)
    if missing:
        fail(f"release payload omits promised files: {sorted(missing)}")


def check_gap_plan_coverage() -> None:
    register = (ROOT / "docs/product-gap-register.md").read_text(encoding="utf-8")
    plan = (ROOT / "contracts/plan/product-gap-closure-plan.yaml").read_text(
        encoding="utf-8"
    )
    registered = set(re.findall(r"\*\*(GAP-\d{3})\*\*", register))
    planned_refs = re.findall(r'"(GAP-\d{3})"', plan)
    if not registered:
        fail("product gap register contains no canonical gap ids")
    counts = Counter(planned_refs)
    duplicate_refs = sorted(gap_id for gap_id, count in counts.items() if count != 1)
    if duplicate_refs:
        fail(f"closure plan must own every gap exactly once: {duplicate_refs}")
    planned = set(planned_refs)
    if registered != planned:
        fail(
            "gap register/closure plan mismatch; "
            f"unplanned={sorted(registered - planned)}, "
            f"unknown={sorted(planned - registered)}"
        )


def check_release_locking() -> None:
    checker_path = ROOT / "scripts/check-release-locking.py"
    spec = importlib.util.spec_from_file_location(
        "forge_release_lock_checker", checker_path
    )
    if spec is None or spec.loader is None:
        fail(f"cannot load release lock checker {checker_path}")
    checker = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(checker)
    try:
        checker.check(ROOT / ".github/workflows/release.yml")
    except checker.ReleaseLockError as error:
        fail(str(error))


def main() -> int:
    cargo = tomllib.loads((ROOT / "Cargo.toml").read_text(encoding="utf-8"))
    version = cargo["workspace"]["package"]["version"]
    readme = (ROOT / "README.md").read_text(encoding="utf-8")
    if len(readme.splitlines()) > 180:
        fail("README exceeds the concise landing-page limit of 180 lines")
    required_readme = [
        f"`{version}`",
        "Latest prebuilt",
        "Workflow release identity",
        "Domain Pack effective epoch",
        "Forge-mediated",
    ]
    for marker in required_readme:
        if marker not in readme:
            fail(f"README omits canonical boundary marker {marker!r}")

    status = (ROOT / "docs/product-status.md").read_text(encoding="utf-8")
    audit = (ROOT / "docs/product-compliance-audit.md").read_text(encoding="utf-8")
    for marker in ("source checkpoint", "prebuilt", "workflow release", "effective epoch"):
        if marker.lower() not in status.lower():
            fail(f"product status omits identity category {marker!r}")
    for marker in ("Mediated writes", "SBOM", "host", "independence", "prebuilt"):
        if marker.lower() not in audit.lower():
            fail(f"promise matrix omits evidence category {marker!r}")

    canonical = canonical_command_keys()
    for path, line, key in documented_command_keys():
        if key not in canonical:
            relative_path = path.relative_to(ROOT)
            fail(
                f"{relative_path}:{line} documents unknown command path "
                f"{' '.join(key)!r}"
            )

    records = check_public_obligation_ownership()
    check_c4_2_support_projection(records)
    check_backup_restore_projection(records, canonical)
    check_release_locking()
    check_gap_plan_coverage()
    check_payload()
    print("Public promise audit: clean")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, tomllib.TOMLDecodeError) as error:
        fail(str(error))
