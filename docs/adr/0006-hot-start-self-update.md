# ADR 0006: Hot Start Self-Update

## Status

Accepted

## Context

Forge Method users invoke the product inside an active Codex chat. If every update forced a new thread and a second `$forge-method` invocation, startup would feel broken and repetitive. Codex can refresh plugin and skill metadata in some app-server paths, but full hot reload of already-loaded skill instructions is not a stable product contract for Forge to depend on.

## Decision

Forge Method uses a compact, stable Hot Start Stub in `SKILL.md` and moves normal product evolution into runtime scripts, workflow references, and release notes. The launcher performs self-update for Git marketplace installs before normal startup, prints compact patch notes when a newer version is installed, and continues the same `preflight` or `start` flow.

## Consequences

- Users normally invoke `$forge-method` once, even when an update is applied.
- Runtime, workflow, and release-note changes can be used immediately after update.
- If a future update changes skill text or plugin manifest, Forge continues the current start and may recommend a later new thread only to fully load refreshed skill instructions.
- The launcher must fail open: offline, timeout, missing Codex CLI, or update errors continue with the local runtime.
