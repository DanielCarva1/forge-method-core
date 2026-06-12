param(
  [string]$WorkRoot,
  [string]$Python
)

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
  param([string]$PythonPath)
  if ($PythonPath) {
    return $PythonPath
  }
  if ($env:PYTHON) {
    return $env:PYTHON
  }
  foreach ($candidate in @("python", "python3", "py")) {
    $command = Get-Command $candidate -ErrorAction SilentlyContinue
    if ($command) {
      return $command.Source
    }
  }
  $codexPython = Join-Path $HOME ".cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe"
  if (Test-Path -LiteralPath $codexPython) {
    return $codexPython
  }
  throw "Python not found. Set PYTHON or -Python to a Python executable."
}

function Assert-SnapshotModule {
  param(
    [Parameter(Mandatory=$true)][string]$ProjectRoot,
    [Parameter(Mandatory=$true)][string]$ExpectedModule
  )
  $snapshotRaw = & $pythonExe $runtime snapshot --root $ProjectRoot
  if ($LASTEXITCODE -ne 0) {
    throw "snapshot failed for $ProjectRoot"
  }
  $snapshot = $snapshotRaw | ConvertFrom-Json
  if ($snapshot.state.module -ne $ExpectedModule) {
    throw "Expected module $ExpectedModule for $ProjectRoot, got $($snapshot.state.module)"
  }
  if (-not $snapshot.quality.audit.passed) {
    throw "Audit did not pass for $ProjectRoot"
  }
}

function Assert-Recommendation {
  param(
    [Parameter(Mandatory=$true)][string]$Objective,
    [Parameter(Mandatory=$true)][string]$ExpectedModule
  )
  $raw = & $pythonExe $runtime module recommend --objective $Objective --json
  if ($LASTEXITCODE -ne 0) {
    throw "module recommend failed for objective: $Objective"
  }
  $payload = $raw | ConvertFrom-Json
  $actual = $payload.recommended[0].id
  if ($actual -ne $ExpectedModule) {
    throw "Expected recommendation $ExpectedModule for '$Objective', got $actual"
  }
  Write-Host "Recommendation passed: $ExpectedModule"
}

$pythonExe = Resolve-Python -PythonPath $Python
$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$runtime = Join-Path $repoRoot "skills\forge-method\scripts\forge_method_runtime.py"

if (-not $WorkRoot) {
  $WorkRoot = Join-Path $env:TEMP ("forge-method-fixture-smoke-" + [guid]::NewGuid().ToString("N"))
}
if (Test-Path -LiteralPath $WorkRoot) {
  Remove-Item -LiteralPath $WorkRoot -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null

$moduleRaw = & $pythonExe $runtime module list --json
if ($LASTEXITCODE -ne 0) {
  throw "module list failed"
}
$modules = @((($moduleRaw | ConvertFrom-Json).modules))
if ($modules.Count -lt 7) {
  throw "Expected packaged module matrix, found $($modules.Count) module(s)"
}

$examplesRoot = Join-Path $WorkRoot "examples"
$projectsRoot = Join-Path $WorkRoot "projects"
New-Item -ItemType Directory -Force -Path $examplesRoot, $projectsRoot | Out-Null

foreach ($module in $modules) {
  $moduleId = [string]$module.id
  $title = [string]$module.title
  $purpose = [string]$module.purpose

  $exampleRoot = Join-Path $examplesRoot $moduleId
  Run $pythonExe $runtime example create --root $exampleRoot --module $moduleId
  Run $pythonExe $runtime gate --root $exampleRoot --require-evals
  Run $pythonExe $runtime context recover --root $exampleRoot --compact --max-chars 1600
  Assert-SnapshotModule -ProjectRoot $exampleRoot -ExpectedModule $moduleId

  $projectPath = "$moduleId-project"
  $projectRoot = Join-Path $projectsRoot $projectPath
  Run $pythonExe $runtime project create --root $projectsRoot --path $projectPath --name "$title Fixture" --module $moduleId --objective "Fixture coverage for $purpose"
  Run $pythonExe $runtime gate --root $projectRoot --require-evals
  Run $pythonExe $runtime context recover --root $projectRoot --compact --max-chars 1600
  Assert-SnapshotModule -ProjectRoot $projectRoot -ExpectedModule $moduleId

  Write-Host "Fixture passed: $moduleId"
}

$preflight = & $pythonExe $runtime preflight --root $projectsRoot
if ($LASTEXITCODE -ne 0) {
  throw "preflight failed for fixture parent"
}
$preflightText = $preflight -join "`n"
if ($preflightText -notmatch "Route: workspace-with-projects" -or $preflightText -notmatch "Decision options:") {
  throw "Fixture parent preflight did not expose project decision options"
}

Assert-Recommendation -ExpectedModule "core-runtime" -Objective "recover context route durable state across sessions"
Assert-Recommendation -ExpectedModule "software-builder" -Objective "build a web API and software product"
Assert-Recommendation -ExpectedModule "creative-studio" -Objective "design a brand campaign and creative direction"
Assert-Recommendation -ExpectedModule "game-studio" -Objective "build a game prototype"
Assert-Recommendation -ExpectedModule "runtime-builder" -Objective "create a new workflow module and agent profile"
Assert-Recommendation -ExpectedModule "test-architect" -Objective "define validation risk checks and test strategy"
Assert-Recommendation -ExpectedModule "launch-ops" -Objective "prepare launch release operations"

Write-Host "Fixture smoke passed: $WorkRoot"
