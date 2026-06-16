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
Run $pythonExe $installedRuntime artifact spec-kernel --root $exampleTmp --path ".forge-method/artifacts/install-spec-kernel.md" --source-artifacts ".forge-method/artifacts/example-brief.md" --why "Installed runtime needs a compact spec kernel proof before downstream workflows." --capabilities "CAP-1 intent: installed runtime can generate a spec kernel; success: spec-check validates the installed artifact" --constraints "Use installed skill files and local project state only." --non-goals "No external services, deployment, or implementation story generation." --success-signal "The installed artifact passes spec-check and gate." --preservation-map "source claim absorbed from example brief into CAP-1; install proof remains in smoke output" --next-workflow "plan-sprint" --eval
Run $pythonExe $installedRuntime artifact spec-check --root $exampleTmp --path ".forge-method/artifacts/install-spec-kernel.md"
Run $pythonExe $installedRuntime artifact research-scan --root $exampleTmp --path ".forge-method/artifacts/install-market-scan.md" --workflow market-scan --research-question "Can the installed runtime generate a market scan artifact?" --decision-to-unlock "decide whether installed guidance can close research before planning" --claim "Installed Forge can preserve compact research evidence without source checkout state." --sources "installed runtime command output, packaged workflow docs, generated artifact files" --source-gaps "no external market data in install smoke" --evidence-grade "recency current, authority high, directness high, bias noted" --findings "installed commands generate and validate a market scan from local inputs." --contradictions-or-falsifiers "if the installed command is missing, package the runtime before release." --uncertainty "the smoke proves packaging, not real market demand." --stance "continue with packaging proof explicit" --alternatives "manual markdown, source checkout command, stale installed skill" --adoption-friction "stale local installs and missing command discoverability" --demand-signal "user complaints showed installed guidance must be reliable" --next-workflow "research-closeout" --eval
Run $pythonExe $installedRuntime artifact research-check --root $exampleTmp --path ".forge-method/artifacts/install-market-scan.md"
Run $pythonExe $installedRuntime artifact game-brief --root $exampleTmp --path ".forge-method/artifacts/install-game-brief.md" --source-material "installed smoke prompt and runtime command output" --player-fantasy "Guide a tiny arcade hero through one readable room." --core-loop "move, dodge, collect, score, restart with clearer timing" --player-verbs "move, dodge, collect, score" --target-player "solo browser game player" --platform-or-engine "browser canvas prototype" --pillars "readable input, immediate feedback, tiny playable loop" --references "classic arcade room, lightweight browser prototype" --mvp-playable-proof "player can move through one room and collect one item" --dream-game "full arcade campaign with levels and powerups" --vertical-slice "one room with one collectible and failure feedback" --playable-slice "player moves, dodges, collects, and sees score feedback" --parked-scope "campaign, editor, multiplayer, advanced art" --rejected-directions "engine architecture before playable room" --decision-log "installed smoke game brief generated from local command" --assumptions "placeholder art and keyboard input" --open-questions "none blocking" --research-needed "none blocking" --next-workflow "game-sprint-planning" --eval
Run $pythonExe $installedRuntime artifact game-check --root $exampleTmp --path ".forge-method/artifacts/install-game-brief.md"
Run $pythonExe $installedRuntime artifact game-sprint-plan --root $exampleTmp --path ".forge-method/artifacts/install-game-sprint-plan.md" --source-material ".forge-method/artifacts/install-game-brief.md" --player-fantasy "Guide a tiny arcade hero through one readable room." --playable-slice "player moves, dodges, collects, and sees score feedback" --playable-slice-goal "one room proves input, feedback, and score loop" --decision-sources ".forge-method/artifacts/install-game-brief.md" --story-batch "input movement, collectible, hazard feedback, score display" --player-value-order "movement before score display" --risk-order "input feel before decorative polish" --dependencies "movement before collectible collision" --engine-or-asset-constraints "browser canvas with placeholder art" --validation-plan "manual smoke play plus artifact game-check" --manual-playtest-plan "player completes one room and sees score feedback" --deferred-scope "campaign, editor, multiplayer" --blocked-items "none blocking" --next-story "story-input-movement" --sprint-update "first playable slice ready for story creation" --next-workflow "game-story-creation" --eval
Run $pythonExe $installedRuntime artifact game-check --root $exampleTmp --path ".forge-method/artifacts/install-game-sprint-plan.md"
Run $pythonExe $installedRuntime artifact test-framework --root $exampleTmp --path ".forge-method/artifacts/install-test-framework.md" --stack "Installed Forge runtime" --detected-framework "unittest plus PowerShell smoke scripts" --framework-detection "installed runtime exposes tests and smoke commands" --package-or-config-files "tests, scripts" --test-levels "unit, runtime smoke, install smoke" --fixture-architecture "temporary Forge projects exercise installed runtime behavior" --pure-helpers "runtime command helpers and fixture builders" --framework-wrappers "unittest subprocess wrapper and PowerShell Run helper" --composition-surface "tests compose installed runtime commands with temp project state" --cleanup-lifecycle "temporary directories are removed by test harness" --data-strategy "local file-backed fixtures" --semantic-locator-policy "not applicable for CLI smoke" --command-contract "python -m unittest discover -s tests; powershell smoke-install.ps1" --commands "powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1" --first-checks "installed artifact generators and gate checks" --evidence-links ".forge-method/evidence/install-test-framework.md" --failure-repair-policy "fix installed command or artifact contract before release" --maintenance-rules "keep install smoke aligned with source smoke" --limitations "browser UI checks live elsewhere" --next-workflow "test-automation" --eval
Run $pythonExe $installedRuntime artifact test-check --root $exampleTmp --path ".forge-method/artifacts/install-test-framework.md"
Run $pythonExe $installedRuntime artifact test-automation --root $exampleTmp --path ".forge-method/artifacts/install-test-automation.md" --framework "unittest and PowerShell" --target-behaviors "installed artifact generation and validation" --selected-scenarios "generate spec, research, game, and test artifacts from installed skill" --risk-reason "installed skill can go stale" --risk-priority "installed package parity first" --test-level "install smoke" --api-checks "installed runtime CLI command assertions" --e2e-workflows "smoke-install reaches gate checks" --fixtures "temporary installed Forge projects" --data-setup "fresh temp root per smoke" --semantic-locator-policy "CLI output text and artifact paths" --assertions "commands exit zero and artifacts validate" --visible-outcome-assertions "artifact check passed messages and gate output are visible" --independent-test-policy "each smoke creates isolated project state" --no-hardcoded-waits "true" --commands "powershell -ExecutionPolicy Bypass -File .\\scripts\\smoke-install.ps1" --evidence-links ".forge-method/evidence/install-test-automation.md" --run-and-fix-result "install smoke command passed with installed skill" --failure-repair-policy "repair installed command or record waiver before gate" --manual-remainders "none blocking" --gate-impact "install smoke blocks release if generator is missing" --next-workflow "test-review" --eval
Run $pythonExe $installedRuntime artifact test-check --root $exampleTmp --path ".forge-method/artifacts/install-test-automation.md"
Run $pythonExe $installedRuntime artifact game-e2e-scaffold --root $exampleTmp --path ".forge-method/artifacts/install-game-e2e.md" --playable-slice "one room arcade smoke" --engine-profile "browser canvas placeholder runtime" --launch-command "npm run game:test" --smoke-path "launch scene, collect item, assert score" --setup-action-assertion-teardown "launch scene, move to collectible, assert score banner, reset state" --observable-success-signal "score banner is visible" --evidence-mode "command log plus screenshot when browser exists" --commands "npm run game:test" --evidence-links ".forge-method/evidence/install-game-e2e.md" --release-gate-link "release-readiness playable smoke gate" --failure-repair-policy "fix launch/action/assertion before readiness" --manual-remainders "feel tuning remains manual" --next-workflow "game-qa-review" --eval
Run $pythonExe $installedRuntime artifact test-check --root $exampleTmp --path ".forge-method/artifacts/install-game-e2e.md"
$installedDocSource = Join-Path $exampleTmp "docs\install-guide.md"
New-Item -ItemType Directory -Force -Path (Split-Path $installedDocSource) | Out-Null
Set-Content -Path $installedDocSource -Value "# Install Guide`n`nInstalled runtime smoke source of truth." -Encoding UTF8
$installedDocShardIndex = Join-Path $exampleTmp "docs\install-guide\index.md"
New-Item -ItemType Directory -Force -Path (Split-Path $installedDocShardIndex) | Out-Null
Set-Content -Path $installedDocShardIndex -Value "# Install Guide Shards`n`n- [Install Guide](../install-guide.md)" -Encoding UTF8
Run $pythonExe $installedRuntime artifact doc-index --root $exampleTmp --path ".forge-method/artifacts/install-doc-index.md" --target-docs "docs" --indexed-docs "docs/install-guide.md" --source-of-truth "docs/install-guide.md" --navigation-rules "read docs/install-guide.md first, then follow shard index when context is tight" --changes-or-findings "install guide is the source of truth for this smoke project" --stale-or-duplicate-notes "no duplicate source in installed smoke project" --stale-check "source hash and mtime verified by artifact doc-check" --next-workflow "editorial-review" --eval
Run $pythonExe $installedRuntime artifact doc-check --root $exampleTmp --path ".forge-method/artifacts/install-doc-index.md"
Run $pythonExe $installedRuntime artifact doc-shard --root $exampleTmp --path ".forge-method/artifacts/install-doc-shard.md" --target-docs "docs/install-guide.md" --source-of-truth "docs/install-guide.md" --generated-or-derived-docs "docs/install-guide/index.md" --shard-index "docs/install-guide/index.md" --original-doc-decision "keep" --precedence-rule "whole source document wins until archive decision" --changes-or-findings "shard index points future agents to focused docs" --stale-or-duplicate-notes "original kept with explicit install waiver" --stale-check "source hash and shard index verified" --stale-waiver "install smoke keeps original and shards to prove precedence handling" --next-workflow "doc-index" --eval
Run $pythonExe $installedRuntime artifact doc-check --root $exampleTmp --path ".forge-method/artifacts/install-doc-shard.md"
Run $pythonExe $installedRuntime artifact enterprise-track-map --root $exampleTmp --path ".forge-method/artifacts/install-enterprise-track.md" --selected-module "software-builder" --scope "enterprise install release" --artifact-evidence-map "each required artifact names owner evidence gate consumer and waiver status" --readiness-gate "readiness-check then traceability-gate and release-readiness" --waiver-policy "waiver owner rationale revisit trigger and release impact required" --next-workflow "readiness-check" --eval
Run $pythonExe $installedRuntime artifact enterprise-check --root $exampleTmp --path ".forge-method/artifacts/install-enterprise-track.md"
Run $pythonExe $installedRuntime artifact enterprise-readiness --root $exampleTmp --path ".forge-method/artifacts/install-enterprise-readiness.md" --scope "enterprise install release" --enterprise-evidence-status "security privacy risk quality NFR and traceability evidence present" --nfr-evidence "nfr-evidence-audit links thresholds to release claims" --release-gate-impact "missing P0 evidence blocks release" --waivers "compliance-checklist waived by owner until SOC2 scope starts" --missing-or-weak-sources "none blocking in install smoke" --next-workflow "traceability-gate" --eval
Run $pythonExe $installedRuntime artifact enterprise-check --root $exampleTmp --path ".forge-method/artifacts/install-enterprise-readiness.md"
Run $pythonExe $installedRuntime artifact enterprise-release-gate --root $exampleTmp --path ".forge-method/artifacts/install-enterprise-release.md" --scope "enterprise install release" --enterprise-evidence-status "required evidence passed and waiver accepted with owner" --gate-decision "hold if traceability evidence is missing" --release-gate-impact "release blocks on missing P0 evidence" --waivers "compliance-checklist waived by owner with revisit trigger" --next-workflow "ready-release" --eval
Run $pythonExe $installedRuntime artifact enterprise-check --root $exampleTmp --path ".forge-method/artifacts/install-enterprise-release.md"
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
Assert-Contains $installedProjectGuideText "First question: who is it for, what should change for them, what is fixed or out, what is still open, and what proof should close discovery?" "installed project first answer guide output"
Assert-NotContains $installedProjectGuideText "Prompt: Let's use" "installed project first answer guide output"
Assert-NotContains $installedProjectGuideText "build-story" "installed project first answer guide output"
$installedBlockedTransitionText = Run-Fails-Capture $pythonExe $installedRuntime transition --root $generatedProjectTmp --phase 2-specification --status specification-ready --workflow write-spec
Assert-Contains $installedBlockedTransitionText "Discovery closeout required before specification" "installed project first answer transition guard output"
$installedProjectCloseoutText = Run-Capture $pythonExe $installedRuntime artifact discovery-closeout --root $generatedProjectTmp --path ".forge-method/artifacts/discovery-intent.md" --audience "independent teachers planning flexible lessons" --outcome "create a guided planning product that turns vague class ideas into reviewable plans" --constraints "browser-first prototype, no login in the first pass, preserve simple language" --non-goals "no scheduling marketplace, no automated grading, no implementation architecture yet" --success-signal "a teacher can produce a reviewable brief with constraints and proof in ten minutes" --open-questions "none blocking; pricing and collaboration can wait"
Assert-Contains $installedProjectCloseoutText ".forge-method/artifacts/discovery-intent.md" "installed project discovery closeout output"
Assert-Contains $installedProjectCloseoutText "Discovery closeout check passed." "installed project discovery closeout output"
Run $pythonExe $installedRuntime artifact discovery-check --root $generatedProjectTmp --path ".forge-method/artifacts/discovery-intent.md"
$installedProjectCloseoutTransitionText = Run-Capture $pythonExe $installedRuntime transition --root $generatedProjectTmp --phase 2-specification --status specification-ready --workflow write-spec
Assert-Contains $installedProjectCloseoutTransitionText "Transition written." "installed project discovery closeout transition output"
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
