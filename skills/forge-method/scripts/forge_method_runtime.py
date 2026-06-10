#!/usr/bin/env python3
"""Minimal file-backed runtime helper for Forge Method."""

from __future__ import annotations

import argparse
import datetime as _dt
from pathlib import Path
from typing import Dict


STATE_DIR = ".forge-method"
STATE_FILE = "state.yaml"
SPRINT_FILE = "sprint.yaml"


DEFAULT_STATE = {
    "runtime": "forge-method",
    "runtime_version": "0.1.0",
    "project": "",
    "mode": "software-builder",
    "phase": "0-route",
    "status": "initialized",
    "active_workflow": "start-runtime",
    "human_input_required": "false",
    "next_action": "confirm project intent and move to discovery",
}


NEXT_BY_PHASE = {
    "0-route": "resolve project route and confirm whether this is a new or existing project",
    "1-discovery": "run a concise discovery interview and write intent brief",
    "2-specification": "convert intent into requirements, acceptance criteria, and constraints",
    "3-plan": "create architecture notes, task graph, sprint plan, and validation plan",
    "4-build-verify": "select next ready story, implement, validate, review, and write evidence",
    "5-ready-operate": "prepare usage notes, release evidence, support status, and future backlog",
    "6-evolve": "collect feedback and start the next version cycle",
}


def now_iso() -> str:
    return _dt.datetime.now(_dt.timezone.utc).replace(microsecond=0).isoformat()


def find_state_root(start: Path) -> Path | None:
    current = start.resolve()
    for candidate in [current, *current.parents]:
        if (candidate / STATE_DIR / STATE_FILE).exists():
            return candidate
    return None


def parse_simple_yaml(path: Path) -> Dict[str, str]:
    values: Dict[str, str] = {}
    if not path.exists():
        return values
    for raw_line in path.read_text(encoding="utf-8").splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or ":" not in line:
            continue
        key, value = line.split(":", 1)
        values[key.strip()] = value.strip().strip('"')
    return values


def write_simple_yaml(path: Path, values: Dict[str, str]) -> None:
    lines = ["# Forge Method state", f"updated_at: {now_iso()}"]
    for key, value in values.items():
        safe = str(value).replace("\n", " ").strip()
        lines.append(f'{key}: "{safe}"')
    path.write_text("\n".join(lines) + "\n", encoding="utf-8")


def ensure_runtime_dirs(root: Path) -> Path:
    method = root / STATE_DIR
    for relative in ["evidence", "handoffs", "ephemeral", "workflows"]:
        (method / relative).mkdir(parents=True, exist_ok=True)
    return method


def cmd_init(args: argparse.Namespace) -> int:
    root = Path(args.root).resolve()
    method = ensure_runtime_dirs(root)
    state_path = method / STATE_FILE
    sprint_path = method / SPRINT_FILE

    state = dict(DEFAULT_STATE)
    state["project"] = args.project
    state["status"] = "route-ready"

    if state_path.exists() and not args.force:
        print(f"State already exists: {state_path}")
        print("Use --force to replace it.")
        return 2

    write_simple_yaml(state_path, state)
    if not sprint_path.exists() or args.force:
        sprint_path.write_text(
            "# Forge Method sprint state\n"
            f"updated_at: {now_iso()}\n"
            "active_story: \"\"\n"
            "stories: []\n",
            encoding="utf-8",
        )

    print(f"Initialized Forge Method project: {args.project}")
    print(f"State: {state_path}")
    print(f"Next: {state['next_action']}")
    return 0


def load_state_or_fail(root: str) -> tuple[Path, Dict[str, str]]:
    start = Path(root).resolve()
    state_root = find_state_root(start)
    if state_root is None:
        raise SystemExit("No .forge-method/state.yaml found. Run init first.")
    state_path = state_root / STATE_DIR / STATE_FILE
    return state_root, parse_simple_yaml(state_path)


def cmd_status(args: argparse.Namespace) -> int:
    state_root, state = load_state_or_fail(args.root)
    print(f"Workspace: {state_root}")
    print(f"Project: {state.get('project', '<unknown>')}")
    print(f"Phase: {state.get('phase', '<unknown>')}")
    print(f"Status: {state.get('status', '<unknown>')}")
    print(f"Workflow: {state.get('active_workflow', '<none>')}")
    print(f"Human input required: {state.get('human_input_required', 'unknown')}")
    print(f"Next: {state.get('next_action', '<unknown>')}")
    return 0


def cmd_next(args: argparse.Namespace) -> int:
    _, state = load_state_or_fail(args.root)
    phase = state.get("phase", "0-route")
    print(NEXT_BY_PHASE.get(phase, "inspect state and choose a valid workflow"))
    return 0


def cmd_transition(args: argparse.Namespace) -> int:
    state_root, state = load_state_or_fail(args.root)
    if args.phase:
        state["phase"] = args.phase
    if args.status:
        state["status"] = args.status
    if args.workflow:
        state["active_workflow"] = args.workflow
    if args.next_action:
        state["next_action"] = args.next_action
    elif args.phase:
        state["next_action"] = NEXT_BY_PHASE.get(args.phase, state.get("next_action", "inspect state"))
    if args.human_input_required is not None:
        state["human_input_required"] = str(args.human_input_required).lower()

    write_simple_yaml(state_root / STATE_DIR / STATE_FILE, state)
    print("Transition written.")
    print(f"Phase: {state.get('phase')}")
    print(f"Status: {state.get('status')}")
    print(f"Next: {state.get('next_action')}")
    return 0


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(description="Forge Method helper")
    sub = parser.add_subparsers(dest="command", required=True)

    init = sub.add_parser("init", help="initialize .forge-method state")
    init.add_argument("--project", required=True)
    init.add_argument("--root", default=".")
    init.add_argument("--force", action="store_true")
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
    transition.set_defaults(func=cmd_transition)

    return parser


def main() -> int:
    parser = build_parser()
    args = parser.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())

