#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runtime="$repo_root/skills/forge-method/scripts/forge_method_runtime.py"
tmp="${TMPDIR:-/tmp}/forge-method-smoke"
python_bin="${PYTHON:-python3}"

rm -rf "$tmp"
mkdir -p "$tmp"

"$python_bin" "$runtime" init --project smoke-test --root "$tmp"
"$python_bin" "$runtime" module list --root "$tmp"
"$python_bin" "$runtime" workflow validate
"$python_bin" "$runtime" status --root "$tmp"
"$python_bin" "$runtime" next --root "$tmp"
"$python_bin" "$runtime" transition --root "$tmp" --phase 1-discovery --status discovery-ready --workflow discover-intent
"$python_bin" "$runtime" transition --root "$tmp" --phase 2-specification --status specification-ready --workflow write-spec
"$python_bin" "$runtime" artifact add --root "$tmp" --kind spec --title "Smoke specification" --summary "The smoke project requires durable state, evidence, and ready gate validation."
"$python_bin" "$runtime" transition --root "$tmp" --phase 3-plan --status planning-ready --workflow plan-sprint
"$python_bin" "$runtime" transition --root "$tmp" --phase 4-build-verify --status build-ready --workflow build-story
"$python_bin" "$runtime" story add --root "$tmp" --id story-1 --title "Prove runtime loop" --acceptance "status can be reconstructed from files" --acceptance "done stories require evidence"
"$python_bin" "$runtime" story start --root "$tmp" --id story-1
"$python_bin" "$runtime" story review --root "$tmp" --id story-1
"$python_bin" "$runtime" story done --root "$tmp" --id story-1 --summary "Runtime loop completed in smoke test." --check "smoke-runtime.sh"
"$python_bin" "$runtime" context pack --root "$tmp"
"$python_bin" "$runtime" artifact list --root "$tmp"
"$python_bin" "$runtime" audit --root "$tmp"
"$python_bin" "$runtime" ready --root "$tmp" --summary "Smoke project is ready." --check audit
"$python_bin" "$runtime" status --root "$tmp"

echo "Smoke test passed: $tmp"

