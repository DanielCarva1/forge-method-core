#!/usr/bin/env bash
set -euo pipefail

repo_url="${REPO_URL:-https://github.com/DanielCarva1/forge-method-core.git}"
ref="${REF:-}"
work_root="${WORK_ROOT:-}"
python_cmd="${PYTHON:-}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
expected_version="${EXPECTED_VERSION:-}"

if [[ -z "$expected_version" && -f "$repo_root/VERSION" ]]; then
  expected_version="$(tr -d '[:space:]' < "$repo_root/VERSION")"
fi

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

python_cmd="$(resolve_python)"

if [[ -z "$work_root" ]]; then
  work_root="$(mktemp -d "${TMPDIR:-/tmp}/forge-method-plugin-clone-smoke.XXXXXX")"
else
  rm -rf "$work_root"
  mkdir -p "$work_root"
fi

clone_root="$work_root/repo"
plugin_parent="$work_root/plugins"
marketplace_path="$work_root/.agents/plugins/marketplace.json"
plugin_root="$plugin_parent/forge-method-core"
project_parent="$work_root/projects"
generated_project="$project_parent/clone-smoke"

mkdir -p "$project_parent"

clone_args=(clone --depth 1)
if [[ -n "$ref" ]]; then
  clone_args+=(--branch "$ref")
fi
clone_args+=("$repo_url" "$clone_root")
git "${clone_args[@]}"

installer="$clone_root/scripts/install-plugin-local.sh"
if [[ ! -f "$installer" ]]; then
  echo "Plugin installer missing from cloned source: $installer" >&2
  exit 1
fi

PYTHON="$python_cmd" PLUGIN_PARENT="$plugin_parent" MARKETPLACE_PATH="$marketplace_path" bash "$installer"

manifest_path="$plugin_root/.codex-plugin/plugin.json"
skill_path="$plugin_root/skills/forge-method/SKILL.md"
runtime_path="$plugin_root/skills/forge-method/scripts/forge_method_runtime.py"

test -f "$manifest_path"
test -f "$skill_path"
test -f "$runtime_path"
test -f "$marketplace_path"

MANIFEST_PATH="$manifest_path" MARKETPLACE_PATH="$marketplace_path" EXPECTED_VERSION="$expected_version" "$python_cmd" - <<'PY'
from __future__ import annotations

import json
import os
from pathlib import Path

manifest = json.loads(Path(os.environ["MANIFEST_PATH"]).read_text(encoding="utf-8"))
if manifest.get("name") != "forge-method-core":
    raise SystemExit(f"Unexpected plugin name: {manifest.get('name')}")
expected = os.environ.get("EXPECTED_VERSION", "")
if expected and manifest.get("version") != expected:
    raise SystemExit(f"Unexpected plugin version: {manifest.get('version')}, expected {expected}")

marketplace = json.loads(Path(os.environ["MARKETPLACE_PATH"]).read_text(encoding="utf-8"))
matches = [
    plugin for plugin in marketplace.get("plugins", [])
    if isinstance(plugin, dict) and plugin.get("name") == "forge-method-core"
]
if not matches:
    raise SystemExit("Marketplace entry missing: forge-method-core")
path = matches[0].get("source", {}).get("path")
if path != "./plugins/forge-method-core":
    raise SystemExit(f"Unexpected marketplace source path: {path}")
PY

preflight="$("$python_cmd" "$runtime_path" preflight --root "$project_parent")"
printf '%s\n' "$preflight"
case "$preflight" in
  *"Decision options:"*) ;;
  *) echo "preflight did not print decision options" >&2; exit 1 ;;
esac

"$python_cmd" "$runtime_path" project create --root "$project_parent" --name "Clone Smoke" --module software-builder --objective "Verify cloned plugin installation."
"$python_cmd" "$runtime_path" preflight --root "$project_parent"
"$python_cmd" "$runtime_path" gate --root "$generated_project" --require-evals

echo "Plugin clone install smoke passed: $plugin_root"
if [[ -n "$ref" ]]; then
  echo "Ref: $ref"
fi
if [[ -n "$expected_version" ]]; then
  echo "Version: $expected_version"
fi
