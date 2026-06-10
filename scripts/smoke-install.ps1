$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent (Split-Path -Parent $MyInvocation.MyCommand.Path)
$installer = Join-Path $repoRoot "install.ps1"
$installedRuntime = Join-Path $HOME ".agents\skills\forge-method\scripts\forge_method_runtime.py"
$tmp = Join-Path $env:TEMP "forge-method-install-smoke"

powershell -ExecutionPolicy Bypass -File $installer

if (-not (Test-Path -LiteralPath $installedRuntime)) {
  throw "Installed runtime helper not found: $installedRuntime"
}

if (Test-Path -LiteralPath $tmp) {
  Remove-Item -LiteralPath $tmp -Recurse -Force
}

New-Item -ItemType Directory -Path $tmp | Out-Null

python $installedRuntime --help | Out-Null
python $installedRuntime init --project install-smoke --root $tmp
python $installedRuntime status --root $tmp
python $installedRuntime next --root $tmp

Write-Host "Install smoke test passed: $tmp"

