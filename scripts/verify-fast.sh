#!/usr/bin/env bash
set -euo pipefail

python_bin="${PYTHON:-python3}"

"$python_bin" -m unittest discover -s tests
"$python_bin" skills/forge-method/scripts/forge_method_runtime.py workflow validate
"$python_bin" skills/forge-method/scripts/forge_method_runtime.py agent validate

echo "Fast verification checks passed."
