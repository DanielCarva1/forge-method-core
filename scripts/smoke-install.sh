#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
installer="$repo_root/install.sh"
installed_runtime="$HOME/.agents/skills/forge-method/scripts/forge_method_runtime.py"
tmp="${TMPDIR:-/tmp}/forge-method-install-smoke"
example_tmp="${TMPDIR:-/tmp}/forge-method-install-example-smoke"
python_bin="${PYTHON:-python3}"

bash "$installer"

if [[ ! -f "$installed_runtime" ]]; then
  echo "Installed runtime helper not found: $installed_runtime" >&2
  exit 1
fi

rm -rf "$tmp"
rm -rf "$example_tmp"
mkdir -p "$tmp"

"$python_bin" "$installed_runtime" --help
"$python_bin" "$installed_runtime" module list
"$python_bin" "$installed_runtime" agent list
"$python_bin" "$installed_runtime" agent validate
"$python_bin" "$installed_runtime" example list
"$python_bin" "$installed_runtime" example create --root "$example_tmp" --module software-builder
"$python_bin" "$installed_runtime" gate --root "$example_tmp" --require-evals
"$python_bin" "$installed_runtime" workflow validate
"$python_bin" "$installed_runtime" start --root "$tmp"
"$python_bin" "$installed_runtime" init --project install-smoke --root "$tmp"
"$python_bin" "$installed_runtime" start --root "$tmp"
"$python_bin" "$installed_runtime" snapshot --root "$tmp"
"$python_bin" "$installed_runtime" agent recommend --root "$tmp"
"$python_bin" "$installed_runtime" workflow create --root "$tmp" --id install-flow --title "Install Flow" --trigger "installed runtime available" --input "installed runtime" --step "validate installed runtime" --output "install proof" --done "install proof exists" --blocked "runtime missing" --handoff "preserve install result" --eval-query "prove install flow"
"$python_bin" "$installed_runtime" eval run --root "$tmp"
"$python_bin" "$installed_runtime" checkpoint --root "$tmp" --title "Install checkpoint" --summary "Installed runtime can persist checkpoint memory." --check "install eval passed" --next-action "continue install smoke"
"$python_bin" "$installed_runtime" context recover --root "$tmp" --max-chars 1200
"$python_bin" "$installed_runtime" transition --root "$tmp" --phase 1-discovery --status discovery-ready --workflow discover-intent
"$python_bin" "$installed_runtime" input add --root "$tmp" --id install-audience --prompt "Who is the install smoke audience?" --reason "Install smoke needs durable input coverage."
"$python_bin" "$installed_runtime" input answer --root "$tmp" --id install-audience --answer "Install smoke users" --next-action "continue install smoke"
"$python_bin" "$installed_runtime" story add --root "$tmp" --id install-story --title "Installed runtime works" --acceptance "installed helper can write durable state"
"$python_bin" "$installed_runtime" artifact verify --root "$tmp"
"$python_bin" "$installed_runtime" gate --root "$tmp" --require-evals --summary "Installed runtime quality gate passed."
"$python_bin" "$installed_runtime" status --root "$tmp"
"$python_bin" "$installed_runtime" next --root "$tmp"
"$python_bin" "$installed_runtime" audit --root "$tmp"

echo "Install smoke test passed: $tmp"
