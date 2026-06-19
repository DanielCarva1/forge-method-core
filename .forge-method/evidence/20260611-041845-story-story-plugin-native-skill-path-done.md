# Story plugin-native-skill-path done

- kind: story
- created_at: 2026-06-11T04:18:45+00:00
- story: plugin-native-skill-path
- checks: powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1 | python -m unittest discover -s tests

## Summary

SKILL.md now tells agents to run helpers relative to the active skill/plugin directory first, with the legacy <user-home>/.agents path only as fallback. smoke-install passed and local skill was updated.
