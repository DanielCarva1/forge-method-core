#!/usr/bin/env bash
set -euo pipefail

python_bin="${PYTHON:-python3}"

"$python_bin" -m unittest discover -s tests
"$python_bin" scripts/verify-onboarding-assets.py
bash scripts/smoke-runtime.sh
bash scripts/smoke-install.sh
bash scripts/smoke-plugin-local.sh
bash scripts/smoke-fixtures.sh
"$python_bin" skills/forge-method/scripts/forge_method_runtime.py workflow validate

echo "All verification checks passed."
