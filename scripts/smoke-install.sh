#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
installer="$repo_root/install.sh"
installed_runtime="$HOME/.agents/skills/forge-method/scripts/forge_method_runtime.py"
tmp="${TMPDIR:-/tmp}/forge-method-install-smoke"
python_bin="${PYTHON:-python3}"

bash "$installer"

if [[ ! -f "$installed_runtime" ]]; then
  echo "Installed runtime helper not found: $installed_runtime" >&2
  exit 1
fi

rm -rf "$tmp"
mkdir -p "$tmp"

"$python_bin" "$installed_runtime" --help
"$python_bin" "$installed_runtime" module list
"$python_bin" "$installed_runtime" workflow validate
"$python_bin" "$installed_runtime" init --project install-smoke --root "$tmp"
"$python_bin" "$installed_runtime" transition --root "$tmp" --phase 1-discovery --status discovery-ready --workflow discover-intent
"$python_bin" "$installed_runtime" story add --root "$tmp" --id install-story --title "Installed runtime works" --acceptance "installed helper can write durable state"
"$python_bin" "$installed_runtime" status --root "$tmp"
"$python_bin" "$installed_runtime" next --root "$tmp"
"$python_bin" "$installed_runtime" audit --root "$tmp"

echo "Install smoke test passed: $tmp"

