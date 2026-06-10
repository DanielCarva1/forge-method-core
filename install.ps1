$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$source = Join-Path $repoRoot "skills\forge-method"
$targetRoot = Join-Path $HOME ".agents\skills"
$target = Join-Path $targetRoot "forge-method"

if (-not (Test-Path -LiteralPath $source)) {
  throw "Skill source not found: $source"
}

New-Item -ItemType Directory -Force -Path $targetRoot | Out-Null

if (Test-Path -LiteralPath $target) {
  Remove-Item -LiteralPath $target -Recurse -Force
}

Copy-Item -LiteralPath $source -Destination $target -Recurse

Write-Host "Installed Codex skill: $target"
Write-Host "Use in Codex: `$forge-method"

