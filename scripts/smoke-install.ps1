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
$installer = Join-Path $repoRoot "install.ps1"
$installedRuntime = Join-Path $HOME ".agents\skills\forge-method\scripts\forge_method_runtime.py"
$tmp = Join-Path $env:TEMP "forge-method-install-smoke"
$exampleTmp = Join-Path $env:TEMP "forge-method-install-example-smoke"
$projectParentTmp = Join-Path $env:TEMP "forge-method-install-project-smoke"
$generatedProjectTmp = Join-Path $projectParentTmp "installed-generated"

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
if (Test-Path -LiteralPath $exampleTmp) {
  Remove-Item -LiteralPath $exampleTmp -Recurse -Force
}
if (Test-Path -LiteralPath $projectParentTmp) {
  Remove-Item -LiteralPath $projectParentTmp -Recurse -Force
}

New-Item -ItemType Directory -Path $tmp | Out-Null
New-Item -ItemType Directory -Path $projectParentTmp | Out-Null

Run $pythonExe $installedRuntime --help
Run $pythonExe $installedRuntime module list
Run $pythonExe $installedRuntime agent list
Run $pythonExe $installedRuntime agent validate
Run $pythonExe $installedRuntime example list
Run $pythonExe $installedRuntime example create --root $exampleTmp --module software-builder
Run $pythonExe $installedRuntime gate --root $exampleTmp --require-evals
Run $pythonExe $installedRuntime project create --root $projectParentTmp --name "Installed Generated" --module software-builder --objective "Verify installed project scaffolding."
Run $pythonExe $installedRuntime project list --root $projectParentTmp
Run $pythonExe $installedRuntime gate --root $generatedProjectTmp --require-evals
Run $pythonExe $installedRuntime workflow validate
Run $pythonExe $installedRuntime start --root $tmp
Run $pythonExe $installedRuntime init --project install-smoke --root $tmp
Run $pythonExe $installedRuntime start --root $tmp
Run $pythonExe $installedRuntime snapshot --root $tmp
Run $pythonExe $installedRuntime agent recommend --root $tmp
Run $pythonExe $installedRuntime workflow create --root $tmp --id install-flow --title "Install Flow" --trigger "installed runtime available" --input "installed runtime" --step "validate installed runtime" --output "install proof" --done "install proof exists" --blocked "runtime missing" --handoff "preserve install result" --eval-query "prove install flow"
Run $pythonExe $installedRuntime eval run --root $tmp
Run $pythonExe $installedRuntime checkpoint --root $tmp --title "Install checkpoint" --summary "Installed runtime can persist checkpoint memory." --check "install eval passed" --next-action "continue install smoke"
Run $pythonExe $installedRuntime context plan --root $tmp --max-chars 1200
Run $pythonExe $installedRuntime context recover --root $tmp --max-chars 1200
Run $pythonExe $installedRuntime transition --root $tmp --phase 1-discovery --status discovery-ready --workflow discover-intent
Run $pythonExe $installedRuntime input add --root $tmp --id install-audience --prompt "Who is the install smoke audience?" --reason "Install smoke needs durable input coverage."
Run $pythonExe $installedRuntime input answer --root $tmp --id install-audience --answer "Install smoke users" --next-action "continue install smoke"
Run $pythonExe $installedRuntime story add --root $tmp --id install-story --title "Installed runtime works" --acceptance "installed helper can write durable state"
Run $pythonExe $installedRuntime artifact verify --root $tmp
Run $pythonExe $installedRuntime gate --root $tmp --require-evals --summary "Installed runtime quality gate passed."
Run $pythonExe $installedRuntime status --root $tmp
Run $pythonExe $installedRuntime next --root $tmp
Run $pythonExe $installedRuntime audit --root $tmp

Write-Host "Install smoke test passed: $tmp"
