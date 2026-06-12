param(
  [string]$RepoUrl = "https://github.com/DanielCarva1/forge-method-core.git",
  [string]$Ref,
  [string]$ExpectedVersion,
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

$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
if (-not $ExpectedVersion -and (Test-Path -LiteralPath (Join-Path $repoRoot "VERSION"))) {
  $ExpectedVersion = (Get-Content -LiteralPath (Join-Path $repoRoot "VERSION") -Raw).Trim()
}

if (-not $WorkRoot) {
  $WorkRoot = Join-Path $env:TEMP ("forge-method-plugin-clone-smoke-" + [guid]::NewGuid().ToString("N"))
}

$pythonExe = Resolve-Python -PythonPath $Python
$cloneRoot = Join-Path $WorkRoot "repo"
$pluginParent = Join-Path $WorkRoot "plugins"
$marketplacePath = Join-Path $WorkRoot ".agents\plugins\marketplace.json"
$pluginRoot = Join-Path $pluginParent "forge-method-core"
$projectParent = Join-Path $WorkRoot "projects"
$generatedProject = Join-Path $projectParent "clone-smoke"

if (Test-Path -LiteralPath $WorkRoot) {
  Remove-Item -LiteralPath $WorkRoot -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null
New-Item -ItemType Directory -Force -Path $projectParent | Out-Null

$cloneArgs = @("clone", "--depth", "1")
if ($Ref) {
  $cloneArgs += @("--branch", $Ref)
}
$cloneArgs += @($RepoUrl, $cloneRoot)
Run git @cloneArgs

$installer = Join-Path $cloneRoot "scripts\install-plugin-local.ps1"
if (-not (Test-Path -LiteralPath $installer)) {
  throw "Plugin installer missing from cloned source: $installer"
}

Run powershell -ExecutionPolicy Bypass -File $installer -PluginParent $pluginParent -MarketplacePath $marketplacePath

$manifestPath = Join-Path $pluginRoot ".codex-plugin\plugin.json"
$skillPath = Join-Path $pluginRoot "skills\forge-method\SKILL.md"
$runtimePath = Join-Path $pluginRoot "skills\forge-method\scripts\forge_method_runtime.py"

if (-not (Test-Path -LiteralPath $manifestPath)) {
  throw "Plugin manifest not installed: $manifestPath"
}
if (-not (Test-Path -LiteralPath $skillPath)) {
  throw "Forge Method skill not installed: $skillPath"
}
if (-not (Test-Path -LiteralPath $runtimePath)) {
  throw "Forge Method runtime not installed: $runtimePath"
}

$manifest = Get-Content -LiteralPath $manifestPath -Raw | ConvertFrom-Json
if ($manifest.name -ne "forge-method-core") {
  throw "Unexpected plugin name: $($manifest.name)"
}
if ($ExpectedVersion -and $manifest.version -ne $ExpectedVersion) {
  throw "Unexpected plugin version: $($manifest.version), expected $ExpectedVersion"
}

$marketplace = Get-Content -LiteralPath $marketplacePath -Raw | ConvertFrom-Json
$entry = @($marketplace.plugins) | Where-Object { $_.name -eq "forge-method-core" } | Select-Object -First 1
if (-not $entry) {
  throw "Marketplace entry missing: forge-method-core"
}
if ($entry.source.path -ne "./plugins/forge-method-core") {
  throw "Unexpected marketplace source path: $($entry.source.path)"
}

$preflight = & $pythonExe $runtimePath preflight --root $projectParent
if ($LASTEXITCODE -ne 0) {
  throw "preflight failed for installed cloned plugin"
}
if (($preflight -join "`n") -notmatch "Decision options:") {
  throw "preflight did not print decision options"
}

Run $pythonExe $runtimePath project create --root $projectParent --name "Clone Smoke" --module software-builder --objective "Verify cloned plugin installation."
Run $pythonExe $runtimePath preflight --root $projectParent
Run $pythonExe $runtimePath gate --root $generatedProject --require-evals

Write-Host "Plugin clone install smoke passed: $pluginRoot"
if ($Ref) {
  Write-Host "Ref: $Ref"
}
if ($ExpectedVersion) {
  Write-Host "Version: $ExpectedVersion"
}
