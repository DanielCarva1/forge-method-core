$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$source = Join-Path $repoRoot "skills\forge-method"
$targetRoot = Join-Path $HOME ".agents\skills"
$target = Join-Path $targetRoot "forge-method"

if (-not (Test-Path -LiteralPath $source)) {
  throw "Skill source not found: $source"
}

New-Item -ItemType Directory -Force -Path $targetRoot | Out-Null

$targetRootResolved = [System.IO.Path]::GetFullPath($targetRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
$targetResolved = [System.IO.Path]::GetFullPath($target)
$targetPrefix = $targetRootResolved + [System.IO.Path]::DirectorySeparatorChar
if (-not $targetResolved.StartsWith($targetPrefix, [System.StringComparison]::OrdinalIgnoreCase)) {
  throw "Refusing to install outside skill directory: $target"
}

if (Test-Path -LiteralPath $target) {
  Get-ChildItem -LiteralPath $target -Recurse -Force | ForEach-Object {
    $_.Attributes = $_.Attributes -band (-bnot [System.IO.FileAttributes]::ReadOnly)
  }
  (Get-Item -LiteralPath $target -Force).Attributes = `
    (Get-Item -LiteralPath $target -Force).Attributes -band (-bnot [System.IO.FileAttributes]::ReadOnly)
  Remove-Item -LiteralPath $target -Recurse -Force
}

Copy-Item -LiteralPath $source -Destination $target -Recurse

Write-Host "Installed Codex skill: $target"
Write-Host "Use in Codex: `$forge-method"
Write-Host "Verify: powershell -ExecutionPolicy Bypass -File `"$target\forge-method.ps1`" --help"
Write-Host "Start: ask Codex to run Forge Method in your project workspace."
