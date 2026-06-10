#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source_dir="$repo_root/skills/forge-method"
target_root="$HOME/.agents/skills"
target_dir="$target_root/forge-method"

if [[ ! -d "$source_dir" ]]; then
  echo "Skill source not found: $source_dir" >&2
  exit 1
fi

mkdir -p "$target_root"
rm -rf "$target_dir"
cp -R "$source_dir" "$target_dir"

echo "Installed Codex skill: $target_dir"
echo 'Use in Codex: $forge-method'

