---
name: forge-update
description: Operational maintenance command for updating or migrating an installed Forge Method Core package and summarizing the new version.
---

# Forge Update

Forge Update is an operational maintenance skill, not a product workflow. It detects the current Forge Method package shape, updates Git marketplace installs, migrates legacy/local installs to the Git marketplace main package when possible, and reports the version plus short patch notes.

## Hot Update

Before answering, resolve this skill directory, then resolve the sibling `forge-method` skill directory from the same `skills/` parent. Read no project docs, source files, git history, or broad workspace context before running the updater.

```powershell
$updateSkill = "<directory-containing-this-SKILL.md>"
$skillsRoot = Split-Path -Parent $updateSkill
$forgeSkill = Join-Path $skillsRoot "forge-method"
if ($env:PYTHON) {
  $python = $env:PYTHON
} else {
  $python = $null
  foreach ($candidate in @("python", "python3", "py")) {
    $command = Get-Command $candidate -ErrorAction SilentlyContinue
    if ($command) {
      $python = $command.Source
      break
    }
  }
  if (-not $python) {
    $codexPython = Join-Path $HOME ".cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe"
    if (Test-Path -LiteralPath $codexPython) { $python = $codexPython }
  }
}
if (-not $python) { throw "Python not found. Set PYTHON to a Python executable." }
& $python (Join-Path $forgeSkill "scripts\forge_method_updater.py") --skill-dir $forgeSkill --manual
& (Join-Path $forgeSkill "forge-method.ps1") reload --root .
```

```bash
update_skill="<directory-containing-this-SKILL.md>"
skills_root="$(dirname "$update_skill")"
forge_skill="$skills_root/forge-method"
python_cmd="${PYTHON:-python3}"
"$python_cmd" "$forge_skill/scripts/forge_method_updater.py" --skill-dir "$forge_skill" --manual
bash "$forge_skill/forge-method.sh" reload --root .
```

If the sibling skill cannot be resolved, fall back to `$HOME/.agents/skills/forge-method`.

## Contract

1. Treat this as maintenance of the plugin package, not project progress.
2. Do not mutate `.forge-method/state.yaml`, sprint files, stories, or project artifacts as part of update.
3. If already updated, say the current version plainly.
4. If the install is not a Git marketplace install, let the updater migrate it through Codex marketplace add. If migration fails, show:

```powershell
codex plugin marketplace add DanielCarva1/forge-method-core --ref main
```

5. If skill instructions changed, keep helping in the current chat and mention a new thread only as an optional way to reload fresh skill text.
