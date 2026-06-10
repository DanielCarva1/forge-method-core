#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
runtime="$repo_root/skills/forge-method/scripts/forge_method_runtime.py"
tmp="${TMPDIR:-/tmp}/forge-method-smoke"
python_bin="${PYTHON:-python3}"

rm -rf "$tmp"
mkdir -p "$tmp"

"$python_bin" "$runtime" init --project smoke-test --root "$tmp"
"$python_bin" "$runtime" start --root "$tmp"
"$python_bin" "$runtime" module list --root "$tmp"
"$python_bin" "$runtime" workflow validate
"$python_bin" "$runtime" workflow create --root "$tmp" --id smoke-flow --title "Smoke Flow" --trigger "state.status == smoke" --input "smoke input" --step "perform smoke step" --output "smoke output" --done "smoke output exists" --blocked "smoke input missing" --handoff "preserve smoke result" --eval-query "run smoke flow"
"$python_bin" "$runtime" module create --root "$tmp" --id smoke-module --title "Smoke Module" --purpose "Exercise project module creation." --phase-span "1-discovery" --workflow smoke-flow
"$python_bin" "$runtime" workflow validate --root "$tmp"
"$python_bin" "$runtime" eval run --root "$tmp"
"$python_bin" "$runtime" checkpoint --root "$tmp" --title "Smoke checkpoint" --summary "Runtime smoke reached generated workflow and eval checks." --decision "Checkpoint memory is available." --check "eval run passed" --touched ".forge-method/workflows/workflow-smoke-flow.md" --next-action "continue smoke runtime verification"
"$python_bin" "$runtime" status --root "$tmp"
"$python_bin" "$runtime" next --root "$tmp"
"$python_bin" "$runtime" transition --root "$tmp" --phase 1-discovery --status discovery-ready --workflow discover-intent
"$python_bin" "$runtime" transition --root "$tmp" --phase 2-specification --status specification-ready --workflow write-spec
"$python_bin" "$runtime" artifact add --root "$tmp" --kind spec --title "Smoke specification" --summary "The smoke project requires durable state, evidence, and ready gate validation." --path ".forge-method/artifacts/smoke-spec.md"
"$python_bin" "$runtime" transition --root "$tmp" --phase 3-plan --status planning-ready --workflow plan-sprint
"$python_bin" "$runtime" transition --root "$tmp" --phase 4-build-verify --status build-ready --workflow build-story
"$python_bin" "$runtime" story add --root "$tmp" --id story-1 --title "Prove runtime loop" --acceptance "status can be reconstructed from files" --acceptance "done stories require evidence"
"$python_bin" "$runtime" artifact link-story --root "$tmp" --path ".forge-method/artifacts/smoke-spec.md" --story story-1
"$python_bin" "$runtime" story start --root "$tmp" --id story-1
"$python_bin" "$runtime" story review --root "$tmp" --id story-1
"$python_bin" "$runtime" story done --root "$tmp" --id story-1 --summary "Runtime loop completed in smoke test." --check "smoke-runtime.sh"
"$python_bin" "$runtime" context pack --root "$tmp" --max-chars 1200
"$python_bin" "$runtime" artifact list --root "$tmp"
"$python_bin" "$runtime" audit --root "$tmp"
"$python_bin" "$runtime" ready --root "$tmp" --summary "Smoke project is ready." --check audit
"$python_bin" "$runtime" status --root "$tmp"

echo "Smoke test passed: $tmp"
