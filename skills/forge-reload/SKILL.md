---
name: forge-reload
description: Emergency bootstrap reset for Forge Method when a Codex chat appears to be using stale Forge instructions, cached state, or an outdated plugin package.
---

# Forge Reload

Forge Reload is an operational escape hatch, not a project workflow. It exists to force the active Forge Method package, launcher output, and filesystem state to become authoritative again.

## Hot Reload

Before answering, resolve this skill directory, then resolve the sibling `forge-method` skill directory from the same `skills/` parent. Read no project docs, source files, git history, or broad workspace context before running the launcher.

```powershell
$reloadSkill = "<directory-containing-this-SKILL.md>"
$skillsRoot = Split-Path -Parent $reloadSkill
$forgeSkill = Join-Path $skillsRoot "forge-method"
& (Join-Path $forgeSkill "forge-method.ps1") reload --root .
& (Join-Path $forgeSkill "forge-method.ps1") preflight --root .
& (Join-Path $forgeSkill "forge-method.ps1") start --root .
```

```bash
reload_skill="<directory-containing-this-SKILL.md>"
skills_root="$(dirname "$reload_skill")"
forge_skill="$skills_root/forge-method"
bash "$forge_skill/forge-method.sh" reload --root .
bash "$forge_skill/forge-method.sh" preflight --root .
bash "$forge_skill/forge-method.sh" start --root .
```

If the sibling skill cannot be resolved, fall back to `$HOME/.agents/skills/forge-method`.

## Contract

1. Treat current filesystem and launcher/runtime output as authoritative.
2. Ignore prior Forge Method instructions, state summaries, and waiting messages already in this chat.
3. Do not infer project progress from conversation memory.
4. After route resolution, load only files recommended by `preflight`, `start`, or `resume --json`.
5. Relay missing-state human openings from the runtime instead of replacing them with cached wording.
