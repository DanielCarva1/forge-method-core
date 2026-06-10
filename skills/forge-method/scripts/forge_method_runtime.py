#!/usr/bin/env python3
"""File-backed runtime helper for Forge Method."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import shutil
import sys
from pathlib import Path
from typing import Any


RUNTIME_NAME = "forge-method"
RUNTIME_REPO_NAME = "forge-method-core"
RUNTIME_VERSION = "1.5.0"
SKILL_DIR = Path(__file__).resolve().parents[1]
PROJECT_TEMPLATE_DIR = SKILL_DIR / "assets" / "project"

STATE_DIR = ".forge-method"
STATE_FILE = "state.yaml"
PROJECTS_FILE = "projects.yaml"
SPRINT_FILE = "sprint.yaml"
LEDGER_FILE = "ledger.ndjson"

PHASES = [
    "0-route",
    "1-discovery",
    "2-specification",
    "3-plan",
    "4-build-verify",
    "5-ready-operate",
    "6-evolve",
]

PHASE_TRANSITIONS = {
    "0-route": {"1-discovery"},
    "1-discovery": {"2-specification"},
    "2-specification": {"3-plan"},
    "3-plan": {"4-build-verify"},
    "4-build-verify": {"5-ready-operate"},
    "5-ready-operate": {"6-evolve"},
    "6-evolve": {"1-discovery", "2-specification", "3-plan", "4-build-verify"},
}

STORY_STATUSES = [
    "planned",
    "ready",
    "in_progress",
    "review",
    "done",
    "blocked",
    "deferred",
]

STORY_TRANSITIONS = {
    "planned": {"ready", "in_progress", "blocked", "deferred"},
    "ready": {"in_progress", "blocked", "deferred"},
    "in_progress": {"review", "done", "blocked"},
    "review": {"in_progress", "done", "blocked"},
    "blocked": {"ready", "in_progress", "deferred"},
    "deferred": {"planned", "ready"},
    "done": set(),
}

ARTIFACT_LIFECYCLES = ["durable", "ephemeral"]

WORKFLOW_REQUIRED_SECTIONS = [
    "trigger:",
    "inputs:",
    "steps:",
    "outputs:",
    "done_when:",
    "blocked_when:",
    "handoff:",
]

SCAN_SKIP_DIRS = {
    ".git",
    ".hg",
    ".svn",
    ".next",
    ".pytest_cache",
    ".ruff_cache",
    "__pycache__",
    "build",
    "dist",
    "node_modules",
    "venv",
    ".venv",
}

NEXT_BY_PHASE = {
    "0-route": "resolve project route and confirm whether this is a new or existing project",
    "1-discovery": "run discovery and capture intent, constraints, and success criteria",
    "2-specification": "write requirements, acceptance criteria, and product constraints",
    "3-plan": "create architecture notes, story plan, risk plan, and validation plan",
    "4-build-verify": "select next ready story, implement, validate, review, and write evidence",
    "5-ready-operate": "use, support, observe, and maintain the ready product",
    "6-evolve": "start the next version cycle from feedback, defects, or new intent",
}


def utc_now() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat()


def slugify(value: str) -> str:
    value = value.strip().lower()
    value = re.sub(r"[^a-z0-9]+", "-", value)
    value = re.sub(r"-+", "-", value).strip("-")
    return value or "item"


def split_list(value: str | None) -> list[str]:
    if not value:
        return []
    return [item.strip() for item in value.split(" | ") if item.strip()]


def join_list(values: list[str]) -> str:
    return " | ".join(value.strip().replace("\n", " ") for value in values if value.strip())


def quote_yaml(value: Any) -> str:
    if isinstance(value, bool):
        return "true" if value else "false"
    if value is None:
        return '""'
    text = str(value).replace("\\", "\\\\").replace('"', '\\"').replace("\n", " ").strip()
    return f'"{text}"'


def read_flat_yaml(path: Path) -> dict[str, str]:
    values: dict[str, str] = {}
    if not path.exists():
        return values
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or ":" not in line:
            continue
        key, value = line.split(":", 1)
        value = value.strip()
        if value.startswith('"') and value.endswith('"'):
            value = value[1:-1].replace('\\"', '"').replace("\\\\", "\\")
        values[key.strip()] = value
    return values


def write_flat_yaml(path: Path, values: dict[str, Any], *, header: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [f"# {header}", f"updated_at: {quote_yaml(utc_now())}"]
    for key, value in values.items():
        if key == "updated_at":
            continue
        lines.append(f"{key}: {quote_yaml(value)}")
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def resolve_root(raw_root: str) -> Path:
    return Path(raw_root).expanduser().resolve()


def project_path(root: Path, raw_path: str) -> tuple[Path, str]:
    candidate = Path(raw_path).expanduser()
    if not candidate.is_absolute():
        candidate = root / candidate
    resolved = candidate.resolve()
    try:
        rel = resolved.relative_to(root.resolve()).as_posix()
    except ValueError as exc:
        raise SystemExit(f"Path must stay inside project root: {raw_path}") from exc
    return resolved, rel


def method_dir(root: Path) -> Path:
    return root / STATE_DIR


def state_path(root: Path) -> Path:
    return method_dir(root) / STATE_FILE


def find_state_root(start: Path) -> Path | None:
    current = start.resolve()
    for candidate in [current, *current.parents]:
        if state_path(candidate).exists():
            return candidate
    return None


def is_runtime_repo(root: Path) -> bool:
    manifest = root / ".codex-plugin" / "plugin.json"
    if not manifest.exists():
        return False
    try:
        payload = json.loads(manifest.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return False
    return payload.get("name") == RUNTIME_REPO_NAME


def ensure_dirs(root: Path) -> Path:
    fm = method_dir(root)
    for name in [
        "artifacts",
        "checkpoints",
        "context",
        "evals",
        "evidence",
        "handoffs",
        "modules",
        "stories",
        "workflows",
    ]:
        (fm / name).mkdir(parents=True, exist_ok=True)
    return fm


def copy_project_guidance(root: Path, *, force: bool = False) -> list[str]:
    copied: list[str] = []
    if not PROJECT_TEMPLATE_DIR.exists():
        return copied
    for source in PROJECT_TEMPLATE_DIR.rglob("*"):
        if source.is_dir():
            continue
        rel = source.relative_to(PROJECT_TEMPLATE_DIR)
        target = root / rel
        if target.exists() and not force:
            continue
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, target)
        copied.append(target.relative_to(root).as_posix())
    return copied


def load_state_or_none(root: Path) -> tuple[Path | None, dict[str, str]]:
    state_root = find_state_root(root)
    if state_root is None:
        return None, {}
    return state_root, read_flat_yaml(state_path(state_root))


def load_state_or_fail(root: Path) -> tuple[Path, dict[str, str]]:
    state_root, state = load_state_or_none(root)
    if state_root is None:
        if is_runtime_repo(root):
            raise SystemExit("Runtime repo detected. No project state found here.")
        raise SystemExit("No .forge-method/state.yaml found. Run init first.")
    return state_root, state


def discover_project_roots(root: Path, *, max_depth: int = 2) -> list[Path]:
    root = root.resolve()
    max_depth = max(0, min(max_depth, 5))
    found: list[Path] = []
    queue: list[tuple[Path, int]] = [(root, 0)]
    seen: set[Path] = set()
    while queue:
        current, depth = queue.pop(0)
        if current in seen:
            continue
        seen.add(current)
        if state_path(current).exists():
            found.append(current)
            continue
        if depth >= max_depth:
            continue
        try:
            children = sorted(current.iterdir(), key=lambda path: path.name.lower())
        except OSError:
            continue
        for child in children:
            if not child.is_dir():
                continue
            if child.is_symlink():
                continue
            if child.name in SCAN_SKIP_DIRS:
                continue
            queue.append((child, depth + 1))
    return found


def display_path(path: Path, *, base: Path) -> str:
    try:
        return path.relative_to(base).as_posix() or "."
    except ValueError:
        return str(path)


def command_hint_value(value: str | Path) -> str:
    text = str(value).replace('"', '\\"')
    return f'"{text}"'


def print_state_summary(state: dict[str, str]) -> None:
    print(f"Project: {state.get('project', '<unknown>')}")
    print(f"Phase: {state.get('phase', '<unknown>')}")
    print(f"Status: {state.get('status', '<unknown>')}")
    print(f"Workflow: {state.get('active_workflow', '<none>')}")
    print(f"Active story: {state.get('active_story', '') or '<none>'}")
    print(f"Human input required: {state.get('human_input_required', 'unknown')}")
    print(f"Readiness: {state.get('readiness', 'unknown')}")
    print(f"Next: {state.get('next_action', NEXT_BY_PHASE.get(state.get('phase', ''), 'inspect state'))}")


def write_state(root: Path, state: dict[str, Any]) -> None:
    state.setdefault("schema_version", "1")
    state.setdefault("runtime", RUNTIME_NAME)
    state.setdefault("runtime_version", RUNTIME_VERSION)
    write_flat_yaml(state_path(root), state, header="Forge Method state")


def ledger_path(root: Path) -> Path:
    return method_dir(root) / LEDGER_FILE


def append_ledger(root: Path, event: str, payload: dict[str, Any] | None = None) -> None:
    ensure_dirs(root)
    entry = {
        "ts": utc_now(),
        "event": event,
        "payload": payload or {},
    }
    with ledger_path(root).open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(entry, ensure_ascii=True, sort_keys=True) + "\n")


def story_path(root: Path, story_id: str) -> Path:
    return method_dir(root) / "stories" / f"{slugify(story_id)}.yaml"


def load_story(root: Path, story_id: str) -> dict[str, str]:
    path = story_path(root, story_id)
    if not path.exists():
        raise SystemExit(f"Story not found: {story_id}")
    return read_flat_yaml(path)


def save_story(root: Path, story: dict[str, Any]) -> None:
    story_id = story.get("id")
    if not story_id:
        raise SystemExit("Story must have an id.")
    write_flat_yaml(story_path(root, str(story_id)), story, header="Forge Method story")


def list_stories(root: Path) -> list[dict[str, str]]:
    stories_dir = method_dir(root) / "stories"
    if not stories_dir.exists():
        return []
    stories = [read_flat_yaml(path) for path in sorted(stories_dir.glob("*.yaml"))]
    return [story for story in stories if story.get("id")]


def update_sprint(root: Path) -> None:
    stories = list_stories(root)
    counts = {status: 0 for status in STORY_STATUSES}
    for story in stories:
        status = story.get("status", "planned")
        counts[status] = counts.get(status, 0) + 1
    state = read_flat_yaml(state_path(root))
    values: dict[str, Any] = {
        "active_story": state.get("active_story", ""),
        "story_count": str(len(stories)),
        "planned_count": str(counts.get("planned", 0)),
        "ready_count": str(counts.get("ready", 0)),
        "in_progress_count": str(counts.get("in_progress", 0)),
        "review_count": str(counts.get("review", 0)),
        "done_count": str(counts.get("done", 0)),
        "blocked_count": str(counts.get("blocked", 0)),
        "deferred_count": str(counts.get("deferred", 0)),
    }
    write_flat_yaml(method_dir(root) / SPRINT_FILE, values, header="Forge Method sprint state")


def select_next_story(root: Path) -> dict[str, str] | None:
    stories = list_stories(root)
    for status in ["in_progress", "review", "ready", "planned", "blocked"]:
        for story in stories:
            if story.get("status") == status:
                return story
    return None


def evidence_file(root: Path, kind: str, title: str) -> Path:
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%d-%H%M%S")
    return method_dir(root) / "evidence" / f"{stamp}-{slugify(kind)}-{slugify(title)[:48]}.md"


def artifact_index_path(root: Path) -> Path:
    return method_dir(root) / "artifacts" / "index.ndjson"


def checkpoint_file(root: Path, title: str) -> Path:
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%d-%H%M%S")
    return method_dir(root) / "checkpoints" / f"{stamp}-{slugify(title)[:48]}.md"


def latest_checkpoint_path(root: Path) -> Path:
    return method_dir(root) / "context" / "latest-checkpoint.md"


def artifact_file(root: Path, kind: str, title: str) -> Path:
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%d-%H%M%S")
    return method_dir(root) / "artifacts" / f"{stamp}-{slugify(kind)}-{slugify(title)[:48]}.md"


def append_artifact_index(root: Path, entry: dict[str, Any]) -> None:
    index_path = artifact_index_path(root)
    index_path.parent.mkdir(parents=True, exist_ok=True)
    payload = {"ts": utc_now(), **entry}
    with index_path.open("a", encoding="utf-8") as handle:
        handle.write(json.dumps(payload, ensure_ascii=True, sort_keys=True) + "\n")


def artifact_index_entries(root: Path) -> list[dict[str, Any]]:
    index_path = artifact_index_path(root)
    if not index_path.exists():
        return []
    entries: list[dict[str, Any]] = []
    for line in index_path.read_text(encoding="utf-8").splitlines():
        if not line.strip():
            continue
        try:
            entries.append(json.loads(line))
        except json.JSONDecodeError:
            continue
    return entries


def recent_artifacts(root: Path, limit: int = 5) -> list[dict[str, Any]]:
    return artifact_index_entries(root)[-limit:]


def artifact_states(root: Path) -> dict[str, dict[str, Any]]:
    states: dict[str, dict[str, Any]] = {}
    for entry in artifact_index_entries(root):
        path = str(entry.get("path", ""))
        if not path:
            continue
        current = states.setdefault(path, {"path": path, "status": "active", "lifecycle": "durable"})
        kind = entry.get("kind")
        if kind == "story-link":
            stories = split_list(str(current.get("stories", "")))
            story = str(entry.get("story", ""))
            if story and story not in stories:
                stories.append(story)
            current["stories"] = join_list(stories)
            current["last_linked_at"] = entry.get("ts", "")
            continue
        current.update({key: value for key, value in entry.items() if value not in {"", None}})
        current.setdefault("status", "active")
        current.setdefault("lifecycle", "durable")
    return states


def artifact_state(root: Path, path: str) -> dict[str, Any]:
    return artifact_states(root).get(path, {"path": path, "status": "unknown", "lifecycle": "durable"})


def artifact_missing_allowed(root: Path, path: str) -> bool:
    return artifact_state(root, path).get("status") == "captured"


def artifact_summaries(root: Path) -> dict[str, str]:
    summaries: dict[str, str] = {}
    for path, state in artifact_states(root).items():
        summary = str(state.get("summary", ""))
        if summary:
            summaries[path] = summary
    return summaries


def parse_timestamp(value: str) -> dt.datetime | None:
    if not value:
        return None
    try:
        parsed = dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return None
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    return parsed.astimezone(dt.timezone.utc)


def artifact_findings(root: Path) -> tuple[list[str], list[str]]:
    errors: list[str] = []
    warnings: list[str] = []
    for path, state in artifact_states(root).items():
        status = str(state.get("status", "active"))
        if status == "captured":
            continue
        target = root / path
        if not target.exists():
            errors.append(f"missing active artifact: {path}")
            continue
        indexed_at = parse_timestamp(str(state.get("ts", "")))
        if indexed_at:
            modified_at = dt.datetime.fromtimestamp(target.stat().st_mtime, tz=dt.timezone.utc)
            if modified_at > indexed_at + dt.timedelta(seconds=1):
                warnings.append(f"artifact summary may be stale: {path}")
    return errors, warnings


def module_manifest_paths(root: Path | None = None) -> list[Path]:
    paths: list[Path] = []
    skill_modules = SKILL_DIR / "modules"
    if skill_modules.exists():
        paths.extend(sorted(skill_modules.glob("*.yaml")))
    if root is not None:
        project_modules = method_dir(root) / "modules"
        if project_modules.exists():
            paths.extend(sorted(project_modules.glob("*.yaml")))
    return paths


def reference_workflow_paths(root: Path | None = None) -> list[Path]:
    paths: list[Path] = []
    refs = SKILL_DIR / "references"
    if refs.exists():
        paths.extend(sorted(refs.glob("workflow-*.md")))
    if root is not None:
        project_workflows = method_dir(root) / "workflows"
        if project_workflows.exists():
            paths.extend(sorted(project_workflows.glob("workflow-*.md")))
    return paths


def workflow_id_from_path(path: Path) -> str:
    stem = path.stem
    if stem.startswith("workflow-"):
        return stem.removeprefix("workflow-")
    return stem


def workflow_path_by_id(root: Path | None, workflow_id: str) -> Path | None:
    normalized = slugify(workflow_id)
    for path in reference_workflow_paths(root):
        if workflow_id_from_path(path) == normalized:
            return path
    return None


def validate_workflow_file(path: Path) -> list[str]:
    if not path.exists():
        return [f"missing workflow file: {path}"]
    text = path.read_text(encoding="utf-8")
    errors = []
    for section in WORKFLOW_REQUIRED_SECTIONS:
        if section not in text:
            errors.append(f"{path.name}: missing section `{section}`")
    if len(text.splitlines()) > 120:
        errors.append(f"{path.name}: too long for an agent-facing workflow")
    return errors


def workflow_text(
    *,
    workflow_id: str,
    title: str,
    triggers: list[str],
    inputs: list[str],
    steps: list[str],
    outputs: list[str],
    done_when: list[str],
    blocked_when: list[str],
    handoff: list[str],
) -> str:
    def section(name: str, values: list[str]) -> list[str]:
        lines = [f"{name}:"]
        lines.extend(f"  - {value}" for value in values)
        return lines

    lines = [
        f"# workflow: {slugify(workflow_id)}",
        "",
        f"title: {title}",
        "",
        *section("trigger", triggers or ["state requires this workflow"]),
        "",
        *section("inputs", inputs or ["current state"]),
        "",
        *section("steps", steps or ["inspect state", "perform scoped work", "update state"]),
        "",
        *section("outputs", outputs or ["updated artifact or state"]),
        "",
        *section("done_when", done_when or ["output exists", "state is updated", "next action is known"]),
        "",
        *section("blocked_when", blocked_when or ["required input is missing", "state is contradictory"]),
        "",
        *section("handoff", handoff or ["preserve current state, outputs, blockers, and next action"]),
        "",
    ]
    return "\n".join(lines)


def eval_path(root: Path, eval_id: str) -> Path:
    return method_dir(root) / "evals" / f"{slugify(eval_id)}.yaml"


def list_evals(root: Path) -> list[dict[str, str]]:
    evals_dir = method_dir(root) / "evals"
    if not evals_dir.exists():
        return []
    return [read_flat_yaml(path) for path in sorted(evals_dir.glob("*.yaml"))]


def write_evidence(root: Path, *, kind: str, title: str, summary: str, story_id: str = "", checks: list[str] | None = None) -> str:
    path = evidence_file(root, kind, title)
    lines = [
        f"# {title}",
        "",
        f"- kind: {kind}",
        f"- created_at: {utc_now()}",
    ]
    if story_id:
        lines.append(f"- story: {story_id}")
    if checks:
        lines.append(f"- checks: {join_list(checks)}")
    lines.extend(["", "## Summary", "", summary.strip()])
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    rel = path.relative_to(root).as_posix()
    append_ledger(root, "evidence.added", {"kind": kind, "path": rel, "story": story_id})
    return rel


def write_artifact(
    root: Path,
    *,
    kind: str,
    title: str,
    summary: str,
    path: str = "",
    lifecycle: str = "durable",
) -> str:
    if path:
        artifact_path, rel = project_path(root, path)
        artifact_path.parent.mkdir(parents=True, exist_ok=True)
        if not artifact_path.exists():
            artifact_path.write_text(f"# {title}\n\n{summary.strip()}\n", encoding="utf-8")
    else:
        artifact_path = artifact_file(root, kind, title)
        artifact_path.write_text(f"# {title}\n\n{summary.strip()}\n", encoding="utf-8")
        rel = artifact_path.relative_to(root).as_posix()
    append_artifact_index(
        root,
        {
            "kind": kind,
            "title": title,
            "path": rel,
            "summary": summary.strip(),
            "lifecycle": lifecycle,
            "status": "active",
        },
    )
    append_ledger(root, "artifact.added", {"kind": kind, "path": rel, "lifecycle": lifecycle})
    return rel


def capture_artifact(
    root: Path,
    *,
    path: str,
    summary: str,
    story_id: str = "",
    evidence: str = "",
    delete: bool = False,
) -> str:
    artifact_path, rel = project_path(root, path)
    if story_id:
        story = load_story(root, story_id)
        artifacts = split_list(story.get("artifacts"))
        if rel not in artifacts:
            artifacts.append(rel)
            story["artifacts"] = join_list(artifacts)
            save_story(root, story)
    if delete and artifact_path.exists():
        if artifact_path.is_dir():
            raise SystemExit(f"Refusing to delete artifact directory: {rel}")
        artifact_path.unlink()
    append_artifact_index(
        root,
        {
            "kind": "artifact-capture",
            "title": f"Captured {rel}",
            "path": rel,
            "summary": summary.strip(),
            "story": story_id,
            "evidence": evidence,
            "lifecycle": "ephemeral",
            "status": "captured",
            "deleted": "true" if delete else "false",
        },
    )
    append_ledger(root, "artifact.captured", {"path": rel, "story": story_id, "deleted": delete})
    return rel


def append_markdown_list(lines: list[str], title: str, values: list[str]) -> None:
    lines.extend(["", f"## {title}", ""])
    if values:
        lines.extend(f"- {value}" for value in values)
    else:
        lines.append("- none")


def write_checkpoint(
    root: Path,
    state: dict[str, str],
    *,
    title: str,
    summary: str,
    decisions: list[str],
    checks: list[str],
    failed_checks: list[str],
    touched: list[str],
    artifacts: list[str],
    next_action: str,
) -> str:
    path = checkpoint_file(root, title)
    path.parent.mkdir(parents=True, exist_ok=True)
    lines = [
        f"# {title}",
        "",
        f"- created_at: {utc_now()}",
        f"- project: {state.get('project', '')}",
        f"- phase: {state.get('phase', '')}",
        f"- status: {state.get('status', '')}",
        f"- workflow: {state.get('active_workflow', '')}",
        f"- active_story: {state.get('active_story', '') or '<none>'}",
        "",
        "## Summary",
        "",
        summary.strip(),
    ]
    append_markdown_list(lines, "Decisions", decisions)
    append_markdown_list(lines, "Checks", checks)
    append_markdown_list(lines, "Failed Checks", failed_checks)
    append_markdown_list(lines, "Touched Files", touched)
    append_markdown_list(lines, "Artifacts", artifacts)
    lines.extend(["", "## Next Action", "", next_action.strip()])
    text = "\n".join(lines).rstrip() + "\n"
    path.write_text(text, encoding="utf-8")
    latest = latest_checkpoint_path(root)
    latest.parent.mkdir(parents=True, exist_ok=True)
    latest.write_text(text, encoding="utf-8")
    rel = path.relative_to(root).as_posix()
    append_ledger(root, "checkpoint.written", {"path": rel, "latest": latest.relative_to(root).as_posix()})
    return rel


def link_artifact_to_story(root: Path, artifact_path: str, story_id: str) -> None:
    story = load_story(root, story_id)
    target, rel = project_path(root, artifact_path)
    if not target.exists():
        raise SystemExit(f"Artifact path does not exist: {rel}")
    artifacts = split_list(story.get("artifacts"))
    if rel not in artifacts:
        artifacts.append(rel)
    story["artifacts"] = join_list(artifacts)
    save_story(root, story)
    append_artifact_index(
        root,
        {
            "kind": "story-link",
            "title": f"{rel} -> {story_id}",
            "path": rel,
            "story": story_id,
            "summary": "Artifact linked to story.",
        },
    )
    append_ledger(root, "artifact.linked_to_story", {"path": rel, "story": story_id})


def validate_phase_transition(current: str, target: str, *, force: bool = False) -> None:
    if target not in PHASES:
        raise SystemExit(f"Invalid phase: {target}")
    if force or current == target:
        return
    allowed = PHASE_TRANSITIONS.get(current, set())
    if target not in allowed:
        raise SystemExit(f"Invalid phase transition: {current} -> {target}")


def validate_story_transition(current: str, target: str, *, force: bool = False) -> None:
    if target not in STORY_STATUSES:
        raise SystemExit(f"Invalid story status: {target}")
    if force or current == target:
        return
    allowed = STORY_TRANSITIONS.get(current, set())
    if target not in allowed:
        raise SystemExit(f"Invalid story transition: {current} -> {target}")


def audit_project(root: Path) -> list[str]:
    errors: list[str] = []
    state = read_flat_yaml(state_path(root))
    if not state:
        return ["missing .forge-method/state.yaml"]
    artifact_errors, _ = artifact_findings(root)
    errors.extend(artifact_errors)
    if state.get("runtime") != RUNTIME_NAME:
        errors.append("state.runtime is not forge-method")
    if state.get("phase") not in PHASES:
        errors.append(f"invalid phase: {state.get('phase')}")
    active_story = state.get("active_story", "")
    story_ids = {story.get("id", "") for story in list_stories(root)}
    if active_story and active_story not in story_ids:
        errors.append(f"active story does not exist: {active_story}")
    for story in list_stories(root):
        status = story.get("status", "")
        if status not in STORY_STATUSES:
            errors.append(f"{story.get('id')}: invalid status {status}")
        if status == "done" and not story.get("evidence"):
            errors.append(f"{story.get('id')}: done story has no evidence")
        if status in {"ready", "in_progress", "review"} and not story.get("acceptance_criteria"):
            errors.append(f"{story.get('id')}: executable story has no acceptance criteria")
        for artifact in split_list(story.get("artifacts")):
            if not (root / artifact).exists() and not artifact_missing_allowed(root, artifact):
                errors.append(f"{story.get('id')}: linked artifact missing: {artifact}")
    return errors


def cmd_init(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    if is_runtime_repo(root) and not args.allow_runtime_state:
        raise SystemExit("Refusing to initialize project state in the runtime repo. Use --allow-runtime-state if intentional.")
    fm = ensure_dirs(root)
    path = state_path(root)
    if path.exists() and not args.force:
        print(f"State already exists: {path}")
        print("Use --force to replace it.")
        return 2

    project_id = slugify(args.project)
    state = {
        "schema_version": "1",
        "runtime": RUNTIME_NAME,
        "runtime_version": RUNTIME_VERSION,
        "project": args.project,
        "project_id": project_id,
        "mode": args.mode,
        "module": args.module,
        "phase": "0-route",
        "status": "route-ready",
        "active_workflow": "start-runtime",
        "active_story": "",
        "human_input_required": "false",
        "readiness": "not_ready",
        "next_action": NEXT_BY_PHASE["0-route"],
    }
    write_state(root, state)
    write_flat_yaml(
        fm / PROJECTS_FILE,
        {
            "project": args.project,
            "project_id": project_id,
            "root": str(root),
            "runtime_repo": "false",
        },
        header="Forge Method project registry",
    )
    update_sprint(root)
    copied_guidance = [] if args.no_project_guidance else copy_project_guidance(root, force=args.force)
    append_ledger(
        root,
        "project.initialized",
        {"project": args.project, "project_id": project_id, "guidance": copied_guidance},
    )
    print(f"Initialized Forge Method project: {args.project}")
    print(f"State: {path}")
    if copied_guidance:
        print(f"Project guidance: {', '.join(copied_guidance)}")
    print(f"Next: {state['next_action']}")
    return 0


def cmd_start(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    state_root, state = load_state_or_none(root)
    print("Forge Method Start")
    print(f"Workspace: {root}")

    if state_root:
        print("Route: existing-method-project")
        print(f"Project root: {state_root}")
        print_state_summary(state)
        errors = audit_project(state_root)
        print(f"Audit: {'passed' if not errors else 'failed'}")
        for error in errors:
            print(f"- {error}")
        return 0

    runtime_repo = is_runtime_repo(root)
    print(f"Runtime repo: {'yes' if runtime_repo else 'no'}")
    print("Project state: missing")
    projects = discover_project_roots(root, max_depth=args.scan_depth)
    if projects:
        print("Known projects:")
        for index, project_root in enumerate(projects, start=1):
            project_state = read_flat_yaml(state_path(project_root))
            label = project_state.get("project", project_root.name)
            phase = project_state.get("phase", "<unknown>")
            status = project_state.get("status", "<unknown>")
            rel = display_path(project_root, base=root)
            print(f"{index}. {label}\t{phase}\t{status}\t{rel}")
        print("Question: Which known project should be opened, or should a new project be created?")
        print("Next: wait for the user's project choice, then run status in that project root or init a new project.")
        return 0

    if runtime_repo:
        print("Known projects: none")
        print("Question: Which project folder should be opened or created outside the runtime repo?")
        print("Next: do not initialize project state in the runtime repo unless explicitly intentional.")
        return 0

    print("Known projects: none")
    print("Question: Create a new method project in this workspace?")
    print(
        "Create command: "
        f"{command_hint_value(sys.executable)} "
        f"{command_hint_value(Path(__file__).resolve())} "
        f"init --project <name> --root {command_hint_value(root)}"
    )
    print("Next: wait for the project name, then initialize durable state.")
    return 0


def cmd_status(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    state_root, state = load_state_or_none(root)
    if state_root is None:
        if is_runtime_repo(root):
            print(f"Runtime repo: {root}")
            print("Project state: not initialized here")
            print("Next: open a project folder or initialize a child project outside the runtime root")
            return 0
        print(f"Workspace: {root}")
        print("Project state: missing")
        print("Next: run init")
        return 1
    print(f"Workspace: {state_root}")
    print_state_summary(state)
    return 0


def cmd_next(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    phase = state.get("phase", "0-route")
    if phase == "4-build-verify":
        story = select_next_story(root)
        if story:
            print(f"{NEXT_BY_PHASE[phase]}: {story.get('id')} - {story.get('title')}")
            return 0
    print(state.get("next_action") or NEXT_BY_PHASE.get(phase, "inspect state and choose a valid workflow"))
    return 0


def cmd_transition(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    current_phase = state.get("phase", "0-route")
    if args.phase:
        validate_phase_transition(current_phase, args.phase, force=args.force)
        state["phase"] = args.phase
        if not args.next_action:
            state["next_action"] = NEXT_BY_PHASE.get(args.phase, "inspect state and choose next workflow")
    if args.status:
        state["status"] = args.status
    if args.workflow:
        state["active_workflow"] = args.workflow
    if args.next_action:
        state["next_action"] = args.next_action
    if args.human_input_required is not None:
        state["human_input_required"] = args.human_input_required
    write_state(root, state)
    append_ledger(root, "state.transitioned", {"phase": state.get("phase"), "status": state.get("status")})
    print("Transition written.")
    print(f"Phase: {state.get('phase')}")
    print(f"Status: {state.get('status')}")
    print(f"Next: {state.get('next_action')}")
    return 0


def cmd_story_add(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    story_id = slugify(args.id or args.title)
    path = story_path(root, story_id)
    if path.exists() and not args.force:
        raise SystemExit(f"Story already exists: {story_id}")
    story = {
        "id": story_id,
        "title": args.title,
        "status": args.status,
        "phase": state.get("phase", "0-route"),
        "acceptance_criteria": join_list(args.acceptance or []),
        "evidence": "",
        "checks": "",
        "blocker": "",
    }
    save_story(root, story)
    update_sprint(root)
    append_ledger(root, "story.added", {"id": story_id, "status": args.status})
    print(f"Story added: {story_id}")
    return 0


def cmd_story_list(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    stories = list_stories(root)
    if not stories:
        print("No stories.")
        return 0
    for story in stories:
        print(f"{story.get('id')}\t{story.get('status')}\t{story.get('title')}")
    return 0


def set_story_status(root: Path, story_id: str, target: str, *, force: bool = False, evidence: str = "", checks: list[str] | None = None, blocker: str = "") -> dict[str, str]:
    story = load_story(root, story_id)
    current = story.get("status", "planned")
    validate_story_transition(current, target, force=force)
    story["status"] = target
    if evidence:
        existing = split_list(story.get("evidence"))
        existing.append(evidence)
        story["evidence"] = join_list(existing)
    if checks:
        existing_checks = split_list(story.get("checks"))
        existing_checks.extend(checks)
        story["checks"] = join_list(existing_checks)
    if blocker:
        story["blocker"] = blocker
    save_story(root, story)
    update_sprint(root)
    append_ledger(root, "story.status_changed", {"id": story.get("id"), "from": current, "to": target})
    return story


def cmd_story_start(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    story = set_story_status(root, args.id, "in_progress", force=args.force)
    state["phase"] = "4-build-verify"
    state["status"] = "story-in-progress"
    state["active_workflow"] = "build-story"
    state["active_story"] = story["id"]
    state["human_input_required"] = "false"
    state["next_action"] = f"implement and validate story {story['id']}"
    write_state(root, state)
    print(f"Story started: {story['id']}")
    return 0


def cmd_story_review(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    story = set_story_status(root, args.id, "review", force=args.force)
    state["status"] = "story-review"
    state["active_story"] = story["id"]
    state["next_action"] = f"review story {story['id']} and repair findings"
    write_state(root, state)
    print(f"Story moved to review: {story['id']}")
    return 0


def cmd_story_done(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    story = load_story(root, args.id)
    evidence = args.evidence
    checks = args.check or []
    if not evidence:
        if not args.summary:
            raise SystemExit("Done stories require --evidence or --summary.")
        evidence = write_evidence(
            root,
            kind="story",
            title=f"Story {story.get('id')} done",
            summary=args.summary,
            story_id=story.get("id", ""),
            checks=checks,
        )
    story = set_story_status(root, args.id, "done", force=args.force, evidence=evidence, checks=checks)
    if state.get("active_story") == story["id"]:
        state["active_story"] = ""
    state["status"] = "story-done"
    state["next_action"] = "select next ready story or move to ready when build scope is complete"
    write_state(root, state)
    print(f"Story done: {story['id']}")
    print(f"Evidence: {evidence}")
    return 0


def cmd_story_block(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    story = set_story_status(root, args.id, "blocked", force=args.force, blocker=args.reason)
    state["status"] = "blocked"
    state["active_story"] = story["id"]
    state["human_input_required"] = "true"
    state["next_action"] = f"resolve blocker for story {story['id']}: {args.reason}"
    write_state(root, state)
    print(f"Story blocked: {story['id']}")
    return 0


def cmd_evidence_add(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    rel = write_evidence(
        root,
        kind=args.kind,
        title=args.title,
        summary=args.summary,
        story_id=args.story or "",
        checks=args.check or [],
    )
    print(rel)
    return 0


def cmd_artifact_add(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    rel = write_artifact(
        root,
        kind=args.kind,
        title=args.title,
        summary=args.summary,
        path=args.path or "",
        lifecycle=args.lifecycle,
    )
    if args.story:
        link_artifact_to_story(root, rel, args.story)
    print(rel)
    return 0


def cmd_artifact_capture(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    rel = capture_artifact(
        root,
        path=args.path,
        summary=args.summary,
        story_id=args.story or "",
        evidence=args.evidence or "",
        delete=args.delete,
    )
    print(f"Captured: {rel}")
    return 0


def cmd_artifact_verify(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    errors, warnings = artifact_findings(root)
    if errors:
        print("Artifact verification failed:")
        for error in errors:
            print(f"- {error}")
    if warnings:
        print("Artifact verification warnings:")
        for warning in warnings:
            print(f"- {warning}")
    if errors or (args.strict and warnings):
        return 1
    print("Artifact verification passed.")
    return 0


def cmd_artifact_link_story(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    link_artifact_to_story(root, args.path, args.story)
    print(f"Linked {args.path} -> {args.story}")
    return 0


def cmd_artifact_list(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    artifacts = recent_artifacts(root, limit=args.limit)
    if not artifacts:
        print("No artifacts.")
        return 0
    for artifact in artifacts:
        print(
            f"{artifact.get('kind')}\t{artifact.get('status', 'active')}\t"
            f"{artifact.get('lifecycle', 'durable')}\t{artifact.get('path')}\t{artifact.get('title')}"
        )
    return 0


def cmd_module_list(args: argparse.Namespace) -> int:
    root, _ = load_state_or_none(resolve_root(args.root))
    manifests = module_manifest_paths(root)
    if not manifests:
        print("No modules.")
        return 0
    seen: set[str] = set()
    for path in manifests:
        module = read_flat_yaml(path)
        module_id = module.get("id", path.stem)
        if module_id in seen:
            continue
        seen.add(module_id)
        print(f"{module_id}\t{module.get('title', '')}\t{module.get('phase_span', '')}")
    return 0


def cmd_module_show(args: argparse.Namespace) -> int:
    root, _ = load_state_or_none(resolve_root(args.root))
    for path in module_manifest_paths(root):
        module = read_flat_yaml(path)
        if module.get("id", path.stem) == args.id:
            print(path.read_text(encoding="utf-8"))
            return 0
    raise SystemExit(f"Module not found: {args.id}")


def cmd_module_create(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    module_id = slugify(args.id)
    path = method_dir(root) / "modules" / f"{module_id}.yaml"
    if path.exists() and not args.force:
        raise SystemExit(f"Module already exists: {module_id}")
    values = {
        "id": module_id,
        "title": args.title,
        "purpose": args.purpose,
        "phase_span": join_list(args.phase_span or []),
        "workflows": join_list(args.workflow or []),
    }
    write_flat_yaml(path, values, header="Forge Method module")
    append_ledger(root, "module.created", {"id": module_id, "path": path.relative_to(root).as_posix()})
    print(path.relative_to(root).as_posix())
    return 0


def cmd_workflow_list(args: argparse.Namespace) -> int:
    root, _ = load_state_or_none(resolve_root(args.root))
    for path in reference_workflow_paths(root):
        location = "project" if root and method_dir(root) in path.parents else "packaged"
        print(f"{workflow_id_from_path(path)}\t{location}\t{path.name}")
    return 0


def cmd_workflow_validate(args: argparse.Namespace) -> int:
    if args.path:
        paths = [Path(args.path)]
    else:
        root, _ = load_state_or_none(resolve_root(args.root))
        paths = reference_workflow_paths(root)
    errors: list[str] = []
    for path in paths:
        errors.extend(validate_workflow_file(path))
    if errors:
        print("Workflow validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1
    print("Workflow validation passed.")
    return 0


def cmd_workflow_create(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    workflow_id = slugify(args.id)
    path = method_dir(root) / "workflows" / f"workflow-{workflow_id}.md"
    if path.exists() and not args.force:
        raise SystemExit(f"Workflow already exists: {workflow_id}")
    text = workflow_text(
        workflow_id=workflow_id,
        title=args.title,
        triggers=args.trigger or [],
        inputs=args.input or [],
        steps=args.step or [],
        outputs=args.output or [],
        done_when=args.done or [],
        blocked_when=args.blocked or [],
        handoff=args.handoff or [],
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    errors = validate_workflow_file(path)
    if errors:
        path.unlink(missing_ok=True)
        raise SystemExit("Generated workflow is invalid: " + "; ".join(errors))
    append_ledger(root, "workflow.created", {"id": workflow_id, "path": path.relative_to(root).as_posix()})
    if args.eval_query:
        eval_id = f"{workflow_id}-routing"
        values = {
            "id": eval_id,
            "kind": "workflow-routing",
            "target": workflow_id,
            "query": args.eval_query,
            "expected": workflow_id,
            "status": "pending",
        }
        write_flat_yaml(eval_path(root, eval_id), values, header="Forge Method eval")
        append_ledger(root, "eval.added", {"id": eval_id, "target": workflow_id})
    print(path.relative_to(root).as_posix())
    return 0


def build_context_pack_text(root: Path, state: dict[str, str]) -> str:
    story = load_story(root, state["active_story"]) if state.get("active_story") else None
    evidence_paths = sorted((method_dir(root) / "evidence").glob("*.md"))[-5:]
    summaries = artifact_summaries(root)
    lines = [
        "# Forge Method Context Pack",
        "",
        "## State",
        "",
        f"- project: {state.get('project', '')}",
        f"- phase: {state.get('phase', '')}",
        f"- status: {state.get('status', '')}",
        f"- workflow: {state.get('active_workflow', '')}",
        f"- active_story: {state.get('active_story', '') or '<none>'}",
        f"- next_action: {state.get('next_action', '')}",
    ]
    latest = latest_checkpoint_path(root)
    lines.extend(["", "## Latest Checkpoint", ""])
    if latest.exists():
        checkpoint_text = latest.read_text(encoding="utf-8").strip()
        if len(checkpoint_text) > 1800:
            checkpoint_text = checkpoint_text[:1770].rstrip() + "\n[checkpoint truncated]"
        lines.extend(checkpoint_text.splitlines())
    else:
        lines.append("- none")
    if story:
        lines.extend(
            [
                "",
                "## Active Story",
                "",
                f"- id: {story.get('id')}",
                f"- title: {story.get('title')}",
                f"- status: {story.get('status')}",
                f"- acceptance_criteria: {story.get('acceptance_criteria')}",
            ]
        )
        linked_artifacts = split_list(story.get("artifacts"))
        if linked_artifacts:
            lines.extend(["", "### Linked Artifacts", ""])
            for artifact in linked_artifacts:
                summary = summaries.get(artifact, "")
                suffix = f" - {summary}" if summary else ""
                lines.append(f"- {artifact}{suffix}")
    lines.extend(["", "## Recent Evidence", ""])
    if evidence_paths:
        for path in evidence_paths:
            lines.append(f"- {path.relative_to(root).as_posix()}")
    else:
        lines.append("- none")
    lines.extend(["", "## Recent Artifacts", ""])
    artifacts = recent_artifacts(root)
    if artifacts:
        for artifact in artifacts:
            summary = artifact.get("summary", "")
            suffix = f" - {summary}" if summary else ""
            status = artifact.get("status", "active")
            lifecycle = artifact.get("lifecycle", "durable")
            lines.append(
                f"- {artifact.get('kind')} [{status}/{lifecycle}]: "
                f"{artifact.get('path')} - {artifact.get('title')}{suffix}"
            )
    else:
        lines.append("- none")
    return "\n".join(lines) + "\n"


def write_context_pack(root: Path, state: dict[str, str], *, out: Path, max_chars: int) -> Path:
    if not out.is_absolute():
        out = root / out
    out.parent.mkdir(parents=True, exist_ok=True)
    text = build_context_pack_text(root, state)
    if max_chars and len(text) > max_chars:
        footer = "\n\n[context-pack truncated to max_chars]\n"
        text = text[: max(0, max_chars - len(footer))].rstrip() + footer
    out.write_text(text, encoding="utf-8")
    append_ledger(root, "context_pack.written", {"path": out.relative_to(root).as_posix()})
    return out


def cmd_context_pack(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    out = Path(args.out) if args.out else method_dir(root) / "context" / "current-pack.md"
    out = write_context_pack(root, state, out=out, max_chars=args.max_chars)
    print(out)
    return 0


def cmd_checkpoint(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    next_action = args.next_action or state.get("next_action", "")
    rel = write_checkpoint(
        root,
        state,
        title=args.title,
        summary=args.summary,
        decisions=args.decision or [],
        checks=args.check or [],
        failed_checks=args.failed_check or [],
        touched=args.touched or [],
        artifacts=args.artifact or [],
        next_action=next_action,
    )
    if next_action:
        state["next_action"] = next_action
        write_state(root, state)
    if not args.no_context_pack:
        write_context_pack(root, state, out=method_dir(root) / "context" / "current-pack.md", max_chars=args.max_chars)
    print(rel)
    return 0


def cmd_eval_add(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    eval_id = slugify(args.id)
    if workflow_path_by_id(root, args.target) is None:
        raise SystemExit(f"Target workflow not found: {args.target}")
    values = {
        "id": eval_id,
        "kind": args.kind,
        "target": slugify(args.target),
        "query": args.query,
        "expected": args.expected or slugify(args.target),
        "status": "pending",
    }
    write_flat_yaml(eval_path(root, eval_id), values, header="Forge Method eval")
    append_ledger(root, "eval.added", {"id": eval_id, "target": values["target"]})
    print(eval_path(root, eval_id).relative_to(root).as_posix())
    return 0


def cmd_eval_list(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    evals = list_evals(root)
    if not evals:
        print("No evals.")
        return 0
    for item in evals:
        print(f"{item.get('id')}\t{item.get('kind')}\t{item.get('target')}\t{item.get('status')}")
    return 0


def run_eval_items(root: Path) -> tuple[list[str], list[str]]:
    evals = list_evals(root)
    passed: list[str] = []
    failures: list[str] = []
    for item in evals:
        eval_id = item.get("id", "")
        target = item.get("target", "")
        expected = item.get("expected", target)
        query = item.get("query", "")
        workflow_path = workflow_path_by_id(root, target)
        errors = validate_workflow_file(workflow_path) if workflow_path else [f"target workflow not found: {target}"]
        if not query:
            errors.append("query is empty")
        if expected != target:
            errors.append(f"expected workflow {expected} does not match target {target}")
        item["status"] = "failed" if errors else "passed"
        item["last_run_at"] = utc_now()
        item["last_error"] = join_list(errors)
        write_flat_yaml(eval_path(root, eval_id), item, header="Forge Method eval")
        if errors:
            failures.append(f"{eval_id}: {', '.join(errors)}")
        else:
            passed.append(eval_id)
    append_ledger(root, "eval.run", {"count": len(evals), "failures": len(failures)})
    return passed, failures


def cmd_eval_run(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    passed, failures = run_eval_items(root)
    for eval_id in passed:
        print(f"PASS {eval_id}")
    if failures:
        print("Eval run failed:")
        for failure in failures:
            print(f"- {failure}")
        return 1
    print(f"Eval run passed: {len(passed)} eval(s)")
    return 0


def workflow_validation_errors(root: Path | None = None) -> list[str]:
    errors: list[str] = []
    for path in reference_workflow_paths(root):
        errors.extend(validate_workflow_file(path))
    return errors


def cmd_audit(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    errors = audit_project(root)
    if errors:
        print("Audit failed:")
        for error in errors:
            print(f"- {error}")
        return 1
    print("Audit passed.")
    return 0


def cmd_gate(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    errors: list[str] = []
    warnings: list[str] = []

    audit_errors = [
        error
        for error in audit_project(root)
        if not error.startswith("missing active artifact:")
    ]
    if audit_errors:
        errors.extend(f"audit: {error}" for error in audit_errors)

    artifact_errors, artifact_warnings = artifact_findings(root)
    if artifact_errors:
        errors.extend(f"artifact: {error}" for error in artifact_errors)
    if artifact_warnings:
        warnings.extend(f"artifact: {warning}" for warning in artifact_warnings)

    workflow_errors = workflow_validation_errors(root)
    if workflow_errors:
        errors.extend(f"workflow: {error}" for error in workflow_errors)

    eval_count = len(list_evals(root))
    passed_evals, eval_failures = run_eval_items(root)
    if args.require_evals and eval_count == 0:
        errors.append("eval: no evals configured")
    if eval_failures:
        errors.extend(f"eval: {failure}" for failure in eval_failures)

    strict_failures = warnings if args.strict else []
    passed = not errors and not strict_failures
    append_ledger(
        root,
        "gate.run",
        {
            "passed": passed,
            "errors": len(errors),
            "warnings": len(warnings),
            "evals": eval_count,
        },
    )

    if passed:
        print("Gate passed.")
        print(f"Audit: passed")
        print(f"Artifacts: passed")
        print(f"Workflows: passed")
        print(f"Evals: {len(passed_evals)}/{eval_count} passed")
        if warnings:
            print("Warnings:")
            for warning in warnings:
                print(f"- {warning}")
        if args.summary:
            evidence = write_evidence(
                root,
                kind="gate",
                title="Quality gate",
                summary=args.summary,
                checks=[
                    "audit",
                    "artifact verify",
                    "workflow validate",
                    "eval run",
                ],
            )
            print(f"Evidence: {evidence}")
        if args.context_pack:
            out = write_context_pack(root, state, out=method_dir(root) / "context" / "current-pack.md", max_chars=args.max_chars)
            print(f"Context pack: {out}")
        return 0

    print("Gate failed:")
    for error in errors:
        print(f"- {error}")
    if strict_failures:
        print("Strict warning failures:")
        for warning in strict_failures:
            print(f"- {warning}")
    elif warnings:
        print("Warnings:")
        for warning in warnings:
            print(f"- {warning}")
    return 1


def cmd_ready(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    errors = audit_project(root)
    active = [story for story in list_stories(root) if story.get("status") in {"in_progress", "review"}]
    if active:
        errors.append("active implementation/review stories remain")
    if errors and not args.force:
        print("Ready gate failed:")
        for error in errors:
            print(f"- {error}")
        return 1
    evidence = write_evidence(
        root,
        kind="release",
        title="Ready gate",
        summary=args.summary,
        checks=args.check or [],
    )
    validate_phase_transition(state.get("phase", "0-route"), "5-ready-operate", force=True)
    state["phase"] = "5-ready-operate"
    state["status"] = "ready"
    state["active_workflow"] = "ready-release"
    state["active_story"] = ""
    state["human_input_required"] = "false"
    state["readiness"] = "ready"
    state["next_action"] = NEXT_BY_PHASE["5-ready-operate"]
    write_state(root, state)
    append_ledger(root, "project.ready", {"evidence": evidence})
    print("Project marked ready.")
    print(f"Evidence: {evidence}")
    return 0


def cmd_handoff(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    stamp = dt.datetime.now(dt.timezone.utc).strftime("%Y%m%d-%H%M%S")
    path = method_dir(root) / "handoffs" / f"{stamp}-handoff.md"
    lines = [
        "# Handoff",
        "",
        f"- created_at: {utc_now()}",
        f"- project: {state.get('project', '')}",
        f"- phase: {state.get('phase', '')}",
        f"- status: {state.get('status', '')}",
        f"- active_story: {state.get('active_story', '') or '<none>'}",
        "",
        "## Summary",
        "",
        args.summary,
        "",
        "## Next Action",
        "",
        args.next_action or state.get("next_action", ""),
    ]
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    state["next_action"] = args.next_action or state.get("next_action", "")
    write_state(root, state)
    append_ledger(root, "handoff.written", {"path": path.relative_to(root).as_posix()})
    print(path)
    return 0


def cmd_doctor(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    state_root, _ = load_state_or_none(root)
    print(f"Workspace: {root}")
    print(f"Runtime repo: {'yes' if is_runtime_repo(root) else 'no'}")
    print(f"Project state root: {state_root if state_root else '<none>'}")
    if state_root:
        errors = audit_project(state_root)
        print(f"Audit: {'passed' if not errors else 'failed'}")
        for error in errors:
            print(f"- {error}")
    return 0


def cmd_version(args: argparse.Namespace) -> int:
    print(RUNTIME_VERSION)
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Forge Method helper")
    sub = parser.add_subparsers(dest="command", required=True)

    version = sub.add_parser("version", help="print runtime version")
    version.set_defaults(func=cmd_version)

    start = sub.add_parser("start", help="resolve project route and next action")
    start.add_argument("--root", default=".")
    start.add_argument("--scan-depth", type=int, default=2)
    start.set_defaults(func=cmd_start)

    init = sub.add_parser("init", help="initialize .forge-method state")
    init.add_argument("--project", required=True)
    init.add_argument("--root", default=".")
    init.add_argument("--mode", default="creation-runtime")
    init.add_argument("--module", default="software-builder")
    init.add_argument("--force", action="store_true")
    init.add_argument("--allow-runtime-state", action="store_true")
    init.add_argument("--no-project-guidance", action="store_true")
    init.set_defaults(func=cmd_init)

    status = sub.add_parser("status", help="print current runtime state")
    status.add_argument("--root", default=".")
    status.set_defaults(func=cmd_status)

    next_cmd = sub.add_parser("next", help="print next recommended action")
    next_cmd.add_argument("--root", default=".")
    next_cmd.set_defaults(func=cmd_next)

    transition = sub.add_parser("transition", help="update phase/status/workflow")
    transition.add_argument("--root", default=".")
    transition.add_argument("--phase")
    transition.add_argument("--status")
    transition.add_argument("--workflow")
    transition.add_argument("--next-action")
    transition.add_argument("--human-input-required", choices=["true", "false"])
    transition.add_argument("--force", action="store_true")
    transition.set_defaults(func=cmd_transition)

    story = sub.add_parser("story", help="manage stories")
    story_sub = story.add_subparsers(dest="story_command", required=True)

    story_add = story_sub.add_parser("add", help="add a story")
    story_add.add_argument("--root", default=".")
    story_add.add_argument("--id")
    story_add.add_argument("--title", required=True)
    story_add.add_argument("--acceptance", action="append")
    story_add.add_argument("--status", choices=STORY_STATUSES, default="ready")
    story_add.add_argument("--force", action="store_true")
    story_add.set_defaults(func=cmd_story_add)

    story_list = story_sub.add_parser("list", help="list stories")
    story_list.add_argument("--root", default=".")
    story_list.set_defaults(func=cmd_story_list)

    story_start = story_sub.add_parser("start", help="start a story")
    story_start.add_argument("--root", default=".")
    story_start.add_argument("--id", required=True)
    story_start.add_argument("--force", action="store_true")
    story_start.set_defaults(func=cmd_story_start)

    story_review = story_sub.add_parser("review", help="move a story to review")
    story_review.add_argument("--root", default=".")
    story_review.add_argument("--id", required=True)
    story_review.add_argument("--force", action="store_true")
    story_review.set_defaults(func=cmd_story_review)

    story_done = story_sub.add_parser("done", help="mark a story done")
    story_done.add_argument("--root", default=".")
    story_done.add_argument("--id", required=True)
    story_done.add_argument("--summary")
    story_done.add_argument("--evidence")
    story_done.add_argument("--check", action="append")
    story_done.add_argument("--force", action="store_true")
    story_done.set_defaults(func=cmd_story_done)

    story_block = story_sub.add_parser("block", help="block a story")
    story_block.add_argument("--root", default=".")
    story_block.add_argument("--id", required=True)
    story_block.add_argument("--reason", required=True)
    story_block.add_argument("--force", action="store_true")
    story_block.set_defaults(func=cmd_story_block)

    evidence = sub.add_parser("evidence", help="write evidence")
    evidence_sub = evidence.add_subparsers(dest="evidence_command", required=True)
    evidence_add = evidence_sub.add_parser("add", help="add evidence")
    evidence_add.add_argument("--root", default=".")
    evidence_add.add_argument("--kind", required=True)
    evidence_add.add_argument("--title", required=True)
    evidence_add.add_argument("--summary", required=True)
    evidence_add.add_argument("--story")
    evidence_add.add_argument("--check", action="append")
    evidence_add.set_defaults(func=cmd_evidence_add)

    module = sub.add_parser("module", help="inspect runtime modules")
    module_sub = module.add_subparsers(dest="module_command", required=True)
    module_list = module_sub.add_parser("list", help="list modules")
    module_list.add_argument("--root", default=".")
    module_list.set_defaults(func=cmd_module_list)
    module_show = module_sub.add_parser("show", help="show a module manifest")
    module_show.add_argument("--root", default=".")
    module_show.add_argument("--id", required=True)
    module_show.set_defaults(func=cmd_module_show)
    module_create = module_sub.add_parser("create", help="create a project module manifest")
    module_create.add_argument("--root", default=".")
    module_create.add_argument("--id", required=True)
    module_create.add_argument("--title", required=True)
    module_create.add_argument("--purpose", required=True)
    module_create.add_argument("--phase-span", action="append")
    module_create.add_argument("--workflow", action="append")
    module_create.add_argument("--force", action="store_true")
    module_create.set_defaults(func=cmd_module_create)

    workflow = sub.add_parser("workflow", help="inspect and validate workflow references")
    workflow_sub = workflow.add_subparsers(dest="workflow_command", required=True)
    workflow_list = workflow_sub.add_parser("list", help="list packaged workflows")
    workflow_list.add_argument("--root", default=".")
    workflow_list.set_defaults(func=cmd_workflow_list)
    workflow_validate = workflow_sub.add_parser("validate", help="validate workflow references")
    workflow_validate.add_argument("--root", default=".")
    workflow_validate.add_argument("--path")
    workflow_validate.set_defaults(func=cmd_workflow_validate)
    workflow_create = workflow_sub.add_parser("create", help="create a project workflow state machine")
    workflow_create.add_argument("--root", default=".")
    workflow_create.add_argument("--id", required=True)
    workflow_create.add_argument("--title", required=True)
    workflow_create.add_argument("--trigger", action="append")
    workflow_create.add_argument("--input", action="append")
    workflow_create.add_argument("--step", action="append")
    workflow_create.add_argument("--output", action="append")
    workflow_create.add_argument("--done", action="append")
    workflow_create.add_argument("--blocked", action="append")
    workflow_create.add_argument("--handoff", action="append")
    workflow_create.add_argument("--eval-query")
    workflow_create.add_argument("--force", action="store_true")
    workflow_create.set_defaults(func=cmd_workflow_create)

    artifact = sub.add_parser("artifact", help="manage artifacts")
    artifact_sub = artifact.add_subparsers(dest="artifact_command", required=True)
    artifact_add = artifact_sub.add_parser("add", help="add an artifact")
    artifact_add.add_argument("--root", default=".")
    artifact_add.add_argument("--kind", required=True)
    artifact_add.add_argument("--title", required=True)
    artifact_add.add_argument("--summary", required=True)
    artifact_add.add_argument("--path")
    artifact_add.add_argument("--lifecycle", choices=ARTIFACT_LIFECYCLES, default="durable")
    artifact_add.add_argument("--story")
    artifact_add.set_defaults(func=cmd_artifact_add)

    artifact_capture = artifact_sub.add_parser("capture", help="capture an artifact result and optionally delete it")
    artifact_capture.add_argument("--root", default=".")
    artifact_capture.add_argument("--path", required=True)
    artifact_capture.add_argument("--summary", required=True)
    artifact_capture.add_argument("--story")
    artifact_capture.add_argument("--evidence")
    artifact_capture.add_argument("--delete", action="store_true")
    artifact_capture.set_defaults(func=cmd_artifact_capture)

    artifact_verify = artifact_sub.add_parser("verify", help="verify artifact files and summaries")
    artifact_verify.add_argument("--root", default=".")
    artifact_verify.add_argument("--strict", action="store_true")
    artifact_verify.set_defaults(func=cmd_artifact_verify)

    artifact_list = artifact_sub.add_parser("list", help="list recent artifacts")
    artifact_list.add_argument("--root", default=".")
    artifact_list.add_argument("--limit", type=int, default=20)
    artifact_list.set_defaults(func=cmd_artifact_list)
    artifact_link = artifact_sub.add_parser("link-story", help="link an artifact to a story")
    artifact_link.add_argument("--root", default=".")
    artifact_link.add_argument("--path", required=True)
    artifact_link.add_argument("--story", required=True)
    artifact_link.set_defaults(func=cmd_artifact_link_story)

    eval_cmd = sub.add_parser("eval", help="manage local runtime evals")
    eval_sub = eval_cmd.add_subparsers(dest="eval_command", required=True)
    eval_add = eval_sub.add_parser("add", help="add a routing eval")
    eval_add.add_argument("--root", default=".")
    eval_add.add_argument("--id", required=True)
    eval_add.add_argument("--kind", default="workflow-routing")
    eval_add.add_argument("--target", required=True)
    eval_add.add_argument("--query", required=True)
    eval_add.add_argument("--expected")
    eval_add.set_defaults(func=cmd_eval_add)
    eval_list = eval_sub.add_parser("list", help="list evals")
    eval_list.add_argument("--root", default=".")
    eval_list.set_defaults(func=cmd_eval_list)
    eval_run = eval_sub.add_parser("run", help="run evals")
    eval_run.add_argument("--root", default=".")
    eval_run.set_defaults(func=cmd_eval_run)

    checkpoint = sub.add_parser("checkpoint", help="write durable progress memory")
    checkpoint.add_argument("--root", default=".")
    checkpoint.add_argument("--title", default="Checkpoint")
    checkpoint.add_argument("--summary", required=True)
    checkpoint.add_argument("--decision", action="append")
    checkpoint.add_argument("--check", action="append")
    checkpoint.add_argument("--failed-check", action="append")
    checkpoint.add_argument("--touched", action="append")
    checkpoint.add_argument("--artifact", action="append")
    checkpoint.add_argument("--next-action")
    checkpoint.add_argument("--max-chars", type=int, default=8000)
    checkpoint.add_argument("--no-context-pack", action="store_true")
    checkpoint.set_defaults(func=cmd_checkpoint)

    context = sub.add_parser("context", help="context pack operations")
    context_sub = context.add_subparsers(dest="context_command", required=True)
    context_pack = context_sub.add_parser("pack", help="write a compact context pack")
    context_pack.add_argument("--root", default=".")
    context_pack.add_argument("--out")
    context_pack.add_argument("--max-chars", type=int, default=8000)
    context_pack.set_defaults(func=cmd_context_pack)

    audit = sub.add_parser("audit", help="validate project state")
    audit.add_argument("--root", default=".")
    audit.set_defaults(func=cmd_audit)

    gate = sub.add_parser("gate", help="run project quality gate")
    gate.add_argument("--root", default=".")
    gate.add_argument("--strict", action="store_true")
    gate.add_argument("--require-evals", action="store_true")
    gate.add_argument("--summary")
    gate.add_argument("--context-pack", action="store_true")
    gate.add_argument("--max-chars", type=int, default=8000)
    gate.set_defaults(func=cmd_gate)

    ready = sub.add_parser("ready", help="mark project ready for use")
    ready.add_argument("--root", default=".")
    ready.add_argument("--summary", required=True)
    ready.add_argument("--check", action="append")
    ready.add_argument("--force", action="store_true")
    ready.set_defaults(func=cmd_ready)

    handoff = sub.add_parser("handoff", help="write a continuation handoff")
    handoff.add_argument("--root", default=".")
    handoff.add_argument("--summary", required=True)
    handoff.add_argument("--next-action")
    handoff.set_defaults(func=cmd_handoff)

    doctor = sub.add_parser("doctor", help="inspect runtime/project detection")
    doctor.add_argument("--root", default=".")
    doctor.set_defaults(func=cmd_doctor)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
