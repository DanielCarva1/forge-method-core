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
$installer = Join-Path $repoRoot "install.ps1"
$installedRuntime = Join-Path $HOME ".agents\skills\forge-method\scripts\forge_method_runtime.py"
$tmp = Join-Path $env:TEMP "forge-method-install-smoke"

powershell -ExecutionPolicy Bypass -File $installer
if ($LASTEXITCODE -ne 0) {
  throw "Installer failed with exit code $LASTEXITCODE"
}

if (-not (Test-Path -LiteralPath $installedRuntime)) {
  throw "Installed runtime helper not found: $installedRuntime"
}

if (Test-Path -LiteralPath $tmp) {
  Remove-Item -LiteralPath $tmp -Recurse -Force
}

New-Item -ItemType Directory -Path $tmp | Out-Null

Run python $installedRuntime --help
Run python $installedRuntime module list
Run python $installedRuntime workflow validate
Run python $installedRuntime start --root $tmp
Run python $installedRuntime init --project install-smoke --root $tmp
Run python $installedRuntime start --root $tmp
Run python $installedRuntime workflow create --root $tmp --id install-flow --title "Install Flow" --trigger "installed runtime available" --input "installed runtime" --step "validate installed runtime" --output "install proof" --done "install proof exists" --blocked "runtime missing" --handoff "preserve install result" --eval-query "prove install flow"
Run python $installedRuntime eval run --root $tmp
Run python $installedRuntime checkpoint --root $tmp --title "Install checkpoint" --summary "Installed runtime can persist checkpoint memory." --check "install eval passed" --next-action "continue install smoke"
Run python $installedRuntime transition --root $tmp --phase 1-discovery --status discovery-ready --workflow discover-intent
Run python $installedRuntime story add --root $tmp --id install-story --title "Installed runtime works" --acceptance "installed helper can write durable state"
Run python $installedRuntime status --root $tmp
Run python $installedRuntime next --root $tmp
Run python $installedRuntime audit --root $tmp

Write-Host "Install smoke test passed: $tmp"
