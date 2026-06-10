#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
installer="$repo_root/scripts/install-plugin-local.sh"
tmp="${TMPDIR:-/tmp}/forge-method-plugin-smoke"
plugin_parent="$tmp/plugins"
marketplace_path="$tmp/.agents/plugins/marketplace.json"
plugin_root="$plugin_parent/forge-method-core"

rm -rf "$tmp"
mkdir -p "$tmp"

PLUGIN_PARENT="$plugin_parent" MARKETPLACE_PATH="$marketplace_path" bash "$installer"

test -f "$plugin_root/.codex-plugin/plugin.json"
test -f "$plugin_root/skills/forge-method/SKILL.md"
test -f "$marketplace_path"

MARKETPLACE_PATH="$marketplace_path" python3 - <<'PY'
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

echo "Plugin local smoke passed: $plugin_root"
