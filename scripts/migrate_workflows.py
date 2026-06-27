#!/usr/bin/env python3
"""Deterministic migration: forge-method markdown workflows -> typed YAML.

S1.3 story. Converts the 110 workflow-*.md files from the Python v2 repo into
WorkflowDocument-shaped YAML that deserializes into the Rust
`forge_core_contracts::WorkflowDocument` type (deny_unknown_fields).

The .md files are NOT valid machine-YAML: list items use markdown ordered-list
syntax ("1. foo") which YAML collapses into a single scalar string. This script
normalizes that ("N. " -> "- ") so every field becomes a proper sequence.

Deterministic + reproducible: same input always yields byte-identical output.
This is MORE consistent than LLM ad-hoc conversion, so no fan-out is used here.

Usage:
    python3 scripts/migrate_workflows.py [--src DIR] [--dst DIR] [--report PATH]
"""
from __future__ import annotations

import argparse
import os
import re
import sys
from collections import Counter
from pathlib import Path

import yaml

SCHEMA_VERSION = "0.1"
CANONICAL_FIELDS = [
    "trigger", "inputs", "steps", "outputs",
    "done_when", "blocked_when", "handoff",
]

ID_HEADER_RE = re.compile(r"^#\s*workflow:\s*(\S+)\s*$")
ORDERED_LIST_RE = re.compile(r"^( +)\d+\.\s+", re.MULTILINE)


class QuotedDumper(yaml.SafeDumper):
    """Force double-quoted string VALUES so Vec<String> items are unambiguous
    scalars (never mis-parsed as maps by a stray colon). Keys stay plain via
    post-processing."""


def _str_representer(dumper, data):
    return dumper.represent_scalar("tag:yaml.org,2002:str", data, style='"')


QuotedDumper.add_representer(str, _str_representer)


def strip_quotes_from_keys(text: str) -> str:
    """QuotedDumper quotes both keys and values; schema keys are known-safe
    identifiers, so unquote the keys for readability."""
    return re.sub(r'^(\s*)"([a-z_]+)":', r"\1\2:", text, flags=re.MULTILINE)


def extract_id(first_line: str, path: Path) -> str:
    m = ID_HEADER_RE.match(first_line.strip())
    if not m:
        raise ValueError(f"{path}: first line is not '# workflow: <id>': {first_line!r}")
    return m.group(1)


def parse_workflow_body(body: str) -> dict:
    """Normalize markdown ordered lists to YAML sequences, then safe_load.
    Backticks (markdown inline code) are stripped: they wrap paths/identifiers
    whose content is what the typed contract carries; the formatting is noise."""
    normalized = ORDERED_LIST_RE.sub(lambda m: m.group(1) + "- ", body)
    normalized = normalized.replace("`", "")
    data = yaml.safe_load(normalized)
    if not isinstance(data, dict):
        raise ValueError(f"body did not parse to a mapping: {type(data)}")
    return data


def build_document(wf_id: str, fields: dict, phases: list[str]) -> tuple[dict, list[str]]:
    """Return (document_dict, dropped_keys). Keeps only CANONICAL_FIELDS;
    every kept field is coerced to a list of strings. `phases` is injected
    from the authoritative catalog (S1.5)."""
    dropped = []
    doc_fields = {"id": wf_id, "phases": phases}
    for key in CANONICAL_FIELDS:
        value = fields.get(key, [])
        if value is None:
            value = []
        if isinstance(value, str):
            value = [value]
        if not isinstance(value, list):
            raise ValueError(f"field {key!r} is {type(value).__name__}, expected list")
        doc_fields[key] = [str(item) for item in value]
    for key in fields:
        if key not in CANONICAL_FIELDS:
            dropped.append(key)
    return {"schema_version": SCHEMA_VERSION, "workflow": doc_fields}, dropped


def load_phase_map(catalog_path: Path) -> dict[str, list[str]]:
    """Load id -> [phase tags] from the authoritative workflow catalog.
    The catalog stores phase as a pipe-separated string
    (e.g. '1-discovery | 2-specification | 3-plan'); split into a list."""
    import json
    data = json.loads(catalog_path.read_text(encoding="utf-8"))
    out = {}
    for w in data["workflows"]:
        raw = w.get("phase") or ""
        tags = [t.strip() for t in raw.split("|") if t.strip()]
        out[w["id"]] = tags
    return out


def emit_yaml(document: dict) -> str:
    raw = yaml.dump(document, Dumper=QuotedDumper, sort_keys=False, allow_unicode=True, width=1000)
    return strip_quotes_from_keys(raw)


def migrate_one(path: Path, phase_map: dict[str, list[str]]) -> tuple[str, dict, list[str]]:
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    if not lines:
        raise ValueError(f"{path}: empty file")
    wf_id = extract_id(lines[0], path)
    body = "\n".join(lines[1:]).strip()
    fields = parse_workflow_body(body)
    phases = phase_map.get(wf_id, [])
    document, dropped = build_document(wf_id, fields, phases)
    return wf_id, document, dropped


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--src", default="/mnt/c/Forge-method-core/skills/forge-method/references")
    ap.add_argument("--dst", default="/mnt/c/forge-method-rust/contracts/workflows")
    ap.add_argument("--catalog", default="/mnt/c/Forge-method-core/skills/forge-method/catalog/workflows.json")
    ap.add_argument("--report", default="/mnt/c/forge-method-rust/.forge-method/artifacts/S1.3-migration-report.yaml")
    args = ap.parse_args()

    src = Path(args.src)
    dst = Path(args.dst)
    dst.mkdir(parents=True, exist_ok=True)

    md_files = sorted(src.glob("workflow-*.md"))
    if not md_files:
        print(f"ERROR: no workflow-*.md under {src}", file=sys.stderr)
        return 2

    catalog_path = Path(args.catalog)
    phase_map = load_phase_map(catalog_path) if catalog_path.exists() else {}
    if not phase_map:
        print(f"WARNING: catalog not found at {catalog_path}; phases will be empty", file=sys.stderr)

    converted = []
    dropped_counter: Counter = Counter()
    anomalies = []
    ids_seen: set[str] = set()

    for md in md_files:
        try:
            wf_id, document, dropped = migrate_one(md, phase_map)
        except Exception as exc:  # noqa: BLE001 - report and fail loud
            anomalies.append({"file": str(md), "error": str(exc)})
            continue
        if wf_id in ids_seen:
            anomalies.append({"file": str(md), "error": f"duplicate id {wf_id!r}"})
            continue
        ids_seen.add(wf_id)
        for d in dropped:
            dropped_counter[d] += 1
        out_path = dst / f"{wf_id}.yaml"
        out_path.write_text(emit_yaml(document), encoding="utf-8")
        converted.append({"id": wf_id, "file": out_path.name, "dropped": dropped})

    report = {
        "schema_version": SCHEMA_VERSION,
        "artifact_kind": "migration-report",
        "story_id": "S1.3",
        "src": str(src),
        "dst": str(dst),
        "total_md_files": len(md_files),
        "converted_count": len(converted),
        "anomaly_count": len(anomalies),
        "dropped_fields_summary": dict(dropped_counter),
        "dropped_fields_note": (
            "top-level keys present in source but NOT in the Workflow schema. "
            "'state' on workflow-start-runtime is runtime routing config, not a "
            "workflow contract field -> open_question for the engine."
        ),
        "converted": converted,
        "anomalies": anomalies,
    }
    Path(args.report).parent.mkdir(parents=True, exist_ok=True)
    Path(args.report).write_text(
        yaml.dump(report, sort_keys=False, allow_unicode=True, width=1000),
        encoding="utf-8",
    )

    print(f"converted: {len(converted)}/{len(md_files)}")
    print(f"anomalies: {len(anomalies)}")
    print(f"dropped fields: {dict(dropped_counter)}")
    print(f"report: {args.report}")
    return 0 if not anomalies else 1


if __name__ == "__main__":
    sys.exit(main())
