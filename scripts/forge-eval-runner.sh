#!/usr/bin/env bash
set -euo pipefail

ROOT="."
MODE="local"
COMMAND=""
TIMEOUT_SECONDS="600"
ALLOW_DOCKER="false"
DRY_RUN="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root) ROOT="$2"; shift 2 ;;
    --mode) MODE="$2"; shift 2 ;;
    --command) COMMAND="$2"; shift 2 ;;
    --timeout-seconds) TIMEOUT_SECONDS="$2"; shift 2 ;;
    --allow-docker) ALLOW_DOCKER="true"; shift ;;
    --dry-run) DRY_RUN="true"; shift ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ -z "$COMMAND" ]]; then
  echo "Command is required. Pass --command '<eval command>'." >&2
  exit 2
fi

normalize_root() {
  local value="$1"
  if command -v cygpath >/dev/null 2>&1 && [[ "$value" =~ ^[A-Za-z]:\\ ]]; then
    cygpath -u "$value"
  else
    printf '%s' "$value"
  fi
}

ROOT="$(cd "$(normalize_root "$ROOT")" && pwd)"

json_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  value="${value//$'\r'/\\r}"
  value="${value//$'\t'/\\t}"
  printf '%s' "$value"
}

if [[ "$DRY_RUN" == "true" ]]; then
  printf '{"root":"%s","mode":"%s","command":"%s","timeout_seconds":%s,"dry_run":true,"allow_docker":%s}\n' "$(json_escape "$ROOT")" "$(json_escape "$MODE")" "$(json_escape "$COMMAND")" "$TIMEOUT_SECONDS" "$ALLOW_DOCKER"
  exit 0
fi

if [[ "$MODE" == "docker" && "$ALLOW_DOCKER" != "true" ]]; then
  echo "Docker mode is opt-in. Re-run with --allow-docker after recording the isolation contract." >&2
  exit 2
fi

cd "$ROOT"

if [[ "$MODE" == "local" ]]; then
  timeout "$TIMEOUT_SECONDS" bash -lc "$COMMAND"
elif [[ "$MODE" == "docker" ]]; then
  docker run --rm -v "$ROOT:/workspace" -w /workspace bash:latest bash -lc "$COMMAND"
else
  echo "Mode must be local or docker." >&2
  exit 2
fi
