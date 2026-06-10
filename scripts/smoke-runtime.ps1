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

$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$runtime = Join-Path $repoRoot "skills\forge-method\scripts\forge_method_runtime.py"
$tmp = Join-Path $env:TEMP "forge-method-smoke"

if (Test-Path -LiteralPath $tmp) {
  Remove-Item -LiteralPath $tmp -Recurse -Force
}

New-Item -ItemType Directory -Path $tmp | Out-Null

Run python $runtime init --project smoke-test --root $tmp
Run python $runtime start --root $tmp
Run python $runtime module list --root $tmp
Run python $runtime workflow validate
Run python $runtime workflow create --root $tmp --id smoke-flow --title "Smoke Flow" --trigger "state.status == smoke" --input "smoke input" --step "perform smoke step" --output "smoke output" --done "smoke output exists" --blocked "smoke input missing" --handoff "preserve smoke result" --eval-query "run smoke flow"
Run python $runtime module create --root $tmp --id smoke-module --title "Smoke Module" --purpose "Exercise project module creation." --phase-span "1-discovery" --workflow smoke-flow
Run python $runtime workflow validate --root $tmp
Run python $runtime eval run --root $tmp
Run python $runtime checkpoint --root $tmp --title "Smoke checkpoint" --summary "Runtime smoke reached generated workflow and eval checks." --decision "Checkpoint memory is available." --check "eval run passed" --touched ".forge-method/workflows/workflow-smoke-flow.md" --next-action "continue smoke runtime verification"
Run python $runtime status --root $tmp
Run python $runtime next --root $tmp
Run python $runtime transition --root $tmp --phase 1-discovery --status discovery-ready --workflow discover-intent
Run python $runtime transition --root $tmp --phase 2-specification --status specification-ready --workflow write-spec
Run python $runtime artifact add --root $tmp --kind spec --title "Smoke specification" --summary "The smoke project requires durable state, evidence, and ready gate validation." --path ".forge-method/artifacts/smoke-spec.md"
Run python $runtime transition --root $tmp --phase 3-plan --status planning-ready --workflow plan-sprint
Run python $runtime transition --root $tmp --phase 4-build-verify --status build-ready --workflow build-story
Run python $runtime story add --root $tmp --id story-1 --title "Prove runtime loop" --acceptance "status can be reconstructed from files" --acceptance "done stories require evidence"
Run python $runtime artifact add --root $tmp --kind task --title "Ephemeral task" --summary "Temporary task docs can be captured and deleted." --path ".forge-method/artifacts/ephemeral-task.md" --lifecycle ephemeral --story story-1
Run python $runtime artifact capture --root $tmp --path ".forge-method/artifacts/ephemeral-task.md" --story story-1 --summary "Ephemeral task result captured in story state." --delete
Run python $runtime artifact link-story --root $tmp --path ".forge-method/artifacts/smoke-spec.md" --story story-1
Run python $runtime story start --root $tmp --id story-1
Run python $runtime story review --root $tmp --id story-1
Run python $runtime story done --root $tmp --id story-1 --summary "Runtime loop completed in smoke test." --check "smoke-runtime.ps1"
Run python $runtime context pack --root $tmp --max-chars 1200
Run python $runtime artifact list --root $tmp
Run python $runtime artifact verify --root $tmp
Run python $runtime audit --root $tmp
Run python $runtime ready --root $tmp --summary "Smoke project is ready." --check audit
Run python $runtime status --root $tmp

Write-Host "Smoke test passed: $tmp"
