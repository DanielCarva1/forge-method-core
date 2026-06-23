# Evidence — v2-001: Version field + optimistic concurrency

- kind: story-evidence
- story: v2-001
- created_at: 2026-06-23T08:30:00Z

## What was done

Added `VersionConflict` exception class and modified `write_state` in `scripts/forge_method_runtime.py` to accept `expected_version` + `agent_id` keyword arguments. When `expected_version` is provided, reads disk version and raises `VersionConflict` on mismatch. On success, bumps version. Default `None` preserves v1.34.1 behavior (C2 backward compat).

## Verification

Functional test confirmed:
- Write without `expected_version`: version stays 0 (backward compat)
- Write with correct `expected_version=0`: version bumps to 1
- Write with wrong `expected_version=999`: VersionConflict raised with informative message
- py_compile: OK

Commit: f0d1abe
