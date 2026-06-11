# Story self-update-hot-start done

- kind: story
- created_at: 2026-06-11T15:24:52+00:00
- story: self-update-hot-start
- checks: python -m unittest discover -s tests | python .\skills\forge-method\scripts\forge_method_runtime.py workflow validate | powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1

## Summary

Implemented Forge 1.25 self-update without double initialization: launchers run updater, Git marketplace installs update before start/preflight/guide/resume, patch notes are emitted once to stderr, SKILL.md is now a compact Hot Start Stub, and tests cover updater policy and JSON stdout cleanliness.
