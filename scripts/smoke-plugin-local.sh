#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
installer="$repo_root/scripts/install-plugin-local.sh"
tmp="${TMPDIR:-/tmp}/forge-method-plugin-smoke"
plugin_parent="$tmp/plugins"
marketplace_path="$tmp/.agents/plugins/marketplace.json"
plugin_root="$plugin_parent/forge-method-core"

resolve_python() {
  if [[ -n "${PYTHON:-}" ]]; then
    if "$PYTHON" -c 'import sys' >/dev/null 2>&1; then
      printf '%s\n' "$PYTHON"
      return 0
    fi
    echo "PYTHON is set but is not executable: $PYTHON" >&2
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

rm -rf "$tmp"
mkdir -p "$tmp"

output="$(PYTHON="$python_cmd" PLUGIN_PARENT="$plugin_parent" MARKETPLACE_PATH="$marketplace_path" bash "$installer")"
printf '%s\n' "$output"

test -f "$plugin_root/.codex-plugin/plugin.json"
test -f "$plugin_root/skills/forge-method/SKILL.md"
test -f "$marketplace_path"

MARKETPLACE_PATH="$marketplace_path" "$python_cmd" - <<'PY'
from __future__ import annotations

import json
import os
from pathlib import Path

payload = json.loads(Path(os.environ["MARKETPLACE_PATH"]).read_text(encoding="utf-8"))
matches = [
    plugin for plugin in payload.get("plugins", [])
    if isinstance(plugin, dict) and plugin.get("name") == "forge-method-core"
]
if not matches:
    raise SystemExit("Marketplace entry missing: forge-method-core")
path = matches[0].get("source", {}).get("path")
if path != "./plugins/forge-method-core":
    raise SystemExit(f"Unexpected marketplace source path: {path}")
PY

case "$output" in
  *"codex plugin marketplace add $tmp"*) ;;
  *) echo "Non-default marketplace registration guidance did not point at marketplace root." >&2; exit 1 ;;
esac

case "$output" in
  *"codex://plugins/forge-method-core?marketplacePath="*) ;;
  *) echo "Plugin local installer did not print a Codex plugin deeplink." >&2; exit 1 ;;
esac

case "$output" in
  *"codex://plugins/forge-method-core?marketplacePath="*"&mode=share"*) ;;
  *) echo "Plugin local installer did not print a Codex plugin share deeplink." >&2; exit 1 ;;
esac

echo "Plugin local smoke passed: $plugin_root"
