$ErrorActionPreference = "Stop"

function Run {
  param(
    [Parameter(Mandatory=$true)]
    [string]$Exe,
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$Args
  )
  & $Exe @Args
  if ($LASTEXITCODE -ne 0) {
    throw "$Exe failed with exit code ${LASTEXITCODE}: $($Args -join ' ')"
  }
}

function Resolve-Python {
  if ($env:PYTHON) {
    return $env:PYTHON
  }
  foreach ($candidate in @("python", "python3", "py")) {
    $command = Get-Command $candidate -ErrorAction SilentlyContinue
    if ($command) {
      return $command.Source
    }
  }
  throw "Python not found. Set PYTHON to a Python executable."
}

$pythonExe = Resolve-Python
$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$runtime = Join-Path $repoRoot "skills\forge-method\scripts\forge_method_runtime.py"
$tmp = Join-Path $env:TEMP "forge-method-smoke"
$exampleTmp = Join-Path $env:TEMP "forge-method-example-smoke"

if (Test-Path -LiteralPath $tmp) {
  Remove-Item -LiteralPath $tmp -Recurse -Force
}
if (Test-Path -LiteralPath $exampleTmp) {
  Remove-Item -LiteralPath $exampleTmp -Recurse -Force
}

New-Item -ItemType Directory -Path $tmp | Out-Null

Run $pythonExe $runtime init --project smoke-test --root $tmp
Run $pythonExe $runtime start --root $tmp
Run $pythonExe $runtime snapshot --root $tmp
Run $pythonExe $runtime module list --root $tmp
Run $pythonExe $runtime example list --root $tmp
Run $pythonExe $runtime example create --root $exampleTmp --module software-builder
Run $pythonExe $runtime gate --root $exampleTmp --require-evals
Run $pythonExe $runtime workflow validate
Run $pythonExe $runtime workflow create --root $tmp --id smoke-flow --title "Smoke Flow" --trigger "state.status == smoke" --input "smoke input" --step "perform smoke step" --output "smoke output" --done "smoke output exists" --blocked "smoke input missing" --handoff "preserve smoke result" --eval-query "run smoke flow"
Run $pythonExe $runtime module create --root $tmp --id smoke-module --title "Smoke Module" --purpose "Exercise project module creation." --phase-span "1-discovery" --workflow smoke-flow
Run $pythonExe $runtime workflow validate --root $tmp
Run $pythonExe $runtime eval run --root $tmp
Run $pythonExe $runtime checkpoint --root $tmp --title "Smoke checkpoint" --summary "Runtime smoke reached generated workflow and eval checks." --decision "Checkpoint memory is available." --check "eval run passed" --touched ".forge-method/workflows/workflow-smoke-flow.md" --next-action "continue smoke runtime verification"
Run $pythonExe $runtime context recover --root $tmp --max-chars 1200
Run $pythonExe $runtime status --root $tmp
Run $pythonExe $runtime next --root $tmp
Run $pythonExe $runtime transition --root $tmp --phase 1-discovery --status discovery-ready --workflow discover-intent
Run $pythonExe $runtime transition --root $tmp --phase 2-specification --status specification-ready --workflow write-spec
Run $pythonExe $runtime artifact add --root $tmp --kind spec --title "Smoke specification" --summary "The smoke project requires durable state, evidence, and ready gate validation." --path ".forge-method/artifacts/smoke-spec.md" --eval
Run $pythonExe $runtime transition --root $tmp --phase 3-plan --status planning-ready --workflow plan-sprint
Run $pythonExe $runtime transition --root $tmp --phase 4-build-verify --status build-ready --workflow build-story
Run $pythonExe $runtime story add --root $tmp --id story-1 --title "Prove runtime loop" --acceptance "status can be reconstructed from files" --acceptance "done stories require evidence"
Run $pythonExe $runtime artifact add --root $tmp --kind task --title "Ephemeral task" --summary "Temporary task docs can be captured and deleted." --path ".forge-method/artifacts/ephemeral-task.md" --lifecycle ephemeral --story story-1
Run $pythonExe $runtime artifact capture --root $tmp --path ".forge-method/artifacts/ephemeral-task.md" --story story-1 --summary "Ephemeral task result captured in story state." --delete
Run $pythonExe $runtime artifact link-story --root $tmp --path ".forge-method/artifacts/smoke-spec.md" --story story-1
Run $pythonExe $runtime story start --root $tmp --id story-1
Run $pythonExe $runtime story review --root $tmp --id story-1
Run $pythonExe $runtime story done --root $tmp --id story-1 --summary "Runtime loop completed in smoke test." --check "smoke-runtime.ps1"
Run $pythonExe $runtime context pack --root $tmp --max-chars 1200
Run $pythonExe $runtime artifact list --root $tmp
Run $pythonExe $runtime artifact verify --root $tmp
Run $pythonExe $runtime audit --root $tmp
Run $pythonExe $runtime gate --root $tmp --require-evals --summary "Runtime smoke quality gate passed." --context-pack --max-chars 1200
Run $pythonExe $runtime ready --root $tmp --summary "Smoke project is ready." --check audit
Run $pythonExe $runtime status --root $tmp

Write-Host "Smoke test passed: $tmp"
