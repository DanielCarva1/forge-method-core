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
Run python $runtime module list --root $tmp
Run python $runtime workflow validate
Run python $runtime status --root $tmp
Run python $runtime next --root $tmp
Run python $runtime transition --root $tmp --phase 1-discovery --status discovery-ready --workflow discover-intent
Run python $runtime transition --root $tmp --phase 2-specification --status specification-ready --workflow write-spec
Run python $runtime artifact add --root $tmp --kind spec --title "Smoke specification" --summary "The smoke project requires durable state, evidence, and ready gate validation."
Run python $runtime transition --root $tmp --phase 3-plan --status planning-ready --workflow plan-sprint
Run python $runtime transition --root $tmp --phase 4-build-verify --status build-ready --workflow build-story
Run python $runtime story add --root $tmp --id story-1 --title "Prove runtime loop" --acceptance "status can be reconstructed from files" --acceptance "done stories require evidence"
Run python $runtime story start --root $tmp --id story-1
Run python $runtime story review --root $tmp --id story-1
Run python $runtime story done --root $tmp --id story-1 --summary "Runtime loop completed in smoke test." --check "smoke-runtime.ps1"
Run python $runtime context pack --root $tmp
Run python $runtime artifact list --root $tmp
Run python $runtime audit --root $tmp
Run python $runtime ready --root $tmp --summary "Smoke project is ready." --check audit
Run python $runtime status --root $tmp

Write-Host "Smoke test passed: $tmp"
