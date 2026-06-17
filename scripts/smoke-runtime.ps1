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

function Run-Fails-Capture {
  param(
    [Parameter(Mandatory=$true)]
    [string]$Exe,
    [Parameter(ValueFromRemainingArguments=$true)]
    [string[]]$Args
  )
  $previousErrorActionPreference = $ErrorActionPreference
  $ErrorActionPreference = "Continue"
  try {
    $output = & $Exe @Args 2>&1 | Out-String
    $exitCode = $LASTEXITCODE
  } finally {
    $ErrorActionPreference = $previousErrorActionPreference
  }
  if ($exitCode -eq 0) {
    throw "$Exe unexpectedly succeeded: $($Args -join ' ')`n$output"
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
Assert-Contains $projectGuideText "First question: give me the whole picture first: who is it for, what should change for them, what is fixed or out, what is still open, and what proof should close discovery?" "project first answer guide output"
Assert-NotContains $projectGuideText "Prompt: Let's use" "project first answer guide output"
Assert-NotContains $projectGuideText "build-story" "project first answer guide output"
$blockedProjectTransitionText = Run-Fails-Capture $pythonExe $runtime transition --root $generatedProjectTmp --phase 2-specification --status specification-ready --workflow write-spec
Assert-Contains $blockedProjectTransitionText "Discovery closeout required before specification" "project first answer transition guard output"
$projectCloseoutText = Run-Capture $pythonExe $runtime artifact discovery-closeout --root $generatedProjectTmp --path ".forge-method/artifacts/discovery-intent.md" --audience "independent teachers planning flexible lessons" --outcome "create a guided planning product that turns vague class ideas into reviewable plans" --constraints "browser-first prototype, no login in the first pass, preserve simple language" --non-goals "no scheduling marketplace, no automated grading, no implementation architecture yet" --success-signal "a teacher can produce a reviewable brief with constraints and proof in ten minutes" --open-questions "none blocking; pricing and collaboration can wait"
Assert-Contains $projectCloseoutText ".forge-method/artifacts/discovery-intent.md" "project discovery closeout output"
Assert-Contains $projectCloseoutText "Discovery closeout check passed." "project discovery closeout output"
Run $pythonExe $runtime artifact discovery-check --root $generatedProjectTmp --path ".forge-method/artifacts/discovery-intent.md"
$projectCloseoutTransitionText = Run-Capture $pythonExe $runtime transition --root $generatedProjectTmp --phase 2-specification --status specification-ready --workflow write-spec
Assert-Contains $projectCloseoutTransitionText "Transition written." "project discovery closeout transition output"
$projectSpecKernelText = Run-Capture $pythonExe $runtime artifact spec-kernel --root $generatedProjectTmp --path ".forge-method/artifacts/spec-kernel.md" --source-artifacts ".forge-method/artifacts/discovery-intent.md" --why "Teachers need a compact WHAT contract before architecture or stories." --capabilities "CAP-1 intent: teacher can turn vague class ideas into a reviewable plan; success: spec-check validates the kernel" --constraints "browser-first prototype, no login in first pass, preserve simple language" --non-goals "no scheduling marketplace, no automated grading, no implementation architecture yet" --success-signal "a teacher can review the kernel and see what will be built without reading chat history" --preservation-map "source claim absorbed into CAP-1 from discovery-intent; open questions preserved as none blocking" --next-workflow "architecture" --eval
Assert-Contains $projectSpecKernelText ".forge-method/artifacts/spec-kernel.md" "project spec kernel output"
Assert-Contains $projectSpecKernelText "Spec kernel check passed." "project spec kernel output"
Run $pythonExe $runtime artifact spec-check --root $generatedProjectTmp --path ".forge-method/artifacts/spec-kernel.md"
$projectResearchText = Run-Capture $pythonExe $runtime artifact research-scan --root $generatedProjectTmp --path ".forge-method/artifacts/market-scan.md" --workflow market-scan --research-question "Would teachers switch from ad hoc notes for guided lesson planning?" --decision-to-unlock "decide whether product requirements should include a guided planning wedge" --claim "Independent teachers have adoption pain worth solving." --sources "first-party prompt transcript, comparable planning tools, public pricing pages" --source-gaps "no primary interviews in smoke" --evidence-grade "recency current, authority mixed, directness medium, bias noted" --findings "alternatives exist but guided constraints remain a differentiator." --contradictions-or-falsifiers "if teachers already complete plans quickly, shrink scope." --uncertainty "demand signal is illustrative in smoke." --stance "continue to research-closeout with adoption risk explicit" --alternatives "notes apps, spreadsheets, lesson templates" --adoption-friction "habit change and trust barrier" --demand-signal "manual workaround implied by prompt" --next-workflow "research-closeout" --eval
Assert-Contains $projectResearchText ".forge-method/artifacts/market-scan.md" "project research scan output"
Assert-Contains $projectResearchText "Research scan check passed." "project research scan output"
Run $pythonExe $runtime artifact research-check --root $generatedProjectTmp --path ".forge-method/artifacts/market-scan.md"
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
Run $pythonExe $runtime artifact spec-kernel --root $tmp --path ".forge-method/artifacts/smoke-spec.md" --source-artifacts ".forge-method/inputs/smoke-audience.yaml" --why "The smoke project requires durable state, evidence, and ready gate validation." --capabilities "CAP-1 intent: runtime can preserve state through discovery, spec, plan, build, and ready; success: smoke-runtime.ps1 reaches ready" --constraints "Use only local files and deterministic runtime commands." --non-goals "No external service, deployment, or product implementation." --success-signal "The smoke project reaches ready with artifact verify, audit, and gate passing." --preservation-map "source input answer absorbed into CAP-1; command evidence remains in smoke output" --next-workflow "plan-sprint" --eval
Run $pythonExe $runtime artifact spec-check --root $tmp --path ".forge-method/artifacts/smoke-spec.md"
Run $pythonExe $runtime artifact research-scan --root $tmp --path ".forge-method/artifacts/smoke-research.md" --workflow technical-feasibility-scan --research-question "Can the smoke runtime prove deterministic artifact generation?" --decision-to-unlock "decide whether the build loop can continue to planning" --claim "Local deterministic commands can generate validated evidence artifacts." --sources "runtime command output, smoke script, local artifact files" --source-gaps "no external services in smoke" --evidence-grade "recency current, authority high, directness high, bias noted" --findings "source checkout commands generate and validate artifacts without external dependencies." --contradictions-or-falsifiers "if generation requires network or stale chat context, block the loop." --uncertainty "package install coverage is checked separately." --stance "continue to planning with install smoke as complementary proof" --feasibility-stance "plausible and demonstrated by deterministic local commands" --riskiest-unknowns "installed package parity and stale local copies" --proof-path "run smoke-runtime and smoke-install in the same batch" --next-workflow "research-closeout" --eval
Run $pythonExe $runtime artifact research-check --root $tmp --path ".forge-method/artifacts/smoke-research.md"
Run $pythonExe $runtime artifact game-brief --root $tmp --path ".forge-method/artifacts/smoke-game-brief.md" --source-material "smoke prompt and runtime command output" --player-fantasy "Guide a tiny arcade hero through one readable room." --core-loop "move, dodge, collect, score, restart with clearer timing" --player-verbs "move, dodge, collect, score" --target-player "solo browser game player" --platform-or-engine "browser canvas prototype" --pillars "readable input, immediate feedback, tiny playable loop" --references "classic arcade room, lightweight browser prototype" --mvp-playable-proof "player can move through one room and collect one item" --dream-game "full arcade campaign with levels and powerups" --vertical-slice "one room with one collectible and failure feedback" --playable-slice "player moves, dodges, collects, and sees score feedback" --parked-scope "campaign, editor, multiplayer, advanced art" --rejected-directions "engine architecture before playable room" --decision-log "smoke game brief generated from local command" --assumptions "placeholder art and keyboard input" --open-questions "none blocking" --research-needed "none blocking" --next-workflow "game-sprint-planning" --eval
Run $pythonExe $runtime artifact game-check --root $tmp --path ".forge-method/artifacts/smoke-game-brief.md"
Run $pythonExe $runtime artifact game-sprint-plan --root $tmp --path ".forge-method/artifacts/smoke-game-sprint-plan.md" --source-material ".forge-method/artifacts/smoke-game-brief.md" --player-fantasy "Guide a tiny arcade hero through one readable room." --playable-slice "player moves, dodges, collects, and sees score feedback" --playable-slice-goal "one room proves input, feedback, and score loop" --decision-sources ".forge-method/artifacts/smoke-game-brief.md" --story-batch "input movement, collectible, hazard feedback, score display" --player-value-order "movement before score display" --risk-order "input feel before decorative polish" --dependencies "movement before collectible collision" --engine-or-asset-constraints "browser canvas with placeholder art" --validation-plan "manual smoke play plus artifact game-check" --manual-playtest-plan "player completes one room and sees score feedback" --deferred-scope "campaign, editor, multiplayer" --blocked-items "none blocking" --next-story "story-input-movement" --sprint-update "first playable slice ready for story creation" --next-workflow "game-story-creation" --eval
Run $pythonExe $runtime artifact game-check --root $tmp --path ".forge-method/artifacts/smoke-game-sprint-plan.md"
Run $pythonExe $runtime artifact test-framework --root $tmp --path ".forge-method/artifacts/smoke-test-framework.md" --stack "PowerShell and Python runtime" --detected-framework "unittest plus PowerShell smoke scripts" --framework-detection "tests directory and scripts smoke-runtime.ps1 exist" --package-or-config-files "tests, scripts" --test-levels "unit, runtime smoke, install smoke" --fixture-architecture "temporary Forge projects exercise runtime behavior" --pure-helpers "runtime command helpers and fixture builders" --framework-wrappers "unittest subprocess wrapper and PowerShell Run helper" --composition-surface "tests compose runtime commands with temp project state" --cleanup-lifecycle "temporary directories are removed by test harness" --data-strategy "local file-backed fixtures" --semantic-locator-policy "not applicable for CLI smoke" --command-contract "python -m unittest discover -s tests; powershell smoke-runtime.ps1" --commands "python -m unittest discover -s tests" --first-checks "artifact generators and gate checks" --evidence-links ".forge-method/evidence/smoke-test-framework.md" --failure-repair-policy "fix command or artifact contract before widening coverage" --maintenance-rules "keep generator tests near validators" --limitations "browser UI checks live elsewhere" --next-workflow "test-automation" --eval
Run $pythonExe $runtime artifact test-check --root $tmp --path ".forge-method/artifacts/smoke-test-framework.md"
Run $pythonExe $runtime artifact test-automation --root $tmp --path ".forge-method/artifacts/smoke-test-automation.md" --framework "unittest and PowerShell" --target-behaviors "artifact generation and validation" --selected-scenarios "generate spec, research, game, and test artifacts" --risk-reason "generators can drift from validators" --risk-priority "runtime contract drift first" --test-level "unit and smoke" --api-checks "runtime CLI command assertions" --e2e-workflows "smoke-runtime reaches ready state" --fixtures "temporary Forge projects" --data-setup "fresh temp root per smoke" --semantic-locator-policy "CLI output text and artifact paths" --assertions "commands exit zero and artifacts validate" --visible-outcome-assertions "artifact check passed messages and gate output are visible" --independent-test-policy "each smoke creates isolated project state" --no-hardcoded-waits "true" --commands "powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-runtime.ps1" --evidence-links ".forge-method/evidence/smoke-test-automation.md" --run-and-fix-result "smoke command passed in source checkout" --failure-repair-policy "repair failing command or record waiver before gate" --manual-remainders "none blocking" --gate-impact "quality gate consumes smoke evidence" --next-workflow "test-review" --eval
Run $pythonExe $runtime artifact test-check --root $tmp --path ".forge-method/artifacts/smoke-test-automation.md"
Run $pythonExe $runtime artifact game-e2e-scaffold --root $tmp --path ".forge-method/artifacts/smoke-game-e2e.md" --playable-slice "one room arcade smoke" --engine-profile "browser canvas placeholder runtime" --launch-command "npm run game:test" --smoke-path "launch scene, collect item, assert score" --setup-action-assertion-teardown "launch scene, move to collectible, assert score banner, reset state" --observable-success-signal "score banner is visible" --evidence-mode "command log plus screenshot when browser exists" --commands "npm run game:test" --evidence-links ".forge-method/evidence/smoke-game-e2e.md" --release-gate-link "release-readiness playable smoke gate" --failure-repair-policy "fix launch/action/assertion before readiness" --manual-remainders "feel tuning remains manual" --next-workflow "game-qa-review" --eval
Run $pythonExe $runtime artifact test-check --root $tmp --path ".forge-method/artifacts/smoke-game-e2e.md"
$docSource = Join-Path $tmp "docs\smoke-guide.md"
New-Item -ItemType Directory -Force -Path (Split-Path $docSource) | Out-Null
Set-Content -Path $docSource -Value "# Smoke Guide`n`nRuntime smoke source of truth." -Encoding UTF8
$docShardIndex = Join-Path $tmp "docs\smoke-guide\index.md"
New-Item -ItemType Directory -Force -Path (Split-Path $docShardIndex) | Out-Null
Set-Content -Path $docShardIndex -Value "# Smoke Guide Shards`n`n- [Smoke Guide](../smoke-guide.md)" -Encoding UTF8
Run $pythonExe $runtime artifact doc-index --root $tmp --path ".forge-method/artifacts/smoke-doc-index.md" --target-docs "docs" --indexed-docs "docs/smoke-guide.md" --source-of-truth "docs/smoke-guide.md" --navigation-rules "read docs/smoke-guide.md first, then follow shard index when context is tight" --changes-or-findings "smoke guide is the source of truth for this project" --stale-or-duplicate-notes "no duplicate source in smoke project" --stale-check "source hash and mtime verified by artifact doc-check" --next-workflow "editorial-review" --eval
Run $pythonExe $runtime artifact doc-check --root $tmp --path ".forge-method/artifacts/smoke-doc-index.md"
Run $pythonExe $runtime artifact doc-shard --root $tmp --path ".forge-method/artifacts/smoke-doc-shard.md" --target-docs "docs/smoke-guide.md" --source-of-truth "docs/smoke-guide.md" --generated-or-derived-docs "docs/smoke-guide/index.md" --shard-index "docs/smoke-guide/index.md" --original-doc-decision "keep" --precedence-rule "whole source document wins until archive decision" --changes-or-findings "shard index points future agents to focused docs" --stale-or-duplicate-notes "original kept with explicit smoke waiver" --stale-check "source hash and shard index verified" --stale-waiver "smoke keeps original and shards to prove precedence handling" --next-workflow "doc-index" --eval
Run $pythonExe $runtime artifact doc-check --root $tmp --path ".forge-method/artifacts/smoke-doc-shard.md"
Run $pythonExe $runtime artifact enterprise-track-map --root $tmp --path ".forge-method/artifacts/smoke-enterprise-track.md" --selected-module "software-builder" --scope "enterprise smoke release" --artifact-evidence-map "each required artifact names owner evidence gate consumer and waiver status" --readiness-gate "readiness-check then traceability-gate and release-readiness" --waiver-policy "waiver owner rationale revisit trigger and release impact required" --next-workflow "readiness-check" --eval
Run $pythonExe $runtime artifact enterprise-check --root $tmp --path ".forge-method/artifacts/smoke-enterprise-track.md"
Run $pythonExe $runtime artifact enterprise-readiness --root $tmp --path ".forge-method/artifacts/smoke-enterprise-readiness.md" --scope "enterprise smoke release" --enterprise-evidence-status "security privacy risk quality NFR and traceability evidence present" --nfr-evidence "nfr-evidence-audit links thresholds to release claims" --release-gate-impact "missing P0 evidence blocks release" --waivers "compliance-checklist waived by owner until SOC2 scope starts" --missing-or-weak-sources "none blocking in smoke" --next-workflow "traceability-gate" --eval
Run $pythonExe $runtime artifact enterprise-check --root $tmp --path ".forge-method/artifacts/smoke-enterprise-readiness.md"
Run $pythonExe $runtime artifact enterprise-release-gate --root $tmp --path ".forge-method/artifacts/smoke-enterprise-release.md" --scope "enterprise smoke release" --enterprise-evidence-status "required evidence passed and waiver accepted with owner" --gate-decision "hold if traceability evidence is missing" --release-gate-impact "release blocks on missing P0 evidence" --waivers "compliance-checklist waived by owner with revisit trigger" --next-workflow "ready-release" --eval
Run $pythonExe $runtime artifact enterprise-check --root $tmp --path ".forge-method/artifacts/smoke-enterprise-release.md"
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
