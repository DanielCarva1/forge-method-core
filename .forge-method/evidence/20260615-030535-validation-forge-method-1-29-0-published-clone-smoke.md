# Forge Method 1.29.0 published clone smoke

- kind: validation
- created_at: 2026-06-15T03:05:35+00:00
- checks: git ls-remote --tags origin v1.29.0: found tag | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-plugin-clone-install.ps1 -Ref v1.29.0 -ExpectedVersion 1.29.0: passed

## Summary

Published tag v1.29.0 was visible on origin, branch codex/script-audit-optimization was pushed to origin, and smoke-plugin-clone-install.ps1 passed against ref v1.29.0 with ExpectedVersion 1.29.0. The release package installs from a clean clone, creates a Forge project, passes gate, audit, artifact, workflow, agent, and eval checks.
