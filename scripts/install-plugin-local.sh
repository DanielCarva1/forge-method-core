#!/usr/bin/env bash
set -euo pipefail

plugin_name="forge-method-core"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
marketplace_path="${MARKETPLACE_PATH:-$HOME/.agents/plugins/marketplace.json}"

marketplace_root="$(MARKETPLACE_PATH="$marketplace_path" python3 - <<'PY'
from pathlib import Path
import os

path = Path(os.environ["MARKETPLACE_PATH"]).expanduser().resolve(strict=False)
if (
    path.name == "marketplace.json"
    and path.parent.name == "plugins"
    and path.parent.parent.name == ".agents"
):
    print(path.parent.parent.parent)
else:
    print(path.parent)
PY
)"

plugin_parent="${PLUGIN_PARENT:-$marketplace_root/plugins}"
target="$plugin_parent/$plugin_name"

mkdir -p "$plugin_parent"
plugin_parent_real="$(cd "$plugin_parent" && pwd)"
target_parent_real="$(cd "$(dirname "$target")" && pwd)"
if [[ "$target_parent_real" != "$plugin_parent_real" ]]; then
  echo "Refusing to write outside plugin parent: $target" >&2
  exit 1
fi

mkdir -p "$marketplace_root"
marketplace_root_real="$(cd "$marketplace_root" && pwd)"
source_path="$(MARKETPLACE_ROOT="$marketplace_root_real" TARGET="$target" python3 - <<'PY'
from pathlib import Path
import os
import sys

root = Path(os.environ["MARKETPLACE_ROOT"]).resolve(strict=False)
target = Path(os.environ["TARGET"]).expanduser().resolve(strict=False)
try:
    relative = target.relative_to(root)
except ValueError:
    print(f"Refusing to place plugin outside marketplace root: {target}", file=sys.stderr)
    raise SystemExit(1)
print("./" + relative.as_posix())
PY
)"

if [[ -e "$target" ]]; then
  chmod -R u+w "$target" 2>/dev/null || true
  rm -rf "$target"
fi
mkdir -p "$target"

entries=(
  ".codex-plugin"
  "assets"
  "docs"
  "examples"
  "scripts"
  "skills"
  "templates"
  "AGENTS.md"
  "CHANGELOG.md"
  "CONTEXT.md"
  "install.ps1"
  "install.sh"
  "README.md"
  "VERSION"
)

for entry in "${entries[@]}"; do
  if [[ -e "$repo_root/$entry" ]]; then
    cp -R "$repo_root/$entry" "$target/"
  fi
done

MARKETPLACE_PATH="$marketplace_path" SOURCE_PATH="$source_path" python3 - <<'PY'
from __future__ import annotations

import json
import os
from pathlib import Path

plugin_name = "forge-method-core"
marketplace_path = Path(os.environ["MARKETPLACE_PATH"]).expanduser()
marketplace_path.parent.mkdir(parents=True, exist_ok=True)
if marketplace_path.exists():
    payload = json.loads(marketplace_path.read_text(encoding="utf-8"))
else:
    payload = {
        "name": "personal",
        "interface": {"displayName": "Personal"},
        "plugins": [],
    }
payload.setdefault("name", "personal")
payload.setdefault("interface", {"displayName": "Personal"})
payload.setdefault("plugins", [])
entry = {
    "name": plugin_name,
    "source": {"source": "local", "path": os.environ["SOURCE_PATH"]},
    "policy": {"installation": "AVAILABLE", "authentication": "ON_INSTALL"},
    "category": "Productivity",
}
payload["plugins"] = [
    plugin for plugin in payload["plugins"]
    if not (isinstance(plugin, dict) and plugin.get("name") == plugin_name)
]
payload["plugins"].append(entry)
marketplace_path.write_text(json.dumps(payload, indent=2) + "\n", encoding="utf-8")
PY

marketplace_name="$(MARKETPLACE_PATH="$marketplace_path" python3 - <<'PY'
import json
import os
from pathlib import Path
payload = json.loads(Path(os.environ["MARKETPLACE_PATH"]).read_text(encoding="utf-8"))
print(payload.get("name", "personal"))
PY
)"

echo "Installed local Codex plugin source: $target"
echo "Updated marketplace: $marketplace_path"
default_marketplace="$HOME/.agents/plugins/marketplace.json"
if [[ "$(python3 -c 'import os,sys; print(os.path.abspath(os.path.expanduser(sys.argv[1])))' "$marketplace_path")" == "$(python3 -c 'import os,sys; print(os.path.abspath(os.path.expanduser(sys.argv[1])))' "$default_marketplace")" ]]; then
  echo "Codex discovers the personal marketplace automatically. Open Codex plugins and select Forge Method Core."
else
  echo "Register marketplace: codex plugin marketplace add $marketplace_root"
fi
