# Installed Forge guidance and verification sync

- kind: validation
- created_at: 2026-06-17T00:48:11+00:00
- checks: scripts/install-plugin-local.ps1: installed local plugin source | install.ps1: installed forge-method and forge-reload skills | installed guide complaint route: correct-course / correct-course / skill:facilitation/correct-course.md / state_update_required true | powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1: passed

## Summary

Synchronized the local plugin and installed Codex skills after guidance and verification changes. Installed runtime now routes Forge human-experience failure complaints to correct-course, not runtime-builder. Install smoke passed, including installed parity replay 92/92 and install-flow evals.
