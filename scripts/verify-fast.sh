#!/usr/bin/env bash
set -euo pipefail

python_bin="${PYTHON:-python3}"

skip_unit=0
tests=()
while [[ $# -gt 0 ]]; do
  case "$1" in
    --skip-unit)
      skip_unit=1
      shift
      ;;
    --test)
      tests+=("$2")
      shift 2
      ;;
    --test=*)
      tests+=("${1#--test=}")
      shift
      ;;
    *)
      tests+=("$1")
      shift
      ;;
  esac
done

if [[ "$skip_unit" -eq 0 ]]; then
  if [[ "${#tests[@]}" -gt 0 ]]; then
    "$python_bin" -m unittest "${tests[@]}"
  else
    "$python_bin" -m unittest discover -s tests
  fi
fi
"$python_bin" scripts/verify-onboarding-assets.py
"$python_bin" skills/forge-method/scripts/forge_method_runtime.py workflow validate
"$python_bin" skills/forge-method/scripts/forge_method_runtime.py agent validate

echo "Fast verification checks passed."
