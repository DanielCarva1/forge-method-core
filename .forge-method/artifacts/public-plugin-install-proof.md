# Public Plugin Install Proof

## Public Flow

The README documents the intended public Codex flow:

1. Install Codex.
2. Run `codex plugin marketplace add DanielCarva1/forge-method-core --ref main`.
3. Open Codex Plugins or `/plugins`.
4. Select `Forge Method`.
5. Install or enable `Forge Method Core`.
6. Start a new thread.
7. Invoke `$forge-method` and ask it to start in the workspace.

## Package Contents

The plugin package contains the runtime skill, runtime helper scripts, workflow references, module manifests, agent profiles, templates, examples, docs, validation scripts, and marketplace metadata.

## Validation

`scripts/verify-fast.ps1` passed. The fast verification covered onboarding asset validation, workflow validation, agent profile validation, and Python unit tests.

`scripts/smoke-install.ps1` also passed for the current batch and refreshed the local installed skill.

## Boundary

This proves repo-based Codex plugin installation and local enablement. It does not claim official Codex marketplace publication, which remains a separate external submission/approval process.
