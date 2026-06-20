#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
skills_source="$repo_root/skills"
target_root="$HOME/.agents/skills"
skill_names=("forge-method" "forge-reload" "forge-update")

if [[ ! -d "$skills_source" ]]; then
  echo "Skills source not found: $skills_source" >&2
  exit 1
fi

mkdir -p "$target_root"
target_root_resolved="$(cd "$target_root" && pwd)"
for skill_name in "${skill_names[@]}"; do
  source_dir="$skills_source/$skill_name"
  target_dir="$target_root/$skill_name"
  if [[ ! -d "$source_dir" ]]; then
    echo "Skill source not found: $source_dir" >&2
    exit 1
  fi
  case "$target_root_resolved/$skill_name" in
    "$target_root_resolved"/*) ;;
    *)
      echo "Refusing to install outside skill directory: $target_dir" >&2
      exit 1
      ;;
  esac
  if [[ -e "$target_dir" ]]; then
    chmod -R u+w "$target_dir" 2>/dev/null || true
  fi
  rm -rf "$target_dir"
  cp -R "$source_dir" "$target_dir"
  echo "Installed Codex skill: $target_dir"
done

echo 'Use in Codex: $forge-method'
echo 'Emergency reload: $forge-reload'
echo 'Manual update: $forge-update'
echo "Verify: bash $target_root/forge-method/forge-method.sh --help"
echo "Start: ask Codex to run Forge Method in your project workspace."
