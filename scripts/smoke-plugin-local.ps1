$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$installer = Join-Path $repoRoot "scripts\install-plugin-local.ps1"
$tmp = Join-Path $env:TEMP "forge-method-plugin-smoke"
$pluginParent = Join-Path $tmp "plugins"
$marketplacePath = Join-Path $tmp ".agents\plugins\marketplace.json"
$pluginRoot = Join-Path $pluginParent "forge-method-core"

if (Test-Path -LiteralPath $tmp) {
  Remove-Item -LiteralPath $tmp -Recurse -Force
}
New-Item -ItemType Directory -Force -Path $tmp | Out-Null

powershell -ExecutionPolicy Bypass -File $installer -PluginParent $pluginParent -MarketplacePath $marketplacePath
if ($LASTEXITCODE -ne 0) {
  throw "Plugin local installer failed with exit code $LASTEXITCODE"
}

if (-not (Test-Path -LiteralPath (Join-Path $pluginRoot ".codex-plugin\plugin.json"))) {
  throw "Plugin manifest not copied to local plugin source."
}
if (-not (Test-Path -LiteralPath (Join-Path $pluginRoot "skills\forge-method\SKILL.md"))) {
  throw "Forge Method skill not copied to local plugin source."
}
if (-not (Test-Path -LiteralPath $marketplacePath)) {
  throw "Marketplace file not written."
}

$marketplace = Get-Content -LiteralPath $marketplacePath -Raw | ConvertFrom-Json
$entry = @($marketplace.plugins) | Where-Object { $_.name -eq "forge-method-core" } | Select-Object -First 1
if (-not $entry) {
  throw "Marketplace entry missing: forge-method-core"
}
if ($entry.source.path -ne "./plugins/forge-method-core") {
  throw "Unexpected marketplace source path: $($entry.source.path)"
}

Write-Host "Plugin local smoke passed: $pluginRoot"
