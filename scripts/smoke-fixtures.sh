#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runtime="$repo_root/skills/forge-method/scripts/forge_method_runtime.py"
python_cmd="${PYTHON:-}"
work_root="${WORK_ROOT:-}"

resolve_python() {
  if [[ -n "$python_cmd" ]]; then
    if "$python_cmd" -c 'import sys' >/dev/null 2>&1; then
      printf '%s\n' "$python_cmd"
      return 0
    fi
    echo "PYTHON is set but is not executable: $python_cmd" >&2
    return 1
  fi
  local candidate
  for candidate in python3 python py; do
    if command -v "$candidate" >/dev/null 2>&1 && "$candidate" -c 'import sys' >/dev/null 2>&1; then
      printf '%s\n' "$candidate"
      return 0
    fi
  done
  echo "Python not found. Set PYTHON to a Python executable." >&2
  return 1
}

assert_snapshot_module() {
  local project_root="$1"
  local expected_module="$2"
  local snapshot
  snapshot="$("$python_cmd" "$runtime" snapshot --root "$project_root")"
  SNAPSHOT="$snapshot" EXPECTED_MODULE="$expected_module" "$python_cmd" - <<'PY'
from __future__ import annotations

import json
import os

payload = json.loads(os.environ["SNAPSHOT"])
expected = os.environ["EXPECTED_MODULE"]
actual = payload.get("state", {}).get("module")
if actual != expected:
    raise SystemExit(f"Expected module {expected}, got {actual}")
if not payload.get("quality", {}).get("audit", {}).get("passed"):
    raise SystemExit("Audit did not pass")
PY
}

assert_recommendation() {
  local expected_module="$1"
  local objective="$2"
  local payload
  payload="$("$python_cmd" "$runtime" module recommend --objective "$objective" --json)"
  RECOMMENDATION="$payload" EXPECTED_MODULE="$expected_module" "$python_cmd" - <<'PY'
from __future__ import annotations

import json
import os

payload = json.loads(os.environ["RECOMMENDATION"])
expected = os.environ["EXPECTED_MODULE"]
actual = payload["recommended"][0]["id"]
if actual != expected:
    raise SystemExit(f"Expected recommendation {expected}, got {actual}")
PY
  echo "Recommendation passed: $expected_module"
}

python_cmd="$(resolve_python)"

if [[ -z "$work_root" ]]; then
  work_root="$(mktemp -d "${TMPDIR:-/tmp}/forge-method-fixture-smoke.XXXXXX")"
else
  rm -rf "$work_root"
  mkdir -p "$work_root"
fi

module_json="$("$python_cmd" "$runtime" module list --json)"
mapfile -t module_rows < <(MODULE_JSON="$module_json" "$python_cmd" - <<'PY'
from __future__ import annotations

import json
import os

modules = json.loads(os.environ["MODULE_JSON"])["modules"]
if len(modules) < 7:
    raise SystemExit(f"Expected packaged module matrix, found {len(modules)} module(s)")
for module in modules:
    print("\t".join([module["id"], module["title"], module["purpose"]]))
PY
)

examples_root="$work_root/examples"
projects_root="$work_root/projects"
mkdir -p "$examples_root" "$projects_root"

for row in "${module_rows[@]}"; do
  IFS=$'\t' read -r module_id title purpose <<< "$row"

  example_root="$examples_root/$module_id"
  "$python_cmd" "$runtime" example create --root "$example_root" --module "$module_id"
  "$python_cmd" "$runtime" gate --root "$example_root" --require-evals
  "$python_cmd" "$runtime" context recover --root "$example_root" --compact --max-chars 1600
  assert_snapshot_module "$example_root" "$module_id"

  project_path="$module_id-project"
  project_root="$projects_root/$project_path"
  "$python_cmd" "$runtime" project create --root "$projects_root" --path "$project_path" --name "$title Fixture" --module "$module_id" --objective "Fixture coverage for $purpose"
  "$python_cmd" "$runtime" gate --root "$project_root" --require-evals
  "$python_cmd" "$runtime" context recover --root "$project_root" --compact --max-chars 1600
  assert_snapshot_module "$project_root" "$module_id"

  echo "Fixture passed: $module_id"
done

preflight="$("$python_cmd" "$runtime" preflight --root "$projects_root")"
printf '%s\n' "$preflight"
case "$preflight" in
  *"Route: workspace-with-projects"*"Decision options:"*) ;;
  *) echo "Fixture parent preflight did not expose project decision options" >&2; exit 1 ;;
esac

assert_recommendation "core-runtime" "recover context route durable state across sessions"
assert_recommendation "software-builder" "build a web API and software product"
assert_recommendation "creative-studio" "design a brand campaign and creative direction"
assert_recommendation "game-studio" "build a game prototype"
assert_recommendation "runtime-builder" "create a new workflow module and agent profile"
assert_recommendation "test-architect" "define validation risk checks and test strategy"
assert_recommendation "launch-ops" "prepare launch release operations"

echo "Fixture smoke passed: $work_root"
