#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
runtime="$script_dir/scripts/forge_method_runtime.py"

if [[ -n "${PYTHON:-}" ]]; then
  python_bin="$PYTHON"
elif command -v python3 >/dev/null 2>&1; then
  python_bin="$(command -v python3)"
elif command -v python >/dev/null 2>&1; then
  python_bin="$(command -v python)"
elif [[ -x "$HOME/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python" ]]; then
  python_bin="$HOME/.cache/codex-runtimes/codex-primary-runtime/dependencies/python/bin/python"
else
  echo "Python not found. Set PYTHON to a Python executable." >&2
  exit 1
fi

updater="$script_dir/scripts/forge_method_updater.py"
if [[ -f "$updater" ]]; then
  "$python_bin" "$updater" --skill-dir "$script_dir" -- "$@" || true
fi

exec "$python_bin" "$runtime" "$@"
