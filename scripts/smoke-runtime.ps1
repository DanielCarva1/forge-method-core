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
$runtime = Join-Path $repoRoot "skills\forge-method\scripts\forge_method_runtime.py"
$tmp = Join-Path $env:TEMP "forge-method-smoke"
$exampleTmp = Join-Path $env:TEMP "forge-method-example-smoke"
$projectParentTmp = Join-Path $env:TEMP "forge-method-project-smoke"
$generatedProjectTmp = Join-Path $projectParentTmp "generated-smoke"

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

Run $pythonExe $runtime init --project smoke-test --root $tmp
Run $pythonExe $runtime preflight --root $tmp
Run $pythonExe $runtime resume --root $tmp
Run $pythonExe $runtime start --root $tmp
Run $pythonExe $runtime snapshot --root $tmp
Run $pythonExe $runtime module list --root $tmp
Run $pythonExe $runtime agent list --root $tmp
Run $pythonExe $runtime agent validate --root $tmp
Run $pythonExe $runtime agent recommend --root $tmp
Run $pythonExe $runtime example list --root $tmp
Run $pythonExe $runtime example create --root $exampleTmp --module software-builder
Run $pythonExe $runtime gate --root $exampleTmp --require-evals
$projectCreateText = Run-Capture $pythonExe $runtime project create --root $projectParentTmp --name "Generated Smoke" --module software-builder --objective "Verify project scaffolding."
Assert-Contains $projectCreateText "Story: <none - facilitation required>" "project create output"
Assert-Contains $projectCreateText "required_next_workflow: discover-intent" "project create output"
Assert-Contains $projectCreateText "initial-facilitation" "project create output"
Assert-Contains $projectCreateText "Antes de criar stories ou desenvolver" "project create output"
Assert-NotContains $projectCreateText "Story: project-kickoff" "project create output"
$projectListText = Run-Capture $pythonExe $runtime project list --root $projectParentTmp
Assert-Contains $projectListText "generated-smoke" "project list output"
Assert-Contains $projectListText "waiting-human-input" "project list output"
$projectPreflightText = Run-Capture $pythonExe $runtime preflight --root $projectParentTmp
Assert-Contains $projectPreflightText "Route: workspace-with-projects" "project parent preflight output"
Assert-Contains $projectPreflightText "Known projects:" "project parent preflight output"
Assert-Contains $projectPreflightText "Next question: Which existing project should be opened" "project parent preflight output"
Assert-Contains $projectPreflightText "Open Generated Smoke" "project parent preflight output"
$projectReloadText = Run-Capture $pythonExe $runtime reload --root $projectParentTmp
Assert-Contains $projectReloadText "Forge Reload" "project parent reload output"
Assert-Contains $projectReloadText "Route: workspace-with-projects" "project parent reload output"
Assert-Contains $projectReloadText "Known projects:" "project parent reload output"
Assert-Contains $projectReloadText "Next: relay the route opening above" "project parent reload output"
$firstFacilitationAnswer = "Usuarios: professores independentes. Dor: organizar aulas vagas em plano testavel. Experiencia: conversa guiada com criterios claros. Restricoes: browser simples sem login. Sucesso: brief revisavel em dez minutos."
$projectAnswerText = Run-Capture $pythonExe $runtime input answer --root $generatedProjectTmp --id initial-facilitation --answer $firstFacilitationAnswer
Assert-Contains $projectAnswerText "required_next_workflow: discover-intent" "project first answer output"
Assert-Contains $projectAnswerText "context_boundary: resume-first -> discover-intent" "project first answer output"
Assert-NotContains $projectAnswerText "Story added" "project first answer output"
if (Test-Path -LiteralPath (Join-Path $generatedProjectTmp ".forge-method\stories\project-kickoff.yaml")) {
  throw "project first answer created a premature story"
}
$projectGuideText = Run-Capture $pythonExe $runtime guide --root $generatedProjectTmp --question $firstFacilitationAnswer
Assert-Contains $projectGuideText "Guidance Engine: operate-support -> discover-intent / 1-discovery" "project first answer guide output"
Assert-Contains $projectGuideText "Grill Gate: required" "project first answer guide output"
Assert-Contains $projectGuideText "First question: what outcome, constraint, and proof should shape the next pass?" "project first answer guide output"
Assert-NotContains $projectGuideText "Prompt: Let's use" "project first answer guide output"
Assert-NotContains $projectGuideText "build-story" "project first answer guide output"
Run $pythonExe $runtime gate --root $generatedProjectTmp --require-evals
Run $pythonExe $runtime workflow validate
Run $pythonExe $runtime workflow compactness
Run $pythonExe $runtime workflow create --root $tmp --id smoke-flow --title "Smoke Flow" --trigger "state.status == smoke" --input "smoke input" --step "perform smoke step" --output "smoke output" --done "smoke output exists" --blocked "smoke input missing" --handoff "preserve smoke result" --eval-query "run smoke flow"
Run $pythonExe $runtime module create --root $tmp --id smoke-module --title "Smoke Module" --purpose "Exercise project module creation." --phase-span "1-discovery" --workflow smoke-flow
Run $pythonExe $runtime workflow validate --root $tmp
Run $pythonExe $runtime eval run --root $tmp
Run $pythonExe $runtime checkpoint --root $tmp --title "Smoke checkpoint" --summary "Runtime smoke reached generated workflow and eval checks." --decision "Checkpoint memory is available." --check "eval run passed" --touched ".forge-method/workflows/workflow-smoke-flow.md" --next-action "continue smoke runtime verification"
Run $pythonExe $runtime context plan --root $tmp --max-chars 1200
Run $pythonExe $runtime context recover --root $tmp --max-chars 1200
Run $pythonExe $runtime context recover --root $tmp --compact --max-chars 1400
Run $pythonExe $runtime status --root $tmp
Run $pythonExe $runtime resume --root $tmp --json
Run $pythonExe $runtime next --root $tmp
Run $pythonExe $runtime transition --root $tmp --phase 1-discovery --status discovery-ready --workflow discover-intent
Run $pythonExe $runtime input add --root $tmp --id smoke-audience --prompt "Who is the target audience?" --reason "Discovery needs an audience before specification."
Run $pythonExe $runtime input list --root $tmp --status open
Run $pythonExe $runtime input answer --root $tmp --id smoke-audience --answer "Runtime smoke users" --next-action "continue smoke discovery"
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
Run $pythonExe $runtime review add --root $tmp --id smoke-review-proof --story story-1 --title "Smoke review proof" --severity medium --summary "Smoke review finding must be durable and resolved."
Run $pythonExe $runtime review list --root $tmp --status open
Run $pythonExe $runtime review resolve --root $tmp --id smoke-review-proof --resolution "Smoke review finding resolved before completion."
Run $pythonExe $runtime story done --root $tmp --id story-1 --summary "Runtime loop completed in smoke test." --check "smoke-runtime.ps1"
Run $pythonExe $runtime context plan --root $tmp --max-chars 1200
Run $pythonExe $runtime context pack --root $tmp --max-chars 1200
Run $pythonExe $runtime artifact list --root $tmp
Run $pythonExe $runtime artifact verify --root $tmp
Run $pythonExe $runtime audit --root $tmp
Run $pythonExe $runtime gate --root $tmp --require-evals --summary "Runtime smoke quality gate passed." --context-pack --max-chars 1200
Run $pythonExe $runtime ready --root $tmp --summary "Smoke project is ready." --check audit
Run $pythonExe $runtime status --root $tmp

Write-Host "Smoke test passed: $tmp"
