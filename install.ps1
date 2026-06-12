$ErrorActionPreference = "Stop"

$repoRoot = Split-Path -Parent $MyInvocation.MyCommand.Path
$skillsSource = Join-Path $repoRoot "skills"
$targetRoot = Join-Path $HOME ".agents\skills"
$skillNames = @("forge-method", "forge-reload")

if (-not (Test-Path -LiteralPath $skillsSource)) {
  throw "Skills source not found: $skillsSource"
}

New-Item -ItemType Directory -Force -Path $targetRoot | Out-Null

$targetRootResolved = [System.IO.Path]::GetFullPath($targetRoot).TrimEnd([System.IO.Path]::DirectorySeparatorChar)
$targetPrefix = $targetRootResolved + [System.IO.Path]::DirectorySeparatorChar
foreach ($skillName in $skillNames) {
  $source = Join-Path $skillsSource $skillName
  $target = Join-Path $targetRoot $skillName
  if (-not (Test-Path -LiteralPath $source)) {
    throw "Skill source not found: $source"
  }
  $targetResolved = [System.IO.Path]::GetFullPath($target)
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
}

Write-Host "Use in Codex: `$forge-method"
Write-Host "Emergency reload: `$forge-reload"
Write-Host "Verify: powershell -ExecutionPolicy Bypass -File `"$targetRoot\forge-method\forge-method.ps1`" --help"
Write-Host "Start: ask Codex to run Forge Method in your project workspace."
