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
python $runtime transition --root $tmp --phase 4-build-verify --status story-ready --workflow build-story
python $runtime status --root $tmp

Write-Host "Smoke test passed: $tmp"

