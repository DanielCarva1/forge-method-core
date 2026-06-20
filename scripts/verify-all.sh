#!/usr/bin/env bash
set -euo pipefail

python_bin="${PYTHON:-python3}"
workers="${FORGE_TEST_WORKERS:-4}"
timeout_seconds="${FORGE_TEST_TIMEOUT_SECONDS:-120}"
debug="${FORGE_TEST_DEBUG:-0}"
report_path="${FORGE_TEST_REPORT:-}"
junit_path="${FORGE_TEST_JUNIT:-}"
no_report=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --debug)
      debug=1
      shift
      ;;
    --no-report)
      no_report=1
      shift
      ;;
    --report)
      report_path="$2"
      shift 2
      ;;
    --report=*)
      report_path="${1#--report=}"
      shift
      ;;
    --junit)
      junit_path="$2"
      shift 2
      ;;
    --junit=*)
      junit_path="${1#--junit=}"
      shift
      ;;
    --workers)
      workers="$2"
      shift 2
      ;;
    --workers=*)
      workers="${1#--workers=}"
      shift
      ;;
    --timeout)
      timeout_seconds="$2"
      shift 2
      ;;
    --timeout=*)
      timeout_seconds="${1#--timeout=}"
      shift
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 2
      ;;
  esac
done

if [[ "$debug" != "0" && "${FORGE_TEST_WORKERS:-}" == "" ]]; then
  workers=1
fi
if [[ "$no_report" -eq 0 && "$report_path" == "" ]]; then
  report_path=".forge-method/test-runs/verify-all-$(date -u +%Y%m%d-%H%M%SZ).json"
fi

runner_args=(scripts/test-runner.py --workers "$workers" --timeout "$timeout_seconds")
if [[ "$debug" != "0" ]]; then
  runner_args+=(--debug)
fi
if [[ "$report_path" != "" ]]; then
  runner_args+=(--report "$report_path")
fi
if [[ "$junit_path" != "" ]]; then
  runner_args+=(--junit "$junit_path")
fi

"$python_bin" "${runner_args[@]}"
"$python_bin" scripts/verify-onboarding-assets.py
bash scripts/smoke-runtime.sh
bash scripts/smoke-install.sh
bash scripts/smoke-plugin-local.sh
bash scripts/smoke-fixtures.sh
"$python_bin" skills/forge-method/scripts/forge_method_runtime.py workflow validate

echo "All verification checks passed."
