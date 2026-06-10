$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$runtime = Join-Path $repoRoot "skills\forge-method\scripts\forge_method_runtime.py"
$tmp = Join-Path $env:TEMP "forge-method-smoke"

if (Test-Path -LiteralPath $tmp) {
  Remove-Item -LiteralPath $tmp -Recurse -Force
}

New-Item -ItemType Directory -Path $tmp | Out-Null

python $runtime init --project smoke-test --root $tmp
python $runtime status --root $tmp
python $runtime next --root $tmp
python $runtime transition --root $tmp --phase 1-discovery --status discovery-ready --workflow discover-intent
python $runtime transition --root $tmp --phase 2-specification --status specification-ready --workflow write-spec
python $runtime artifact add --root $tmp --kind spec --title "Smoke specification" --summary "The smoke project requires durable state, evidence, and ready gate validation."
python $runtime transition --root $tmp --phase 3-plan --status planning-ready --workflow plan-sprint
python $runtime transition --root $tmp --phase 4-build-verify --status build-ready --workflow build-story
python $runtime story add --root $tmp --id story-1 --title "Prove runtime loop" --acceptance "status can be reconstructed from files" --acceptance "done stories require evidence"
python $runtime story start --root $tmp --id story-1
python $runtime story review --root $tmp --id story-1
python $runtime story done --root $tmp --id story-1 --summary "Runtime loop completed in smoke test." --check "smoke-runtime.ps1"
python $runtime context pack --root $tmp
python $runtime artifact list --root $tmp
python $runtime audit --root $tmp
python $runtime ready --root $tmp --summary "Smoke project is ready." --check "audit"
python $runtime status --root $tmp

Write-Host "Smoke test passed: $tmp"
