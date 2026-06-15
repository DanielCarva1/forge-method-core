$ErrorActionPreference = "Stop"
$env:FORGE_METHOD_SKIP_UPDATE = "1"

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

function Run-Capture {
  param(
    [Parameter(Mandatory=$true)]
    [string]$Exe,
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$Args
  )
  $output = & $Exe @Args 2>&1 | Out-String
  if ($LASTEXITCODE -ne 0) {
    throw "$Exe failed with exit code ${LASTEXITCODE}: $($Args -join ' ')`n$output"
  }
  return $output
}

function Assert-Contains {
  param(
    [Parameter(Mandatory=$true)]
    [string]$Text,
    [Parameter(Mandatory=$true)]
    [string]$Expected,
    [Parameter(Mandatory=$true)]
    [string]$Label
  )
  if (-not $Text.Contains($Expected)) {
    throw "$Label did not contain expected text: $Expected`n$Text"
  }
}

function Assert-NotContains {
  param(
    [Parameter(Mandatory=$true)]
    [string]$Text,
    [Parameter(Mandatory=$true)]
    [string]$Unexpected,
    [Parameter(Mandatory=$true)]
    [string]$Label
  )
  if ($Text.Contains($Unexpected)) {
    throw "$Label contained unexpected text: $Unexpected`n$Text"
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
  $codexPython = Join-Path $HOME ".cache\codex-runtimes\codex-primary-runtime\dependencies\python\python.exe"
  if (Test-Path -LiteralPath $codexPython) {
    return $codexPython
  }
  throw "Python not found. Set PYTHON to a Python executable."
}

$pythonExe = Resolve-Python
$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$installer = Join-Path $repoRoot "install.ps1"
$installedRuntime = Join-Path $HOME ".agents\skills\forge-method\scripts\forge_method_runtime.py"
$installedLauncher = Join-Path $HOME ".agents\skills\forge-method\forge-method.ps1"
$installedReloadSkill = Join-Path $HOME ".agents\skills\forge-reload\SKILL.md"
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
if (-not (Test-Path -LiteralPath $installedLauncher)) {
  throw "Installed runtime launcher not found: $installedLauncher"
}
if (-not (Test-Path -LiteralPath $installedReloadSkill)) {
  throw "Installed reload skill not found: $installedReloadSkill"
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
Run powershell -ExecutionPolicy Bypass -File $installedLauncher --help
Run powershell -ExecutionPolicy Bypass -File $installedLauncher reload --root $tmp
Run $pythonExe $installedRuntime module list
Run $pythonExe $installedRuntime agent list
Run $pythonExe $installedRuntime agent validate
Run $pythonExe $installedRuntime example list
Run $pythonExe $installedRuntime example create --root $exampleTmp --module software-builder
Run $pythonExe $installedRuntime gate --root $exampleTmp --require-evals
$installedProjectCreateText = Run-Capture $pythonExe $installedRuntime project create --root $projectParentTmp --name "Installed Generated" --module software-builder --objective "Verify installed project scaffolding."
Assert-Contains $installedProjectCreateText "Story: <none - facilitation required>" "installed project create output"
Assert-Contains $installedProjectCreateText "required_next_workflow: discover-intent" "installed project create output"
Assert-Contains $installedProjectCreateText "initial-facilitation" "installed project create output"
Assert-Contains $installedProjectCreateText "Antes de criar stories ou desenvolver" "installed project create output"
Assert-NotContains $installedProjectCreateText "Story: project-kickoff" "installed project create output"
$installedProjectListText = Run-Capture $pythonExe $installedRuntime project list --root $projectParentTmp
Assert-Contains $installedProjectListText "installed-generated" "installed project list output"
Assert-Contains $installedProjectListText "waiting-human-input" "installed project list output"
$installedProjectPreflightText = Run-Capture $pythonExe $installedRuntime preflight --root $projectParentTmp
Assert-Contains $installedProjectPreflightText "Route: workspace-with-projects" "installed project parent preflight output"
Assert-Contains $installedProjectPreflightText "Known projects:" "installed project parent preflight output"
Assert-Contains $installedProjectPreflightText "Next question: Which existing project should be opened" "installed project parent preflight output"
Assert-Contains $installedProjectPreflightText "Open Installed Generated" "installed project parent preflight output"
$installedProjectReloadText = Run-Capture $pythonExe $installedRuntime reload --root $projectParentTmp
Assert-Contains $installedProjectReloadText "Forge Reload" "installed project parent reload output"
Assert-Contains $installedProjectReloadText "Route: workspace-with-projects" "installed project parent reload output"
Assert-Contains $installedProjectReloadText "Known projects:" "installed project parent reload output"
Assert-Contains $installedProjectReloadText "Next: relay the route opening above" "installed project parent reload output"
$installedFirstFacilitationAnswer = "Usuarios: professores independentes. Dor: organizar aulas vagas em plano testavel. Experiencia: conversa guiada com criterios claros. Restricoes: browser simples sem login. Sucesso: brief revisavel em dez minutos."
$installedProjectAnswerText = Run-Capture $pythonExe $installedRuntime input answer --root $generatedProjectTmp --id initial-facilitation --answer $installedFirstFacilitationAnswer
Assert-Contains $installedProjectAnswerText "required_next_workflow: discover-intent" "installed project first answer output"
Assert-Contains $installedProjectAnswerText "context_boundary: resume-first -> discover-intent" "installed project first answer output"
Assert-NotContains $installedProjectAnswerText "Story added" "installed project first answer output"
if (Test-Path -LiteralPath (Join-Path $generatedProjectTmp ".forge-method\stories\project-kickoff.yaml")) {
  throw "installed project first answer created a premature story"
}
$installedProjectGuideText = Run-Capture $pythonExe $installedRuntime guide --root $generatedProjectTmp --question $installedFirstFacilitationAnswer
Assert-Contains $installedProjectGuideText "Guidance Engine: operate-support -> discover-intent / 1-discovery" "installed project first answer guide output"
Assert-Contains $installedProjectGuideText "Grill Gate: required" "installed project first answer guide output"
Assert-Contains $installedProjectGuideText "First question: what outcome, constraint, and proof should shape the next pass?" "installed project first answer guide output"
Assert-NotContains $installedProjectGuideText "Prompt: Let's use" "installed project first answer guide output"
Assert-NotContains $installedProjectGuideText "build-story" "installed project first answer guide output"
Run $pythonExe $installedRuntime gate --root $generatedProjectTmp --require-evals
Run $pythonExe $installedRuntime workflow validate
Run $pythonExe $installedRuntime parity replay
Run $pythonExe $installedRuntime preflight --root $tmp
Run $pythonExe $installedRuntime reload --root $tmp
Run $pythonExe $installedRuntime start --root $tmp
Run $pythonExe $installedRuntime init --project install-smoke --root $tmp
Run $pythonExe $installedRuntime preflight --root $tmp
Run $pythonExe $installedRuntime reload --root $tmp
Run $pythonExe $installedRuntime resume --root $tmp
Run $pythonExe $installedRuntime start --root $tmp
$installedGuideText = Run-Capture $pythonExe $installedRuntime guide --root $tmp --question "quero fazer brainstorm de alternativas para este produto"
Assert-Contains $installedGuideText "Guidance: Let's use ``brainstorming`` as the guided path." "installed guide brainstorm output"
Assert-Contains $installedGuideText "First question:" "installed guide brainstorm output"
Assert-NotContains $installedGuideText "Prompt: Let's use ``brainstorming``" "installed guide brainstorm output"
Run $pythonExe $installedRuntime snapshot --root $tmp
Run $pythonExe $installedRuntime agent recommend --root $tmp
Run $pythonExe $installedRuntime workflow create --root $tmp --id install-flow --title "Install Flow" --trigger "installed runtime available" --input "installed runtime" --step "validate installed runtime" --output "install proof" --done "install proof exists" --blocked "runtime missing" --handoff "preserve install result" --eval-query "prove install flow"
Run $pythonExe $installedRuntime eval run --root $tmp
Run $pythonExe $installedRuntime checkpoint --root $tmp --title "Install checkpoint" --summary "Installed runtime can persist checkpoint memory." --check "install eval passed" --next-action "continue install smoke"
Run $pythonExe $installedRuntime context plan --root $tmp --max-chars 1200
Run $pythonExe $installedRuntime context recover --root $tmp --max-chars 1200
Run $pythonExe $installedRuntime context recover --root $tmp --compact --max-chars 1400
Run $pythonExe $installedRuntime resume --root $tmp --json
Run $pythonExe $installedRuntime transition --root $tmp --phase 1-discovery --status discovery-ready --workflow discover-intent
Run $pythonExe $installedRuntime input add --root $tmp --id install-audience --prompt "Who is the install smoke audience?" --reason "Install smoke needs durable input coverage."
Run $pythonExe $installedRuntime input answer --root $tmp --id install-audience --answer "Install smoke users" --next-action "continue install smoke"
Run $pythonExe $installedRuntime story add --root $tmp --id install-story --title "Installed runtime works" --acceptance "installed helper can write durable state"
Run $pythonExe $installedRuntime review add --root $tmp --id install-review-proof --story install-story --title "Installed review proof" --severity low --summary "Installed runtime can store review findings."
Run $pythonExe $installedRuntime review list --root $tmp --status open
Run $pythonExe $installedRuntime review resolve --root $tmp --id install-review-proof --resolution "Installed review finding resolved."
Run $pythonExe $installedRuntime artifact verify --root $tmp
Run $pythonExe $installedRuntime gate --root $tmp --require-evals --summary "Installed runtime quality gate passed."
Run $pythonExe $installedRuntime status --root $tmp
Run $pythonExe $installedRuntime next --root $tmp
Run $pythonExe $installedRuntime audit --root $tmp

Write-Host "Install smoke test passed: $tmp"
