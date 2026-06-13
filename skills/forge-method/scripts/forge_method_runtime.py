#!/usr/bin/env python3
"""File-backed runtime helper for Forge Method."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import re
import shutil
import subprocess
import sys
import tempfile
import unicodedata
from pathlib import Path
from typing import Any
from urllib.parse import quote


RUNTIME_NAME = "forge-method"
RUNTIME_REPO_NAME = "forge-method-core"
RUNTIME_VERSION = "1.28.0"
SKILL_DIR = Path(__file__).resolve().parents[1]
PROJECT_TEMPLATE_DIR = SKILL_DIR / "assets" / "project"
WORKFLOW_CATALOG_PATH = SKILL_DIR / "catalog" / "workflows.json"
FACILITATION_DIR = SKILL_DIR / "facilitation"
TEMPLATES_DIR = SKILL_DIR / "templates"
PARITY_REPLAY_FIXTURE_PATH = SKILL_DIR / "fixtures" / "guidance-parity-replay.json"

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

HUMAN_INPUT_STATUSES = [
    "open",
    "answered",
    "deferred",
]

REVIEW_FINDING_STATUSES = [
    "open",
    "resolved",
    "waived",
]

REVIEW_FINDING_SEVERITIES = [
    "critical",
    "high",
    "medium",
    "low",
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
EVAL_KINDS = ["workflow-routing", "workflow-trigger", "artifact-exists"]
AGENT_PROFILE_REQUIRED_FIELDS = [
    "id",
    "title",
    "purpose",
    "when",
    "inputs",
    "outputs",
    "handoff",
]

WORKFLOW_REQUIRED_SECTIONS = [
    "trigger:",
    "inputs:",
    "steps:",
    "outputs:",
    "done_when:",
    "blocked_when:",
    "handoff:",
]

FACILITATION_REQUIRED_SECTIONS = [
    "purpose:",
    "open_floor:",
    "source_material:",
    "follow_up_batches:",
    "conversation_stages:",
    "elicitation_options:",
    "facilitator_moves:",
    "quality_bar:",
    "anti_patterns:",
    "paths:",
    "checkpoint_options:",
    "artifact_rules:",
    "headless:",
]

PARITY_REPLAY_REQUIRED_FAMILIES = {
    "help",
    "confusion",
    "brainstorm",
    "research",
    "prd",
    "ux",
    "architecture",
    "quick-dev",
    "story-cycle",
    "correct-course",
    "builder",
    "cis",
    "game",
    "tea",
}

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

WORKFLOW_BY_PHASE = {
    "0-route": "start-runtime",
    "1-discovery": "discover-intent",
    "2-specification": "write-spec",
    "3-plan": "plan-sprint",
    "4-build-verify": "build-story",
    "5-ready-operate": "ready-release",
    "6-evolve": "evolve-project",
}

HUMAN_FACING_REQUIRED_WORKFLOWS = {
    "discover-intent",
    "reality-evidence-gate",
    "market-scan",
    "domain-scan",
    "technical-feasibility-scan",
    "brainstorming",
    "problem-solving",
    "design-thinking",
    "innovation-strategy",
    "storytelling",
    "creative-session",
    "concept-selection",
    "write-spec",
    "quick-dev",
    "product-requirements",
    "architecture",
    "ux-plan",
    "story-creation",
    "create-epics",
    "plan-sprint",
    "readiness-check",
    "grill-gate",
    "ready-release",
    "release-readiness",
    "evolve-project",
    "correct-course",
    "runtime-builder",
    "game-brief",
    "gdd",
    "narrative-design",
    "mechanics-design",
    "game-ux-design",
    "game-prd",
    "engine-architecture",
    "quick-prototype",
    "game-story-creation",
    "game-sprint-status",
    "game-retrospective",
    "game-test-framework",
    "game-test-automation",
    "game-e2e-scaffold",
    "playtest-plan",
    "performance-plan",
    "game-qa-review",
    "test-strategy",
    "teach-testing",
    "test-engagement-model",
    "test-framework",
    "ci-quality-pipeline",
    "atdd-plan",
    "test-automation",
    "test-review",
    "nfr-evidence-audit",
    "traceability-gate",
    "risk-register",
    "security-plan",
    "privacy-data-plan",
    "devops-deployment-plan",
    "compliance-checklist",
    "observability-plan",
}

HUMAN_EXPERIENCE_POLICY: dict[str, Any] = {
    "voice": "warm, direct, opinionated, and useful",
    "adaptive_energy": "match the user's energy without attacking the user",
    "pushback": "respect the human; be ruthless with weak ideas, broken process, and bad assumptions",
    "runtime_boundary": "keep JSON, state, evidence, workflows, and recovery artifacts compact",
}

REALITY_SCAN_WORKFLOWS = [
    "reality-evidence-gate",
    "market-scan",
    "domain-scan",
    "technical-feasibility-scan",
]

TRACKS: list[dict[str, str]] = [
    {
        "id": "quick-flow",
        "title": "Quick Flow",
        "complexity": "low",
        "project_kind": "small-change",
        "module": "software-builder",
        "purpose": "Move a narrow request through discovery, build, validation, and ready without heavy planning.",
        "when": "small fix, prototype, short automation, or low-risk enhancement",
    },
    {
        "id": "standard-product",
        "title": "Standard Product",
        "complexity": "medium",
        "project_kind": "product-software",
        "module": "software-builder",
        "purpose": "Create product requirements, architecture, epics, implementation stories, checks, and ready evidence.",
        "when": "normal app, tool, product, API, site, integration, or software workflow",
    },
    {
        "id": "enterprise",
        "title": "Enterprise",
        "complexity": "high",
        "project_kind": "enterprise-system",
        "module": "test-architect",
        "purpose": "Add security, privacy, compliance, deployment, observability, risk, and release readiness.",
        "when": "regulated, multi-team, security-sensitive, compliance-heavy, or production operations work",
    },
    {
        "id": "creative-studio",
        "title": "Creative Studio",
        "complexity": "medium",
        "project_kind": "creative-work",
        "module": "creative-studio",
        "purpose": "Facilitate ideation, direction selection, storytelling, and creative artifact production.",
        "when": "brand, story, campaign, content, concept, presentation, or creative direction",
    },
    {
        "id": "game-studio",
        "title": "Game Studio",
        "complexity": "medium",
        "project_kind": "game",
        "module": "game-studio",
        "purpose": "Shape game concept, GDD, mechanics, narrative, prototype, playtest, and vertical slice work.",
        "when": "game, mechanic, player experience, prototype, engine, level, economy, or playtest work",
    },
    {
        "id": "runtime-builder",
        "title": "Runtime Builder",
        "complexity": "high",
        "project_kind": "runtime-extension",
        "module": "runtime-builder",
        "purpose": "Create or improve workflows, modules, skills, templates, evals, and runtime behavior.",
        "when": "method, runtime, workflow, skill, agent profile, template, eval, or plugin work",
    },
    {
        "id": "test-architect",
        "title": "Test Architect",
        "complexity": "medium",
        "project_kind": "quality-strategy",
        "module": "test-architect",
        "purpose": "Design risk-based validation, review gates, evidence, fixtures, and acceptance checks.",
        "when": "QA, test plan, risk matrix, review criteria, evidence, regression, or validation strategy",
    },
    {
        "id": "launch-ops",
        "title": "Launch Ops",
        "complexity": "medium",
        "project_kind": "launch-operations",
        "module": "launch-ops",
        "purpose": "Move finished work into ready/operate, release evidence, support, feedback, and evolution.",
        "when": "release, launch, operations, support, feedback, monitoring, or next-version planning",
    },
]

TRACK_IDS = {track["id"] for track in TRACKS}
TRACK_BY_MODULE = {
    "software-builder": "standard-product",
    "creative-studio": "creative-studio",
    "game-studio": "game-studio",
    "runtime-builder": "runtime-builder",
    "test-architect": "test-architect",
    "enterprise": "enterprise",
    "launch-ops": "launch-ops",
    "core-runtime": "standard-product",
}
CONFIG_ALLOWED_KEYS = {
    "default_track",
    "human_tone",
    "required_checks",
    "artifact_template",
    "output_path",
    "project_conventions",
    "council_mode",
    "autonomy_mode",
    "commit_policy",
}
AUTONOMY_MODES = {"auto", "manual"}
COMMIT_POLICIES = {"off", "story", "epic"}
GRILL_GATE_PHASES = {"1-discovery", "2-specification", "3-plan"}
MECHANICAL_ACTIONS = {
    "start_next_story",
    "continue_active_story",
    "review_active_story",
    "resolve_review_findings",
    "repair_project_state",
    "run_ready_gate",
}

STORY_DECISION_ARTIFACT_KINDS = {
    "architecture",
    "architecture-plan",
    "atdd-plan",
    "decision",
    "decision-gate",
    "epics",
    "game-prd",
    "gdd",
    "mechanics-matrix",
    "prd",
    "product-requirements",
    "quick-dev",
    "requirements",
    "risk-register",
    "spec",
    "specification",
    "story-creation",
    "test-strategy",
    "traceability-gate",
    "ux",
    "ux-design",
    "ux-plan",
    "validation-map",
}
STORY_DECISION_PATH_HINTS = {
    "architecture",
    "atdd",
    "epic",
    "gdd",
    "mechanics",
    "prd",
    "quick-dev",
    "requirements",
    "risk",
    "spec",
    "story-creation",
    "test-strategy",
    "traceability",
    "ux",
    "validation",
}
BUILDER_KINDS = ["workflow", "module", "agent", "skill", "template", "eval"]
COUNCIL_DEFAULT_AGENTS = ["facilitator", "researcher", "spec-architect", "planner", "quality-reviewer"]

SEMVER_RE = re.compile(r"^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$")


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


def find_runtime_repo_root(start: Path) -> Path | None:
    current = start.resolve()
    for candidate in [current, *current.parents]:
        if is_runtime_repo(candidate):
            return candidate
    return None


def ensure_dirs(root: Path) -> Path:
    fm = method_dir(root)
    for name in [
        "artifacts",
        "checkpoints",
        "context",
        "evals",
        "evidence",
        "handoffs",
        "agents",
        "config",
        "inputs",
        "modules",
        "reviews",
        "skills",
        "stories",
        "templates",
        "workflows",
    ]:
        (fm / name).mkdir(parents=True, exist_ok=True)
    return fm


def track_by_id(track_id: str) -> dict[str, str] | None:
    normalized = slugify(track_id)
    for track in TRACKS:
        if track["id"] == normalized:
            return track
    return None


def default_track_for_module(module_id: str) -> dict[str, str]:
    return track_by_id(TRACK_BY_MODULE.get(slugify(module_id), "standard-product")) or TRACKS[1]


def score_track_for_objective(track: dict[str, str], objective: str) -> tuple[int, str]:
    if not objective:
        return 0, "no objective supplied"
    tokens = objective_tokens(objective)
    haystack = " ".join(
        [
            track.get("id", ""),
            track.get("title", ""),
            track.get("purpose", ""),
            track.get("when", ""),
            track.get("project_kind", ""),
        ]
    ).lower()
    score = sum(1 for token in tokens if token in haystack)
    keywords = {
        "quick-flow": {"quick", "small", "fix", "patch", "prototype", "tiny"},
        "standard-product": {"app", "api", "software", "web", "product", "tool"},
        "enterprise": {"security", "privacy", "compliance", "enterprise", "risk", "production"},
        "creative-studio": {"creative", "brand", "story", "content", "campaign", "concept"},
        "game-studio": {
            "game",
            "player",
            "mechanic",
            "gdd",
            "engine",
            "playtest",
            "rpg",
            "vtt",
            "tabletop",
            "campaign",
            "dice",
            "rulebook",
            "jogar",
            "mesa",
            "regras",
        },
        "runtime-builder": {"runtime", "method", "workflow", "skill", "agent", "plugin"},
        "test-architect": {"test", "qa", "validation", "review", "evidence", "regression"},
        "launch-ops": {"launch", "release", "operate", "support", "monitoring", "feedback"},
    }
    hits = sorted(tokens & keywords.get(track["id"], set()))
    score += len(hits) * 3
    reason = f"matched {', '.join(hits)}" if hits else "matched objective language"
    return score, reason


def recommended_tracks(objective: str, *, limit: int = 5) -> list[dict[str, Any]]:
    scored: list[dict[str, Any]] = []
    for track in TRACKS:
        score, reason = score_track_for_objective(track, objective)
        item = dict(track)
        item["score"] = score
        item["reason"] = reason
        scored.append(item)
    scored.sort(key=lambda item: (-int(item.get("score", 0)), item.get("id", "")))
    return scored[:limit]


def config_paths(root: Path) -> tuple[Path, Path]:
    config_dir = method_dir(root) / "config"
    return config_dir / "team.yaml", config_dir / "local.yaml"


def merged_config(root: Path) -> tuple[dict[str, str], list[str]]:
    team, local = config_paths(root)
    merged: dict[str, str] = {}
    sources: list[str] = []
    for path in [team, local]:
        if path.exists():
            merged.update(read_flat_yaml(path))
            sources.append(path.relative_to(root).as_posix())
    return merged, sources


def validate_config_values(values: dict[str, str], *, source: str) -> list[str]:
    errors: list[str] = []
    for key, value in values.items():
        if key == "updated_at":
            continue
        if key not in CONFIG_ALLOWED_KEYS:
            errors.append(f"{source}: unsupported config key `{key}`")
        if key == "default_track" and value and value not in TRACK_IDS:
            errors.append(f"{source}: default_track must be one of {', '.join(sorted(TRACK_IDS))}")
        if key == "autonomy_mode" and value and value not in AUTONOMY_MODES:
            errors.append(f"{source}: autonomy_mode must be one of {', '.join(sorted(AUTONOMY_MODES))}")
        if key == "commit_policy" and value and value not in COMMIT_POLICIES:
            errors.append(f"{source}: commit_policy must be one of {', '.join(sorted(COMMIT_POLICIES))}")
    return errors


def apply_state_defaults(state: dict[str, str]) -> dict[str, str]:
    state.setdefault("autonomy_mode", "auto")
    state.setdefault("commit_policy", "off")
    state.setdefault("last_grill_artifact", "")
    state.setdefault("last_correct_course_artifact", "")
    return state


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
    return state_root, apply_state_defaults(read_flat_yaml(state_path(state_root)))


def load_state_or_fail(root: Path) -> tuple[Path, dict[str, str]]:
    state_root, state = load_state_or_none(root)
    if state_root is None:
        if find_runtime_repo_root(root):
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


def workspace_entries_for_brownfield(root: Path) -> list[Path]:
    if not root.exists() or not root.is_dir():
        return []
    ignored = {STATE_DIR, *SCAN_SKIP_DIRS}
    entries: list[Path] = []
    try:
        children = sorted(root.iterdir(), key=lambda path: path.name.lower())
    except OSError:
        return []
    for child in children:
        if child.name in ignored:
            continue
        entries.append(child)
    return entries


def is_brownfield_workspace(root: Path) -> bool:
    return bool(workspace_entries_for_brownfield(root)) and not state_path(root).exists()


def display_path(path: Path, *, base: Path) -> str:
    try:
        return path.relative_to(base).as_posix() or "."
    except ValueError:
        return str(path)


def command_hint_value(value: str | Path) -> str:
    text = str(value).replace('"', '\\"')
    return f'"{text}"'


def command_hint_part(value: str | Path | int) -> str:
    if isinstance(value, Path):
        return command_hint_value(value)
    text = str(value)
    if not text:
        return command_hint_value(text)
    if any(char.isspace() for char in text) or any(char in text for char in '<>"'):
        return command_hint_value(text)
    return text


def print_state_summary(state: dict[str, str]) -> None:
    print(f"Project: {state.get('project', '<unknown>')}")
    if state.get("track"):
        print(f"Track: {state.get('track')} ({state.get('complexity', 'unknown')})")
    print(f"Phase: {state.get('phase', '<unknown>')}")
    print(f"Status: {state.get('status', '<unknown>')}")
    print(f"Workflow: {state.get('active_workflow', '<none>')}")
    print(f"Active story: {state.get('active_story', '') or '<none>'}")
    print(f"Human input required: {state.get('human_input_required', 'unknown')}")
    print(f"Readiness: {state.get('readiness', 'unknown')}")
    print(f"Next: {state.get('next_action', NEXT_BY_PHASE.get(state.get('phase', ''), 'inspect state'))}")


def build_status_brief(root: Path, state: dict[str, str]) -> dict[str, Any]:
    snapshot = build_snapshot(root, state)
    next_story = snapshot["stories"]["next"] or {}
    required_inputs = snapshot["human_inputs"]["required_open"]
    open_findings = snapshot["review_findings"]["open"]
    audit_errors = snapshot["quality"]["audit"]["errors"]
    return {
        "runtime": snapshot["runtime"],
        "runtime_version": snapshot["runtime_version"],
        "root": snapshot["root"],
        "project": state.get("project", ""),
        "track": state.get("track", ""),
        "complexity": state.get("complexity", ""),
        "project_kind": state.get("project_kind", ""),
        "phase": state.get("phase", ""),
        "status": state.get("status", ""),
        "workflow": state.get("active_workflow", ""),
        "active_story": state.get("active_story", ""),
        "readiness": state.get("readiness", ""),
        "route": snapshot["route"],
        "stories": {
            "total": snapshot["stories"]["total"],
            "counts": snapshot["stories"]["counts"],
            "next": next_story,
        },
        "open_required_input": required_inputs[0] if required_inputs else None,
        "open_review_findings": open_findings[:5],
        "audit": {
            "passed": snapshot["quality"]["audit"]["passed"],
            "error_count": len(audit_errors),
            "errors": audit_errors[:5],
        },
        "resume": snapshot["resume"],
        "recommended_agents": [
            item.get("id", "")
            for item in snapshot["agents"]["recommended"]
            if item.get("id")
        ],
        "context": snapshot["context"],
    }


def print_status_brief(root: Path, state: dict[str, str]) -> None:
    brief = build_status_brief(root, state)
    story_counts = brief["stories"]["counts"]
    next_story = brief["stories"]["next"]
    route = brief["route"]
    print("Forge Method Status")
    print(f"Workspace: {brief['root']}")
    print(f"Project: {brief['project']}")
    if brief.get("track"):
        print(f"Track: {brief['track']} ({brief.get('complexity', '')})")
    print(f"State: {brief['phase']} / {brief['status']} / {brief['workflow']}")
    print(f"Readiness: {brief['readiness']}")
    print(f"Route: {route.get('recommendation', '')}")
    print(f"Next action: {route.get('next_action', '')}")
    resume = brief.get("resume", {})
    if resume:
        print(f"Resume: {resume.get('action', '')} ({'autonomous' if resume.get('autonomous') else 'human-gated'})")
        print(f"Resume summary: {resume.get('summary', '')}")
    print(
        "Stories: "
        f"ready={story_counts.get('ready', 0)} "
        f"in_progress={story_counts.get('in_progress', 0)} "
        f"review={story_counts.get('review', 0)} "
        f"blocked={story_counts.get('blocked', 0)} "
        f"done={story_counts.get('done', 0)}"
    )
    if next_story:
        print(f"Next story: {next_story.get('id')} [{next_story.get('status')}] {next_story.get('title')}")
    else:
        print("Next story: <none>")
    open_input = brief["open_required_input"]
    if open_input:
        print(f"Open required input: {open_input.get('id')} - {open_input.get('prompt')}")
    else:
        print("Open required input: <none>")
    open_findings = brief["open_review_findings"]
    print(f"Open review findings: {len(open_findings)}")
    for item in open_findings[:3]:
        print(f"- {item.get('id')} [{item.get('severity')}] story={item.get('story')}: {item.get('title')}")
    audit = brief["audit"]
    print(f"Audit: {'passed' if audit['passed'] else 'failed'}")
    for error in audit["errors"][:3]:
        print(f"- {error}")
    agents = brief["recommended_agents"]
    print(f"Recommended agents: {', '.join(agents) if agents else '<none>'}")
    context = brief["context"]
    if context.get("load_plan"):
        print(f"Context load plan: {context.get('load_plan')}")


def project_route_summary(project_root: Path, *, base: Path) -> dict[str, str]:
    state = apply_state_defaults(read_flat_yaml(state_path(project_root)))
    return {
        "root": str(project_root),
        "path": display_path(project_root, base=base),
        "project": state.get("project", project_root.name),
        "module": state.get("module", ""),
        "phase": state.get("phase", ""),
        "status": state.get("status", ""),
        "workflow": state.get("active_workflow", ""),
        "next_action": state.get("next_action", ""),
    }


def preflight_command(name: str, *parts: str | Path | int) -> dict[str, str]:
    command_parts: list[str] = [
        command_hint_value(sys.executable),
        command_hint_value(Path(__file__).resolve()),
    ]
    command_parts.extend(command_hint_part(part) for part in parts)
    return {"name": name, "command": " ".join(command_parts)}


def decision_option(
    option_id: str,
    label: str,
    action: str,
    *,
    description: str,
    command: dict[str, str] | None = None,
    project_path: str = "",
    requires: list[str] | None = None,
) -> dict[str, Any]:
    option: dict[str, Any] = {
        "id": option_id,
        "label": label,
        "action": action,
        "description": description,
        "requires": requires or [],
    }
    if command:
        option["command"] = command
    if project_path:
        option["project_path"] = project_path
    return option


def project_route_decision(
    *,
    route: str,
    question: str,
    projects: list[dict[str, str]],
    root: Path,
    scan_depth: int,
    objective: str,
    runtime_repo: bool,
) -> dict[str, Any]:
    options: list[dict[str, Any]] = []
    if route == "workspace-with-projects":
        for project in projects:
            project_root = Path(project["root"])
            options.append(
                decision_option(
                    f"open-{slugify(project.get('path') or project.get('project') or 'project')}",
                    f"Open {project.get('project')}",
                    "open_existing_project",
                    description="Resume this project from its file-backed state.",
                    project_path=project.get("path", ""),
                    command=preflight_command("status", "status", "--root", project_root, "--brief"),
                )
            )
    create_root: str | Path = "<parent-folder-outside-runtime-repo>" if runtime_repo else root
    options.append(
        decision_option(
            "create-new-project",
            "Create a new project",
            "create_new_project",
            description="Create scaffolded durable state from the selected module and objective.",
            requires=["project name", "project objective"],
            command=preflight_command(
                "project-create",
                "project",
                "create",
                "--root",
                create_root,
                "--name",
                "<name>",
                "--module",
                "auto",
                "--objective",
                objective or "<objective>",
            ),
        )
    )
    if route == "runtime-repo":
        options.insert(
            0,
            decision_option(
                "choose-external-workspace",
                "Choose a workspace outside this runtime repo",
                "choose_external_workspace",
                description="Avoid writing project state into the runtime package unless explicitly intentional.",
                requires=["workspace path"],
            ),
        )
    return {
        "required": True,
        "type": "project-route",
        "question": question,
        "options": options,
        "default_option": options[0]["id"] if options else "",
    }


def human_experience_for_route(route: str, *, question: str = "") -> dict[str, Any]:
    route_copy = {
        "existing-method-project": "Achei o estado Forge deste projeto. Vou retomar pelos arquivos, sem fingir que lembro tudo pelo chat.",
        "runtime-repo": "Esta pasta é o motor do Forge Method. Para criar algo com ele, escolha uma pasta de projeto fora do runtime.",
        "workspace-with-projects": "Achei projetos Forge aqui. Me diga qual vamos abrir, ou se vamos começar uma coisa nova.",
        "existing-codebase": "Achei código aqui, mas ainda não achei estado Forge. Isso parece brownfield: primeiro entendo o que já existe, depois mexo.",
        "empty-workspace": "Ainda não achei um projeto Forge nesta pasta. Bora começar direito: me diz o que você quer criar hoje.",
    }.get(route, "Resolve the route before loading broad context.")
    prompt = {
        "existing-method-project": "Vou carregar o próximo passo seguro e seguir pelo estado durável.",
        "runtime-repo": "Qual pasta fora do runtime vamos usar para o projeto real?",
        "workspace-with-projects": "Qual deles é o da vez?",
        "existing-codebase": "Quer que eu inicialize o Forge aqui e comece por discovery brownfield?",
        "empty-workspace": "Me manda um nome e um objetivo em linguagem normal. Eu transformo isso em estado, trilha e próximos passos.",
    }.get(route, "O que o Forge deve resolver agora?")
    return {
        **HUMAN_EXPERIENCE_POLICY,
        "route_summary": route_copy,
        "opening": "Forge Method pega uma ideia e conduz por discovery, spec, build, validação e pronto-para-uso.",
        "prompt": prompt,
        "question_context": question,
    }


def reality_evidence_assessment(text: str) -> dict[str, Any]:
    normalized = text.lower()
    if not normalized.strip():
        return {
            "required": False,
            "status": "not-applicable",
            "score": None,
            "summary": "",
            "workflows": REALITY_SCAN_WORKFLOWS,
            "next_step": "",
        }
    dog_impossible = any(term in normalized for term in ["dog", "cachorro"]) and any(
        term in normalized for term in ["delegate", "delegado", "speech", "speach", "speak", "talk", "falar", "fala"]
    )
    cat_cruel = any(term in normalized for term in ["cat", "gato"]) and any(
        term in normalized for term in ["spray", "water", "agua", "espirra", "cuspir", "jato"]
    )
    if dog_impossible:
        return {
            "required": True,
            "status": "blocked",
            "score": 0,
            "summary": "Physical or biological impossibility. Separate the fantasy from any useful product seed before market claims.",
            "workflows": ["reality-evidence-gate", "technical-feasibility-scan", "domain-scan"],
            "next_step": "Pivot to a realistic assistant, simulation, training aid, or entertainment concept.",
        }
    if cat_cruel:
        return {
            "required": True,
            "status": "blocked",
            "score": 0,
            "summary": "Animal-welfare and product-safety failure. Scarcity in the market does not make a cruel idea viable.",
            "workflows": ["reality-evidence-gate", "domain-scan", "market-scan"],
            "next_step": "Pivot to humane deterrence, environmental design, or owner education.",
        }
    return {
        "required": True,
        "status": "needs-evidence",
        "score": None,
        "summary": "Run the Reality/Evidence Gate before treating the idea as a market opportunity.",
        "workflows": REALITY_SCAN_WORKFLOWS,
        "next_step": "Check feasibility, user pain, alternatives, risks, and minimum evidence during discovery.",
    }


def print_human_experience_intro(payload: dict[str, Any]) -> None:
    experience = payload.get("human_experience", {})
    if not experience:
        return
    print("Forge Method")
    print(experience.get("opening", "Forge Method helps turn intent into validated work."))
    if experience.get("route_summary"):
        print(experience["route_summary"])
    if experience.get("prompt"):
        print(experience["prompt"])
    assessment = payload.get("reality_evidence_gate") or {}
    if assessment.get("required") and assessment.get("status") != "not-applicable":
        score = assessment.get("score")
        score_text = "" if score is None else f" ({score}/10)"
        print(f"Reality/Evidence Gate: {assessment.get('status')}{score_text}")
        print(f"Reality check: {assessment.get('summary')}")
        if assessment.get("next_step"):
            print(f"Reality next: {assessment.get('next_step')}")
    print("")


def print_missing_state_start_intro(route: str) -> None:
    print_human_experience_intro(
        {
            "human_experience": human_experience_for_route(route),
            "reality_evidence_gate": reality_evidence_assessment(""),
        }
    )


def setup_label_for_route(route: str) -> str:
    return {
        "runtime-repo": "choose a project workspace outside the runtime",
        "workspace-with-projects": "choose an existing project or start a new one",
        "existing-codebase": "ready for brownfield discovery",
        "empty-workspace": "ready to create the first Forge project here",
    }.get(route, "resolve route")


def build_preflight(root: Path, *, scan_depth: int, max_chars: int, objective: str = "") -> dict[str, Any]:
    state_root, state = load_state_or_none(root)
    runtime_root = find_runtime_repo_root(root)
    runtime_repo = runtime_root is not None
    if state_root:
        status = build_status_brief(state_root, state)
        context_plan = build_context_load_plan(state_root, state, max_chars=max_chars)
        context_health = build_context_health(state_root, state, max_chars=max_chars, plan=context_plan)
        commands = [
            preflight_command("status", "status", "--root", state_root, "--brief"),
            preflight_command("context-plan", "context", "plan", "--root", state_root, "--json", "--max-chars", max_chars),
            preflight_command("context-health", "context", "health", "--root", state_root, "--json", "--max-chars", max_chars),
            preflight_command("next", "next", "--root", state_root),
        ]
        route = status["route"].get("recommendation", "")
        next_story = status["stories"].get("next") or {}
        if route == "wait_for_human_input":
            commands.append(preflight_command("human-input", "input", "list", "--root", state_root))
        elif route == "resolve_review_findings":
            commands.append(preflight_command("review-findings", "review", "list", "--root", state_root))
        elif route == "start_next_story" and next_story.get("id"):
            commands.append(preflight_command("start-story", "story", "start", "--root", state_root, "--id", next_story["id"]))
        open_required_input = status.get("open_required_input") or {}
        return {
            "runtime": RUNTIME_NAME,
            "runtime_version": RUNTIME_VERSION,
            "generated_at": utc_now(),
            "workspace": str(root),
            "route": "existing-method-project",
            "human_experience": human_experience_for_route("existing-method-project", question=objective),
            "reality_evidence_gate": reality_evidence_assessment(objective),
            "runtime_repo": runtime_repo,
            "runtime_root": str(runtime_root) if runtime_root else "",
            "project_root": str(state_root),
            "project_path": display_path(state_root, base=root),
            "decision_required": bool(status.get("open_required_input")),
            "question": "",
            "decision": {
                "required": bool(open_required_input),
                "type": "resume",
                "question": open_required_input.get("prompt", ""),
                "options": [
                    decision_option(
                        "continue-current-project",
                        "Continue current project",
                        "continue_current_project",
                        description="Resume from the current file-backed state and recommended next action.",
                        project_path=display_path(state_root, base=root),
                        command=preflight_command("resume", "resume", "--root", state_root),
                    )
                ],
                "default_option": "continue-current-project",
            },
            "status": status,
            "context_load_plan": context_plan,
            "context_health": context_health,
            "commands": commands,
            "rules": [
                "treat project_root as the authoritative working directory",
                "load context_load_plan.selected before reading broader docs",
                "do not infer phase or project identity from chat history",
                "write evidence, checkpoint, or state before marking progress done",
            ],
        }

    projects = [] if runtime_repo else [
        project_route_summary(project, base=root)
        for project in discover_project_roots(root, max_depth=scan_depth)
    ]
    if runtime_repo:
        route = "runtime-repo"
        question = "Which project folder should be opened or created outside the runtime repo?"
    elif projects:
        route = "workspace-with-projects"
        question = "Which existing project should be opened, or should a new project be created?"
    elif is_brownfield_workspace(root):
        route = "existing-codebase"
        question = "Initialize Forge Method for this existing project as brownfield?"
    else:
        route = "empty-workspace"
        question = "Create a new method project in this workspace?"

    module_choices = project_creation_module_choices(None, objective, limit=8)
    create_root: str | Path = "<parent-folder-outside-runtime-repo>" if runtime_repo else root
    list_root: str | Path = create_root if runtime_repo else root
    commands = [
        preflight_command("project-list", "project", "list", "--root", list_root, "--scan-depth", scan_depth),
        preflight_command(
            "project-create",
            "project",
            "create",
            "--root",
            create_root,
            "--name",
            "<name>",
            "--module",
            "auto",
            "--objective",
            objective or "<objective>",
        ),
    ]
    decision = project_route_decision(
        route=route,
        question=question,
        projects=projects,
        root=root,
        scan_depth=scan_depth,
        objective=objective,
        runtime_repo=runtime_repo,
    )
    if route == "existing-codebase":
        decision = {
            "required": True,
            "type": "project-route",
            "question": question,
            "options": [
                decision_option(
                    "initialize-brownfield-project",
                    "Initialize this existing project",
                    "initialize_brownfield_project",
                    description="Create Forge Method state in this codebase and start with brownfield discovery.",
                    requires=["project name", "project objective"],
                    command=preflight_command(
                        "project-create-brownfield",
                        "project",
                        "create",
                        "--root",
                        root.parent,
                        "--path",
                        root,
                        "--name",
                        "<name>",
                        "--module",
                        "auto",
                        "--objective",
                        objective or "<objective>",
                        "--brownfield",
                    ),
                    project_path=".",
                ),
                decision_option(
                    "create-new-project",
                    "Create a separate new project",
                    "create_new_project",
                    description="Create a new scaffolded project beside this existing codebase.",
                    requires=["project name", "project objective"],
                    command=preflight_command(
                        "project-create",
                        "project",
                        "create",
                        "--root",
                        root.parent,
                        "--name",
                        "<name>",
                        "--module",
                        "auto",
                        "--objective",
                        objective or "<objective>",
                    ),
                ),
            ],
            "default_option": "initialize-brownfield-project",
        }
        commands = [
            preflight_command(
                "project-create-brownfield",
                "project",
                "create",
                "--root",
                root.parent,
                "--path",
                root,
                "--name",
                "<name>",
                "--module",
                "auto",
                "--objective",
                objective or "<objective>",
                "--brownfield",
            ),
            preflight_command("module-recommend", "module", "recommend", "--root", root, "--objective", objective or "<objective>"),
        ]
    return {
        "runtime": RUNTIME_NAME,
        "runtime_version": RUNTIME_VERSION,
        "generated_at": utc_now(),
        "workspace": str(root),
        "route": route,
        "human_experience": human_experience_for_route(route, question=objective),
        "reality_evidence_gate": reality_evidence_assessment(objective),
        "runtime_repo": runtime_repo,
        "runtime_root": str(runtime_root) if runtime_root else "",
        "project_state": "missing",
        "decision_required": True,
        "question": question,
        "decision": decision,
        "known_projects": projects,
        "module_choices": module_choices,
        "commands": commands,
        "rules": [
            "do not initialize project state in the runtime repo unless explicitly intentional",
            "ask the user to choose an existing project or name a new one",
            "use module auto-selection only after the objective is known",
        ],
    }


def print_preflight(payload: dict[str, Any]) -> None:
    if payload.get("route") != "existing-method-project":
        print_human_experience_intro(payload)
    print("Forge Method Preflight")
    print(f"Workspace: {payload['workspace']}")
    print(f"Route: {payload['route']}")
    if payload.get("route") == "existing-method-project":
        status = payload["status"]
        route = status["route"]
        story = status["stories"].get("next") or {}
        print(f"Project root: {payload['project_root']}")
        print(f"Project: {status.get('project', '')}")
        print(f"State: {status.get('phase', '')} / {status.get('status', '')} / {status.get('workflow', '')}")
        print(f"Recommendation: {route.get('recommendation', '')}")
        print(f"Next action: {route.get('next_action', '')}")
        resume = status.get("resume", {})
        if resume:
            print(f"Resume: {resume.get('action', '')} ({'autonomous' if resume.get('autonomous') else 'human-gated'})")
            print(f"Resume summary: {resume.get('summary', '')}")
        if story:
            print(f"Next story: {story.get('id')} [{story.get('status')}] {story.get('title')}")
        else:
            print("Next story: <none>")
        if status.get("open_required_input"):
            item = status["open_required_input"]
            print(f"Open required input: {item.get('id')} - {item.get('prompt')}")
        else:
            print("Open required input: <none>")
        print(f"Open review findings: {len(status.get('open_review_findings', []))}")
        audit = status["audit"]
        print(f"Audit: {'passed' if audit['passed'] else 'failed'}")
        for error in audit["errors"][:3]:
            print(f"- {error}")
        plan = payload["context_load_plan"]
        print(
            "Context budget: "
            f"{plan.get('estimated_selected_chars', 0)}/{plan.get('budget_chars', 0)} chars selected"
        )
        print("Read first:")
        for item in plan.get("selected", [])[:8]:
            print(f"- {item.get('path')} [{item.get('section')}]: {item.get('reason')}")
        if not plan.get("selected"):
            print("- <none>")
        decision = payload.get("decision", {})
        if decision.get("options"):
            print("Decision options:")
            for index, option in enumerate(decision.get("options", []), start=1):
                print(f"{index}. {option.get('label')} ({option.get('action')})")
    else:
        print(f"Runtime repo: {'yes' if payload.get('runtime_repo') else 'no'}")
        if payload.get("runtime_root"):
            print(f"Runtime root: {payload.get('runtime_root')}")
        print(f"Forge setup: {setup_label_for_route(payload.get('route', ''))}")
        projects = payload.get("known_projects", [])
        if projects:
            print("Known projects:")
            for index, project in enumerate(projects, start=1):
                print(
                    f"{index}. {project.get('project')}\t"
                    f"{project.get('phase')}\t"
                    f"{project.get('status')}\t"
                    f"{project.get('path')}"
                )
        else:
            print("Known projects: none")
        print(f"Next question: {payload.get('question', '')}")
        print("Module choices:")
        for item in payload.get("module_choices", []):
            print(f"- {item.get('id')}: {item.get('purpose')}")
        decision = payload.get("decision", {})
        if decision.get("options"):
            print("Decision options:")
            for index, option in enumerate(decision.get("options", []), start=1):
                requirement = ""
                if option.get("requires"):
                    requirement = f" requires: {', '.join(option.get('requires', []))}"
                print(f"{index}. {option.get('label')} ({option.get('action')}){requirement}")
    print("Commands:")
    for item in payload.get("commands", []):
        print(f"- {item.get('name')}: {item.get('command')}")


def build_reload_payload(root: Path, *, scan_depth: int) -> dict[str, Any]:
    state_root, state = load_state_or_none(root)
    runtime_root = find_runtime_repo_root(root)
    runtime_repo = runtime_root is not None
    commands = [
        preflight_command("preflight", "preflight", "--root", root),
        preflight_command("start", "start", "--root", root),
    ]
    bootstrap_contract = {
        "current_filesystem_authoritative": True,
        "launcher_output_authoritative": True,
        "do_not_replay_chat_state": True,
        "read_before_route": ["active SKILL.md", "launcher/runtime output"],
        "avoid_before_route": ["project docs", "source files", "git history", "broad workspace scans"],
        "stale_reply_policy": "Do not answer with prior waiting/init/state-file wording.",
    }
    base: dict[str, Any] = {
        "runtime": RUNTIME_NAME,
        "runtime_version": RUNTIME_VERSION,
        "generated_at": utc_now(),
        "workspace": str(root),
        "skill_dir": str(SKILL_DIR),
        "bootstrap_contract": bootstrap_contract,
        "runtime_repo": runtime_repo,
        "runtime_root": str(runtime_root) if runtime_root else "",
    }
    if state_root:
        commands.extend(
            [
                preflight_command("resume", "resume", "--root", state_root, "--json"),
                preflight_command("next", "next", "--root", state_root),
            ]
        )
        base.update(
            {
                "route": "existing-method-project",
                "project_root": str(state_root),
                "project": state.get("project", ""),
                "phase": state.get("phase", ""),
                "status": state.get("status", ""),
                "workflow": state.get("active_workflow", ""),
                "human_experience": human_experience_for_route("existing-method-project"),
                "decision_required": False,
                "question": "",
                "commands": commands,
            }
        )
        return base

    projects = [] if runtime_repo else [
        project_route_summary(project, base=root)
        for project in discover_project_roots(root, max_depth=scan_depth)
    ]
    if runtime_repo:
        route = "runtime-repo"
        question = "Which project folder should be opened or created outside the runtime repo?"
    elif projects:
        route = "workspace-with-projects"
        question = "Which existing project should be opened, or should a new project be created?"
        commands.append(preflight_command("project-list", "project", "list", "--root", root, "--scan-depth", scan_depth))
    elif is_brownfield_workspace(root):
        route = "existing-codebase"
        question = "Initialize Forge Method for this existing project as brownfield?"
    else:
        route = "empty-workspace"
        question = "Create a new method project in this workspace?"
    base.update(
        {
            "route": route,
            "human_experience": human_experience_for_route(route),
            "reality_evidence_gate": reality_evidence_assessment(""),
            "decision_required": True,
            "question": question,
            "known_projects": projects,
            "commands": commands,
        }
    )
    return base


def print_reload(payload: dict[str, Any]) -> None:
    contract = payload["bootstrap_contract"]
    print("Forge Reload")
    print(f"Runtime: {payload['runtime']} {payload['runtime_version']}")
    print(f"Workspace: {payload['workspace']}")
    print(f"Skill dir: {payload['skill_dir']}")
    print("Contract: current filesystem and launcher output override prior Forge chat state.")
    print("Budget: read only the active skill and launcher output before route; avoid broad project reads.")
    print(f"Stale reply guard: {contract['stale_reply_policy']}")
    print(f"Route: {payload.get('route', '')}")
    if payload.get("route") == "existing-method-project":
        print(f"Project root: {payload.get('project_root', '')}")
        print(f"Project: {payload.get('project', '')}")
        print(f"State: {payload.get('phase', '')} / {payload.get('status', '')} / {payload.get('workflow', '')}")
        print("Next: run resume --json, then continue from durable state.")
    else:
        print_human_experience_intro(payload)
        projects = payload.get("known_projects", [])
        if projects:
            print("Known projects:")
            for index, project in enumerate(projects, start=1):
                print(
                    f"{index}. {project.get('project')}\t"
                    f"{project.get('phase')}\t"
                    f"{project.get('status')}\t"
                    f"{project.get('path')}"
                )
        print(f"Next question: {payload.get('question', '')}")
        print("Next: relay the route opening above; do not replace it with cached initialization copy.")
    print("Commands:")
    for item in payload.get("commands", []):
        print(f"- {item.get('name')}: {item.get('command')}")


def write_state(root: Path, state: dict[str, Any]) -> None:
    state.setdefault("schema_version", "1")
    state.setdefault("runtime", RUNTIME_NAME)
    state.setdefault("runtime_version", RUNTIME_VERSION)
    state.setdefault("autonomy_mode", "auto")
    state.setdefault("commit_policy", "off")
    state.setdefault("last_grill_artifact", "")
    state.setdefault("last_correct_course_artifact", "")
    write_flat_yaml(state_path(root), state, header="Forge Method state")


def initialize_project_state(
    root: Path,
    *,
    project: str,
    mode: str,
    module: str,
    force: bool = False,
    allow_runtime_state: bool = False,
    no_project_guidance: bool = False,
) -> tuple[dict[str, str], Path, list[str]]:
    if is_runtime_repo(root) and not allow_runtime_state:
        raise SystemExit("Refusing to initialize project state in the runtime repo. Use --allow-runtime-state if intentional.")
    fm = ensure_dirs(root)
    path = state_path(root)
    if path.exists() and not force:
        raise FileExistsError(str(path))

    project_id = slugify(project)
    track = default_track_for_module(module)
    state = {
        "schema_version": "1",
        "runtime": RUNTIME_NAME,
        "runtime_version": RUNTIME_VERSION,
        "project": project,
        "project_id": project_id,
        "mode": mode,
        "module": module,
        "track": track["id"],
        "complexity": track["complexity"],
        "project_kind": track["project_kind"],
        "phase": "0-route",
        "status": "route-ready",
        "active_workflow": "start-runtime",
        "active_story": "",
        "human_input_required": "false",
        "readiness": "not_ready",
        "guide_summary": "",
        "last_council_artifact": "",
        "last_grill_artifact": "",
        "last_correct_course_artifact": "",
        "autonomy_mode": "auto",
        "commit_policy": "off",
        "next_action": NEXT_BY_PHASE["0-route"],
    }
    write_state(root, state)
    write_flat_yaml(
        fm / PROJECTS_FILE,
        {
            "project": project,
            "project_id": project_id,
            "root": str(root),
            "runtime_repo": "false",
        },
        header="Forge Method project registry",
    )
    update_sprint(root)
    copied_guidance = [] if no_project_guidance else copy_project_guidance(root, force=force)
    append_ledger(
        root,
        "project.initialized",
        {"project": project, "project_id": project_id, "guidance": copied_guidance},
    )
    return state, path, copied_guidance


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
    state = apply_state_defaults(read_flat_yaml(state_path(root)))
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


def human_input_path(root: Path, input_id: str) -> Path:
    return method_dir(root) / "inputs" / f"{slugify(input_id)}.yaml"


def load_human_input(root: Path, input_id: str) -> dict[str, str]:
    path = human_input_path(root, input_id)
    if not path.exists():
        raise SystemExit(f"Human input not found: {input_id}")
    return read_flat_yaml(path)


def save_human_input(root: Path, item: dict[str, Any]) -> None:
    input_id = item.get("id")
    if not input_id:
        raise SystemExit("Human input must have an id.")
    write_flat_yaml(human_input_path(root, str(input_id)), item, header="Forge Method human input")


def list_human_inputs(root: Path) -> list[dict[str, str]]:
    inputs_dir = method_dir(root) / "inputs"
    if not inputs_dir.exists():
        return []
    items = [read_flat_yaml(path) for path in sorted(inputs_dir.glob("*.yaml"))]
    return [item for item in items if item.get("id")]


def open_required_inputs(root: Path) -> list[dict[str, str]]:
    return [
        item
        for item in list_human_inputs(root)
        if item.get("status") == "open" and item.get("required", "true") == "true"
    ]


def human_input_summary(item: dict[str, str] | None) -> dict[str, str] | None:
    if not item:
        return None
    return {
        "id": item.get("id", ""),
        "prompt": item.get("prompt", ""),
        "reason": item.get("reason", ""),
        "status": item.get("status", ""),
        "phase": item.get("phase", ""),
        "required": item.get("required", ""),
        "answer": item.get("answer", ""),
    }


def sync_human_input_state(root: Path, state: dict[str, str], *, next_action: str = "") -> None:
    open_inputs = open_required_inputs(root)
    if open_inputs:
        first = open_inputs[0]
        state["human_input_required"] = "true"
        state["next_action"] = next_action or f"answer human input {first.get('id')}: {first.get('prompt')}"
        state["status"] = "waiting-human-input"
    else:
        state["human_input_required"] = "false"
        if state.get("status") == "waiting-human-input" or state.get("next_action", "").startswith("answer human input "):
            state["status"] = "input-resolved"
        if next_action:
            state["next_action"] = next_action
        elif state.get("status") == "input-resolved":
            state["next_action"] = NEXT_BY_PHASE.get(state.get("phase", ""), "inspect state and choose next workflow")


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


def story_decision_artifact_sources(root: Path) -> list[str]:
    sources: list[str] = []
    for entry in artifact_states(root).values():
        if entry.get("status", "active") == "captured":
            continue
        kind = slugify(str(entry.get("kind", "")))
        path = str(entry.get("path", ""))
        title = normalize_text(str(entry.get("title", "")))
        haystack = normalize_text(f"{kind} {path} {title}")
        if kind in STORY_DECISION_ARTIFACT_KINDS or any(hint in haystack for hint in STORY_DECISION_PATH_HINTS):
            sources.append(path)
    return sorted(set(sources))


def story_is_implementation_ready_candidate(story: dict[str, str]) -> bool:
    if story.get("status", "") not in {"ready", "in_progress", "review"}:
        return False
    return story.get("phase", "") == "4-build-verify"


def story_decision_source_errors(root: Path, story: dict[str, str], sources: list[str] | None = None) -> list[str]:
    if not story_is_implementation_ready_candidate(story):
        return []
    available = sources if sources is not None else story_decision_artifact_sources(root)
    if available:
        return []
    return [
        (
            f"{story.get('id')}: implementation-ready story has no decision artifact source "
            "(run story-creation/readiness-check or add/link PRD, spec, UX, architecture, test, or validation artifact)"
        )
    ]


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


def module_manifests(root: Path | None = None) -> list[tuple[dict[str, str], Path]]:
    manifests: list[tuple[dict[str, str], Path]] = []
    seen: set[str] = set()
    for path in module_manifest_paths(root):
        module = read_flat_yaml(path)
        module_id = module.get("id", path.stem)
        if module_id in seen:
            continue
        module.setdefault("id", module_id)
        seen.add(module_id)
        manifests.append((module, path))
    return manifests


def module_manifest_by_id(root: Path | None, module_id: str) -> tuple[dict[str, str], Path] | None:
    normalized = slugify(module_id)
    for module, path in module_manifests(root):
        if module.get("id") == normalized:
            return module, path
    return None


def module_summary(module: dict[str, str], *, score: int | None = None, reason: str = "") -> dict[str, Any]:
    summary: dict[str, Any] = {
        "id": module.get("id", ""),
        "title": module.get("title", ""),
        "purpose": module.get("purpose", ""),
        "phase_span": module.get("phase_span", ""),
        "workflows": module.get("workflows", ""),
    }
    if score is not None:
        summary["score"] = score
    if reason:
        summary["reason"] = reason
    return summary


def normalize_text(value: str) -> str:
    return unicodedata.normalize("NFKD", value.lower()).encode("ascii", "ignore").decode("ascii")


def objective_tokens(value: str) -> set[str]:
    tokens = set(re.findall(r"[a-z0-9]+", normalize_text(value)))
    stopwords = {
        "and",
        "any",
        "build",
        "create",
        "for",
        "from",
        "into",
        "make",
        "that",
        "the",
        "with",
        "quero",
        "queria",
        "fazer",
        "criar",
        "preciso",
    }
    return {token for token in tokens if len(token) > 2 and token not in stopwords}


def score_module_for_objective(module: dict[str, str], objective: str) -> tuple[int, str]:
    tokens = objective_tokens(objective)
    if not tokens:
        return 0, "no objective supplied"
    searchable = " ".join(
        [
            module.get("id", ""),
            module.get("title", ""),
            module.get("purpose", ""),
            module.get("phase_span", ""),
            module.get("workflows", ""),
        ]
    ).lower()
    matches = sorted(token for token in tokens if token in searchable)
    score = len(matches)
    if module.get("id") == "software-builder" and any(token in tokens for token in {"app", "api", "code", "software", "web"}):
        score += 2
        matches.append("software")
    if module.get("id") == "game-studio" and any(
        token in tokens
        for token in {
            "game",
            "play",
            "prototype",
            "rpg",
            "vtt",
            "tabletop",
            "campaign",
            "dice",
            "rulebook",
            "rules",
            "jogar",
            "mesa",
            "regras",
            "livro",
        }
    ):
        score += 4
        matches.append("game")
    if module.get("id") == "creative-studio" and any(token in tokens for token in {"brand", "creative", "content", "story"}):
        score += 2
        matches.append("creative")
    if module.get("id") == "launch-ops" and any(token in tokens for token in {"launch", "release", "operate", "ops"}):
        score += 2
        matches.append("operate")
    if module.get("id") == "enterprise" and any(token in tokens for token in {"enterprise", "security", "privacy", "compliance", "risk", "production"}):
        score += 2
        matches.append("enterprise")
    if module.get("id") == "runtime-builder" and any(token in tokens for token in {"runtime", "workflow", "module", "agent"}):
        score += 2
        matches.append("runtime")
    reason = f"matched: {', '.join(dict.fromkeys(matches))}" if matches else "fallback option"
    return score, reason


def recommended_modules(root: Path | None, objective: str, *, limit: int = 5) -> list[dict[str, Any]]:
    scored: list[dict[str, Any]] = []
    for module, _ in module_manifests(root):
        score, reason = score_module_for_objective(module, objective)
        scored.append(module_summary(module, score=score, reason=reason))
    scored.sort(key=lambda item: (-int(item.get("score", 0)), item.get("id", "")))
    if objective and scored and int(scored[0].get("score", 0)) == 0:
        for item in scored:
            item["reason"] = "no strong objective match; choose by purpose"
    return scored[:limit]


def project_creation_module_choices(root: Path | None, objective: str, *, limit: int = 5) -> list[dict[str, Any]]:
    choices = recommended_modules(root, objective, limit=20)
    if objective:
        choices = [
            item for item in choices
            if item.get("id") != "core-runtime" or int(item.get("score", 0)) > 0
        ]
    else:
        preferred_order = {
            "software-builder": 0,
            "creative-studio": 1,
            "game-studio": 2,
            "runtime-builder": 3,
            "test-architect": 4,
            "enterprise": 5,
            "launch-ops": 6,
        }
        choices = [item for item in choices if item.get("id") in preferred_order]
        choices.sort(key=lambda item: (preferred_order.get(str(item.get("id", "")), 99), str(item.get("id", ""))))
    return choices[:limit]


def initial_workflow_for_module(module_id: str) -> str:
    return {
        "game-studio": "game-brief",
        "creative-studio": "creative-session",
        "runtime-builder": "runtime-builder",
        "test-architect": "test-engagement-model",
    }.get(module_id, "discover-intent")


def initial_facilitation_prompt(module_id: str, objective: str) -> str:
    prompts = {
        "game-studio": (
            "Antes de stories, arquitetura ou seguranca: qual fantasia do jogador, tema/tom visual, "
            "referencias boas e ruins, primeiro modo jogavel, publico, postura da IA e o que isso nao deve virar?"
        ),
        "creative-studio": (
            "Antes de especificar: qual sensacao, audiencia, referencias boas e ruins, criterios de gosto, "
            "restricoes e direcoes que devemos rejeitar?"
        ),
        "runtime-builder": (
            "Antes de mexer no runtime: qual comportamento humano falhou, qual contrato compacto o agente precisa, "
            "e qual fixture/eval prova que nao ficou so bonito no texto?"
        ),
        "test-architect": (
            "Antes de criar testes: que risco ou decisao de qualidade estamos respondendo, qual usuario ou fluxo importa, "
            "e que evidencia faria voce confiar no resultado?"
        ),
    }
    if module_id in prompts:
        return prompts[module_id]
    if objective:
        return (
            "Antes de criar stories ou desenvolver: quem usa, qual dor/desejo isso resolve, "
            "qual experiencia faria a pessoa gostar, qual taste/UX importa, quais restricoes existem, "
            "e como saberemos que funcionou?"
        )
    return "Me da o quadro inteiro: para quem e isso, por que importa, que experiencia deve ter, e como saberemos que funcionou?"


def agent_profile_paths(root: Path | None = None) -> list[Path]:
    paths: list[Path] = []
    packaged = SKILL_DIR / "agents" / "profiles"
    if packaged.exists():
        paths.extend(sorted(packaged.glob("*.yaml")))
    if root is not None:
        project_profiles = method_dir(root) / "agents"
        if project_profiles.exists():
            paths.extend(sorted(project_profiles.glob("*.yaml")))
    return paths


def agent_profiles(root: Path | None = None) -> list[tuple[dict[str, str], Path]]:
    profiles: list[tuple[dict[str, str], Path]] = []
    seen: set[str] = set()
    for path in agent_profile_paths(root):
        profile = read_flat_yaml(path)
        profile_id = slugify(profile.get("id", path.stem))
        if profile_id in seen:
            continue
        profile["id"] = profile_id
        seen.add(profile_id)
        profiles.append((profile, path))
    return profiles


def agent_profile_by_id(root: Path | None, profile_id: str) -> tuple[dict[str, str], Path] | None:
    normalized = slugify(profile_id)
    for profile, path in agent_profiles(root):
        if profile.get("id") == normalized:
            return profile, path
    return None


def validate_agent_profile_file(path: Path) -> list[str]:
    if not path.exists():
        return [f"missing agent profile file: {path}"]
    profile = read_flat_yaml(path)
    errors: list[str] = []
    for field in AGENT_PROFILE_REQUIRED_FIELDS:
        if not profile.get(field):
            errors.append(f"{path.name}: missing field `{field}`")
    profile_id = profile.get("id", path.stem)
    if profile_id and profile_id != slugify(profile_id):
        errors.append(f"{path.name}: id must be slug-safe")
    if len(path.read_text(encoding="utf-8").splitlines()) > 80:
        errors.append(f"{path.name}: too long for an agent profile")
    return errors


def agent_profile_validation_errors(root: Path | None = None) -> list[str]:
    errors: list[str] = []
    for path in agent_profile_paths(root):
        errors.extend(validate_agent_profile_file(path))
    return errors


def agent_profile_summary(profile: dict[str, str]) -> dict[str, str]:
    return {
        "id": profile.get("id", ""),
        "title": profile.get("title", ""),
        "purpose": profile.get("purpose", ""),
        "when": profile.get("when", ""),
        "inputs": profile.get("inputs", ""),
        "outputs": profile.get("outputs", ""),
        "handoff": profile.get("handoff", ""),
    }


def recommended_agent_ids(
    state: dict[str, str],
    next_story: dict[str, str] | None,
    audit_errors: list[str],
) -> list[str]:
    if state.get("human_input_required") == "true":
        return ["facilitator"]
    if audit_errors:
        return ["quality-reviewer", "planner"]

    phase = state.get("phase", "0-route")
    if phase == "0-route":
        return ["facilitator"]
    if phase == "1-discovery":
        return ["facilitator", "researcher"]
    if phase == "2-specification":
        return ["spec-architect", "researcher"]
    if phase == "3-plan":
        return ["planner", "quality-reviewer"]
    if phase == "4-build-verify":
        status = (next_story or {}).get("status", "")
        if status == "review":
            return ["quality-reviewer"]
        if status == "blocked":
            return ["facilitator", "planner"]
        return ["implementer", "quality-reviewer"]
    if phase == "5-ready-operate":
        return ["operator", "quality-reviewer"]
    if phase == "6-evolve":
        return ["facilitator", "planner"]
    return ["facilitator"]


def recommended_agent_profiles(
    root: Path | None,
    state: dict[str, str],
    next_story: dict[str, str] | None,
    audit_errors: list[str],
) -> list[dict[str, str]]:
    by_id = {profile.get("id", ""): profile for profile, _ in agent_profiles(root)}
    recommendations: list[dict[str, str]] = []
    for profile_id in recommended_agent_ids(state, next_story, audit_errors):
        profile = by_id.get(profile_id)
        if profile:
            recommendations.append(agent_profile_summary(profile))
    return recommendations


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
    paths: list[Path] = []
    if root is not None:
        project_workflows = method_dir(root) / "workflows"
        if project_workflows.exists():
            paths.extend(sorted(project_workflows.glob("workflow-*.md")))
    refs = SKILL_DIR / "references"
    if refs.exists():
        paths.extend(sorted(refs.glob("workflow-*.md")))
    for path in paths:
        if workflow_id_from_path(path) == normalized:
            return path
    return None


def workflow_catalog_payload() -> dict[str, Any]:
    if not WORKFLOW_CATALOG_PATH.exists():
        return {"schema_version": "", "workflows": []}
    try:
        payload = json.loads(WORKFLOW_CATALOG_PATH.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return {"schema_version": "invalid-json", "workflows": []}
    if not isinstance(payload, dict):
        return {"schema_version": "invalid", "workflows": []}
    workflows = payload.get("workflows", [])
    if not isinstance(workflows, list):
        payload["workflows"] = []
    return payload


def workflow_catalog_entries() -> dict[str, dict[str, Any]]:
    entries: dict[str, dict[str, Any]] = {}
    for item in workflow_catalog_payload().get("workflows", []):
        if not isinstance(item, dict):
            continue
        workflow_id = slugify(str(item.get("id", "")))
        if workflow_id:
            entries[workflow_id] = item
    return entries


def workflow_catalog_entry(workflow_id: str) -> dict[str, Any]:
    return workflow_catalog_entries().get(slugify(workflow_id), {})


def workflow_reference_id(workflow_id: str) -> str:
    entry = workflow_catalog_entry(workflow_id)
    return slugify(str(entry.get("reference", workflow_id)))


def workflow_path_by_catalog_id(root: Path | None, workflow_id: str) -> Path | None:
    return workflow_path_by_id(root, workflow_reference_id(workflow_id))


def facilitation_pack_for_workflow(workflow_id: str) -> str:
    entry = workflow_catalog_entry(workflow_id)
    raw_pack = str(entry.get("facilitation_pack", ""))
    pack_id = slugify(raw_pack) if raw_pack.strip() else ""
    if not pack_id:
        return ""
    path = FACILITATION_DIR / f"{pack_id}.md"
    if path.exists():
        return f"skill:facilitation/{pack_id}.md"
    return ""


def validate_workflow_catalog(root: Path | None = None) -> list[str]:
    errors: list[str] = []
    payload = workflow_catalog_payload()
    if payload.get("schema_version") != "forge-workflow-catalog.v1":
        errors.append("workflow catalog missing schema_version forge-workflow-catalog.v1")
    raw_entries = payload.get("workflows", [])
    if not isinstance(raw_entries, list) or not raw_entries:
        errors.append("workflow catalog has no workflows")
        raw_entries = []
    seen: set[str] = set()
    checked_packs: set[str] = set()
    entries = workflow_catalog_entries()
    for item in raw_entries:
        if not isinstance(item, dict):
            errors.append("workflow catalog entry is not an object")
            continue
        workflow_id = slugify(str(item.get("id", "")))
        if not workflow_id:
            errors.append("workflow catalog entry missing id")
            continue
        if workflow_id in seen:
            errors.append(f"workflow catalog duplicate id: {workflow_id}")
        seen.add(workflow_id)
        for field in ["phase", "required", "outputs"]:
            if field not in item:
                errors.append(f"workflow catalog {workflow_id} missing {field}")
        if not workflow_path_by_id(root, slugify(str(item.get("reference", workflow_id)))):
            errors.append(f"workflow catalog {workflow_id} references missing workflow: {item.get('reference', workflow_id)}")
        raw_pack = str(item.get("facilitation_pack", ""))
        pack_id = slugify(raw_pack) if raw_pack.strip() else ""
        if workflow_id in HUMAN_FACING_REQUIRED_WORKFLOWS and not pack_id:
            errors.append(f"workflow catalog {workflow_id} is human-facing but has no facilitation_pack")
        if pack_id:
            pack_path = FACILITATION_DIR / f"{pack_id}.md"
            if not pack_path.exists():
                errors.append(f"workflow catalog {workflow_id} references missing facilitation pack: {pack_id}")
            elif pack_id not in checked_packs:
                errors.extend(validate_facilitation_pack(pack_path))
                checked_packs.add(pack_id)
        raw_template = str(item.get("template", ""))
        template_id = slugify(raw_template) if raw_template.strip() else ""
        if template_id and not (TEMPLATES_DIR / f"{template_id}.md").exists():
            errors.append(f"workflow catalog {workflow_id} references missing template: {template_id}")
    for module, path in module_manifests(root):
        packaged = SKILL_DIR in path.parents
        for workflow_id in split_list(module.get("workflows", "")):
            normalized = slugify(workflow_id)
            if not workflow_path_by_catalog_id(root, normalized):
                errors.append(f"{path.name}: workflow `{normalized}` has no reference or catalog alias")
            if packaged and normalized not in entries:
                errors.append(f"{path.name}: workflow `{normalized}` missing catalog metadata")
    return errors


def validate_facilitation_pack(path: Path) -> list[str]:
    if not path.exists():
        return [f"missing facilitation pack: {path}"]
    text = path.read_text(encoding="utf-8")
    errors = []
    for section in FACILITATION_REQUIRED_SECTIONS:
        if section not in text:
            errors.append(f"{path.name}: missing facilitation section `{section}`")
    if text.count("\n  - ") < 12:
        errors.append(f"{path.name}: too thin for a human-facing facilitation pack")
    return errors


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


def write_eval(
    root: Path,
    *,
    eval_id: str,
    kind: str,
    target: str,
    query: str,
    expected: str = "",
) -> str:
    if kind not in EVAL_KINDS:
        raise SystemExit(f"Invalid eval kind: {kind}")
    normalized_id = slugify(eval_id)
    values = {
        "id": normalized_id,
        "kind": kind,
        "target": target,
        "query": query,
        "expected": expected or target,
        "status": "pending",
    }
    write_flat_yaml(eval_path(root, normalized_id), values, header="Forge Method eval")
    append_ledger(root, "eval.added", {"id": normalized_id, "kind": kind, "target": target})
    return eval_path(root, normalized_id).relative_to(root).as_posix()


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


def write_artifact_eval(root: Path, artifact_path: str, *, title: str, summary: str) -> str:
    return write_eval(
        root,
        eval_id=f"artifact-{artifact_path}-exists",
        kind="artifact-exists",
        target=artifact_path,
        query=summary or f"{title} exists",
        expected="exists",
    )


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


def recent_checkpoint_paths(root: Path, *, limit: int = 5) -> list[Path]:
    checkpoints = method_dir(root) / "checkpoints"
    if not checkpoints.exists():
        return []
    return sorted(checkpoints.glob("*.md"))[-limit:]


def markdown_section_items(text: str, heading: str) -> list[str]:
    items: list[str] = []
    in_section = False
    target = f"## {heading}"
    for raw_line in text.splitlines():
        line = raw_line.strip()
        if line == target:
            in_section = True
            continue
        if in_section and line.startswith("## "):
            break
        if not in_section or not line.startswith("- "):
            continue
        item = line[2:].strip()
        if item and item.lower() != "none":
            items.append(item)
    return items


def checkpoint_section_items(root: Path, heading: str, *, checkpoint_limit: int = 5, item_limit: int = 12) -> list[str]:
    seen: set[str] = set()
    values: list[str] = []
    for path in recent_checkpoint_paths(root, limit=checkpoint_limit):
        for item in markdown_section_items(path.read_text(encoding="utf-8"), heading):
            if item in seen:
                continue
            seen.add(item)
            values.append(item)
            if len(values) >= item_limit:
                return values
    return values


def append_markdown_items(lines: list[str], values: list[str]) -> None:
    if values:
        lines.extend(f"- {value}" for value in values)
    else:
        lines.append("- none")


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
    state = apply_state_defaults(read_flat_yaml(state_path(root)))
    if not state:
        return ["missing .forge-method/state.yaml"]
    artifact_errors, _ = artifact_findings(root)
    errors.extend(artifact_errors)
    if state.get("runtime") != RUNTIME_NAME:
        errors.append("state.runtime is not forge-method")
    if state.get("phase") not in PHASES:
        errors.append(f"invalid phase: {state.get('phase')}")
    required_inputs = open_required_inputs(root)
    if required_inputs and state.get("human_input_required") != "true":
        errors.append("open required human input exists but state.human_input_required is not true")
    if state.get("human_input_required") == "true" and not required_inputs:
        errors.append("state.human_input_required is true but no open required human input exists")
    if state.get("status") == "waiting-human-input" and not required_inputs:
        errors.append("state is waiting-human-input but no open required human input exists")
    active_story = state.get("active_story", "")
    story_ids = {story.get("id", "") for story in list_stories(root)}
    stories_by_id = {story.get("id", ""): story for story in list_stories(root)}
    if active_story and active_story not in story_ids:
        errors.append(f"active story does not exist: {active_story}")
    decision_sources = story_decision_artifact_sources(root)
    for finding in list_review_findings(root):
        finding_id = finding.get("id", "")
        story_id = finding.get("story", "")
        status = finding.get("status", "")
        severity = finding.get("severity", "")
        if status not in REVIEW_FINDING_STATUSES:
            errors.append(f"{finding_id}: invalid review finding status {status}")
        if severity not in REVIEW_FINDING_SEVERITIES:
            errors.append(f"{finding_id}: invalid review finding severity {severity}")
        story = stories_by_id.get(story_id)
        if not story:
            errors.append(f"{finding_id}: review finding story does not exist: {story_id}")
        elif status == "open" and story.get("status") == "done":
            errors.append(f"{story_id}: done story has open review finding: {finding_id}")
    for story in list_stories(root):
        status = story.get("status", "")
        if status not in STORY_STATUSES:
            errors.append(f"{story.get('id')}: invalid status {status}")
        if status == "done" and not story.get("evidence"):
            errors.append(f"{story.get('id')}: done story has no evidence")
        if status in {"ready", "in_progress", "review"} and not story.get("acceptance_criteria"):
            errors.append(f"{story.get('id')}: executable story has no acceptance criteria")
        errors.extend(story_decision_source_errors(root, story, decision_sources))
        for artifact in split_list(story.get("artifacts")):
            if not (root / artifact).exists() and not artifact_missing_allowed(root, artifact):
                errors.append(f"{story.get('id')}: linked artifact missing: {artifact}")
    return errors


def cmd_init(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    try:
        state, path, copied_guidance = initialize_project_state(
            root,
            project=args.project,
            mode=args.mode,
            module=args.module,
            force=args.force,
            allow_runtime_state=args.allow_runtime_state,
            no_project_guidance=args.no_project_guidance,
        )
    except FileExistsError as exc:
        path = Path(str(exc))
        print(f"State already exists: {path}")
        print("Use --force to replace it.")
        return 2
    print(f"Initialized Forge Method project: {args.project}")
    print(f"State: {path}")
    if copied_guidance:
        print(f"Project guidance: {', '.join(copied_guidance)}")
    print(f"Next: {state['next_action']}")
    return 0


def cmd_start(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    state_root, state = load_state_or_none(root)

    if state_root:
        print("Forge Method Start")
        print(f"Workspace: {root}")
        print("Route: existing-method-project")
        print(f"Project root: {state_root}")
        print_state_summary(state)
        errors = audit_project(state_root)
        print(f"Audit: {'passed' if not errors else 'failed'}")
        for error in errors:
            print(f"- {error}")
        return 0

    runtime_root = find_runtime_repo_root(root)
    runtime_repo = runtime_root is not None
    if runtime_root:
        print_missing_state_start_intro("runtime-repo")
        print("Forge Method Start")
        print(f"Workspace: {root}")
        print(f"Runtime repo: {'yes' if runtime_repo else 'no'}")
        print(f"Runtime root: {runtime_root}")
    if runtime_repo:
        print(f"Forge setup: {setup_label_for_route('runtime-repo')}")
        print("Known projects: not scanned inside runtime repo")
        print("Next question: Which project folder should be opened or created outside the runtime repo?")
        print("Module choices:")
        for item in project_creation_module_choices(None, "", limit=8):
            print(f"- {item.get('id')}: {item.get('purpose')}")
        print("Next: do not initialize project state in the runtime repo unless explicitly intentional.")
        return 0

    projects = discover_project_roots(root, max_depth=args.scan_depth)
    if projects:
        print_missing_state_start_intro("workspace-with-projects")
        print("Forge Method Start")
        print(f"Workspace: {root}")
        print(f"Runtime repo: {'yes' if runtime_repo else 'no'}")
        print(f"Forge setup: {setup_label_for_route('workspace-with-projects')}")
        print("Known projects:")
        for index, project_root in enumerate(projects, start=1):
            project_state = apply_state_defaults(read_flat_yaml(state_path(project_root)))
            label = project_state.get("project", project_root.name)
            phase = project_state.get("phase", "<unknown>")
            status = project_state.get("status", "<unknown>")
            rel = display_path(project_root, base=root)
            print(f"{index}. {label}\t{phase}\t{status}\t{rel}")
        print("Next question: Which known project should be opened, or should a new project be created?")
        print("Module choices for a new project:")
        for item in project_creation_module_choices(None, "", limit=8):
            print(f"- {item.get('id')}: {item.get('purpose')}")
        print("Next: wait for the user's project choice, then run status in that project root or create a scaffolded project.")
        return 0

    if is_brownfield_workspace(root):
        print_missing_state_start_intro("existing-codebase")
        print("Forge Method Start")
        print(f"Workspace: {root}")
        print("Route: existing-codebase")
        print(f"Runtime repo: {'yes' if runtime_repo else 'no'}")
        print(f"Forge setup: {setup_label_for_route('existing-codebase')}")
        print("Known projects: none")
        print("Next question: Initialize Forge Method for this existing project as brownfield?")
        print("Module choices:")
        for item in project_creation_module_choices(None, "", limit=8):
            print(f"- {item.get('id')}: {item.get('purpose')}")
        print(
            "Create command: "
            f"{command_hint_value(sys.executable)} "
            f"{command_hint_value(Path(__file__).resolve())} "
            f"project create --root {command_hint_value(root.parent)} "
            f"--path {command_hint_value(root)} "
            "--name <name> --module auto --objective <objective> --brownfield"
        )
        print("Next: run brownfield discovery before specification, planning, or implementation.")
        return 0

    print_missing_state_start_intro("empty-workspace")
    print("Forge Method Start")
    print(f"Workspace: {root}")
    print(f"Runtime repo: {'yes' if runtime_repo else 'no'}")
    print(f"Forge setup: {setup_label_for_route('empty-workspace')}")
    print("Known projects: none")
    print("Next question: Create a new method project in this workspace?")
    print("Module choices:")
    for item in project_creation_module_choices(None, "", limit=8):
        print(f"- {item.get('id')}: {item.get('purpose')}")
    print(
        "Create command: "
        f"{command_hint_value(sys.executable)} "
        f"{command_hint_value(Path(__file__).resolve())} "
        f"project create --root {command_hint_value(root)} --name <name> --module software-builder"
    )
    print("Next: wait for the project name, then create scaffolded durable state.")
    return 0


def cmd_preflight(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    payload = build_preflight(
        root,
        scan_depth=args.scan_depth,
        max_chars=args.max_chars,
        objective=args.objective or "",
    )
    if args.json:
        print(json.dumps(payload, ensure_ascii=True, sort_keys=True, indent=2))
    else:
        print_preflight(payload)
    return 0


def cmd_reload(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    payload = build_reload_payload(root, scan_depth=args.scan_depth)
    if args.json:
        print(json.dumps(payload, ensure_ascii=True, sort_keys=True, indent=2))
    else:
        print_reload(payload)
    return 0


def cmd_status(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    state_root, state = load_state_or_none(root)
    if state_root is None:
        runtime_root = find_runtime_repo_root(root)
        if runtime_root:
            print(f"Runtime repo: {runtime_root}")
            print("Forge setup: choose a project workspace outside the runtime")
            print("Next: open a project folder or initialize a child project outside the runtime root")
            return 0
        print(f"Workspace: {root}")
        print("Forge setup: ready to create the first Forge project here")
        print("Next: run preflight or start, then ask for project name and objective")
        return 1
    if args.json:
        print(json.dumps(build_status_brief(state_root, state), ensure_ascii=True, sort_keys=True, indent=2))
        return 0
    if args.brief:
        print_status_brief(state_root, state)
        return 0
    print(f"Workspace: {state_root}")
    print_state_summary(state)
    return 0


def story_summary(story: dict[str, str] | None) -> dict[str, str] | None:
    if not story:
        return None
    return {
        "id": story.get("id", ""),
        "title": story.get("title", ""),
        "status": story.get("status", ""),
        "phase": story.get("phase", ""),
        "acceptance_criteria": story.get("acceptance_criteria", ""),
        "checks": story.get("checks", ""),
        "evidence": story.get("evidence", ""),
        "blocker": story.get("blocker", ""),
    }


def review_finding_path(root: Path, finding_id: str) -> Path:
    return method_dir(root) / "reviews" / f"{slugify(finding_id)}.yaml"


def load_review_finding(root: Path, finding_id: str) -> dict[str, str]:
    path = review_finding_path(root, finding_id)
    if not path.exists():
        raise SystemExit(f"Review finding not found: {finding_id}")
    return read_flat_yaml(path)


def save_review_finding(root: Path, finding: dict[str, Any]) -> None:
    finding_id = finding.get("id")
    if not finding_id:
        raise SystemExit("Review finding must have an id.")
    write_flat_yaml(review_finding_path(root, str(finding_id)), finding, header="Forge Method review finding")


def list_review_findings(
    root: Path,
    *,
    story_id: str | None = None,
    status: str | None = None,
) -> list[dict[str, str]]:
    reviews_dir = method_dir(root) / "reviews"
    if not reviews_dir.exists():
        return []
    story_filter = slugify(story_id) if story_id else ""
    findings = [read_flat_yaml(path) for path in sorted(reviews_dir.glob("*.yaml"))]
    items = [item for item in findings if item.get("id")]
    if story_filter:
        items = [item for item in items if item.get("story") == story_filter]
    if status:
        items = [item for item in items if item.get("status") == status]
    return items


def open_review_findings(root: Path, story_id: str | None = None) -> list[dict[str, str]]:
    return list_review_findings(root, story_id=story_id, status="open")


def review_finding_summary(finding: dict[str, str] | None) -> dict[str, str] | None:
    if not finding:
        return None
    return {
        "id": finding.get("id", ""),
        "story": finding.get("story", ""),
        "title": finding.get("title", ""),
        "status": finding.get("status", ""),
        "severity": finding.get("severity", ""),
        "summary": finding.get("summary", ""),
        "source": finding.get("source", ""),
        "resolution": finding.get("resolution", ""),
        "evidence": finding.get("evidence", ""),
    }


def review_finding_counts(findings: list[dict[str, str]]) -> dict[str, int]:
    counts = {status: 0 for status in REVIEW_FINDING_STATUSES}
    for finding in findings:
        status = finding.get("status", "open")
        counts[status] = counts.get(status, 0) + 1
    return counts


def route_recommendation(
    state: dict[str, str],
    next_story: dict[str, str] | None,
    audit_errors: list[str],
    review_findings: list[dict[str, str]] | None = None,
) -> str:
    if state.get("human_input_required") == "true":
        return "wait_for_human_input"
    if state.get("readiness") == "ready" or state.get("phase") == "5-ready-operate":
        return "operate_or_evolve"
    if review_findings and any(item.get("status") == "open" for item in review_findings):
        if not audit_errors or all("open review finding" in error for error in audit_errors):
            return "resolve_review_findings"
    if audit_errors:
        return "repair_project_state"
    if next_story and state.get("phase") == "4-build-verify":
        status = next_story.get("status", "")
        if status in {"ready", "planned"}:
            return "start_next_story"
        if status == "in_progress":
            return "continue_active_story"
        if status == "review":
            return "review_active_story"
        if status == "blocked":
            return "resolve_story_blocker"
    return "continue_current_workflow"


def method_relative_path(root: Path, path: Path) -> str:
    try:
        return path.relative_to(root).as_posix()
    except ValueError:
        return str(path)


def resume_payload(
    *,
    action: str,
    summary: str,
    autonomous: bool,
    commands: list[dict[str, str]],
    target: dict[str, str] | None = None,
    read: list[str] | None = None,
    done_when: list[str] | None = None,
    blocked_when: list[str] | None = None,
) -> dict[str, Any]:
    return {
        "action": action,
        "summary": summary,
        "autonomous": autonomous,
        "target": target or {},
        "read": read or [],
        "commands": commands,
        "next_command": commands[0]["command"] if commands else "",
        "done_when": done_when or [],
        "blocked_when": blocked_when or [],
    }


def build_resume_guidance(
    root: Path,
    state: dict[str, str],
    next_story: dict[str, str] | None,
    audit_errors: list[str],
    required_inputs: list[dict[str, str]],
    open_findings: list[dict[str, str]],
    story_counts: dict[str, int],
) -> dict[str, Any]:
    base_read = [
        method_relative_path(root, state_path(root)),
        method_relative_path(root, method_dir(root) / SPRINT_FILE),
    ]
    if required_inputs:
        item = required_inputs[0]
        input_id = item.get("id", "")
        return resume_payload(
            action="answer_required_input",
            summary=f"Wait for required human input: {item.get('prompt', '')}",
            autonomous=False,
            target=human_input_summary(item) or {},
            read=[*base_read, method_relative_path(root, human_input_path(root, input_id))],
            commands=[
                preflight_command("input-list", "input", "list", "--root", root, "--status", "open"),
                preflight_command("input-answer", "input", "answer", "--root", root, "--id", input_id, "--answer", "<answer>"),
            ],
            done_when=[f"human input {input_id} is answered or deferred"],
            blocked_when=["the answer changes project scope, risk, budget, or acceptance criteria"],
        )

    if state.get("phase") == "5-ready-operate" or (state.get("readiness") == "ready" and state.get("phase") != "6-evolve"):
        return resume_payload(
            action="operate_or_evolve",
            summary="Project is ready for use; operate it or route a new evolution request.",
            autonomous=False,
            target={"phase": state.get("phase", ""), "readiness": state.get("readiness", "")},
            read=base_read,
            commands=[
                preflight_command("status", "status", "--root", root, "--brief"),
                preflight_command("snapshot", "snapshot", "--root", root, "--pretty"),
            ],
            done_when=["user asks for support, operation, or a new evolution objective"],
            blocked_when=["no operation or evolution request is present"],
        )

    review_findings_clear_audit = bool(open_findings) and (
        not audit_errors or all("open review finding" in error for error in audit_errors)
    )
    if open_findings and review_findings_clear_audit:
        finding = open_findings[0]
        finding_id = finding.get("id", "")
        story_id = finding.get("story", "")
        read = [*base_read, method_relative_path(root, review_finding_path(root, finding_id))]
        if story_id:
            read.append(method_relative_path(root, story_path(root, story_id)))
        return resume_payload(
            action="resolve_review_findings",
            summary=f"Resolve open review finding {finding_id} before completing story {story_id}.",
            autonomous=True,
            target=review_finding_summary(finding) or {},
            read=read,
            commands=[
                preflight_command("review-list", "review", "list", "--root", root, "--status", "open"),
                preflight_command("context-plan", "context", "plan", "--root", root, "--json"),
                preflight_command("review-resolve", "review", "resolve", "--root", root, "--id", finding_id, "--resolution", "<resolution>"),
            ],
            done_when=[f"review finding {finding_id} is resolved or waived with evidence"],
            blocked_when=["finding requires product judgment or acceptance criteria change"],
        )

    if audit_errors:
        return resume_payload(
            action="repair_project_state",
            summary=f"Repair project state before continuing: {audit_errors[0]}",
            autonomous=True,
            target={"error_count": str(len(audit_errors)), "first_error": audit_errors[0]},
            read=base_read,
            commands=[
                preflight_command("audit", "audit", "--root", root),
                preflight_command("status", "status", "--root", root, "--brief"),
            ],
            done_when=["audit passes"],
            blocked_when=["state repair would delete user work or change project intent"],
        )

    if state.get("phase") == "4-build-verify":
        if next_story:
            story_id = next_story.get("id", "")
            status = next_story.get("status", "")
            story_read = [*base_read, method_relative_path(root, story_path(root, story_id))]
            if status in {"ready", "planned"}:
                return resume_payload(
                    action="start_next_story",
                    summary=f"Start next story {story_id}: {next_story.get('title', '')}",
                    autonomous=True,
                    target=story_summary(next_story) or {},
                    read=story_read,
                    commands=[
                        preflight_command("story-start", "story", "start", "--root", root, "--id", story_id),
                        preflight_command("context-plan", "context", "plan", "--root", root, "--json"),
                    ],
                    done_when=[f"story {story_id} moves to in_progress and implementation work begins"],
                    blocked_when=["story lacks acceptance criteria or conflicts with current project state"],
                )
            if status == "in_progress":
                return resume_payload(
                    action="continue_active_story",
                    summary=f"Continue active story {story_id}: {next_story.get('title', '')}",
                    autonomous=True,
                    target=story_summary(next_story) or {},
                    read=story_read,
                    commands=[
                        preflight_command("context-plan", "context", "plan", "--root", root, "--json"),
                        preflight_command("status", "status", "--root", root, "--brief"),
                    ],
                    done_when=[f"story {story_id} reaches review or done with evidence"],
                    blocked_when=["implementation needs missing external credentials, user decision, or unavailable dependency"],
                )
            if status == "review":
                return resume_payload(
                    action="review_active_story",
                    summary=f"Review active story {story_id}: {next_story.get('title', '')}",
                    autonomous=True,
                    target=story_summary(next_story) or {},
                    read=story_read,
                    commands=[
                        preflight_command("review-list", "review", "list", "--root", root, "--story", story_id),
                        preflight_command("gate", "gate", "--root", root, "--require-evals"),
                    ],
                    done_when=[f"story {story_id} is marked done or durable findings are created"],
                    blocked_when=["review finds a product decision or acceptance gap"],
                )
            if status == "blocked":
                return resume_payload(
                    action="resolve_story_blocker",
                    summary=f"Resolve blocker on story {story_id}: {next_story.get('blocker', '')}",
                    autonomous=False,
                    target=story_summary(next_story) or {},
                    read=story_read,
                    commands=[
                        preflight_command("story-list", "story", "list", "--root", root),
                        preflight_command("status", "status", "--root", root, "--brief"),
                    ],
                    done_when=[f"story {story_id} returns to ready or in_progress"],
                    blocked_when=["blocker requires human decision or unavailable external dependency"],
                )

        unfinished = sum(story_counts.get(status, 0) for status in ["planned", "ready", "in_progress", "review", "blocked"])
        if story_counts.get("done", 0) > 0 and unfinished == 0:
            return resume_payload(
                action="run_ready_gate",
                summary="All implementation stories are done; run the quality gate and move to ready when it passes.",
                autonomous=True,
                target={"done_stories": str(story_counts.get("done", 0))},
                read=base_read,
                commands=[
                    preflight_command("gate", "gate", "--root", root, "--require-evals", "--summary", "<gate summary>"),
                    preflight_command("ready", "ready", "--root", root, "--summary", "<ready summary>", "--check", "quality gate"),
                ],
                done_when=["quality gate passes", "project phase is 5-ready-operate", "readiness is ready"],
                blocked_when=["gate fails or release evidence is incomplete"],
            )
        return resume_payload(
            action="plan_next_story",
            summary="Build phase has no executable story; plan or import the next story batch.",
            autonomous=False,
            target={"phase": state.get("phase", "")},
            read=base_read,
            commands=[
                preflight_command("story-list", "story", "list", "--root", root),
                preflight_command("story-import", "story", "import", "--root", root, "--file", "<backlog.json>"),
            ],
            done_when=["at least one ready or planned story exists"],
            blocked_when=["project owner has not chosen the next build objective"],
        )

    return resume_payload(
        action="continue_current_workflow",
        summary=state.get("next_action") or NEXT_BY_PHASE.get(state.get("phase", ""), "inspect state and choose next workflow"),
        autonomous=True,
        target={"phase": state.get("phase", ""), "workflow": state.get("active_workflow", "")},
        read=base_read,
        commands=[
            preflight_command("context-plan", "context", "plan", "--root", root, "--json"),
            preflight_command("next", "next", "--root", root),
        ],
        done_when=["workflow done_when conditions are satisfied and state advances"],
        blocked_when=["workflow requires durable human input"],
    )


def effective_config_value(root: Path, state: dict[str, str], key: str, default: str) -> str:
    config, _ = merged_config(root)
    return config.get(key) or state.get(key) or default


def grill_gate_required_for_state(state: dict[str, str]) -> bool:
    return state.get("phase", "") in GRILL_GATE_PHASES


def remaining_mechanical_stories(story_counts: dict[str, int]) -> int:
    return sum(story_counts.get(status, 0) for status in ["planned", "ready", "in_progress", "review"])


def empty_mechanical_work_order(root: Path, state: dict[str, str], resume: dict[str, Any]) -> dict[str, Any]:
    return {
        "autonomous": False,
        "goal_recommended": False,
        "next_mechanical_step": "",
        "required_context": resume.get("read", []),
        "commands": [],
        "done_when": [],
        "self_repair_when": [],
        "stop_only_when": [],
        "correct_course_policy": "",
        "commit_policy": effective_config_value(root, state, "commit_policy", "off"),
    }


def build_mechanical_work_order(
    root: Path,
    state: dict[str, str],
    resume: dict[str, Any],
    story_counts: dict[str, int],
) -> dict[str, Any]:
    action = resume.get("action", "")
    autonomy_mode = effective_config_value(root, state, "autonomy_mode", "auto")
    if autonomy_mode != "auto" or action not in MECHANICAL_ACTIONS:
        return empty_mechanical_work_order(root, state, resume)
    stop_only_when = [
        "missing external credential or access",
        "destructive action requires explicit approval",
        "unavailable external service prevents verification",
        "user explicitly changes scope or constraints",
    ]
    self_repair_when = [
        "required check fails",
        "review finding is open",
        "artifact or evidence is missing",
        "state, sprint, or story status is stale",
    ]
    return {
        "autonomous": bool(resume.get("autonomous")),
        "goal_recommended": bool(resume.get("autonomous")),
        "next_mechanical_step": resume.get("summary", ""),
        "required_context": resume.get("read", []),
        "commands": resume.get("commands", []),
        "done_when": resume.get("done_when", []),
        "self_repair_when": self_repair_when,
        "stop_only_when": stop_only_when,
        "correct_course_policy": (
            "If late contradiction appears, write a compact correct-course artifact, "
            "choose the conservative interpretation that preserves the approved spec, and continue."
        ),
        "commit_policy": effective_config_value(root, state, "commit_policy", "off"),
    }


def build_codex_goal_handoff(state: dict[str, str], work_order: dict[str, Any]) -> dict[str, Any]:
    if not work_order.get("goal_recommended"):
        return {"recommended": False}
    goal_text = "\n".join(
        [
            f"Complete Forge mechanical work for {state.get('project', 'this project')}.",
            f"Start with: {work_order.get('next_mechanical_step', '')}",
            "Run story implementation, review, fixes, tests, evidence, sprint updates, and ready gate autonomously.",
            "Use correct-course continuation for late contradictions and stop only for credentials/access, destructive approval, external-service unavailability, or explicit user scope changes.",
            f"Commit policy: {work_order.get('commit_policy', 'off')}.",
            "Done when all mechanical work is complete, required checks pass, evidence is written, and project state is ready or the next non-mechanical phase is explicit.",
        ]
    )
    return {
        "recommended": True,
        "command": "/goal",
        "goal_text": goal_text,
        "enable_hint": "If /goal is unavailable, enable Codex goals with `codex features enable goals` or `[features] goals = true` in config.toml.",
    }


def default_workflow_for_phase(phase: str) -> str:
    return WORKFLOW_BY_PHASE.get(phase, "guide-route")


def help_oracle_workflow_for_resume(state: dict[str, str], resume: dict[str, Any]) -> tuple[str, str]:
    action = resume.get("action", "")
    active_workflow = state.get("active_workflow", "")
    phase = state.get("phase", "")
    if action == "answer_required_input":
        workflow = active_workflow if active_workflow and active_workflow != "start-runtime" else default_workflow_for_phase(phase)
        return workflow, "Required human input blocks progress; answer or defer it before running another workflow."
    if action == "operate_or_evolve":
        return "guidance-engine", "Ready projects must route new support, feedback, or evolution intent through Guidance Engine."
    if action == "resolve_review_findings":
        return "test-review", "Open review findings are the next required quality workflow before story completion."
    if action == "repair_project_state":
        return "context-recovery", "Project state must be repaired before normal workflow guidance is trustworthy."
    if action in {"start_next_story", "continue_active_story", "review_active_story"}:
        return "build-story", "An executable implementation story is active; continue the build-story loop mechanically."
    if action == "resolve_story_blocker":
        return "correct-course", "A blocked story needs a route correction or human decision before build can continue."
    if action == "run_ready_gate":
        return "ready-release", "All implementation stories are done; run gate and ready-release next."
    if action == "plan_next_story":
        return "plan-sprint", "Build phase has no executable story; plan or import the next story batch."
    if action == "continue_current_workflow":
        return active_workflow or default_workflow_for_phase(phase), "Continue the active workflow selected by durable state."
    return active_workflow or default_workflow_for_phase(phase), "Use durable state and resume action to choose the next workflow."


def build_help_oracle(
    root: Path,
    state: dict[str, str],
    resume: dict[str, Any],
) -> dict[str, Any]:
    workflow_id, reason = help_oracle_workflow_for_resume(state, resume)
    workflow_metadata = workflow_catalog_entry(workflow_id)
    facilitation_pack = facilitation_pack_for_workflow(workflow_id)
    commands = resume.get("commands", [])
    target = resume.get("target", {})
    action = resume.get("action", "")
    human_next_step = resume.get("summary", "") or state.get("next_action", "")
    if action == "answer_required_input" and target.get("id"):
        human_next_step = f"answer human input {target.get('id')}: {target.get('prompt', '')}"
    state_update_required = False
    state_updates: dict[str, str] = {}
    active_workflow = state.get("active_workflow", "")
    if workflow_id and active_workflow and active_workflow != workflow_id and action in {"repair_project_state", "resolve_story_blocker"}:
        state_update_required = True
        state_updates["active_workflow"] = workflow_id
    return {
        "source": "help-oracle",
        "required_next_workflow": workflow_id,
        "recommended_phase": recommended_phase_for_workflow(workflow_metadata, state.get("phase", "")) if workflow_metadata else state.get("phase", ""),
        "reason": reason,
        "human_next_step": human_next_step,
        "facilitation_pack": facilitation_pack,
        "workflow_metadata": workflow_metadata,
        "commands": commands,
        "state_update_required": state_update_required,
        "state_updates": state_updates,
        "stale_state_guard": "Help Oracle is derived from current state, open inputs/findings, stories, audit status, and catalog metadata; do not follow stale chat next steps.",
        "alternatives": guidance_alternatives(
            ("guide-route", "orient the human when intent is unclear"),
            ("guidance-engine", "classify fresh human intent before continuing stale state"),
            ("context-recovery", "recover when state or context looks stale"),
        ),
        "command": commands[0]["command"] if commands else "",
    }


def build_snapshot(root: Path, state: dict[str, str]) -> dict[str, Any]:
    sprint = read_flat_yaml(method_dir(root) / SPRINT_FILE)
    stories = list_stories(root)
    next_story = select_next_story(root)
    inputs = list_human_inputs(root)
    open_inputs = [item for item in inputs if item.get("status") == "open"]
    required_inputs = open_required_inputs(root)
    review_findings = list_review_findings(root)
    open_findings = [item for item in review_findings if item.get("status") == "open"]
    audit_errors = audit_project(root)
    artifact_errors, artifact_warnings = artifact_findings(root)
    agent_errors = agent_profile_validation_errors(root)
    config_errors = config_validation_errors(root)
    evals = list_evals(root)
    eval_counts: dict[str, int] = {"total": len(evals), "passed": 0, "failed": 0, "pending": 0}
    for item in evals:
        status = item.get("status", "pending")
        eval_counts[status] = eval_counts.get(status, 0) + 1
    story_counts = {status: 0 for status in STORY_STATUSES}
    for story in stories:
        status = story.get("status", "planned")
        story_counts[status] = story_counts.get(status, 0) + 1
    resume = build_resume_guidance(
        root,
        state,
        next_story,
        audit_errors,
        required_inputs,
        open_findings,
        story_counts,
    )
    resume["grill_gate_required"] = grill_gate_required_for_state(state)
    resume["mechanical_work_order"] = build_mechanical_work_order(root, state, resume, story_counts)
    resume["codex_goal_handoff"] = build_codex_goal_handoff(state, resume["mechanical_work_order"])
    help_oracle = build_help_oracle(root, state, resume)
    resume["help_oracle"] = help_oracle
    context_dir = method_dir(root) / "context"
    current_pack = context_dir / "current-pack.md"
    recovery = context_dir / "recovery.md"
    compact_recovery = context_dir / "recovery-compact.md"
    load_plan = context_dir / "load-plan.json"
    latest_checkpoint = latest_checkpoint_path(root)
    return {
        "runtime": RUNTIME_NAME,
        "runtime_version": RUNTIME_VERSION,
        "root": str(root),
        "state": state,
        "sprint": sprint,
        "stories": {
            "total": len(stories),
            "counts": story_counts,
            "next": story_summary(next_story),
        },
        "route": {
            "recommendation": route_recommendation(state, next_story, audit_errors, review_findings),
            "next_action": state.get("next_action", ""),
            "human_input_required": state.get("human_input_required", "false"),
        },
        "help_oracle": help_oracle,
        "resume": resume,
        "human_inputs": {
            "total": len(inputs),
            "open": [human_input_summary(item) for item in open_inputs],
            "required_open": [human_input_summary(item) for item in required_inputs],
        },
        "review_findings": {
            "total": len(review_findings),
            "counts": review_finding_counts(review_findings),
            "open": [review_finding_summary(item) for item in open_findings],
        },
        "quality": {
            "audit": {
                "passed": not audit_errors,
                "errors": audit_errors,
            },
            "artifacts": {
                "errors": artifact_errors,
                "warnings": artifact_warnings,
            },
            "agents": {
                "errors": agent_errors,
            },
            "config": {
                "errors": config_errors,
            },
            "evals": eval_counts,
        },
        "agents": {
            "available": len(agent_profiles(root)),
            "recommended": recommended_agent_profiles(root, state, next_story, audit_errors),
        },
        "context": {
            "current_pack": current_pack.relative_to(root).as_posix() if current_pack.exists() else "",
            "recovery_brief": recovery.relative_to(root).as_posix() if recovery.exists() else "",
            "compact_recovery": compact_recovery.relative_to(root).as_posix() if compact_recovery.exists() else "",
            "load_plan": load_plan.relative_to(root).as_posix() if load_plan.exists() else "",
            "latest_checkpoint": latest_checkpoint.relative_to(root).as_posix() if latest_checkpoint.exists() else "",
        },
        "recent_artifacts": recent_artifacts(root, limit=5),
    }


def cmd_snapshot(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    snapshot = build_snapshot(root, state)
    indent = 2 if args.pretty else None
    print(json.dumps(snapshot, ensure_ascii=True, sort_keys=True, indent=indent))
    return 0


def cmd_next(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    snapshot = build_snapshot(root, state)
    help_oracle = snapshot.get("help_oracle", {})
    if help_oracle.get("human_next_step"):
        print(help_oracle["human_next_step"])
        if help_oracle.get("required_next_workflow"):
            print(f"Next required workflow: {help_oracle.get('required_next_workflow')}")
        work_order = snapshot["resume"].get("mechanical_work_order", {})
        if work_order.get("autonomous") and work_order.get("goal_recommended"):
            print("Goal recommended: use /goal with the generated Forge mechanical goal handoff.")
        return 0
    work_order = snapshot["resume"].get("mechanical_work_order", {})
    if work_order.get("autonomous") and work_order.get("next_mechanical_step"):
        print(work_order["next_mechanical_step"])
        if work_order.get("goal_recommended"):
            print("Goal recommended: use /goal with the generated Forge mechanical goal handoff.")
        return 0
    phase = state.get("phase", "0-route")
    if phase == "4-build-verify":
        story = snapshot["stories"]["next"]
        if story:
            print(f"{NEXT_BY_PHASE[phase]}: {story.get('id')} - {story.get('title')}")
            return 0
    print(state.get("next_action") or NEXT_BY_PHASE.get(phase, "inspect state and choose a valid workflow"))
    return 0


def print_resume_guidance(root: Path, resume: dict[str, Any]) -> None:
    print("Forge Method Resume")
    print(f"Workspace: {root}")
    print(f"Action: {resume.get('action', '')}")
    print(f"Autonomous: {'yes' if resume.get('autonomous') else 'no'}")
    print(f"Summary: {resume.get('summary', '')}")
    target = resume.get("target", {})
    if target:
        print("Target:")
        for key, value in target.items():
            if value not in {"", None} and value != []:
                print(f"- {key}: {value}")
    read = resume.get("read", [])
    print("Read:")
    if read:
        for item in read:
            print(f"- {item}")
    else:
        print("- <none>")
    print("Commands:")
    for item in resume.get("commands", []):
        print(f"- {item.get('name')}: {item.get('command')}")
    done_when = resume.get("done_when", [])
    print("Done when:")
    if done_when:
        for item in done_when:
            print(f"- {item}")
    else:
        print("- <not specified>")
    blocked_when = resume.get("blocked_when", [])
    print("Blocked when:")
    if blocked_when:
        for item in blocked_when:
            print(f"- {item}")
    else:
        print("- <not specified>")
    if resume.get("grill_gate_required"):
        print("Grill Gate: required before leaving this decision phase.")
    help_oracle = resume.get("help_oracle", {})
    if help_oracle:
        print("Help Oracle:")
        print(f"- required_next_workflow: {help_oracle.get('required_next_workflow', '')}")
        print(f"- reason: {help_oracle.get('reason', '')}")
        if help_oracle.get("facilitation_pack"):
            print(f"- facilitation: {help_oracle.get('facilitation_pack')}")
    work_order = resume.get("mechanical_work_order", {})
    if work_order.get("autonomous"):
        print("Mechanical Work Order:")
        print(f"- next: {work_order.get('next_mechanical_step', '')}")
        print(f"- commit_policy: {work_order.get('commit_policy', 'off')}")
        print(f"- goal_recommended: {'yes' if work_order.get('goal_recommended') else 'no'}")
        if work_order.get("self_repair_when"):
            print("- self_repair_when: " + " | ".join(work_order["self_repair_when"]))
        if work_order.get("stop_only_when"):
            print("- stop_only_when: " + " | ".join(work_order["stop_only_when"]))
    goal = resume.get("codex_goal_handoff", {})
    if goal.get("recommended"):
        print("Codex Goal Handoff:")
        print(goal.get("goal_text", ""))
        print(goal.get("enable_hint", ""))


def cmd_resume(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    snapshot = build_snapshot(root, state)
    resume = snapshot["resume"]
    if args.json:
        print(json.dumps(resume, ensure_ascii=True, sort_keys=True, indent=2))
    else:
        print_resume_guidance(root, resume)
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
    oracle = build_snapshot(root, state).get("help_oracle", {})
    if oracle.get("required_next_workflow"):
        print(f"Required next workflow: {oracle.get('required_next_workflow')}")
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


def story_export_item(story: dict[str, str]) -> dict[str, Any]:
    return {
        "id": story.get("id", ""),
        "title": story.get("title", ""),
        "status": story.get("status", "ready"),
        "phase": story.get("phase", ""),
        "acceptance_criteria": split_list(story.get("acceptance_criteria")),
        "checks": split_list(story.get("checks")),
        "evidence": split_list(story.get("evidence")),
        "artifacts": split_list(story.get("artifacts")),
        "blocker": story.get("blocker", ""),
    }


def normalize_story_import_item(item: dict[str, Any], state: dict[str, str]) -> dict[str, Any]:
    title = str(item.get("title", "")).strip()
    if not title:
        raise SystemExit("Imported stories require a title.")
    story_id = slugify(str(item.get("id") or title))
    status = str(item.get("status") or "ready")
    if status not in STORY_STATUSES:
        raise SystemExit(f"{story_id}: invalid imported story status {status}")

    def list_field(name: str) -> str:
        value = item.get(name, [])
        if isinstance(value, list):
            return join_list([str(part) for part in value])
        return str(value or "")

    return {
        "id": story_id,
        "title": title,
        "status": status,
        "phase": str(item.get("phase") or state.get("phase", "0-route")),
        "acceptance_criteria": list_field("acceptance_criteria"),
        "evidence": list_field("evidence"),
        "checks": list_field("checks"),
        "artifacts": list_field("artifacts"),
        "blocker": str(item.get("blocker") or ""),
    }


def cmd_story_export(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    stories = list_stories(root)
    if args.status:
        stories = [story for story in stories if story.get("status") == args.status]
    payload = {
        "runtime": RUNTIME_NAME,
        "runtime_version": RUNTIME_VERSION,
        "generated_at": utc_now(),
        "project": state.get("project", ""),
        "story_count": len(stories),
        "stories": [story_export_item(story) for story in stories],
    }
    text = json.dumps(payload, ensure_ascii=True, sort_keys=True, indent=2) + "\n"
    if args.out:
        out, rel = project_path(root, args.out)
        out.parent.mkdir(parents=True, exist_ok=True)
        out.write_text(text, encoding="utf-8")
        append_ledger(root, "story.backlog_exported", {"path": rel, "stories": len(stories)})
        print(rel)
        return 0
    print(text.rstrip())
    return 0


def cmd_story_import(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    source, rel = project_path(root, args.file)
    if not source.exists():
        raise SystemExit(f"Story import file not found: {args.file}")
    payload = json.loads(source.read_text(encoding="utf-8"))
    raw_stories = payload.get("stories") if isinstance(payload, dict) else payload
    if not isinstance(raw_stories, list):
        raise SystemExit("Story import file must contain a list or an object with a stories list.")
    imported = 0
    for raw_item in raw_stories:
        if not isinstance(raw_item, dict):
            raise SystemExit("Imported story entries must be objects.")
        story = normalize_story_import_item(raw_item, state)
        path = story_path(root, story["id"])
        if path.exists() and not args.force:
            raise SystemExit(f"Story already exists: {story['id']}")
        save_story(root, story)
        imported += 1
    update_sprint(root)
    append_ledger(root, "story.backlog_imported", {"path": rel, "stories": imported})
    print(f"Stories imported: {imported}")
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
    story_phase = story.get("phase", "4-build-verify")
    if story_phase not in PHASES:
        story_phase = "4-build-verify"
    state["phase"] = story_phase
    state["status"] = "story-in-progress"
    state["active_workflow"] = WORKFLOW_BY_PHASE.get(story_phase, "build-story")
    state["active_story"] = story["id"]
    state["human_input_required"] = "false"
    if story_phase == "4-build-verify":
        state["next_action"] = f"implement and validate story {story['id']}"
    else:
        state["next_action"] = f"{NEXT_BY_PHASE.get(story_phase, 'continue workflow')} for story {story['id']}"
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
    validate_story_transition(story.get("status", "planned"), "done", force=args.force)
    open_findings = open_review_findings(root, story.get("id", ""))
    if open_findings and not args.force:
        ids = ", ".join(item.get("id", "") for item in open_findings)
        raise SystemExit(f"Open review findings must be resolved or waived before done: {ids}")
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
    state["human_input_required"] = "false"
    state["next_action"] = f"resolve blocker for story {story['id']}: {args.reason}"
    write_state(root, state)
    print(f"Story blocked: {story['id']}")
    return 0


def cmd_review_add(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    story_id = slugify(args.story)
    load_story(root, story_id)
    finding_id = slugify(args.id or f"{story_id}-{args.title}")
    path = review_finding_path(root, finding_id)
    if path.exists() and not args.force:
        raise SystemExit(f"Review finding already exists: {finding_id}")
    finding = {
        "id": finding_id,
        "story": story_id,
        "title": args.title,
        "severity": args.severity,
        "status": "open",
        "summary": args.summary,
        "source": args.source or "",
        "resolution": "",
        "evidence": "",
        "created_at": utc_now(),
        "resolved_at": "",
    }
    save_review_finding(root, finding)
    append_ledger(root, "review_finding.added", {"id": finding_id, "story": story_id, "severity": args.severity})
    print(f"Review finding added: {finding_id}")
    return 0


def cmd_review_list(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    findings = list_review_findings(root, story_id=args.story, status=args.status)
    if not findings:
        print("No review findings.")
        return 0
    for finding in findings:
        print(
            f"{finding.get('id')}\t{finding.get('status')}\t{finding.get('severity')}\t"
            f"{finding.get('story')}\t{finding.get('title')}"
        )
    return 0


def cmd_review_resolve(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    finding = load_review_finding(root, args.id)
    evidence = ""
    if args.evidence:
        evidence_path, evidence = project_path(root, args.evidence)
        if not evidence_path.exists():
            raise SystemExit(f"Review evidence not found: {args.evidence}")
    finding["status"] = "resolved"
    finding["resolution"] = args.resolution
    finding["evidence"] = evidence
    finding["resolved_at"] = utc_now()
    save_review_finding(root, finding)
    append_ledger(root, "review_finding.resolved", {"id": finding.get("id"), "story": finding.get("story")})
    print(f"Review finding resolved: {finding.get('id')}")
    return 0


def cmd_review_waive(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    finding = load_review_finding(root, args.id)
    finding["status"] = "waived"
    finding["resolution"] = args.reason
    finding["resolved_at"] = utc_now()
    save_review_finding(root, finding)
    append_ledger(root, "review_finding.waived", {"id": finding.get("id"), "story": finding.get("story")})
    print(f"Review finding waived: {finding.get('id')}")
    return 0


def cmd_input_add(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    input_id = slugify(args.id or args.prompt)
    path = human_input_path(root, input_id)
    if path.exists() and not args.force:
        raise SystemExit(f"Human input already exists: {input_id}")
    item = {
        "id": input_id,
        "prompt": args.prompt,
        "reason": args.reason or "",
        "status": "open",
        "phase": args.phase or state.get("phase", ""),
        "required": "true" if args.required else "false",
        "answer": "",
        "created_at": utc_now(),
        "answered_at": "",
        "deferred_reason": "",
    }
    save_human_input(root, item)
    if args.required:
        sync_human_input_state(root, state)
        write_state(root, state)
    append_ledger(root, "human_input.added", {"id": input_id, "required": args.required})
    print(f"Human input added: {input_id}")
    return 0


def cmd_input_list(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    items = list_human_inputs(root)
    if args.status:
        items = [item for item in items if item.get("status") == args.status]
    if not items:
        print("No human inputs.")
        return 0
    for item in items:
        required = item.get("required", "true")
        print(f"{item.get('id')}\t{item.get('status')}\trequired={required}\t{item.get('prompt')}")
    return 0


def cmd_input_answer(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    item = load_human_input(root, args.id)
    if item.get("status") == "answered" and not args.force:
        raise SystemExit(f"Human input already answered: {args.id}")
    item["status"] = "answered"
    item["answer"] = args.answer
    item["answered_at"] = utc_now()
    item["deferred_reason"] = ""
    save_human_input(root, item)
    sync_human_input_state(root, state, next_action=args.next_action or "")
    write_state(root, state)
    append_ledger(root, "human_input.answered", {"id": item.get("id")})
    print(f"Human input answered: {item.get('id')}")
    return 0


def cmd_input_defer(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    item = load_human_input(root, args.id)
    item["status"] = "deferred"
    item["deferred_reason"] = args.reason
    save_human_input(root, item)
    sync_human_input_state(root, state, next_action=args.next_action or "")
    write_state(root, state)
    append_ledger(root, "human_input.deferred", {"id": item.get("id"), "reason": args.reason})
    print(f"Human input deferred: {item.get('id')}")
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
    if args.eval:
        write_artifact_eval(root, rel, title=args.title, summary=args.summary)
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
    manifests = module_manifests(root)
    if not manifests:
        print("No modules.")
        return 0
    if args.json:
        print(json.dumps({"modules": [module_summary(module) for module, _ in manifests]}, ensure_ascii=True, sort_keys=True, indent=2))
        return 0
    for module, _ in manifests:
        print(f"{module.get('id', '')}\t{module.get('title', '')}\t{module.get('phase_span', '')}\t{module.get('purpose', '')}")
    return 0


def cmd_module_recommend(args: argparse.Namespace) -> int:
    root, _ = load_state_or_none(resolve_root(args.root))
    recommendations = recommended_modules(root, args.objective or "", limit=args.limit)
    if args.json:
        print(json.dumps({"recommended": recommendations}, ensure_ascii=True, sort_keys=True, indent=2))
        return 0
    if not recommendations:
        print("No modules.")
        return 0
    for item in recommendations:
        print(
            f"{item.get('id')}\t{item.get('score', 0)}\t"
            f"{item.get('title')}\t{item.get('reason')}\t{item.get('purpose')}"
        )
    return 0


def cmd_module_show(args: argparse.Namespace) -> int:
    root, _ = load_state_or_none(resolve_root(args.root))
    match = module_manifest_by_id(root, args.id)
    if match:
        _, path = match
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


def resolve_new_project_root(parent: Path, raw_path: str | None, name: str) -> Path:
    if raw_path:
        candidate = Path(raw_path).expanduser()
        if not candidate.is_absolute():
            candidate = parent / candidate
    else:
        candidate = parent / slugify(name)
    return candidate.resolve()


def cmd_project_list(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    projects = discover_project_roots(root, max_depth=args.scan_depth)
    if not projects:
        print("No method projects found.")
        return 0
    for project_root in projects:
        state = apply_state_defaults(read_flat_yaml(state_path(project_root)))
        print(
            f"{display_path(project_root, base=root)}\t"
            f"{state.get('project', project_root.name)}\t"
            f"{state.get('module', '')}\t"
            f"{state.get('phase', '')}\t"
            f"{state.get('status', '')}"
        )
    return 0


def cmd_project_create(args: argparse.Namespace) -> int:
    parent = resolve_root(args.root)
    project_root = resolve_new_project_root(parent, args.path, args.name)
    existing_entries = workspace_entries_for_brownfield(project_root)
    brownfield = bool(args.brownfield or (project_root.exists() and existing_entries and not state_path(project_root).exists() and args.force))
    if project_root.exists() and not project_root.is_dir():
        raise SystemExit(f"Project root must be a directory: {project_root}")
    if args.brownfield and (not project_root.exists() or not existing_entries):
        raise SystemExit(f"--brownfield requires an existing non-empty project root: {project_root}")
    if project_root.exists() and existing_entries and not state_path(project_root).exists() and not args.force and not args.brownfield:
        raise SystemExit(
            f"Project root is not empty: {project_root}. Use --brownfield to initialize an existing project."
        )
    if args.brownfield:
        brownfield = True
    project_root.mkdir(parents=True, exist_ok=True)

    module_id = slugify(args.module)
    if module_id == "auto":
        if not args.objective:
            raise SystemExit("--module auto requires --objective.")
        recommendations = recommended_modules(parent if state_path(parent).exists() else None, args.objective, limit=1)
        if not recommendations:
            raise SystemExit("No modules available for automatic selection.")
        module_id = str(recommendations[0]["id"])
    match = module_manifest_by_id(parent if state_path(parent).exists() else None, module_id)
    if not match:
        match = module_manifest_by_id(None, module_id)
    if not match:
        raise SystemExit(f"Module not found: {args.module}")
    module, _ = match

    project = args.name
    objective = args.objective or module.get("purpose", f"Create {project}.")
    state, path, copied_guidance = initialize_project_state(
        project_root,
        project=project,
        mode="brownfield" if brownfield and args.mode == "creation-runtime" else args.mode,
        module=module_id,
        force=args.force,
        allow_runtime_state=args.allow_runtime_state,
        no_project_guidance=args.no_project_guidance,
    )
    state["phase"] = "1-discovery"
    state["status"] = "brownfield-discovery" if brownfield else "facilitation-needed"
    state["active_workflow"] = "discover-intent" if brownfield else initial_workflow_for_module(module_id)
    if brownfield:
        state["next_action"] = (
            "run brownfield discovery: inventory existing files, current behavior, "
            "in-progress work, constraints, risks, and safe next changes"
        )
    else:
        state["next_action"] = f"run facilitated {state['active_workflow']} before specification, stories, or build"
    write_state(project_root, state)

    story_id = ""
    if brownfield:
        story_id = "project-kickoff"
        story = {
            "id": story_id,
            "title": "Run brownfield discovery",
            "status": "ready",
            "phase": "1-discovery",
            "acceptance_criteria": join_list(
                [
                    "existing project inventory is captured",
                    "current in-progress work is identified",
                    "constraints, risks, and safe change boundaries are documented",
                    "context load plan exists",
                    "quality gate passes with required evals",
                ]
            ),
            "evidence": "",
            "checks": "context plan | gate --require-evals",
            "blocker": "",
        }
        save_story(project_root, story)
    else:
        save_human_input(
            project_root,
            {
                "id": "initial-facilitation",
                "phase": "1-discovery",
                "status": "open",
                "required": "true",
                "prompt": initial_facilitation_prompt(module_id, objective),
                "reason": "New projects must pass human-facing discovery before stories, architecture, or implementation.",
                "answer": "",
            },
        )
        sync_human_input_state(project_root, state)
        write_state(project_root, state)
    update_sprint(project_root)

    workflows = split_list(module.get("workflows"))
    brief_rel = ".forge-method/artifacts/project-brief.md"
    summary = (
        f"Project: {project}. Module: {module_id}. "
        f"Project type: {'brownfield existing codebase' if brownfield else 'new scaffold'}. "
        f"Objective: {objective} "
        f"Module purpose: {module.get('purpose', '')}. "
        f"Workflow set: {join_list(workflows) or 'none'}."
    )
    artifact = write_artifact(
        project_root,
        kind="brief",
        title=f"{project} project brief",
        summary=summary,
        path=brief_rel,
        lifecycle="durable",
    )
    if story_id:
        link_artifact_to_story(project_root, artifact, story_id)
    eval_path_rel = write_artifact_eval(project_root, artifact, title=f"{project} project brief", summary=summary)
    touched = [STATE_FILE, SPRINT_FILE, artifact]
    if story_id:
        touched.insert(2, story_path(project_root, story_id).relative_to(project_root).as_posix())
    else:
        touched.insert(2, human_input_path(project_root, "initial-facilitation").relative_to(project_root).as_posix())
    checkpoint = write_checkpoint(
        project_root,
        state,
        title="Project created",
        summary=(
            f"Initialized brownfield project from module {module_id}."
            if brownfield
            else f"Created project from module {module_id}."
        ),
        decisions=[
            f"Use module {module_id} as the initial route.",
            *(
                ["Treat existing files as brownfield context and run discovery before specification or build."]
                if brownfield
                else [
                    "Do not create implementation stories until the initial human facilitation question is answered.",
                    f"Start human-facing discovery with {state['active_workflow']}.",
                ]
            ),
        ],
        checks=["context plan", "gate --require-evals"],
        failed_checks=[],
        touched=touched,
        artifacts=[artifact],
        next_action=state["next_action"],
    )
    load_plan = write_context_load_plan(
        project_root,
        state,
        out=method_dir(project_root) / "context" / "load-plan.json",
        max_chars=args.max_chars,
    )
    context_pack = write_context_pack(
        project_root,
        state,
        out=method_dir(project_root) / "context" / "current-pack.md",
        max_chars=args.max_chars,
    )

    readme = project_root / "README.md"
    if args.force or not readme.exists():
        readme.write_text(
            "\n".join(
                [
                    f"# {project}",
                    "",
                    f"Module: `{module_id}`",
                    "",
                    "Start by inspecting runtime state and the context load plan:",
                    "",
                    "```powershell",
                    "python \"$HOME\\.agents\\skills\\forge-method\\scripts\\forge_method_runtime.py\" status --root .",
                    "python \"$HOME\\.agents\\skills\\forge-method\\scripts\\forge_method_runtime.py\" context plan --root .",
                    "python \"$HOME\\.agents\\skills\\forge-method\\scripts\\forge_method_runtime.py\" gate --root . --require-evals",
                    "```",
                ]
            )
            + "\n",
            encoding="utf-8",
        )

    append_ledger(
        project_root,
        "project.created",
        {
            "module": module_id,
            "story": story_id,
            "artifact": artifact,
            "eval": eval_path_rel,
            "checkpoint": checkpoint,
            "load_plan": load_plan.relative_to(project_root).as_posix(),
        },
    )

    print(f"Project created: {project}")
    print(f"Project type: {'brownfield' if brownfield else 'new'}")
    print(f"Root: {project_root}")
    print(f"State: {path}")
    print(f"Module: {module_id}")
    print(f"Story: {story_id or '<none - facilitation required>'}")
    print(f"Artifact: {artifact}")
    print(f"Eval: {eval_path_rel}")
    print(f"Checkpoint: {checkpoint}")
    print(f"Context plan: {load_plan.relative_to(project_root).as_posix()}")
    print(f"Context pack: {context_pack.relative_to(project_root).as_posix()}")
    if copied_guidance:
        print(f"Project guidance: {', '.join(copied_guidance)}")
    return 0


def cmd_agent_list(args: argparse.Namespace) -> int:
    root, _ = load_state_or_none(resolve_root(args.root))
    profiles = agent_profiles(root)
    if not profiles:
        print("No agent profiles.")
        return 0
    for profile, _ in profiles:
        print(f"{profile.get('id', '')}\t{profile.get('title', '')}\t{profile.get('when', '')}")
    return 0


def cmd_agent_show(args: argparse.Namespace) -> int:
    root, _ = load_state_or_none(resolve_root(args.root))
    match = agent_profile_by_id(root, args.id)
    if match:
        _, path = match
        print(path.read_text(encoding="utf-8"))
        return 0
    raise SystemExit(f"Agent profile not found: {args.id}")


def cmd_agent_recommend(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    next_story = select_next_story(root)
    audit_errors = audit_project(root)
    recommendations = recommended_agent_profiles(root, state, next_story, audit_errors)
    if args.json:
        print(json.dumps({"recommended": recommendations}, ensure_ascii=True, sort_keys=True))
        return 0
    if not recommendations:
        print("No agent recommendations.")
        return 0
    for profile in recommendations:
        print(f"{profile.get('id', '')}\t{profile.get('title', '')}\t{profile.get('purpose', '')}")
    return 0


def cmd_agent_validate(args: argparse.Namespace) -> int:
    root, _ = load_state_or_none(resolve_root(args.root))
    errors = agent_profile_validation_errors(root)
    if errors:
        print("Agent profile validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1
    print("Agent profile validation passed.")
    return 0


def cmd_track_list(args: argparse.Namespace) -> int:
    if args.json:
        print(json.dumps({"tracks": TRACKS}, ensure_ascii=True, sort_keys=True, indent=2))
        return 0
    for track in TRACKS:
        print(
            f"{track['id']}\t{track['complexity']}\t"
            f"{track['project_kind']}\t{track['module']}\t{track['purpose']}"
        )
    return 0


def cmd_track_recommend(args: argparse.Namespace) -> int:
    recommendations = recommended_tracks(args.objective or "", limit=args.limit)
    if args.json:
        print(json.dumps({"recommended": recommendations}, ensure_ascii=True, sort_keys=True, indent=2))
        return 0
    for item in recommendations:
        print(
            f"{item.get('id')}\t{item.get('score', 0)}\t"
            f"{item.get('complexity')}\t{item.get('title')}\t{item.get('reason')}"
        )
    return 0


def cmd_track_set(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    track = track_by_id(args.track)
    if not track:
        raise SystemExit(f"Unknown track: {args.track}")
    state["track"] = track["id"]
    state["complexity"] = track["complexity"]
    state["project_kind"] = track["project_kind"]
    state["guide_summary"] = f"Use {track['title']} for {track['purpose']}"
    if args.set_module:
        state["module"] = track["module"]
    state["next_action"] = args.next_action or f"continue on {track['title']} track"
    write_state(root, state)
    append_ledger(root, "track.set", {"track": track["id"], "module": state.get("module", "")})
    print(f"Track set: {track['id']}")
    print(f"Next: {state['next_action']}")
    return 0


def detect_guidance_signals(question: str) -> list[str]:
    tokens = objective_tokens(question)
    normalized = normalize_text(question)
    signals: list[str] = []
    phrase_signals = {
        "correct-course": [
            "correct course",
            "corrigir curso",
            "curso errado",
            "wrong direction",
            "back up",
            "voltar atras",
            "nao era pra",
            "nao e pra",
            "nao deveria",
            "sem perguntar",
            "nao guiou",
            "nao conduziu",
            "nao conduz",
            "cedo demais",
        ],
        "confusion": ["nao sei", "não sei", "em duvida", "em dúvida", "what should", "o que fazer", "proximo passo", "próximo passo"],
        "research-needed": ["deep research", "pesquisa profunda", "consultar documentacao", "ler docs", "benchmark"],
        "document-utility": ["index docs", "shard document", "editorial review", "edge case", "spec distillation"],
        "quality-flow": ["teach me testing"],
        "story-flow": [
            "story lifecycle",
            "story creation",
            "create story",
            "create stories",
            "implementation-ready",
            "implementation ready",
            "criar story",
            "criar stories",
            "criar historias",
            "historias prontas",
        ],
        "product-flow": [
            "product requirements",
            "requisitos de produto",
            "prd",
            "prfaq",
            "ux design",
            "ux plan",
            "user experience",
            "experiencia de usuario",
            "plano de ux",
            "quick dev",
            "quick flow",
            "spec lite",
            "spec-lite",
        ],
        "builder-flow": [
            "audit runtime",
            "audit scripts",
            "auditoria do runtime",
            "auditoria de scripts",
            "codigo morto",
            "dead code",
            "doc misleading",
            "docs misleading",
            "misleading doc",
            "misleading docs",
            "missleading doc",
            "missleading docs",
            "doc enganoso",
            "docs enganosos",
            "documentacao enganosa",
            "parte guiada",
            "experiencia guiada",
            "fluxo guiado",
            "guided flow",
            "human guidance",
        ],
    }
    for signal, phrases in phrase_signals.items():
        if any(phrase in normalized for phrase in phrases):
            signals.append(signal)
    keyword_signals = {
        "correct-course": {
            "errado",
            "falhou",
            "falha",
            "problema",
            "corrigir",
            "faltando",
            "pular",
            "pula",
            "pulou",
            "ignorar",
            "ignora",
            "ignorou",
            "escapar",
            "quebrado",
        },
        "frustration": {"frustrado", "frustrante", "cansado", "vergonha", "burro", "merda", "pessimo", "horrivel", "inaceitavel"},
        "confusion": {"duvida", "confuso", "perdido", "incerto", "ajuda", "orientar", "guiar"},
        "brainstorm": {"brainstorm", "ideia", "ideias", "ideation", "explorar", "opcoes", "alternativas"},
        "research-needed": {"pesquisa", "research", "mercado", "documentacao", "docs", "evidencia", "fontes", "benchmark"},
        "creative-flow": {"creative", "criativo", "cis", "storytelling", "marca", "campanha", "conceito"},
        "game-flow": {"game", "jogo", "jogar", "player", "mecanica", "rpg", "mesa", "dice", "engine"},
        "quality-flow": {"test", "testing", "teste", "qa", "qualidade", "risco", "nfr", "gate", "review"},
        "story-flow": {"backlog", "epic", "epics", "sprint", "stories", "story", "historia", "historias"},
        "product-flow": {
            "prd",
            "prfaq",
            "requirements",
            "requisitos",
            "produto",
            "product",
            "architecture",
            "arquitetura",
            "ux",
            "ui",
            "interface",
            "journey",
            "jornada",
            "quick",
            "rapido",
            "rapida",
            "pequeno",
            "small",
        },
        "document-utility": {
            "index",
            "shard",
            "editorial",
            "prose",
            "estrutura",
            "structure",
            "distill",
            "distillation",
            "adversarial",
            "edge",
        },
        "builder-flow": {
            "forge",
            "method",
            "bmad",
            "runtime",
            "workflow",
            "workflows",
            "skill",
            "agent",
            "agente",
            "agentes",
            "plugin",
            "guidance",
            "router",
            "catalog",
            "catalogo",
            "metadata",
            "facilitation",
            "packs",
            "parity",
            "paridade",
        },
        "mechanical-build": {"implementar", "implementa", "implement", "build", "corrigir", "fix", "rodar", "story", "historia", "testes", "tests"},
        "operate-support": {"publicar", "publish", "release", "suporte", "support", "operar", "operate"},
    }
    for signal, keywords in keyword_signals.items():
        if tokens & keywords and signal not in signals:
            signals.append(signal)
    return signals


def guidance_alternatives(*items: tuple[str, str]) -> list[dict[str, str]]:
    return [{"workflow": workflow, "reason": reason} for workflow, reason in items]


def routed_game_workflow(question: str) -> str:
    tokens = objective_tokens(question)
    normalized = normalize_text(question)
    if {"ux", "hud", "controls", "controle", "onboarding", "accessibility", "acessibilidade"} & tokens:
        return "game-ux-design"
    if {"prd", "requirements", "requisitos"} & tokens or "game prd" in normalized:
        return "game-prd"
    if "sprint status" in normalized or {"status"} & tokens:
        return "game-sprint-status"
    if {"retro", "retrospective", "retrospectiva"} & tokens:
        return "game-retrospective"
    if {"e2e", "smoke"} & tokens or "end to end" in normalized:
        return "game-e2e-scaffold"
    if {"automation", "automacao", "automate", "automatizar"} & tokens:
        return "game-test-automation"
    if {"framework", "harness"} & tokens and {"test", "teste", "qa"} & tokens:
        return "game-test-framework"
    if {"story", "stories", "historia", "historias"} & tokens or "create story" in normalized or "criar story" in normalized:
        return "game-story-creation"
    return "game-brief"


def routed_quality_workflow(question: str) -> str:
    tokens = objective_tokens(question)
    normalized = normalize_text(question)
    if {"teach", "ensina", "ensinar", "explica", "explicar", "learn", "aprender"} & tokens or "teach me testing" in normalized:
        return "teach-testing"
    if {"traceability", "traceabilidade", "matrix", "matriz"} & tokens:
        return "traceability-gate"
    if {"nfr", "nonfunctional", "non-functional"} & tokens or "non functional" in normalized:
        return "nfr-evidence-audit"
    if {"atdd", "acceptance"} & tokens or "acceptance test" in normalized:
        return "atdd-plan"
    if {"ci", "pipeline"} & tokens:
        return "ci-quality-pipeline"
    if {"automation", "automacao", "automate", "automatizar"} & tokens:
        return "test-automation"
    if {"framework", "harness"} & tokens:
        return "test-framework"
    if {"review", "revisao", "audit", "auditoria"} & tokens:
        return "test-review"
    if {"engagement", "modelo"} & tokens:
        return "test-engagement-model"
    return "test-strategy"


def routed_product_workflow(question: str) -> str:
    tokens = objective_tokens(question)
    normalized = normalize_text(question)
    padded = f" {normalized} "
    if (
        {"quick", "rapido", "rapida", "pequeno", "small"} & tokens
        or "quick dev" in normalized
        or "quick flow" in normalized
        or "spec lite" in normalized
        or "spec-lite" in normalized
    ):
        return "quick-dev"
    if {"architecture", "arquitetura"} & tokens and "antes de arquitetura" not in normalized and "before architecture" not in normalized:
        return "architecture"
    if (
        {"ux", "ui", "interface", "journey", "jornada"} & tokens
        or " ux " in padded
        or "ui " in padded
        or "user experience" in normalized
        or "experiencia de usuario" in normalized
    ):
        return "ux-plan"
    if {"prd", "prfaq", "requirements", "requisitos"} & tokens:
        return "product-requirements"
    if {"produto", "product"} & tokens:
        return "product-requirements"
    return "product-requirements"


def routed_story_workflow(question: str) -> str:
    tokens = objective_tokens(question)
    normalized = normalize_text(question)
    if "create story" in normalized or "create stories" in normalized or "criar story" in normalized or "criar stories" in normalized or "criar historias" in normalized:
        return "story-creation"
    if {"epic", "epics"} & tokens or "create epics" in normalized:
        return "create-epics"
    if {"sprint"} & tokens or "plan sprint" in normalized or "sprint planning" in normalized:
        return "plan-sprint"
    if {"readiness", "ready"} & tokens or "implementation ready" in normalized or "implementation-ready" in normalized:
        return "readiness-check"
    return "story-creation"


def routed_builder_workflow(question: str) -> str:
    tokens = objective_tokens(question)
    normalized = normalize_text(question)
    if {"convert", "converter", "conversion"} & tokens or "convert skill" in normalized or "converter skill" in normalized:
        return "skill-convert"
    if {"agent", "agents"} & tokens and {"analyze", "analise", "analisar", "analysis"} & tokens:
        return "agent-analyze"
    if {"workflow", "workflows"} & tokens and {"analyze", "analise", "analisar", "analysis"} & tokens:
        return "workflow-analyze"
    return "runtime-builder"


def routed_document_workflow(question: str) -> str:
    tokens = objective_tokens(question)
    normalized = normalize_text(question)
    if {"shard", "split", "quebrar", "dividir"} & tokens:
        return "doc-shard"
    if {"editorial", "prose", "estrutura", "structure"} & tokens:
        return "editorial-review"
    if {"adversarial", "edge"} & tokens or "edge case" in normalized:
        return "edge-case-review"
    if {"distill", "distillation", "kernel"} & tokens or "spec distillation" in normalized:
        return "spec-distillation"
    if {"index", "map", "mapa"} & tokens:
        return "doc-index"
    return "doc-index"


def catalog_phase_options(metadata: dict[str, Any]) -> list[str]:
    phase_text = str(metadata.get("phase", ""))
    return [part.strip() for part in phase_text.split("|") if part.strip()]


def recommended_phase_for_workflow(metadata: dict[str, Any], current_phase: str) -> str:
    phases = catalog_phase_options(metadata)
    if not phases:
        return current_phase or "0-route"
    if "anytime" in phases:
        return current_phase or "1-discovery"
    if current_phase in phases:
        return current_phase
    return phases[0]


def should_transition_to_guided_workflow(
    *,
    state: dict[str, str] | None,
    has_question: bool,
    classification: str,
    recommended_workflow: str,
    metadata: dict[str, Any],
    commands: list[dict[str, str]],
) -> bool:
    if not state or not has_question or not metadata.get("modes"):
        return False
    if commands:
        return False
    if recommended_workflow == state.get("active_workflow", ""):
        return False
    return classification in {"game-flow", "quality-flow", "builder-flow", "document-utility", "product-flow", "story-flow", "creative-flow"}


def build_guidance_decision(
    root: Path,
    state: dict[str, str] | None,
    *,
    question: str,
    current_next_action: str,
    next_story: dict[str, Any] | None,
) -> dict[str, Any]:
    signals = detect_guidance_signals(question)
    signal_set = set(signals)
    phase = state.get("phase", "") if state else ""
    module_id = state.get("module", "") if state else ""
    has_question = bool(question.strip())
    commands: list[dict[str, str]] = []
    classification = "operate-support"
    recommended_phase = phase or "0-route"
    recommended_workflow = state.get("active_workflow", "start-runtime") if state else "start-runtime"
    recommended_action = current_next_action or "resolve project route"
    human_prompt = "I can continue from the current state, or you can give me the outcome you want to route."
    alternatives = guidance_alternatives(
        ("guide-route", "orient the human without changing project state"),
        ("context-recovery", "recover state when context is stale or overloaded"),
    )
    state_update_required = False
    reason = "No stronger human intent was detected, so the durable state remains authoritative."

    if not state:
        recommended_phase = "0-route"
        recommended_workflow = "start-runtime"
        recommended_action = "answer the route question, then create or open the selected project"
        human_prompt = "Tell me the project name and the outcome you want; I will choose the track and first workflow."
        reason = "No Forge project state exists in this workspace."
        if "game-flow" in signal_set:
            classification = "game-flow"
            recommended_phase = "1-discovery"
            recommended_workflow = routed_game_workflow(question)
            recommended_action = f"create a game-studio project, then run {recommended_workflow} before build"
            human_prompt = "I should shape player fantasy, loop, scope, and proof target before technical planning."
            reason = "The first intent is game-shaped, so the first guided flow should be game-specific."
        elif "creative-flow" in signal_set:
            classification = "creative-flow"
            recommended_phase = "1-discovery"
            recommended_workflow = "creative-session"
            recommended_action = "create a creative-studio project, then explore and select a creative direction"
            human_prompt = "I should preserve taste, constraints, and rejected directions before writing a spec."
            reason = "The first intent is taste-heavy or creative."
        elif "document-utility" in signal_set:
            classification = "document-utility"
            recommended_phase = "1-discovery"
            recommended_workflow = routed_document_workflow(question)
            recommended_action = f"create the project, then run {recommended_workflow} to make the source material usable"
            human_prompt = "I should clarify the document job and source-of-truth boundary before editing docs."
            reason = "The first intent is documentation utility work."
        elif "research-needed" in signal_set:
            classification = "research-needed"
            recommended_phase = "1-discovery"
            recommended_workflow = "domain-scan"
            recommended_action = "create the project with an evidence-first discovery flow, then write a compact scan"
            human_prompt = "I should gather enough evidence to make the next decision defensible."
            reason = "The first intent depends on docs, evidence, or external benchmark behavior."
        elif "brainstorm" in signal_set:
            classification = "brainstorm"
            recommended_phase = "1-discovery"
            recommended_workflow = "brainstorming"
            recommended_action = "create the project, then generate and compare options before specification"
            human_prompt = "I should keep this divergent until the direction is chosen."
            reason = "The first intent asks for ideas, options, or exploration."
        elif "product-flow" in signal_set:
            classification = "product-flow"
            recommended_phase = "2-specification"
            recommended_workflow = routed_product_workflow(question)
            recommended_action = f"create the project, then run {recommended_workflow} before implementation"
            human_prompt = "I should shape product requirements, UX, or a spec-lite quick-dev path before building."
            reason = "The first intent asks for product planning, UX design, PRD validation, or a small guided build."
        elif "story-flow" in signal_set:
            classification = "story-flow"
            recommended_phase = "3-plan"
            recommended_workflow = routed_story_workflow(question)
            recommended_action = f"create the project, then run {recommended_workflow} with decision-source and readiness checks before build"
            human_prompt = "I should not create implementation stories until accepted decisions and validation evidence exist."
            reason = "The first intent asks for story lifecycle planning rather than direct implementation."
        elif "builder-flow" in signal_set:
            classification = "builder-flow"
            recommended_phase = "1-discovery"
            recommended_workflow = routed_builder_workflow(question)
            recommended_action = f"create a runtime-builder project, then run {recommended_workflow} before scaffolding"
            human_prompt = "I should clarify the method artifact and acceptance criteria before editing runtime files."
            reason = "The first intent is about runtime, workflow, skill, or plugin behavior."
    elif has_question and ({"correct-course", "frustration"} & signal_set):
        classification = "correct-course"
        recommended_phase = "6-evolve" if phase == "5-ready-operate" else phase
        recommended_workflow = "correct-course"
        recommended_action = (
            "write a correct-course artifact, classify what failed in the method guidance, "
            "then route the smallest fix through runtime-builder"
        )
        human_prompt = "I should step back, name what failed, compare it against the method docs, and route a repair before building more."
        alternatives = guidance_alternatives(
            ("problem-solving", "diagnose symptoms and causes before selecting a repair"),
            ("council-decision", "use multiple perspectives when the correction is taste-heavy or strategic"),
            ("runtime-builder", "turn the corrected decision into compact workflows, tests, and runtime changes"),
        )
        state_update_required = True
        reason = "The latest user message contradicts or rejects the current route, so stale next_action must not dominate."
        if phase == "5-ready-operate":
            commands.append(
                preflight_command(
                    "transition-evolve",
                    "transition",
                    "--root",
                    root,
                    "--phase",
                    "6-evolve",
                    "--status",
                    "evolution-intake",
                    "--workflow",
                    "correct-course",
                    "--next-action",
                    recommended_action,
                )
            )
        commands.append(
            preflight_command(
                "correct-course",
                "correct-course",
                "--root",
                root,
                "--summary",
                "<summarize the guidance failure and user correction>",
                "--impact",
                "human guidance routed to stale state instead of the user's current intent",
                "--next-action",
                recommended_action,
            )
        )
    elif has_question and phase == "5-ready-operate" and "research-needed" in signal_set:
        classification = "research-needed"
        recommended_phase = "6-evolve"
        recommended_workflow = "domain-scan"
        recommended_action = "open an evolution cycle, collect grounded evidence, then decide the next runtime change"
        human_prompt = "This ready project has new evidence needs; I should research before publishing or building."
        alternatives = guidance_alternatives(
            ("market-scan", "when alternatives and adoption risk matter"),
            ("technical-feasibility-scan", "when implementation feasibility is the main uncertainty"),
            ("evolve-project", "when the evidence question is already answered"),
        )
        state_update_required = True
        reason = "A ready project with a new research request should enter evolve and run the evidence workflow."
        commands.append(
            preflight_command(
                "transition-evolve",
                "transition",
                "--root",
                root,
                "--phase",
                "6-evolve",
                "--status",
                "evolution-research",
                "--workflow",
                "domain-scan",
                "--next-action",
                recommended_action,
            )
        )
    elif has_question and phase == "5-ready-operate" and "brainstorm" in signal_set:
        classification = "brainstorm"
        recommended_phase = "6-evolve"
        recommended_workflow = "brainstorming"
        recommended_action = "open an evolution cycle, explore options, then select the smallest coherent direction"
        human_prompt = "This ready project has new direction-finding work; I should brainstorm before making a plan."
        alternatives = guidance_alternatives(
            ("concept-selection", "choose between candidate directions after divergence"),
            ("problem-solving", "use when the issue is a failure mode rather than an opportunity"),
            ("evolve-project", "use when the direction is already clear"),
        )
        state_update_required = True
        reason = "A ready project with a new brainstorm request should enter evolve and run an exploratory workflow."
        commands.append(
            preflight_command(
                "transition-evolve",
                "transition",
                "--root",
                root,
                "--phase",
                "6-evolve",
                "--status",
                "evolution-brainstorm",
                "--workflow",
                "brainstorming",
                "--next-action",
                recommended_action,
            )
        )
    elif has_question and phase == "5-ready-operate" and "builder-flow" in signal_set:
        classification = "evolution-request"
        recommended_phase = "6-evolve"
        recommended_workflow = "evolve-project"
        recommended_action = "classify the new method improvement request, then choose discovery, research, planning, or build"
        human_prompt = "This is new intent for a ready project; I should open an evolution cycle instead of publishing the old batch."
        alternatives = guidance_alternatives(
            ("problem-solving", "use when the failure mode is still unclear"),
            ("runtime-builder", "use after the improvement target is clear"),
            ("research-needed", "use when external benchmark or docs should drive the change"),
        )
        state_update_required = True
        reason = "Ready projects should enter evolve when the human brings new feedback or product intent."
        commands.append(
            preflight_command(
                "transition-evolve",
                "transition",
                "--root",
                root,
                "--phase",
                "6-evolve",
                "--status",
                "evolution-intake",
                "--workflow",
                "evolve-project",
                "--next-action",
                recommended_action,
            )
        )
    elif has_question and "builder-flow" in signal_set and (module_id == "runtime-builder" or phase == "6-evolve"):
        classification = "builder-flow"
        recommended_workflow = routed_builder_workflow(question)
        if recommended_workflow == "runtime-builder":
            recommended_action = "shape and implement the Forge runtime/workflow/catalog change with tests and evidence"
            human_prompt = "This is about improving Forge itself; I should route through runtime-builder before any domain-specific game, test, or research workflow."
        else:
            recommended_action = f"run {recommended_workflow} to analyze or convert the runtime artifact before scaffolding"
            human_prompt = "This is a specific runtime-builder utility task; I should analyze or convert before patching."
        alternatives = guidance_alternatives(
            ("agent-analyze", "analyze agent behavior and boundaries"),
            ("workflow-analyze", "analyze workflow state-machine and metadata gaps"),
            ("skill-convert", "convert source skill material into Forge-native artifacts"),
            ("workflow-validate", "prove workflow catalog, state-machine docs, and facilitation packs are consistent"),
        )
        reason = "Runtime-builder context and builder signals outrank domain words; explicit analysis/conversion words select the narrower builder utility workflow."
    elif has_question and "confusion" in signal_set:
        classification = "confusion"
        recommended_workflow = "problem-solving"
        recommended_action = "frame the confusion, identify candidate routes, and ask one high-leverage question"
        human_prompt = "I should orient you with one recommendation and two alternatives, not dump the workflow catalog."
        alternatives = guidance_alternatives(
            ("guide-route", "if the route is merely unclear"),
            ("brainstorming", "if you need options before deciding"),
        )
        reason = "The message asks for orientation or indicates uncertainty."
    elif has_question and "creative-flow" in signal_set:
        classification = "creative-flow"
        recommended_workflow = "creative-session"
        recommended_action = "explore and select a creative direction before specification"
        human_prompt = "I should help choose the creative mode and preserve rejected directions compactly."
        reason = "The message is taste-heavy or creative."
    elif has_question and "brainstorm" in signal_set:
        classification = "brainstorm"
        recommended_workflow = "brainstorming"
        recommended_action = "generate and compare options before committing to specification"
        human_prompt = "I should keep this divergent until the direction is chosen."
        alternatives = guidance_alternatives(
            ("concept-selection", "choose between candidate directions"),
            ("reality-evidence-gate", "filter impossible or risky promises"),
        )
        reason = "The message asks for ideas, options, or exploration."
    elif has_question and "document-utility" in signal_set:
        classification = "document-utility"
        recommended_workflow = routed_document_workflow(question)
        recommended_action = f"run {recommended_workflow} to make the document set usable for humans and future agents"
        human_prompt = "I should clarify the document job and source-of-truth boundary before editing docs."
        alternatives = guidance_alternatives(
            ("editorial-review", "when clarity, tone, or structure is the main problem"),
            ("edge-case-review", "when the artifact needs adversarial stress testing"),
            ("spec-distillation", "when messy notes must become a compact machine contract"),
        )
        reason = "The message is documentation utility work rather than general research."
    elif has_question and "research-needed" in signal_set:
        classification = "research-needed"
        recommended_workflow = "domain-scan"
        recommended_action = "collect grounded evidence before deciding or building"
        human_prompt = "I should research enough to make the next decision defensible, then write a compact scan."
        alternatives = guidance_alternatives(
            ("market-scan", "when market alternatives matter"),
            ("technical-feasibility-scan", "when architecture feasibility is the main risk"),
        )
        reason = "The message depends on docs, evidence, or external benchmark behavior."
    elif has_question and "product-flow" in signal_set and "game-flow" not in signal_set:
        classification = "product-flow"
        recommended_workflow = routed_product_workflow(question)
        if recommended_workflow == "quick-dev":
            recommended_action = "run quick-dev to clarify scope, write spec-lite, implement or hand off, review, validate, and record evidence"
            human_prompt = "This sounds small enough for a fast path, but I should still lock scope, proof, and non-goals before building."
        elif recommended_workflow == "ux-plan":
            recommended_action = "run ux-plan to calibrate taste, journeys, interaction model, accessibility, rejection log, and proof"
            human_prompt = "I should make the human experience concrete before implementation stories."
        elif recommended_workflow == "architecture":
            recommended_action = "run architecture to connect accepted product decisions to technical constraints, interfaces, risks, and story boundaries"
            human_prompt = "I should turn accepted requirements into implementation architecture before story creation."
        else:
            recommended_action = "run product-requirements in create/update/validate mode with decisions, addendum, findings, and next workflow"
            human_prompt = "I should turn product intent into testable requirements and a durable decision log before architecture or stories."
        alternatives = guidance_alternatives(
            ("product-requirements", "when product promise, scope, or acceptance criteria need a PRD"),
            ("ux-plan", "when taste, journey, interface, states, or accessibility are the main uncertainty"),
            ("quick-dev", "when the change is small, bounded, and ready for spec-lite plus implementation evidence"),
        )
        reason = "The message asks for product requirements, UX design, PRD validation, or a quick guided build path."
    elif next_story and phase == "4-build-verify" and (not has_question or "mechanical-build" in signal_set or "operate-support" in signal_set):
        classification = "mechanical-build"
        recommended_workflow = "build-story"
        recommended_action = f"implement and validate story {next_story.get('id')}"
        human_prompt = "The approved decision work is done; I should continue mechanically and write evidence."
        reason = "A build-ready story exists in build/verify and the human asked for implementation."
    elif has_question and "story-flow" in signal_set and "game-flow" not in signal_set:
        classification = "story-flow"
        recommended_workflow = routed_story_workflow(question)
        if recommended_workflow == "story-creation":
            recommended_action = "run story-creation to verify decision sources, write implementation-ready stories, checks, evidence map, and next build step"
            human_prompt = "I should convert approved decisions into stories, not use stories as a substitute for decisions."
        elif recommended_workflow == "readiness-check":
            recommended_action = "run readiness-check to prove stories have accepted sources, acceptance criteria, checks, and evidence expectations"
            human_prompt = "I should prove the backlog is implementation-ready before build-story starts."
        else:
            recommended_action = f"run {recommended_workflow} to plan the next story batch from accepted decision artifacts"
            human_prompt = "I should preserve story order, dependencies, acceptance, checks, and decision sources."
        alternatives = guidance_alternatives(
            ("story-creation", "when implementation-ready story files need to be authored from accepted decisions"),
            ("readiness-check", "when stories exist but their decision sources or validation map may be weak"),
            ("plan-sprint", "when the next slice and sequencing need sprint planning"),
        )
        reason = "The message asks for story lifecycle work, so stories must be generated from accepted decision artifacts before build."
    elif has_question and "game-flow" in signal_set:
        classification = "game-flow"
        recommended_workflow = routed_game_workflow(question)
        recommended_action = f"run {recommended_workflow} before game implementation"
        human_prompt = "I should define player fantasy, loop, scope, and proof target before technical planning."
        alternatives = guidance_alternatives(
            ("game-brief", "when the player fantasy and loop are still unclear"),
            ("quick-prototype", "if a playable experiment matters more than documentation"),
            ("game-test-framework", "if proof strategy is the main gap"),
        )
        reason = "The message is game-shaped and includes enough detail to choose a game-specific workflow."
    elif has_question and "quality-flow" in signal_set:
        classification = "quality-flow"
        recommended_workflow = routed_quality_workflow(question)
        recommended_action = f"run {recommended_workflow} to produce quality evidence before implementation or release"
        human_prompt = "I should classify the quality engagement and risks before writing tests."
        alternatives = guidance_alternatives(
            ("test-engagement-model", "when the quality mode is ambiguous"),
            ("test-strategy", "when risks and checks need the broad plan first"),
            ("traceability-gate", "when release depends on requirement/test/evidence mapping"),
        )
        reason = "The message is primarily about quality, risk, review, or test architecture."
    elif has_question and "builder-flow" in signal_set:
        classification = "builder-flow"
        recommended_workflow = routed_builder_workflow(question)
        recommended_action = f"run {recommended_workflow} before scaffolding or editing runtime files"
        human_prompt = "I should clarify the method artifact and acceptance criteria before editing runtime files."
        reason = "The message is about the method, runtime, workflows, skills, or plugin behavior."
    elif next_story and phase == "4-build-verify":
        classification = "mechanical-build"
        recommended_workflow = "build-story"
        recommended_action = f"implement and validate story {next_story.get('id')}"
        human_prompt = "The approved decision work is done; I should continue mechanically and write evidence."
        reason = "A build-ready story exists in build/verify."

    workflow_metadata = workflow_catalog_entry(recommended_workflow)
    facilitation_pack = facilitation_pack_for_workflow(recommended_workflow)
    if workflow_metadata:
        recommended_phase = recommended_phase_for_workflow(workflow_metadata, recommended_phase)
    if should_transition_to_guided_workflow(
        state=state,
        has_question=has_question,
        classification=classification,
        recommended_workflow=recommended_workflow,
        metadata=workflow_metadata,
        commands=commands,
    ):
        state_update_required = True
        commands.append(
            preflight_command(
                "transition-workflow",
                "transition",
                "--root",
                root,
                "--phase",
                recommended_phase,
                "--status",
                "workflow-selected",
                "--workflow",
                recommended_workflow,
                "--next-action",
                recommended_action,
            )
        )
        reason = f"{reason} The selected catalog workflow is narrow and executable, so the runtime should enter it before continuing."
    if not commands:
        commands = [preflight_command("guide", "guide", "--root", root, "--question", question or "<question>", "--json")]
    return {
        "intent_classification": classification,
        "signals": signals,
        "route_reason": reason,
        "recommended_phase": recommended_phase,
        "recommended_workflow": recommended_workflow,
        "workflow_metadata": workflow_metadata,
        "facilitation_pack": facilitation_pack,
        "recommended_action": recommended_action,
        "human_prompt": human_prompt,
        "alternatives": alternatives,
        "state_update_required": state_update_required,
        "state_updates": {
            "last_intent_classification": classification,
            "last_route_reason": reason,
            "active_guidance_mode": recommended_workflow,
        },
        "commands": commands,
        "source": "guidance-engine",
    }


def build_guide_payload(root: Path, *, question: str, max_chars: int) -> dict[str, Any]:
    state_root, state = load_state_or_none(root)
    if not state_root:
        preflight = build_preflight(root, scan_depth=2, max_chars=max_chars, objective=question)
        tracks = recommended_tracks(question, limit=3)
        guidance = build_guidance_decision(
            root,
            None,
            question=question,
            current_next_action="answer the preflight route question, then create or open the selected project",
            next_story=None,
        )
        return {
            "runtime": RUNTIME_NAME,
            "runtime_version": RUNTIME_VERSION,
            "workspace": str(root),
            "state_found": False,
            "route": preflight.get("route", ""),
            "human_experience": preflight.get("human_experience", {}),
            "reality_evidence_gate": preflight.get("reality_evidence_gate", {}),
            "question": preflight.get("question", ""),
            "recommended_tracks": tracks,
            "next_action": guidance["recommended_action"],
            "guidance_engine": guidance,
            "intent_classification": guidance["intent_classification"],
            "signals": guidance["signals"],
            "recommended_phase": guidance["recommended_phase"],
            "recommended_workflow": guidance["recommended_workflow"],
            "workflow_metadata": guidance["workflow_metadata"],
            "facilitation_pack": guidance["facilitation_pack"],
            "recommended_action": guidance["recommended_action"],
            "human_prompt": guidance["human_prompt"],
            "alternatives": guidance["alternatives"],
            "state_update_required": guidance["state_update_required"],
            "state_updates": guidance["state_updates"],
            "commands": preflight.get("commands", []) + guidance["commands"],
        }
    snapshot = build_snapshot(state_root, state)
    track = track_by_id(state.get("track", "")) or default_track_for_module(state.get("module", "software-builder"))
    next_story = snapshot["stories"].get("next") or {}
    current_next_action = snapshot["route"].get("next_action", "") or state.get("next_action", "")
    guidance = build_guidance_decision(
        state_root,
        state,
        question=question,
        current_next_action=current_next_action,
        next_story=next_story,
    )
    return {
        "runtime": RUNTIME_NAME,
        "runtime_version": RUNTIME_VERSION,
        "workspace": str(root),
        "state_found": True,
        "human_experience": human_experience_for_route("existing-method-project", question=question),
        "reality_evidence_gate": reality_evidence_assessment(question),
        "project_root": str(state_root),
        "project": state.get("project", ""),
        "track": track,
        "phase": state.get("phase", ""),
        "workflow": state.get("active_workflow", ""),
        "readiness": state.get("readiness", ""),
        "route": snapshot["route"].get("recommendation", ""),
        "next_story": next_story,
        "recommended_agents": snapshot["agents"].get("recommended", []),
        "next_action": guidance["recommended_action"],
        "guidance_engine": guidance,
        "intent_classification": guidance["intent_classification"],
        "signals": guidance["signals"],
        "recommended_phase": guidance["recommended_phase"],
        "recommended_workflow": guidance["recommended_workflow"],
        "workflow_metadata": guidance["workflow_metadata"],
        "facilitation_pack": guidance["facilitation_pack"],
        "recommended_action": guidance["recommended_action"],
        "human_prompt": guidance["human_prompt"],
        "alternatives": guidance["alternatives"],
        "state_update_required": guidance["state_update_required"],
        "state_updates": guidance["state_updates"],
        "commands": guidance["commands"],
        "grill_gate_required": snapshot["resume"].get("grill_gate_required", False),
        "mechanical_work_order": snapshot["resume"].get("mechanical_work_order", {}),
        "codex_goal_handoff": snapshot["resume"].get("codex_goal_handoff", {}),
        "council_recommended": bool(question and state.get("readiness") == "ready"),
    }


def print_guidance_engine_summary(payload: dict[str, Any]) -> None:
    guidance = payload.get("guidance_engine") or {}
    if not guidance:
        return
    intent = guidance.get("intent_classification", "")
    workflow = guidance.get("recommended_workflow", "")
    phase = guidance.get("recommended_phase", "")
    print(f"Guidance Engine: {intent} -> {workflow} / {phase}")
    metadata = guidance.get("workflow_metadata") or {}
    if metadata:
        required = "required" if metadata.get("required") else "optional"
        outputs = join_list([str(item) for item in metadata.get("outputs", [])]) if isinstance(metadata.get("outputs"), list) else str(metadata.get("outputs", ""))
        print(f"Workflow metadata: {required}; outputs: {outputs}")
    if guidance.get("facilitation_pack"):
        print(f"Facilitation: {guidance.get('facilitation_pack')}")
    signals = guidance.get("signals") or []
    if signals:
        print(f"Signals: {join_list(signals)}")
    if guidance.get("route_reason"):
        print(f"Route reason: {guidance.get('route_reason')}")
    if guidance.get("human_prompt"):
        print(f"Prompt: {guidance.get('human_prompt')}")
    alternatives = guidance.get("alternatives") or []
    if alternatives:
        print("Alternatives:")
        for item in alternatives[:3]:
            print(f"- {item.get('workflow')}: {item.get('reason')}")
    if guidance.get("state_update_required"):
        print("State update: required before continuing this route.")


def cmd_guide(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    payload = build_guide_payload(root, question=args.question or "", max_chars=args.max_chars)
    if args.json:
        print(json.dumps(payload, ensure_ascii=True, sort_keys=True, indent=2))
        return 0
    if not payload.get("state_found"):
        print_human_experience_intro(payload)
    print("Forge Guide")
    print(f"Workspace: {payload.get('workspace')}")
    if not payload.get("state_found"):
        print(f"Route: {payload.get('route')}")
        print(f"Question: {payload.get('question')}")
        print("Recommended tracks:")
        for track in payload.get("recommended_tracks", []):
            print(f"- {track.get('id')}: {track.get('title')} ({track.get('reason')})")
        print_guidance_engine_summary(payload)
        print(f"Next: {payload.get('next_action')}")
        return 0
    track = payload.get("track", {})
    print(f"Project: {payload.get('project')}")
    print(f"Track: {track.get('id')} ({track.get('complexity')})")
    print(f"State: {payload.get('phase')} / {payload.get('workflow')}")
    print(f"Route: {payload.get('route')}")
    story = payload.get("next_story") or {}
    if story:
        print(f"Next story: {story.get('id')} - {story.get('title')}")
    print("Recommended agents:")
    for agent in payload.get("recommended_agents", []):
        print(f"- {agent.get('id')}: {agent.get('purpose')}")
    if payload.get("grill_gate_required"):
        print("Grill Gate: required before leaving this decision phase.")
    assessment = payload.get("reality_evidence_gate") or {}
    if assessment.get("required") and assessment.get("status") != "not-applicable":
        score = assessment.get("score")
        score_text = "" if score is None else f" ({score}/10)"
        print(f"Reality/Evidence Gate: {assessment.get('status')}{score_text}")
        print(f"Reality check: {assessment.get('summary')}")
    work_order = payload.get("mechanical_work_order", {})
    if work_order.get("autonomous"):
        print(f"Mechanical: {work_order.get('next_mechanical_step')}")
        if work_order.get("goal_recommended"):
            print("Goal: recommended for this mechanical loop.")
    print_guidance_engine_summary(payload)
    print(f"Next: {payload.get('next_action')}")
    if payload.get("council_recommended"):
        print("Council: optional for this question if the decision is high-risk or taste-heavy.")
    return 0


def read_parity_replay_cases(path: Path) -> list[dict[str, Any]]:
    if not path.exists():
        raise SystemExit(f"Parity replay fixture not found: {path}")
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise SystemExit(f"Invalid parity replay fixture JSON: {path}: {exc}") from exc
    if not isinstance(payload, list):
        raise SystemExit("Parity replay fixture must be a JSON list.")
    required = {"id", "family", "state", "question", "expected_classification", "expected_workflow"}
    for index, case in enumerate(payload, start=1):
        if not isinstance(case, dict):
            raise SystemExit(f"Parity replay case #{index} must be an object.")
        missing = sorted(required - set(case))
        if missing:
            raise SystemExit(f"Parity replay case {case.get('id', index)} missing keys: {', '.join(missing)}")
    return payload


def set_replay_state(
    root: Path,
    *,
    phase: str,
    status: str,
    workflow: str,
    next_action: str,
    readiness: str = "not_ready",
) -> None:
    state = apply_state_defaults(read_flat_yaml(state_path(root)))
    state["phase"] = phase
    state["status"] = status
    state["active_workflow"] = workflow
    state["next_action"] = next_action
    state["readiness"] = readiness
    state["human_input_required"] = "false"
    write_state(root, state)
    update_sprint(root)


def prepare_parity_replay_state(root: Path, state_kind: str) -> None:
    if state_kind == "none":
        return
    initialize_project_state(
        root,
        project="Parity Replay Fixture",
        mode="creation-runtime",
        module="software-builder",
        no_project_guidance=True,
    )
    if state_kind == "discovery":
        set_replay_state(
            root,
            phase="1-discovery",
            status="discovery-ready",
            workflow="discover-intent",
            next_action=NEXT_BY_PHASE["1-discovery"],
        )
        return
    if state_kind == "ready":
        set_replay_state(
            root,
            phase="5-ready-operate",
            status="story-done",
            workflow="ready-release",
            next_action="publish current batch",
            readiness="ready",
        )
        return
    if state_kind == "evolve_runtime":
        set_replay_state(
            root,
            phase="6-evolve",
            status="evolution-intake",
            workflow="evolve-project",
            next_action="compare and implement guided-flow parity gaps",
            readiness="ready",
        )
        return
    if state_kind == "build_story_ready":
        set_replay_state(
            root,
            phase="4-build-verify",
            status="build-ready",
            workflow="build-story",
            next_action=NEXT_BY_PHASE["4-build-verify"],
        )
        decision_source = write_artifact(
            root,
            kind="spec",
            title="Replay decision source",
            summary="Accepted decision source for parity replay build-story routing.",
            path=".forge-method/artifacts/replay-decision-source.md",
        )
        save_story(
            root,
            {
                "id": "story-replay",
                "title": "Build replay target",
                "status": "ready",
                "phase": "4-build-verify",
                "acceptance_criteria": "target works",
                "checks": "python -m unittest discover -s tests",
                "artifacts": decision_source,
                "evidence": "",
                "blocker": "",
            },
        )
        update_sprint(root)
        return
    raise SystemExit(f"Unknown parity replay fixture state: {state_kind}")


def command_names(commands: list[dict[str, Any]]) -> list[str]:
    return [str(item.get("name", "")) for item in commands]


def parity_case_failures(case: dict[str, Any], payload: dict[str, Any]) -> list[str]:
    failures: list[str] = []

    def expect_equal(field: str, actual: Any, expected: Any) -> None:
        if expected is not None and actual != expected:
            failures.append(f"{field}: expected {expected!r}, got {actual!r}")

    expect_equal("intent_classification", payload.get("intent_classification"), case.get("expected_classification"))
    expect_equal("recommended_workflow", payload.get("recommended_workflow"), case.get("expected_workflow"))
    expect_equal("recommended_phase", payload.get("recommended_phase"), case.get("expected_phase"))
    expect_equal("state_update_required", payload.get("state_update_required"), case.get("state_update_required"))
    expect_equal("facilitation_pack", payload.get("facilitation_pack"), case.get("expected_facilitation_pack"))
    metadata = payload.get("workflow_metadata") or {}
    expect_equal("workflow_metadata.id", metadata.get("id"), case.get("expected_workflow"))
    expect_equal("workflow_metadata.template", metadata.get("template"), case.get("expected_template"))
    expected_command = case.get("expected_command")
    if expected_command and expected_command not in command_names(payload.get("commands") or []):
        failures.append(f"commands: expected {expected_command!r}, got {command_names(payload.get('commands') or [])!r}")
    for signal in case.get("expected_signals", []):
        if signal not in payload.get("signals", []):
            failures.append(f"signals: missing {signal!r}")
    for forbidden in case.get("forbidden_action_terms", []):
        if forbidden and forbidden in str(payload.get("recommended_action", "")):
            failures.append(f"recommended_action contains forbidden term {forbidden!r}")
    if not payload.get("recommended_action"):
        failures.append("recommended_action is empty")
    if not payload.get("human_prompt"):
        failures.append("human_prompt is empty")
    if not isinstance(payload.get("alternatives"), list):
        failures.append("alternatives is not a list")
    return failures


def run_parity_replay(*, fixture_path: Path, max_chars: int) -> dict[str, Any]:
    cases = read_parity_replay_cases(fixture_path)
    covered_families = sorted({str(case.get("family", "")) for case in cases if case.get("family")})
    missing_families = sorted(PARITY_REPLAY_REQUIRED_FAMILIES - set(covered_families))
    results: list[dict[str, Any]] = []
    failures: list[dict[str, Any]] = []

    for case in cases:
        with tempfile.TemporaryDirectory(prefix="forge-parity-replay-") as raw:
            replay_root = Path(raw)
            prepare_parity_replay_state(replay_root, str(case.get("state", "")))
            payload = build_guide_payload(replay_root, question=str(case.get("question", "")), max_chars=max_chars)
            case_failures = parity_case_failures(case, payload)
            result = {
                "id": case.get("id"),
                "family": case.get("family"),
                "state": case.get("state"),
                "passed": not case_failures,
                "failures": case_failures,
                "actual": {
                    "intent_classification": payload.get("intent_classification"),
                    "recommended_phase": payload.get("recommended_phase"),
                    "recommended_workflow": payload.get("recommended_workflow"),
                    "facilitation_pack": payload.get("facilitation_pack"),
                    "template": (payload.get("workflow_metadata") or {}).get("template"),
                    "state_update_required": payload.get("state_update_required"),
                    "commands": command_names(payload.get("commands") or []),
                },
            }
            results.append(result)
            if case_failures:
                failures.append(result)

    if missing_families:
        failures.append(
            {
                "id": "required-family-coverage",
                "family": "coverage",
                "passed": False,
                "failures": [f"missing required families: {', '.join(missing_families)}"],
                "actual": {"covered_families": covered_families},
            }
        )

    return {
        "runtime": RUNTIME_NAME,
        "runtime_version": RUNTIME_VERSION,
        "fixture": str(fixture_path),
        "required_families": sorted(PARITY_REPLAY_REQUIRED_FAMILIES),
        "covered_families": covered_families,
        "missing_families": missing_families,
        "total": len(cases),
        "passed": len([item for item in results if item["passed"]]),
        "failed": len(failures),
        "failures": failures,
        "results": results,
    }


def cmd_parity_replay(args: argparse.Namespace) -> int:
    fixture_path = Path(args.fixture).resolve() if args.fixture else PARITY_REPLAY_FIXTURE_PATH
    payload = run_parity_replay(fixture_path=fixture_path, max_chars=args.max_chars)
    if args.json:
        print(json.dumps(payload, ensure_ascii=True, sort_keys=True, indent=2))
    else:
        print("Forge Parity Replay")
        print(f"Fixture: {payload['fixture']}")
        print(f"Cases: {payload['passed']}/{payload['total']} passed")
        print(f"Families: {', '.join(payload['covered_families'])}")
        if payload["missing_families"]:
            print(f"Missing families: {', '.join(payload['missing_families'])}")
        for failure in payload["failures"]:
            print(f"FAIL {failure.get('id')}: {'; '.join(failure.get('failures', []))}")
        if not payload["failures"]:
            print("Parity replay passed.")
    return 0 if not payload["failures"] else 1


def council_participants(root: Path, raw_agents: list[str] | None) -> list[dict[str, str]]:
    ids = [slugify(item) for item in (raw_agents or []) if item.strip()]
    if not ids:
        ids = COUNCIL_DEFAULT_AGENTS
    participants: list[dict[str, str]] = []
    for profile_id in ids:
        match = agent_profile_by_id(root, profile_id)
        if match:
            profile, _ = match
            participants.append(profile)
    return participants


def council_decision_summary(topic: str, participants: list[dict[str, str]]) -> str:
    names = ", ".join(profile.get("title", profile.get("id", "")) for profile in participants)
    return (
        f"Topic: {topic}. Participants: {names}. Recommendation: run the smallest reversible next step, "
        "preserve dissent as risk, and update state/evidence before implementation continues. "
        "Agreements: keep one public entrypoint and separate Human Experience from Agent Runtime. "
        "Risks: cost, context growth, unclear ownership, and false consensus. "
        "Next action: convert the decision into a story or explicit human input."
    )


def cmd_council_run(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    topic = args.topic or state.get("next_action") or "current Forge Method decision"
    participants = council_participants(root, args.agent)
    if not participants:
        raise SystemExit("No council participants available.")
    print("Forge Agent Council")
    print(f"Topic: {topic}")
    print("Mode: live transcript on screen; compact decision persisted.")
    print("")
    for profile in participants:
        title = profile.get("title", profile.get("id", "Agent"))
        persona = profile.get("persona", profile.get("purpose", "Focused specialist."))
        print(f"[{title}]")
        print(f"{persona}")
        print(f"View: {profile.get('purpose', '')}")
        print(f"Guardrail: {profile.get('handoff', '')}")
        print("")
    summary = council_decision_summary(topic, participants)
    rel = write_artifact(
        root,
        kind="council-decision",
        title=f"Council decision: {topic[:48]}",
        summary=summary,
        lifecycle="durable",
    )
    if args.eval:
        write_artifact_eval(root, rel, title="Council decision artifact", summary=summary)
    state["last_council_artifact"] = rel
    state["guide_summary"] = f"Council reviewed: {topic}"
    state["next_action"] = args.next_action or "turn council decision into the next story, artifact, or human input"
    write_state(root, state)
    append_ledger(
        root,
        "council.run",
        {"topic": topic, "artifact": rel, "participants": [item.get("id", "") for item in participants]},
    )
    print(f"Persisted decision artifact: {rel}")
    print(f"Next: {state['next_action']}")
    return 0


def cmd_correct_course(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    impact = args.impact or "late contradiction discovered during mechanical work"
    next_action = args.next_action or state.get("next_action") or NEXT_BY_PHASE.get(state.get("phase", ""), "continue workflow")
    summary = "\n\n".join(
        [
            args.summary.strip(),
            f"Impact: {impact}.",
            "Policy: choose the conservative interpretation that preserves the approved spec.",
            f"Continuation: {next_action}.",
        ]
    )
    rel = write_artifact(
        root,
        kind="correct-course",
        title=args.title or "Correct-course continuation",
        summary=summary,
        lifecycle="durable",
    )
    if args.eval:
        write_artifact_eval(root, rel, title="Correct-course artifact", summary=summary)
    state["last_correct_course_artifact"] = rel
    state["human_input_required"] = "false"
    state["status"] = "correct-course-continued"
    state["active_workflow"] = "correct-course"
    state["last_intent_classification"] = "correct-course"
    state["last_route_reason"] = impact
    state["active_guidance_mode"] = "correct-course"
    state["next_action"] = next_action
    write_state(root, state)
    append_ledger(root, "correct_course.continued", {"artifact": rel, "impact": impact})
    print(f"Correct-course artifact: {rel}")
    print(f"Next: {next_action}")
    return 0


def cmd_config_inspect(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    config, sources = merged_config(root)
    payload = {
        "root": str(root),
        "sources": sources,
        "effective": config,
        "allowed_keys": sorted(CONFIG_ALLOWED_KEYS),
    }
    if args.json:
        print(json.dumps(payload, ensure_ascii=True, sort_keys=True, indent=2))
        return 0
    print("Forge Method Config")
    print(f"Sources: {', '.join(sources) if sources else '<none>'}")
    if config:
        for key, value in config.items():
            if key != "updated_at":
                print(f"{key}: {value}")
    else:
        print("No config overrides.")
    return 0


def config_validation_errors(root: Path) -> list[str]:
    errors: list[str] = []
    for path in config_paths(root):
        if not path.exists():
            continue
        values = read_flat_yaml(path)
        errors.extend(validate_config_values(values, source=path.relative_to(root).as_posix()))
    return errors


def cmd_config_validate(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    errors = config_validation_errors(root)
    if errors:
        print("Config validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1
    print("Config validation passed.")
    return 0


def builder_path(root: Path, kind: str, item_id: str) -> Path:
    normalized = slugify(item_id)
    if kind == "workflow":
        return method_dir(root) / "workflows" / f"workflow-{normalized}.md"
    if kind == "module":
        return method_dir(root) / "modules" / f"{normalized}.yaml"
    if kind == "agent":
        return method_dir(root) / "agents" / f"{normalized}.yaml"
    if kind == "skill":
        return method_dir(root) / "skills" / normalized / "SKILL.md"
    if kind == "template":
        return method_dir(root) / "templates" / f"{normalized}.md"
    if kind == "eval":
        return eval_path(root, normalized)
    raise SystemExit(f"Unsupported builder kind: {kind}")


def cmd_builder_scaffold(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    kind = args.kind
    item_id = slugify(args.id)
    title = args.title or item_id.replace("-", " ").title()
    purpose = args.purpose or f"Project-local {kind} scaffold for {title}."
    path = builder_path(root, kind, item_id)
    if path.exists() and not args.force:
        raise SystemExit(f"{kind} already exists: {path.relative_to(root).as_posix()}")
    path.parent.mkdir(parents=True, exist_ok=True)
    if kind == "workflow":
        path.write_text(
            workflow_text(
                workflow_id=item_id,
                title=title,
                triggers=[args.trigger or f"user asks for {title}"],
                inputs=["current state", "user intent", "relevant artifacts"],
                steps=["resolve scope", "create or update the artifact", "run the validation check", "update state"],
                outputs=[f"{title} artifact", "updated state"],
                done_when=["artifact exists", "validation passes", "next action is known"],
                blocked_when=["required input is missing", "scope conflicts with current state"],
                handoff=["preserve artifact path, validation result, blockers, and next action"],
            ),
            encoding="utf-8",
        )
        write_eval(root, eval_id=f"{item_id}-routing", kind="workflow-routing", target=item_id, query=title, expected=item_id)
    elif kind == "module":
        write_flat_yaml(
            path,
            {
                "id": item_id,
                "title": title,
                "purpose": purpose,
                "phase_span": args.phase_span or "1-discovery | 2-specification | 3-plan | 4-build-verify",
                "workflows": args.workflows or item_id,
            },
            header="Forge Method module",
        )
    elif kind == "agent":
        write_flat_yaml(
            path,
            {
                "id": item_id,
                "title": title,
                "purpose": purpose,
                "when": args.when or "when this specialist perspective is needed",
                "inputs": "state snapshot | relevant artifact | current question",
                "outputs": "recommendation | risks | next action",
                "handoff": "Preserve decision, unresolved risks, and next action.",
                "persona": args.persona or f"{title} speaks clearly to humans and keeps task output compact.",
                "council_role": args.council_role or "specialist perspective",
            },
            header="Forge Method agent profile",
        )
    elif kind == "skill":
        path.write_text(
            "\n".join(
                [
                    "---",
                    f"name: {item_id}",
                    f"description: {purpose}",
                    "---",
                    "",
                    f"# {title}",
                    "",
                    "Use compact state, artifacts, and evidence as source of truth.",
                    "Keep human-facing explanations helpful; keep agent-facing outputs concise.",
                    "",
                ]
            ),
            encoding="utf-8",
        )
    elif kind == "template":
        path.write_text(f"# {title}\n\n{purpose}\n\n## Inputs\n\n## Output\n", encoding="utf-8")
    elif kind == "eval":
        target = args.target or ".forge-method/artifacts/project-brief.md"
        query = args.query or f"{title} is available"
        rel = write_eval(root, eval_id=item_id, kind=args.eval_kind, target=target, query=query, expected=args.expected or "")
        print(rel)
        return 0
    append_ledger(root, "builder.scaffolded", {"kind": kind, "id": item_id, "path": path.relative_to(root).as_posix()})
    print(path.relative_to(root).as_posix())
    return 0


def cmd_builder_validate(args: argparse.Namespace) -> int:
    root, _ = load_state_or_fail(resolve_root(args.root))
    errors: list[str] = []
    errors.extend(workflow_validation_errors(root))
    errors.extend(agent_profile_validation_errors(root))
    errors.extend(config_validation_errors(root))
    for skill_path in sorted((method_dir(root) / "skills").glob("*/SKILL.md")):
        text = skill_path.read_text(encoding="utf-8")
        if not text.startswith("---"):
            errors.append(f"{skill_path.relative_to(root).as_posix()}: missing skill frontmatter")
    if errors:
        print("Builder validation failed:")
        for error in errors:
            print(f"- {error}")
        return 1
    print("Builder validation passed.")
    return 0


def cmd_example_list(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    manifests = module_manifests(root)
    if not manifests:
        print("No example modules.")
        return 0
    for module, _ in manifests:
        print(f"{module.get('id', '')}\t{module.get('title', '')}\t{module.get('purpose', '')}")
    return 0


def cmd_example_create(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    if root.exists() and not root.is_dir():
        raise SystemExit(f"Example root must be a directory: {root}")
    if root.exists() and any(root.iterdir()) and not state_path(root).exists() and not args.force:
        raise SystemExit(f"Example root is not empty: {root}. Use --force if this is intentional.")

    module_id = slugify(args.module)
    match = module_manifest_by_id(root, module_id)
    if not match:
        raise SystemExit(f"Module not found: {args.module}")
    module, _ = match

    title = module.get("title", module_id)
    purpose = module.get("purpose", "Start a Forge Method project from this module.")
    workflows = split_list(module.get("workflows"))
    phases = [phase for phase in split_list(module.get("phase_span")) if phase in PHASES]
    first_phase = phases[0] if phases else "1-discovery"
    first_workflow = workflows[0] if workflows else "start-runtime"
    project = args.project or f"{title} Example"

    try:
        state, path, copied_guidance = initialize_project_state(
            root,
            project=project,
            mode=args.mode,
            module=module_id,
            force=args.force,
            allow_runtime_state=False,
            no_project_guidance=args.no_project_guidance,
        )
    except FileExistsError as exc:
        raise SystemExit(f"State already exists: {exc}. Use --force to replace it.") from exc

    state["phase"] = first_phase
    state["status"] = "example-seeded"
    state["active_workflow"] = first_workflow
    state["human_input_required"] = "false"
    state["next_action"] = f"review the seeded {module_id} story, then run the quality gate"
    write_state(root, state)

    story_id = "example-start"
    story = {
        "id": story_id,
        "title": f"Run {title} example loop",
        "status": "ready",
        "phase": first_phase,
        "acceptance_criteria": join_list(
            [
                f"project state identifies module {module_id}",
                "example brief artifact exists",
                "quality gate passes with required evals",
            ]
        ),
        "evidence": "",
        "checks": "gate --require-evals",
        "blocker": "",
    }
    save_story(root, story)
    update_sprint(root)

    brief_rel = ".forge-method/artifacts/example-brief.md"
    brief_path = root / brief_rel
    if args.force and brief_path.exists() and brief_path.is_file():
        brief_path.unlink()
    summary = (
        f"Module: {module_id}. Purpose: {purpose} "
        f"Starting phase: {first_phase}. Starting workflow: {first_workflow}. "
        f"Workflow set: {join_list(workflows) or 'none'}."
    )
    artifact = write_artifact(
        root,
        kind="brief",
        title=f"{title} example brief",
        summary=summary,
        path=brief_rel,
        lifecycle="durable",
    )
    link_artifact_to_story(root, artifact, story_id)
    eval_path_rel = write_artifact_eval(root, artifact, title=f"{title} example brief", summary=summary)
    checkpoint = write_checkpoint(
        root,
        state,
        title="Example seed",
        summary=f"Seeded a runnable example project from module {module_id}.",
        decisions=[f"Use packaged module {module_id} as the initial route."],
        checks=["gate --require-evals"],
        failed_checks=[],
        touched=[STATE_FILE, SPRINT_FILE, "README.md", story_path(root, story_id).relative_to(root).as_posix(), artifact],
        artifacts=[artifact],
        next_action=state["next_action"],
    )
    write_context_pack(root, state, out=method_dir(root) / "context" / "current-pack.md", max_chars=args.max_chars)
    readme = root / "README.md"
    if args.force or not readme.exists():
        readme.write_text(
            "\n".join(
                [
                    f"# {project}",
                    "",
                    f"Module: `{module_id}`",
                    "",
                    "Use the installed Forge Method runtime helper to inspect this project:",
                    "",
                    "```powershell",
                    "python \"$HOME\\.agents\\skills\\forge-method\\scripts\\forge_method_runtime.py\" status --root .",
                    "python \"$HOME\\.agents\\skills\\forge-method\\scripts\\forge_method_runtime.py\" gate --root . --require-evals",
                    "```",
                ]
            )
            + "\n",
            encoding="utf-8",
        )
    append_ledger(
        root,
        "example.created",
        {
            "module": module_id,
            "story": story_id,
            "artifact": artifact,
            "eval": eval_path_rel,
            "checkpoint": checkpoint,
        },
    )

    print(f"Example created: {project}")
    print(f"Root: {root}")
    print(f"State: {path}")
    print(f"Module: {module_id}")
    print(f"Story: {story_id}")
    print(f"Artifact: {artifact}")
    print(f"Eval: {eval_path_rel}")
    print(f"Checkpoint: {checkpoint}")
    if copied_guidance:
        print(f"Project guidance: {', '.join(copied_guidance)}")
    print(
        "Gate command: "
        f"{command_hint_value(sys.executable)} "
        f"{command_hint_value(Path(__file__).resolve())} "
        f"gate --root {command_hint_value(root)} --require-evals"
    )
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
        root = None
    else:
        root, _ = load_state_or_none(resolve_root(args.root))
        paths = reference_workflow_paths(root)
    errors: list[str] = []
    for path in paths:
        errors.extend(validate_workflow_file(path))
    if not args.path:
        errors.extend(validate_workflow_catalog(root))
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
        write_eval(
            root,
            eval_id=f"{workflow_id}-routing",
            kind="workflow-routing",
            target=workflow_id,
            query=args.eval_query,
            expected=workflow_id,
        )
        if args.trigger:
            write_eval(
                root,
                eval_id=f"{workflow_id}-trigger",
                kind="workflow-trigger",
                target=workflow_id,
                query=args.trigger[0],
                expected=args.trigger[0],
            )
    print(path.relative_to(root).as_posix())
    return 0


def context_item_path(root: Path, path: Path) -> tuple[str, str]:
    resolved = path.resolve()
    try:
        return "project", resolved.relative_to(root.resolve()).as_posix()
    except ValueError:
        pass
    try:
        return "packaged", "skill:" + resolved.relative_to(SKILL_DIR.resolve()).as_posix()
    except ValueError:
        return "external", str(resolved)


def context_file_size(path: Path) -> int:
    if not path.exists() or not path.is_file():
        return 0
    try:
        return len(path.read_text(encoding="utf-8"))
    except UnicodeDecodeError:
        return int(path.stat().st_size)


def add_context_candidate(
    candidates: list[dict[str, Any]],
    seen: set[str],
    root: Path,
    path: Path,
    *,
    reason: str,
    priority: int,
    required: bool = False,
    section: str = "project",
) -> None:
    if not path.exists() or not path.is_file():
        return
    location, display_path = context_item_path(root, path)
    key = f"{location}:{display_path}"
    if key in seen:
        return
    seen.add(key)
    candidates.append(
        {
            "path": display_path,
            "location": location,
            "section": section,
            "reason": reason,
            "priority": priority,
            "required": required,
            "estimated_chars": context_file_size(path),
        }
    )


def build_context_load_plan(root: Path, state: dict[str, str], *, max_chars: int) -> dict[str, Any]:
    candidates: list[dict[str, Any]] = []
    seen: set[str] = set()
    next_story = select_next_story(root)
    active_story_id = state.get("active_story", "")

    add_context_candidate(
        candidates,
        seen,
        root,
        state_path(root),
        reason="authoritative phase, status, workflow, and next action",
        priority=100,
        required=True,
        section="state",
    )
    add_context_candidate(
        candidates,
        seen,
        root,
        method_dir(root) / SPRINT_FILE,
        reason="story counts and active story pointer",
        priority=96,
        required=True,
        section="state",
    )

    workflow_path = workflow_path_by_id(root, state.get("active_workflow", ""))
    if workflow_path:
        add_context_candidate(
            candidates,
            seen,
            root,
            workflow_path,
            reason="active workflow state machine",
            priority=92,
            required=True,
            section="workflow",
        )

    latest = latest_checkpoint_path(root)
    add_context_candidate(
        candidates,
        seen,
        root,
        latest,
        reason="latest durable progress memory",
        priority=88,
        required=False,
        section="memory",
    )

    story_for_context = active_story_id or (next_story or {}).get("id", "")
    story: dict[str, str] | None = None
    if story_for_context:
        story = load_story(root, story_for_context)
        add_context_candidate(
            candidates,
            seen,
            root,
            story_path(root, story_for_context),
            reason="active or next executable story",
            priority=86,
            required=bool(active_story_id),
            section="story",
        )

    for item in open_required_inputs(root):
        add_context_candidate(
            candidates,
            seen,
            root,
            human_input_path(root, item.get("id", "")),
            reason="open required human decision",
            priority=84,
            required=True,
            section="human-input",
        )

    for item in open_review_findings(root):
        finding_story = item.get("story", "")
        add_context_candidate(
            candidates,
            seen,
            root,
            review_finding_path(root, item.get("id", "")),
            reason=f"open review finding for story {finding_story}",
            priority=83,
            required=bool(active_story_id and finding_story == active_story_id),
            section="review-finding",
        )

    recommended_ids = recommended_agent_ids(state, next_story, audit_project(root))
    for index, profile_id in enumerate(recommended_ids):
        match = agent_profile_by_id(root, profile_id)
        if not match:
            continue
        _, path = match
        add_context_candidate(
            candidates,
            seen,
            root,
            path,
            reason=f"recommended agent profile for current state: {profile_id}",
            priority=80 - index,
            required=False,
            section="agent-profile",
        )

    add_context_candidate(
        candidates,
        seen,
        root,
        artifact_index_path(root),
        reason="artifact provenance and lifecycle index",
        priority=76,
        required=False,
        section="artifact",
    )

    if story:
        for artifact in split_list(story.get("artifacts")):
            add_context_candidate(
                candidates,
                seen,
                root,
                root / artifact,
                reason=f"artifact linked to story {story.get('id', '')}",
                priority=72,
                required=False,
                section="artifact",
            )

    evidence_paths = sorted((method_dir(root) / "evidence").glob("*.md"))[-3:]
    for offset, path in enumerate(reversed(evidence_paths)):
        add_context_candidate(
            candidates,
            seen,
            root,
            path,
            reason="recent validation or release evidence",
            priority=68 - offset,
            required=False,
            section="evidence",
        )

    candidates.sort(key=lambda item: (-int(item["priority"]), item["path"]))
    selected: list[dict[str, Any]] = []
    deferred: list[dict[str, Any]] = []
    selected_chars = 0
    for item in candidates:
        item_chars = int(item.get("estimated_chars", 0))
        if item.get("required") or not max_chars or selected_chars + item_chars <= max_chars:
            selected.append(item)
            selected_chars += item_chars
        else:
            deferred.append(item)

    required_chars = sum(int(item.get("estimated_chars", 0)) for item in candidates if item.get("required"))
    return {
        "runtime": RUNTIME_NAME,
        "runtime_version": RUNTIME_VERSION,
        "generated_at": utc_now(),
        "root": str(root),
        "budget_chars": max_chars,
        "estimated_selected_chars": selected_chars,
        "estimated_required_chars": required_chars,
        "over_budget": bool(max_chars and required_chars > max_chars),
        "state": {
            "project": state.get("project", ""),
            "phase": state.get("phase", ""),
            "status": state.get("status", ""),
            "workflow": state.get("active_workflow", ""),
            "active_story": active_story_id,
            "next_action": state.get("next_action", ""),
        },
        "rules": [
            "load selected items in order",
            "prefer selected files over conversation memory",
            "do not load deferred files unless the current task explicitly needs them",
            "after meaningful progress, write checkpoint or evidence before ending",
        ],
        "selected": selected,
        "deferred": deferred,
    }


def build_context_health(
    root: Path,
    state: dict[str, str],
    *,
    max_chars: int,
    plan: dict[str, Any] | None = None,
) -> dict[str, Any]:
    plan = plan or build_context_load_plan(root, state, max_chars=max_chars)
    budget = int(plan.get("budget_chars", 0) or 0)
    selected_chars = int(plan.get("estimated_selected_chars", 0) or 0)
    required_chars = int(plan.get("estimated_required_chars", 0) or 0)
    selected_ratio = (selected_chars / budget) if budget else 0.0
    required_ratio = (required_chars / budget) if budget else 0.0
    deferred_count = len(plan.get("deferred", []))
    over_budget = bool(plan.get("over_budget"))

    if over_budget:
        level = "blocked"
        recommended_action = "split work or write compact recovery before loading more context"
    elif budget and (selected_ratio >= 0.90 or deferred_count):
        level = "compact"
        recommended_action = "write compact recovery before continuing the work block"
    elif budget and selected_ratio >= 0.65:
        level = "watch"
        recommended_action = "continue, then checkpoint before the next substantial step"
    else:
        level = "ok"
        recommended_action = "continue with selected context"

    commands = [
        preflight_command("context-plan", "context", "plan", "--root", root, "--json", "--max-chars", max_chars),
    ]
    if level in {"compact", "blocked"}:
        commands.append(
            preflight_command("compact-recovery", "context", "recover", "--root", root, "--compact", "--max-chars", max_chars)
        )
    if level in {"watch", "compact"}:
        commands.append(
            preflight_command(
                "checkpoint",
                "checkpoint",
                "--root",
                root,
                "--summary",
                "<progress memory>",
                "--next-action",
                "<next action>",
            )
        )

    return {
        "runtime": RUNTIME_NAME,
        "runtime_version": RUNTIME_VERSION,
        "generated_at": utc_now(),
        "root": str(root),
        "level": level,
        "recommended_action": recommended_action,
        "budget_chars": budget,
        "estimated_selected_chars": selected_chars,
        "estimated_required_chars": required_chars,
        "selected_ratio": round(selected_ratio, 3),
        "required_ratio": round(required_ratio, 3),
        "selected_count": len(plan.get("selected", [])),
        "deferred_count": deferred_count,
        "over_budget": over_budget,
        "state": plan.get("state", {}),
        "commands": commands,
        "rules": [
            "treat blocked or compact health as a handoff signal",
            "prefer compact recovery over loading deferred files",
            "write checkpoint memory before ending a substantial work block",
        ],
    }


def write_context_load_plan(root: Path, state: dict[str, str], *, out: Path, max_chars: int) -> Path:
    if not out.is_absolute():
        out = root / out
    out.parent.mkdir(parents=True, exist_ok=True)
    plan = build_context_load_plan(root, state, max_chars=max_chars)
    out.write_text(json.dumps(plan, ensure_ascii=True, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    append_ledger(root, "context_load_plan.written", {"path": out.relative_to(root).as_posix()})
    return out


def build_context_pack_text(root: Path, state: dict[str, str]) -> str:
    story = load_story(root, state["active_story"]) if state.get("active_story") else None
    evidence_paths = sorted((method_dir(root) / "evidence").glob("*.md"))[-5:]
    summaries = artifact_summaries(root)
    failed_checks = checkpoint_section_items(root, "Failed Checks")
    touched_files = checkpoint_section_items(root, "Touched Files")
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
    lines.extend(["", "## Recovery Signals", "", "### Failed Checks", ""])
    append_markdown_items(lines, failed_checks)
    lines.extend(["", "### Touched Files", ""])
    append_markdown_items(lines, touched_files)
    lines.extend(["", "## Open Human Inputs", ""])
    open_inputs = list_human_inputs(root)
    open_inputs = [item for item in open_inputs if item.get("status") == "open"]
    if open_inputs:
        for item in open_inputs:
            required = item.get("required", "true")
            lines.append(f"- {item.get('id')} [required={required}]: {item.get('prompt')}")
    else:
        lines.append("- none")
    lines.extend(["", "## Open Review Findings", ""])
    open_findings = open_review_findings(root)
    if open_findings:
        for item in open_findings:
            lines.append(
                f"- {item.get('id')} [{item.get('severity')}] "
                f"story={item.get('story')}: {item.get('title')} - {item.get('summary')}"
            )
    else:
        lines.append("- none")
    lines.extend(["", "## Recommended Agent Profiles", ""])
    recommendations = recommended_agent_profiles(root, state, select_next_story(root), audit_project(root))
    if recommendations:
        for profile in recommendations:
            lines.append(f"- {profile.get('id')} ({profile.get('title')}): {profile.get('purpose')}")
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


def build_recovery_brief_text(root: Path, state: dict[str, str], *, checkpoint_limit: int = 5) -> str:
    failed_checks = checkpoint_section_items(root, "Failed Checks", checkpoint_limit=checkpoint_limit)
    touched_files = checkpoint_section_items(root, "Touched Files", checkpoint_limit=checkpoint_limit)
    checkpoints = recent_checkpoint_paths(root, limit=checkpoint_limit)
    active_story = state.get("active_story", "")
    read_order = [
        state_path(root).relative_to(root).as_posix(),
        (method_dir(root) / SPRINT_FILE).relative_to(root).as_posix(),
    ]
    latest = latest_checkpoint_path(root)
    current_pack = method_dir(root) / "context" / "current-pack.md"
    load_plan = method_dir(root) / "context" / "load-plan.json"
    if latest.exists():
        read_order.append(latest.relative_to(root).as_posix())
    if load_plan.exists():
        read_order.append(load_plan.relative_to(root).as_posix())
    if current_pack.exists():
        read_order.append(current_pack.relative_to(root).as_posix())
    if active_story:
        read_order.append(story_path(root, active_story).relative_to(root).as_posix())
    for item in open_review_findings(root):
        path = review_finding_path(root, item.get("id", ""))
        if path.exists():
            read_order.append(path.relative_to(root).as_posix())
    if artifact_index_path(root).exists():
        read_order.append(artifact_index_path(root).relative_to(root).as_posix())

    runtime = command_hint_value(Path(__file__).resolve())
    root_hint = command_hint_value(root)
    lines = [
        "# Forge Method Recovery Brief",
        "",
        "## State",
        "",
        f"- project: {state.get('project', '')}",
        f"- phase: {state.get('phase', '')}",
        f"- status: {state.get('status', '')}",
        f"- workflow: {state.get('active_workflow', '')}",
        f"- active_story: {active_story or '<none>'}",
        f"- readiness: {state.get('readiness', '')}",
        f"- human_input_required: {state.get('human_input_required', '')}",
        f"- next_action: {state.get('next_action', '')}",
        "",
        "## Read Order",
        "",
    ]
    append_markdown_items(lines, read_order)
    lines.extend(["", "## Resume Commands", "", "```powershell"])
    lines.extend(
        [
            f"python {runtime} start --root {root_hint}",
            f"python {runtime} status --root {root_hint}",
            f"python {runtime} context plan --root {root_hint}",
            f"python {runtime} context pack --root {root_hint}",
            f"python {runtime} gate --root {root_hint} --require-evals",
        ]
    )
    lines.extend(["```", "", "## Recent Checkpoints", ""])
    append_markdown_items(lines, [path.relative_to(root).as_posix() for path in checkpoints])
    lines.extend(["", "## Failed Checks", ""])
    append_markdown_items(lines, failed_checks)
    lines.extend(["", "## Touched Files", ""])
    append_markdown_items(lines, touched_files)
    lines.extend(["", "## Open Human Inputs", ""])
    open_inputs = [item for item in list_human_inputs(root) if item.get("status") == "open"]
    if open_inputs:
        for item in open_inputs:
            required = item.get("required", "true")
            lines.append(f"- {item.get('id')} [required={required}]: {item.get('prompt')}")
    else:
        lines.append("- none")
    lines.extend(["", "## Open Review Findings", ""])
    open_findings = open_review_findings(root)
    if open_findings:
        for item in open_findings:
            lines.append(
                f"- {item.get('id')} [{item.get('severity')}] "
                f"story={item.get('story')}: {item.get('title')} - {item.get('summary')}"
            )
    else:
        lines.append("- none")
    lines.extend(["", "## Recommended Agent Profiles", ""])
    recommendations = recommended_agent_profiles(root, state, select_next_story(root), audit_project(root))
    if recommendations:
        for profile in recommendations:
            lines.append(f"- {profile.get('id')} ({profile.get('title')}): {profile.get('purpose')}")
    else:
        lines.append("- none")
    lines.extend(["", "## Recent Artifacts", ""])
    artifacts = recent_artifacts(root)
    if artifacts:
        for artifact in artifacts:
            lines.append(f"- {artifact.get('path')} [{artifact.get('status', 'active')}/{artifact.get('lifecycle', 'durable')}]")
    else:
        lines.append("- none")
    return "\n".join(lines) + "\n"


def markdown_section(title: str, body: list[str]) -> str:
    return "\n".join([f"## {title}", "", *body]).rstrip() + "\n"


def append_compact_section(sections: list[str], title: str, body: list[str], *, max_items: int | None = None) -> None:
    items = body[:max_items] if max_items is not None else body
    if not items:
        items = ["- none"]
    sections.append(markdown_section(title, items))


def compact_command_summary(command: dict[str, str]) -> str:
    text = command.get("command", "")
    script_name = Path(__file__).name
    marker = f"{script_name}\" "
    if marker in text:
        text = text.split(marker, 1)[1]
    return f"- {command.get('name')}: {text}"


def build_compact_recovery_brief_text(
    root: Path,
    state: dict[str, str],
    *,
    checkpoint_limit: int = 3,
    max_chars: int = 4000,
) -> str:
    snapshot = build_snapshot(root, state)
    resume = snapshot["resume"]
    context_plan = build_context_load_plan(root, state, max_chars=max(1200, max_chars // 2 if max_chars else 2000))
    selected = context_plan.get("selected", [])
    checkpoints = recent_checkpoint_paths(root, limit=checkpoint_limit)
    failed_checks = checkpoint_section_items(root, "Failed Checks", checkpoint_limit=checkpoint_limit)
    touched_files = checkpoint_section_items(root, "Touched Files", checkpoint_limit=checkpoint_limit)

    command_section = markdown_section(
        "Commands",
        [compact_command_summary(item) for item in resume.get("commands", [])[:6]] or ["- none"],
    )
    sections: list[str] = [
        "# Forge Method Compact Recovery",
        "",
        markdown_section(
            "State",
            [
                f"- project: {state.get('project', '')}",
                f"- phase: {state.get('phase', '')}",
                f"- status: {state.get('status', '')}",
                f"- workflow: {state.get('active_workflow', '')}",
                f"- active_story: {state.get('active_story', '') or '<none>'}",
                f"- readiness: {state.get('readiness', '')}",
                f"- next_action: {state.get('next_action', '')}",
            ],
        ),
        markdown_section(
            "Resume",
            [
                f"- action: {resume.get('action', '')}",
                f"- autonomous: {'true' if resume.get('autonomous') else 'false'}",
                f"- summary: {resume.get('summary', '')}",
                f"- next_command: {resume.get('next_command', '')}",
            ],
        ),
        command_section,
    ]
    append_compact_section(sections, "Read First", [f"- {item}" for item in resume.get("read", [])], max_items=8)
    append_compact_section(
        sections,
        "Context Selection",
        [
            f"- {item.get('path')} [{item.get('section')}]: {item.get('reason')}"
            for item in selected[:8]
        ],
    )
    append_compact_section(sections, "Done When", [f"- {item}" for item in resume.get("done_when", [])], max_items=5)
    append_compact_section(sections, "Blocked When", [f"- {item}" for item in resume.get("blocked_when", [])], max_items=5)
    append_compact_section(
        sections,
        "Open Human Inputs",
        [
            f"- {item.get('id')} [required={item.get('required', 'true')}]: {item.get('prompt')}"
            for item in snapshot["human_inputs"]["open"][:3]
        ],
    )
    append_compact_section(
        sections,
        "Open Review Findings",
        [
            f"- {item.get('id')} [{item.get('severity')}] story={item.get('story')}: {item.get('title')}"
            for item in snapshot["review_findings"]["open"][:3]
        ],
    )
    append_compact_section(sections, "Recent Checkpoints", [f"- {path.relative_to(root).as_posix()}" for path in checkpoints], max_items=3)
    append_compact_section(sections, "Failed Checks", [f"- {item}" for item in failed_checks], max_items=5)
    append_compact_section(sections, "Touched Files", [f"- {item}" for item in touched_files], max_items=8)

    text = "\n".join(sections).rstrip() + "\n"
    if max_chars and len(text) > max_chars:
        required_sections = sections[:5]
        optional_sections = sections[5:]
        footer = "\n[compact-recovery omitted lower-priority sections to fit max_chars]\n"
        text = "\n".join(required_sections).rstrip() + "\n"
        for section in optional_sections:
            candidate = text.rstrip() + "\n\n" + section.rstrip() + "\n"
            if len(candidate) + len(footer) <= max_chars:
                text = candidate
        if len(text) + len(footer) <= max_chars:
            text = text.rstrip() + "\n" + footer
        elif len(text) > max_chars:
            text = text[: max(0, max_chars - len(footer))].rstrip() + footer
    return text


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


def write_recovery_brief(
    root: Path,
    state: dict[str, str],
    *,
    out: Path,
    max_chars: int,
    checkpoint_limit: int,
    compact: bool = False,
) -> Path:
    if not out.is_absolute():
        out = root / out
    out.parent.mkdir(parents=True, exist_ok=True)
    if compact:
        text = build_compact_recovery_brief_text(
            root,
            state,
            checkpoint_limit=checkpoint_limit,
            max_chars=max_chars,
        )
    else:
        text = build_recovery_brief_text(root, state, checkpoint_limit=checkpoint_limit)
    if max_chars and len(text) > max_chars:
        footer = "\n\n[recovery-brief truncated to max_chars]\n"
        text = text[: max(0, max_chars - len(footer))].rstrip() + footer
    out.write_text(text, encoding="utf-8")
    append_ledger(root, "recovery_brief.written", {"path": out.relative_to(root).as_posix()})
    return out


def cmd_context_pack(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    out = Path(args.out) if args.out else method_dir(root) / "context" / "current-pack.md"
    out = write_context_pack(root, state, out=out, max_chars=args.max_chars)
    print(out)
    return 0


def cmd_context_plan(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    out = Path(args.out) if args.out else method_dir(root) / "context" / "load-plan.json"
    out = write_context_load_plan(root, state, out=out, max_chars=args.max_chars)
    if args.json:
        print(out.read_text(encoding="utf-8").rstrip())
    else:
        print(out)
    return 0


def cmd_context_health(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    health = build_context_health(root, state, max_chars=args.max_chars)
    if args.json:
        print(json.dumps(health, ensure_ascii=True, sort_keys=True, indent=2))
    else:
        print(f"Context health: {health['level']}")
        print(
            "Budget: "
            f"{health['estimated_selected_chars']}/{health['budget_chars']} chars selected "
            f"({health['selected_ratio']})"
        )
        print(f"Required: {health['estimated_required_chars']} chars ({health['required_ratio']})")
        print(f"Deferred files: {health['deferred_count']}")
        print(f"Recommended action: {health['recommended_action']}")
        next_command = health["commands"][0]["command"] if health["commands"] else ""
        if next_command:
            print(f"Next command: {next_command}")
    return 0


def cmd_context_recover(args: argparse.Namespace) -> int:
    root, state = load_state_or_fail(resolve_root(args.root))
    write_context_load_plan(root, state, out=method_dir(root) / "context" / "load-plan.json", max_chars=args.max_chars)
    write_context_pack(root, state, out=method_dir(root) / "context" / "current-pack.md", max_chars=args.max_chars)
    default_name = "recovery-compact.md" if args.compact else "recovery.md"
    out = Path(args.out) if args.out else method_dir(root) / "context" / default_name
    out = write_recovery_brief(
        root,
        state,
        out=out,
        max_chars=args.max_chars,
        checkpoint_limit=args.checkpoints,
        compact=args.compact,
    )
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
    kind = args.kind
    if kind in {"workflow-routing", "workflow-trigger"}:
        target = slugify(args.target)
        if workflow_path_by_id(root, target) is None:
            raise SystemExit(f"Target workflow not found: {args.target}")
    elif kind == "artifact-exists":
        _, target = project_path(root, args.target)
    else:
        raise SystemExit(f"Invalid eval kind: {kind}")
    path = write_eval(
        root,
        eval_id=args.id,
        kind=kind,
        target=target,
        query=args.query,
        expected=args.expected or ("exists" if kind == "artifact-exists" else target),
    )
    print(path)
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
        kind = item.get("kind", "workflow-routing")
        target = item.get("target", "")
        expected = item.get("expected", target)
        query = item.get("query", "")
        errors: list[str] = []
        if not query:
            errors.append("query is empty")
        if kind == "workflow-routing":
            workflow_path = workflow_path_by_id(root, target)
            errors.extend(validate_workflow_file(workflow_path) if workflow_path else [f"target workflow not found: {target}"])
            if expected != target:
                errors.append(f"expected workflow {expected} does not match target {target}")
        elif kind == "workflow-trigger":
            workflow_path = workflow_path_by_id(root, target)
            errors.extend(validate_workflow_file(workflow_path) if workflow_path else [f"target workflow not found: {target}"])
            trigger_text = expected or query
            if workflow_path and trigger_text not in workflow_path.read_text(encoding="utf-8"):
                errors.append(f"trigger text not found: {trigger_text}")
        elif kind == "artifact-exists":
            try:
                artifact_path, rel = project_path(root, target)
            except SystemExit as exc:
                errors.append(str(exc))
            else:
                if not artifact_path.exists() and not artifact_missing_allowed(root, rel):
                    errors.append(f"artifact not available: {rel}")
                if expected and expected != "exists":
                    errors.append(f"expected artifact result must be exists: {expected}")
        else:
            errors.append(f"unknown eval kind: {kind}")
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

    agent_errors = agent_profile_validation_errors(root)
    if agent_errors:
        errors.extend(f"agent: {error}" for error in agent_errors)
    config_errors = config_validation_errors(root)
    if config_errors:
        errors.extend(f"config: {error}" for error in config_errors)

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
        print("Audit: passed")
        print("Artifacts: passed")
        print("Workflows: passed")
        print("Agents: passed")
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
                    "agent validate",
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
    audit_errors = audit_project(state_root) if state_root else []
    payload = {
        "workspace": str(root),
        "runtime_repo": is_runtime_repo(root),
        "project_state_root": str(state_root) if state_root else None,
        "audit": {
            "passed": not audit_errors if state_root else None,
            "errors": audit_errors,
        },
        "plugin_installation": collect_plugin_installation(),
        "toolchain": collect_toolchain(),
        "verification": verification_recommendation(args.mode, args.touches or []),
    }
    if args.json:
        print(json.dumps(payload, ensure_ascii=True, sort_keys=True, indent=2))
        return 0
    print("Forge Method Doctor")
    print(f"Workspace: {payload['workspace']}")
    print(f"Runtime repo: {'yes' if payload['runtime_repo'] else 'no'}")
    print(f"Project state root: {payload['project_state_root'] or '<none>'}")
    if state_root:
        print(f"Audit: {'passed' if not audit_errors else 'failed'}")
        for error in audit_errors:
            print(f"- {error}")
    plugin = payload["plugin_installation"]
    print("Plugin installation:")
    print(f"- Status: {plugin['status']}")
    print(f"- Marketplace: {plugin['marketplace_path']}")
    print(f"- Plugin source: {plugin['plugin_path'] or '<none>'}")
    print(f"- Installed version: {plugin['installed_version'] or '<none>'}")
    if plugin["codex_deeplink"]:
        print(f"- Open in Codex: {plugin['codex_deeplink']}")
    if plugin["status"] != "ready":
        for command in plugin["repair_commands"]["windows"]:
            print(f"- Repair: {command}")
    toolchain = payload["toolchain"]
    python = toolchain["python"]
    print("Toolchain:")
    print(f"- Python current: {python['current']['version']} at {python['current']['path']}")
    print(f"- Python command: {python['command_status']}")
    print(f"- Git: {toolchain['git']['status']}")
    print(f"- GitHub CLI: {toolchain['github_cli']['status']}")
    print(f"- WSL: {toolchain['wsl']['status']}")
    verification = payload["verification"]
    validation = verification["validation"]
    print("Verification:")
    print(f"- Development validation: {validation['development']}")
    for command in verification["development_commands"]["windows"]:
        print(f"  - {command}")
    print(f"- Release validation: {validation['release']}")
    for command in verification["release_commands"]["windows"]:
        print(f"  - {command}")
    print(f"- Reason: {validation['reason']}")
    return 0


def marketplace_root_for(marketplace_path: Path) -> Path:
    path = marketplace_path.expanduser().resolve()
    if (
        path.name == "marketplace.json"
        and path.parent.name == "plugins"
        and path.parent.parent.name == ".agents"
    ):
        return path.parent.parent.parent
    return path.parent


def collect_plugin_installation() -> dict[str, Any]:
    marketplace_path = Path.home() / ".agents" / "plugins" / "marketplace.json"
    marketplace_root = marketplace_root_for(marketplace_path)
    encoded_marketplace = quote(str(marketplace_path.resolve()), safe="")
    base = {
        "name": RUNTIME_REPO_NAME,
        "expected_version": RUNTIME_VERSION,
        "available": False,
        "status": "missing marketplace",
        "marketplace_path": str(marketplace_path),
        "marketplace_root": str(marketplace_root),
        "marketplace_exists": marketplace_path.exists(),
        "marketplace_name": None,
        "entry_found": False,
        "source_path": None,
        "plugin_path": None,
        "manifest_path": None,
        "skill_path": None,
        "manifest_exists": False,
        "skill_exists": False,
        "installed_version": None,
        "codex_deeplink": f"codex://plugins/{RUNTIME_REPO_NAME}?marketplacePath={encoded_marketplace}",
        "share_deeplink": f"codex://plugins/{RUNTIME_REPO_NAME}?marketplacePath={encoded_marketplace}&mode=share",
        "repair_commands": {
            "windows": ["powershell -ExecutionPolicy Bypass -File .\\scripts\\install-plugin-local.ps1"],
            "posix": ["bash scripts/install-plugin-local.sh"],
        },
    }
    if not marketplace_path.exists():
        return base
    try:
        marketplace = json.loads(marketplace_path.read_text(encoding="utf-8-sig"))
    except (OSError, json.JSONDecodeError) as exc:
        base["status"] = f"invalid marketplace: {str(exc)[:120]}"
        return base
    base["marketplace_name"] = marketplace.get("name")
    plugins = marketplace.get("plugins", [])
    entry = next(
        (plugin for plugin in plugins if isinstance(plugin, dict) and plugin.get("name") == RUNTIME_REPO_NAME),
        None,
    )
    if not entry:
        base["status"] = "missing marketplace entry"
        return base
    base["entry_found"] = True
    source = entry.get("source", {})
    source_path = source.get("path")
    base["source_path"] = source_path
    if source.get("source") != "local" or not source_path:
        base["status"] = "marketplace entry is not a local plugin source"
        return base
    plugin_path = (marketplace_root / source_path).resolve()
    try:
        plugin_path.relative_to(marketplace_root.resolve())
    except ValueError:
        base["plugin_path"] = str(plugin_path)
        base["status"] = "plugin source escapes marketplace root"
        return base
    manifest_path = plugin_path / ".codex-plugin" / "plugin.json"
    skill_path = plugin_path / "skills" / "forge-method" / "SKILL.md"
    base["plugin_path"] = str(plugin_path)
    base["manifest_path"] = str(manifest_path)
    base["skill_path"] = str(skill_path)
    base["manifest_exists"] = manifest_path.exists()
    base["skill_exists"] = skill_path.exists()
    if not plugin_path.exists():
        base["status"] = "plugin source path missing"
        return base
    if not manifest_path.exists():
        base["status"] = "plugin manifest missing"
        return base
    if not skill_path.exists():
        base["status"] = "forge-method skill missing"
        return base
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8-sig"))
    except (OSError, json.JSONDecodeError) as exc:
        base["status"] = f"invalid plugin manifest: {str(exc)[:120]}"
        return base
    base["installed_version"] = manifest.get("version")
    if manifest.get("name") != RUNTIME_REPO_NAME:
        base["status"] = "plugin manifest name mismatch"
        return base
    if manifest.get("version") != RUNTIME_VERSION:
        base["status"] = "plugin version mismatch"
        return base
    base["available"] = True
    base["status"] = "ready"
    base["repair_commands"] = {"windows": [], "posix": []}
    return base


def run_probe(command: list[str], timeout: float = 3.0) -> dict[str, Any]:
    try:
        result = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
        )
        output = " ".join(decode_probe_output(result.stdout + b"\n" + result.stderr).split())
        return {
            "ok": result.returncode == 0,
            "returncode": result.returncode,
            "summary": output[:240],
        }
    except (OSError, subprocess.TimeoutExpired) as exc:
        return {
            "ok": False,
            "returncode": None,
            "summary": str(exc)[:240],
        }


def decode_probe_output(raw: bytes) -> str:
    if not raw:
        return ""
    sample = raw[:80]
    if sample.count(b"\x00") > len(sample) // 4:
        return raw.decode("utf-16-le", errors="replace").replace("\x00", "")
    return raw.decode("utf-8", errors="replace").replace("\x00", "")


def probe_command(name: str, version_args: list[str]) -> dict[str, Any]:
    path = shutil.which(name)
    if not path:
        return {"available": False, "path": None, "status": "missing"}
    probe = run_probe([path, *version_args])
    summary = probe["summary"] or ("available" if probe["ok"] else "installed but version check failed")
    return {
        "available": True,
        "path": path,
        "version_ok": probe["ok"],
        "status": summary,
    }


def codex_python_candidates() -> list[Path]:
    home = Path.home()
    return [
        home / ".cache" / "codex-runtimes" / "codex-primary-runtime" / "dependencies" / "python" / "python.exe",
        home / ".cache" / "codex-runtimes" / "codex-primary-runtime" / "dependencies" / "python" / "bin" / "python",
    ]


def collect_python_toolchain() -> dict[str, Any]:
    commands = []
    for name in ["python", "python3", "py"]:
        path = shutil.which(name)
        commands.append({"name": name, "available": bool(path), "path": path})
    bundled = [{"path": str(path), "available": path.exists()} for path in codex_python_candidates()]
    command_available = any(item["available"] for item in commands)
    bundled_available = any(item["available"] for item in bundled)
    if command_available:
        status = "available on PATH"
    elif bundled_available:
        status = "available through Codex bundled runtime"
    else:
        status = "missing command; current interpreter can still run this helper"
    return {
        "current": {
            "path": sys.executable,
            "version": sys.version.split()[0],
        },
        "commands": commands,
        "bundled_candidates": bundled,
        "command_status": status,
    }


def collect_wsl_toolchain() -> dict[str, Any]:
    path = shutil.which("wsl")
    if not path:
        return {"available": False, "path": None, "has_distribution": False, "status": "missing"}
    status_probe = run_probe([path, "--status"], timeout=4.0)
    list_probe = run_probe([path, "--list", "--verbose"], timeout=4.0)
    has_distribution = list_probe["ok"]
    if has_distribution:
        status = "available with registered distribution"
    elif "no distributions" in list_probe["summary"].lower() or "não tem distribui" in list_probe["summary"].lower():
        status = "available but no Linux distribution is installed"
    else:
        status = "available; distribution check did not pass"
    return {
        "available": True,
        "path": path,
        "has_distribution": has_distribution,
        "status": status,
        "status_probe": status_probe,
        "list_probe": list_probe,
    }


def collect_toolchain() -> dict[str, Any]:
    return {
        "python": collect_python_toolchain(),
        "git": probe_command("git", ["--version"]),
        "github_cli": probe_command("gh", ["--version"]),
        "wsl": collect_wsl_toolchain(),
    }


def verification_commands_for_profile(profile: str, touches: list[str]) -> dict[str, list[str]]:
    windows: list[str] = []
    posix: list[str] = []
    if profile == "fast":
        windows.append("powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1")
        posix.append("bash scripts/verify-fast.sh")
    elif profile == "targeted-smoke":
        windows.append("powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-fast.ps1")
        posix.append("bash scripts/verify-fast.sh")
        touched = set(touches)
        if touched & {"runtime", "workflow", "state"}:
            windows.append("powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1")
            posix.append("bash scripts/smoke-runtime.sh")
        if touched & {"install", "package"}:
            windows.append("powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1")
            posix.append("bash scripts/smoke-install.sh")
    else:
        windows.append("powershell -ExecutionPolicy Bypass -File .\\scripts\\verify-all.ps1")
        posix.append("bash scripts/verify-all.sh")
    return {"windows": windows, "posix": posix}


def verification_recommendation(mode: str, touches: list[str]) -> dict[str, Any]:
    validation = release_validation_tier(mode, touches)
    return {
        "mode": mode,
        "touches": touches,
        "validation": validation,
        "development_commands": verification_commands_for_profile(validation["development"], touches),
        "release_commands": verification_commands_for_profile(validation["release"], touches),
    }


def parse_semver(value: str) -> tuple[int, int, int]:
    match = SEMVER_RE.match(value.strip())
    if not match:
        raise SystemExit(f"Expected semantic version X.Y.Z, got: {value}")
    return int(match.group(1)), int(match.group(2)), int(match.group(3))


def bump_semver(value: str, mode: str) -> str:
    major, minor, patch = parse_semver(value)
    if mode == "hotfix":
        return f"{major}.{minor}.{patch + 1}"
    if mode in {"story", "batch"}:
        return f"{major}.{minor + 1}.0"
    if mode == "breaking":
        return f"{major + 1}.0.0"
    raise SystemExit(f"Invalid release mode: {mode}")


def current_version_for_release_plan(root: Path, explicit: str | None) -> str:
    if explicit:
        parse_semver(explicit)
        return explicit
    version_file = root / "VERSION"
    if version_file.exists():
        value = version_file.read_text(encoding="utf-8").strip()
        parse_semver(value)
        return value
    state_root, state = load_state_or_none(root)
    if state_root and state.get("runtime_version"):
        parse_semver(state["runtime_version"])
        return state["runtime_version"]
    return RUNTIME_VERSION


def release_validation_tier(mode: str, touches: list[str]) -> dict[str, str]:
    touched = set(touches)
    if mode == "breaking" or "install" in touched or "package" in touched:
        return {
            "development": "targeted-smoke",
            "release": "full",
            "reason": "public surface or distribution changed",
        }
    if "runtime" in touched or "workflow" in touched or "state" in touched:
        return {
            "development": "targeted-smoke",
            "release": "full",
            "reason": "runtime behavior or state transitions changed",
        }
    return {
        "development": "fast",
        "release": "full",
        "reason": "normal development can stay fast; publishing still needs full verification",
    }


def cmd_release_plan(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    current = current_version_for_release_plan(root, args.current_version)
    suggested = bump_semver(current, args.mode)
    validation = release_validation_tier(args.mode, args.touches or [])
    plan = {
        "runtime": RUNTIME_NAME,
        "current_version": current,
        "suggested_version": suggested,
        "mode": args.mode,
        "touches": args.touches or [],
        "validation": validation,
        "publish": {
            "create_tag": False,
            "create_release": False,
            "rule": "plan only; tag and release after the batch is complete and full verification passes",
        },
    }
    if args.json:
        print(json.dumps(plan, ensure_ascii=True, sort_keys=True, indent=2))
        return 0
    print("Release plan")
    print(f"Current version: {current}")
    print(f"Suggested version: {suggested}")
    print(f"Mode: {args.mode}")
    print(f"Development validation: {validation['development']}")
    print(f"Release validation: {validation['release']}")
    print(f"Reason: {validation['reason']}")
    print("Publish: no tag or release from this command")
    return 0


def changelog_section_items(path: Path, heading: str) -> list[str]:
    if not path.exists():
        return []
    lines = path.read_text(encoding="utf-8").splitlines()
    in_section = False
    items: list[str] = []
    for line in lines:
        stripped = line.strip()
        if stripped.startswith("## "):
            if in_section:
                break
            in_section = stripped == heading
            continue
        if in_section and stripped.startswith("- "):
            items.append(stripped[2:].strip())
    return items


def changelog_unreleased_items(path: Path) -> list[str]:
    return changelog_section_items(path, "## Unreleased")


def runtime_version_in_file(path: Path) -> str:
    if not path.exists():
        return ""
    match = re.search(r'^RUNTIME_VERSION\s*=\s*"([^"]+)"', path.read_text(encoding="utf-8"), re.MULTILINE)
    return match.group(1) if match else ""


def plugin_manifest_version(path: Path) -> str:
    if not path.exists():
        return ""
    try:
        payload = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return ""
    return str(payload.get("version", ""))


def git_clean_state(root: Path) -> tuple[bool | None, str]:
    if not (root / ".git").exists():
        return None, "not a git checkout"
    try:
        result = subprocess.run(
            ["git", "-C", str(root), "status", "--porcelain"],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            check=False,
        )
    except OSError as exc:
        return None, f"git unavailable: {exc}"
    if result.returncode != 0:
        return None, result.stderr.strip() or "git status failed"
    dirty_lines = [line for line in result.stdout.splitlines() if line.strip()]
    if dirty_lines:
        return False, f"{len(dirty_lines)} changed path(s)"
    return True, "clean"


def release_check_payload(root: Path, *, mode: str, touches: list[str], current_version: str | None) -> dict[str, Any]:
    current = current_version_for_release_plan(root, current_version)
    suggested = bump_semver(current, mode)
    validation = release_validation_tier(mode, touches)
    checks: list[dict[str, Any]] = []

    def add_check(name: str, passed: bool, detail: str, *, required: bool = True) -> None:
        checks.append({"name": name, "passed": passed, "required": required, "detail": detail})

    version_file = root / "VERSION"
    add_check("version_file", version_file.exists(), "VERSION exists" if version_file.exists() else "VERSION missing")
    if version_file.exists():
        add_check(
            "version_file_matches_current",
            version_file.read_text(encoding="utf-8").strip() == current,
            f"VERSION is {version_file.read_text(encoding='utf-8').strip()}",
        )

    plugin_version = plugin_manifest_version(root / ".codex-plugin" / "plugin.json")
    if plugin_version:
        add_check("plugin_version_matches_current", plugin_version == current, f"plugin version is {plugin_version}")

    runtime_version = runtime_version_in_file(root / "skills" / "forge-method" / "scripts" / "forge_method_runtime.py")
    if runtime_version:
        add_check("runtime_version_matches_current", runtime_version == current, f"runtime version is {runtime_version}")

    changelog = root / "CHANGELOG.md"
    unreleased = changelog_unreleased_items(changelog)
    current_release_items = changelog_section_items(changelog, f"## {current}")
    add_check("changelog_exists", changelog.exists(), "CHANGELOG.md exists" if changelog.exists() else "CHANGELOG.md missing")
    add_check(
        "changelog_release_items",
        bool(unreleased or current_release_items),
        f"{len(unreleased)} unreleased item(s), {len(current_release_items)} current release item(s)",
    )

    git_clean, git_detail = git_clean_state(root)
    if git_clean is not None:
        add_check("git_clean", git_clean, git_detail)

    ready = all(item["passed"] for item in checks if item["required"])
    return {
        "runtime": RUNTIME_NAME,
        "current_version": current,
        "suggested_version": suggested,
        "mode": mode,
        "touches": touches,
        "validation": validation,
        "checks": checks,
        "ready": ready,
        "publish": {
            "create_tag": False,
            "create_release": False,
            "rule": "check only; publish after full verification passes",
        },
    }


def cmd_release_check(args: argparse.Namespace) -> int:
    root = resolve_root(args.root)
    payload = release_check_payload(
        root,
        mode=args.mode,
        touches=args.touches or [],
        current_version=args.current_version,
    )
    if args.json:
        print(json.dumps(payload, ensure_ascii=True, sort_keys=True, indent=2))
    else:
        print("Release check")
        print(f"Current version: {payload['current_version']}")
        print(f"Suggested version: {payload['suggested_version']}")
        print(f"Ready: {'yes' if payload['ready'] else 'no'}")
        for item in payload["checks"]:
            marker = "PASS" if item["passed"] else "FAIL"
            print(f"{marker} {item['name']}: {item['detail']}")
        print("Publish: no tag or release from this command")
    return 0 if payload["ready"] else 1


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

    preflight = sub.add_parser("preflight", help="resolve route, project identity, and context to load before acting")
    preflight.add_argument("--root", default=".")
    preflight.add_argument("--scan-depth", type=int, default=2)
    preflight.add_argument("--max-chars", type=int, default=12000)
    preflight.add_argument("--objective")
    preflight.add_argument("--json", action="store_true")
    preflight.set_defaults(func=cmd_preflight)

    reload_cmd = sub.add_parser("reload", help="refresh Forge bootstrap contract without broad context loading")
    reload_cmd.add_argument("--root", default=".")
    reload_cmd.add_argument("--scan-depth", type=int, default=2)
    reload_cmd.add_argument("--json", action="store_true")
    reload_cmd.set_defaults(func=cmd_reload)

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
    status.add_argument("--brief", action="store_true")
    status.add_argument("--json", action="store_true")
    status.set_defaults(func=cmd_status)

    snapshot = sub.add_parser("snapshot", help="print machine-readable project state")
    snapshot.add_argument("--root", default=".")
    snapshot.add_argument("--pretty", action="store_true")
    snapshot.set_defaults(func=cmd_snapshot)

    next_cmd = sub.add_parser("next", help="print next recommended action")
    next_cmd.add_argument("--root", default=".")
    next_cmd.set_defaults(func=cmd_next)

    resume = sub.add_parser("resume", help="print structured resume guidance for the current project state")
    resume.add_argument("--root", default=".")
    resume.add_argument("--json", action="store_true")
    resume.set_defaults(func=cmd_resume)

    guide = sub.add_parser("guide", help="print human-friendly guidance for the current workspace")
    guide.add_argument("--root", default=".")
    guide.add_argument("--question")
    guide.add_argument("--max-chars", type=int, default=12000)
    guide.add_argument("--json", action="store_true")
    guide.set_defaults(func=cmd_guide)

    parity = sub.add_parser("parity", help="run internal parity checks against benchmark-shaped fixtures")
    parity_sub = parity.add_subparsers(dest="parity_command", required=True)
    parity_replay = parity_sub.add_parser("replay", help="replay guidance parity fixtures through Guidance Engine")
    parity_replay.add_argument("--fixture")
    parity_replay.add_argument("--max-chars", type=int, default=12000)
    parity_replay.add_argument("--json", action="store_true")
    parity_replay.set_defaults(func=cmd_parity_replay)

    track = sub.add_parser("track", help="inspect and set Forge Method tracks")
    track_sub = track.add_subparsers(dest="track_command", required=True)
    track_list = track_sub.add_parser("list", help="list packaged tracks")
    track_list.add_argument("--json", action="store_true")
    track_list.set_defaults(func=cmd_track_list)
    track_recommend = track_sub.add_parser("recommend", help="recommend tracks for an objective")
    track_recommend.add_argument("--objective")
    track_recommend.add_argument("--limit", type=int, default=5)
    track_recommend.add_argument("--json", action="store_true")
    track_recommend.set_defaults(func=cmd_track_recommend)
    track_set = track_sub.add_parser("set", help="set the current project track")
    track_set.add_argument("--root", default=".")
    track_set.add_argument("--track", required=True, choices=sorted(TRACK_IDS))
    track_set.add_argument("--set-module", action="store_true")
    track_set.add_argument("--next-action")
    track_set.set_defaults(func=cmd_track_set)

    council = sub.add_parser("council", help="run an optional Forge Agent Council")
    council_sub = council.add_subparsers(dest="council_command", required=True)
    council_run = council_sub.add_parser("run", help="show a live council transcript and persist a compact decision")
    council_run.add_argument("--root", default=".")
    council_run.add_argument("--topic")
    council_run.add_argument("--agent", action="append")
    council_run.add_argument("--next-action")
    council_run.add_argument("--eval", action="store_true")
    council_run.set_defaults(func=cmd_council_run)

    correct_course = sub.add_parser("correct-course", help="write a compact correct-course continuation artifact")
    correct_course.add_argument("--root", default=".")
    correct_course.add_argument("--summary", required=True)
    correct_course.add_argument("--impact")
    correct_course.add_argument("--title")
    correct_course.add_argument("--next-action")
    correct_course.add_argument("--eval", action="store_true")
    correct_course.set_defaults(func=cmd_correct_course)

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

    story_export = story_sub.add_parser("export", help="export stories as JSON")
    story_export.add_argument("--root", default=".")
    story_export.add_argument("--status", choices=STORY_STATUSES)
    story_export.add_argument("--out")
    story_export.set_defaults(func=cmd_story_export)

    story_import = story_sub.add_parser("import", help="import stories from JSON")
    story_import.add_argument("--root", default=".")
    story_import.add_argument("--file", required=True)
    story_import.add_argument("--force", action="store_true")
    story_import.set_defaults(func=cmd_story_import)

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

    review = sub.add_parser("review", help="manage durable review findings")
    review_sub = review.add_subparsers(dest="review_command", required=True)
    review_add = review_sub.add_parser("add", help="add a review finding")
    review_add.add_argument("--root", default=".")
    review_add.add_argument("--id")
    review_add.add_argument("--story", required=True)
    review_add.add_argument("--title", required=True)
    review_add.add_argument("--severity", choices=REVIEW_FINDING_SEVERITIES, default="medium")
    review_add.add_argument("--summary", required=True)
    review_add.add_argument("--source")
    review_add.add_argument("--force", action="store_true")
    review_add.set_defaults(func=cmd_review_add)
    review_list = review_sub.add_parser("list", help="list review findings")
    review_list.add_argument("--root", default=".")
    review_list.add_argument("--story")
    review_list.add_argument("--status", choices=REVIEW_FINDING_STATUSES)
    review_list.set_defaults(func=cmd_review_list)
    review_resolve = review_sub.add_parser("resolve", help="resolve a review finding")
    review_resolve.add_argument("--root", default=".")
    review_resolve.add_argument("--id", required=True)
    review_resolve.add_argument("--resolution", required=True)
    review_resolve.add_argument("--evidence")
    review_resolve.set_defaults(func=cmd_review_resolve)
    review_waive = review_sub.add_parser("waive", help="waive a review finding")
    review_waive.add_argument("--root", default=".")
    review_waive.add_argument("--id", required=True)
    review_waive.add_argument("--reason", required=True)
    review_waive.set_defaults(func=cmd_review_waive)

    input_cmd = sub.add_parser("input", help="manage durable human input")
    input_sub = input_cmd.add_subparsers(dest="input_command", required=True)
    input_add = input_sub.add_parser("add", help="add a human input request")
    input_add.add_argument("--root", default=".")
    input_add.add_argument("--id")
    input_add.add_argument("--prompt", required=True)
    input_add.add_argument("--reason")
    input_add.add_argument("--phase")
    input_add.add_argument("--required", action="store_true", default=True)
    input_add.add_argument("--optional", dest="required", action="store_false")
    input_add.add_argument("--force", action="store_true")
    input_add.set_defaults(func=cmd_input_add)
    input_list = input_sub.add_parser("list", help="list human input requests")
    input_list.add_argument("--root", default=".")
    input_list.add_argument("--status", choices=HUMAN_INPUT_STATUSES)
    input_list.set_defaults(func=cmd_input_list)
    input_answer = input_sub.add_parser("answer", help="answer a human input request")
    input_answer.add_argument("--root", default=".")
    input_answer.add_argument("--id", required=True)
    input_answer.add_argument("--answer", required=True)
    input_answer.add_argument("--next-action")
    input_answer.add_argument("--force", action="store_true")
    input_answer.set_defaults(func=cmd_input_answer)
    input_defer = input_sub.add_parser("defer", help="defer a human input request")
    input_defer.add_argument("--root", default=".")
    input_defer.add_argument("--id", required=True)
    input_defer.add_argument("--reason", required=True)
    input_defer.add_argument("--next-action")
    input_defer.set_defaults(func=cmd_input_defer)

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
    module_list.add_argument("--json", action="store_true")
    module_list.set_defaults(func=cmd_module_list)
    module_recommend = module_sub.add_parser("recommend", help="recommend modules for an objective")
    module_recommend.add_argument("--root", default=".")
    module_recommend.add_argument("--objective")
    module_recommend.add_argument("--limit", type=int, default=5)
    module_recommend.add_argument("--json", action="store_true")
    module_recommend.set_defaults(func=cmd_module_recommend)
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

    project = sub.add_parser("project", help="create and list method projects")
    project_sub = project.add_subparsers(dest="project_command", required=True)
    project_list = project_sub.add_parser("list", help="list method projects under a folder")
    project_list.add_argument("--root", default=".")
    project_list.add_argument("--scan-depth", type=int, default=2)
    project_list.set_defaults(func=cmd_project_list)
    project_create = project_sub.add_parser("create", help="create a method project from a module")
    project_create.add_argument("--root", default=".")
    project_create.add_argument("--name", required=True)
    project_create.add_argument("--module", default="software-builder")
    project_create.add_argument("--objective")
    project_create.add_argument("--path")
    project_create.add_argument("--mode", default="creation-runtime")
    project_create.add_argument("--max-chars", type=int, default=8000)
    project_create.add_argument("--brownfield", action="store_true")
    project_create.add_argument("--force", action="store_true")
    project_create.add_argument("--allow-runtime-state", action="store_true")
    project_create.add_argument("--no-project-guidance", action="store_true")
    project_create.set_defaults(func=cmd_project_create)

    agent = sub.add_parser("agent", help="inspect and recommend agent profiles")
    agent_sub = agent.add_subparsers(dest="agent_command", required=True)
    agent_list = agent_sub.add_parser("list", help="list agent profiles")
    agent_list.add_argument("--root", default=".")
    agent_list.set_defaults(func=cmd_agent_list)
    agent_show = agent_sub.add_parser("show", help="show an agent profile")
    agent_show.add_argument("--root", default=".")
    agent_show.add_argument("--id", required=True)
    agent_show.set_defaults(func=cmd_agent_show)
    agent_recommend = agent_sub.add_parser("recommend", help="recommend agent profiles from current state")
    agent_recommend.add_argument("--root", default=".")
    agent_recommend.add_argument("--json", action="store_true")
    agent_recommend.set_defaults(func=cmd_agent_recommend)
    agent_validate = agent_sub.add_parser("validate", help="validate agent profiles")
    agent_validate.add_argument("--root", default=".")
    agent_validate.set_defaults(func=cmd_agent_validate)

    example = sub.add_parser("example", help="create runnable example projects from modules")
    example_sub = example.add_subparsers(dest="example_command", required=True)
    example_list = example_sub.add_parser("list", help="list example modules")
    example_list.add_argument("--root", default=".")
    example_list.set_defaults(func=cmd_example_list)
    example_create = example_sub.add_parser("create", help="seed a runnable example project")
    example_create.add_argument("--root", required=True)
    example_create.add_argument("--module", required=True)
    example_create.add_argument("--project")
    example_create.add_argument("--mode", default="creation-runtime")
    example_create.add_argument("--force", action="store_true")
    example_create.add_argument("--no-project-guidance", action="store_true")
    example_create.add_argument("--max-chars", type=int, default=8000)
    example_create.set_defaults(func=cmd_example_create)

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

    builder = sub.add_parser("builder", help="scaffold and validate local Forge Method extensions")
    builder_sub = builder.add_subparsers(dest="builder_command", required=True)
    builder_scaffold = builder_sub.add_parser("scaffold", help="scaffold a workflow, module, agent, skill, template, or eval")
    builder_scaffold.add_argument("--root", default=".")
    builder_scaffold.add_argument("--kind", required=True, choices=BUILDER_KINDS)
    builder_scaffold.add_argument("--id", required=True)
    builder_scaffold.add_argument("--title")
    builder_scaffold.add_argument("--purpose")
    builder_scaffold.add_argument("--trigger")
    builder_scaffold.add_argument("--phase-span")
    builder_scaffold.add_argument("--workflows")
    builder_scaffold.add_argument("--when")
    builder_scaffold.add_argument("--persona")
    builder_scaffold.add_argument("--council-role")
    builder_scaffold.add_argument("--target")
    builder_scaffold.add_argument("--query")
    builder_scaffold.add_argument("--expected")
    builder_scaffold.add_argument("--eval-kind", choices=EVAL_KINDS, default="artifact-exists")
    builder_scaffold.add_argument("--force", action="store_true")
    builder_scaffold.set_defaults(func=cmd_builder_scaffold)
    builder_validate = builder_sub.add_parser("validate", help="validate generated local method extensions")
    builder_validate.add_argument("--root", default=".")
    builder_validate.set_defaults(func=cmd_builder_validate)

    config = sub.add_parser("config", help="inspect and validate Forge Method customization")
    config_sub = config.add_subparsers(dest="config_command", required=True)
    config_inspect = config_sub.add_parser("inspect", help="print merged team/local configuration")
    config_inspect.add_argument("--root", default=".")
    config_inspect.add_argument("--json", action="store_true")
    config_inspect.set_defaults(func=cmd_config_inspect)
    config_validate = config_sub.add_parser("validate", help="validate team/local configuration")
    config_validate.add_argument("--root", default=".")
    config_validate.set_defaults(func=cmd_config_validate)

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
    artifact_add.add_argument("--eval", action="store_true")
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
    eval_add.add_argument("--kind", choices=EVAL_KINDS, default="workflow-routing")
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
    context_plan = context_sub.add_parser("plan", help="write a machine-readable context load plan")
    context_plan.add_argument("--root", default=".")
    context_plan.add_argument("--out")
    context_plan.add_argument("--max-chars", type=int, default=12000)
    context_plan.add_argument("--json", action="store_true")
    context_plan.set_defaults(func=cmd_context_plan)
    context_health = context_sub.add_parser("health", help="inspect context budget and handoff risk")
    context_health.add_argument("--root", default=".")
    context_health.add_argument("--max-chars", type=int, default=12000)
    context_health.add_argument("--json", action="store_true")
    context_health.set_defaults(func=cmd_context_health)
    context_recover = context_sub.add_parser("recover", help="write a focused recovery brief")
    context_recover.add_argument("--root", default=".")
    context_recover.add_argument("--out")
    context_recover.add_argument("--max-chars", type=int, default=8000)
    context_recover.add_argument("--checkpoints", type=int, default=5)
    context_recover.add_argument("--compact", action="store_true")
    context_recover.set_defaults(func=cmd_context_recover)

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

    release = sub.add_parser("release", help="plan release version and validation")
    release_sub = release.add_subparsers(dest="release_command", required=True)
    release_plan = release_sub.add_parser("plan", help="plan version bump and validation tier without publishing")
    release_plan.add_argument("--root", default=".")
    release_plan.add_argument("--mode", choices=["story", "batch", "hotfix", "breaking"], default="batch")
    release_plan.add_argument("--touches", action="append", choices=["docs", "runtime", "workflow", "state", "install", "package"])
    release_plan.add_argument("--current-version")
    release_plan.add_argument("--json", action="store_true")
    release_plan.set_defaults(func=cmd_release_plan)
    release_check = release_sub.add_parser("check", help="check release readiness without publishing")
    release_check.add_argument("--root", default=".")
    release_check.add_argument("--mode", choices=["story", "batch", "hotfix", "breaking"], default="batch")
    release_check.add_argument("--touches", action="append", choices=["docs", "runtime", "workflow", "state", "install", "package"])
    release_check.add_argument("--current-version")
    release_check.add_argument("--json", action="store_true")
    release_check.set_defaults(func=cmd_release_check)

    handoff = sub.add_parser("handoff", help="write a continuation handoff")
    handoff.add_argument("--root", default=".")
    handoff.add_argument("--summary", required=True)
    handoff.add_argument("--next-action")
    handoff.set_defaults(func=cmd_handoff)

    doctor = sub.add_parser("doctor", help="inspect runtime/project detection and local toolchain readiness")
    doctor.add_argument("--root", default=".")
    doctor.add_argument("--mode", choices=["story", "batch", "hotfix", "breaking"], default="batch")
    doctor.add_argument("--touches", action="append", choices=["docs", "runtime", "workflow", "state", "install", "package"])
    doctor.add_argument("--json", action="store_true")
    doctor.set_defaults(func=cmd_doctor)

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = build_parser()
    args = parser.parse_args(argv)
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
