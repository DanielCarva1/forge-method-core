#!/usr/bin/env bash
set -euo pipefail

ROOT="."
EVENT=""
PAYLOAD="{}"
TIMEOUT_SECONDS="120"
RUN="false"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --root) ROOT="$2"; shift 2 ;;
    --event) EVENT="$2"; shift 2 ;;
    --payload) PAYLOAD="$2"; shift 2 ;;
    --timeout-seconds) TIMEOUT_SECONDS="$2"; shift 2 ;;
    --run) RUN="true"; shift ;;
    *) echo "Unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ -z "$EVENT" ]]; then
  echo "--event is required." >&2
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
HOOK_PATH="$ROOT/.forge-method/hooks/$EVENT.sh"

json_escape() {
  local value="$1"
  value="${value//\\/\\\\}"
  value="${value//\"/\\\"}"
  value="${value//$'\n'/\\n}"
  value="${value//$'\r'/\\r}"
  value="${value//$'\t'/\\t}"
  printf '%s' "$value"
}

if [[ "$RUN" != "true" ]]; then
  exists="false"
  [[ -f "$HOOK_PATH" ]] && exists="true"
  printf '{"root":"%s","event":"%s","hook_path":"%s","payload":"%s","timeout_seconds":%s,"run":false,"exists":%s}\n' "$(json_escape "$ROOT")" "$(json_escape "$EVENT")" "$(json_escape "$HOOK_PATH")" "$(json_escape "$PAYLOAD")" "$TIMEOUT_SECONDS" "$exists"
  exit 0
fi

if [[ ! -f "$HOOK_PATH" ]]; then
  echo "Hook not found: $HOOK_PATH" >&2
  exit 2
fi

timeout "$TIMEOUT_SECONDS" bash "$HOOK_PATH" "$ROOT" "$EVENT" "$PAYLOAD"
