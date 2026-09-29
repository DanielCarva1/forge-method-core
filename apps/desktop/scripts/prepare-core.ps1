$ErrorActionPreference = 'Stop'

# Package the Core from this checkout, not from a machine-wide installation or
# an older release. Desktop and Core have independent version numbers.
$desktop = Split-Path -Parent $PSScriptRoot
$root = (Resolve-Path (Join-Path $desktop '..\..')).Path
$skill = Join-Path $root 'skill\start-forge\SKILL.md'
$expectedSkill = 'C4EA074BADC24B420170ABAFC894807432A57F553D453865B4CC448E62579E49'
function Get-Sha256([string]$path) {
  $stream = [System.IO.File]::OpenRead($path)
  $sha = [System.Security.Cryptography.SHA256]::Create()
  try { return [System.BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '') }
  finally { $sha.Dispose(); $stream.Dispose() }
}
if (-not (Test-Path -LiteralPath $skill) -or
    (Get-Sha256 $skill) -ne $expectedSkill) {
  throw 'The Start Forge guidance differs from the reviewed 0.13.3 source.'
}

$targetRoot = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else {
  Push-Location $root
  try {
    $metadata = cargo metadata --locked --offline --no-deps --format-version 1 | ConvertFrom-Json
    if ($LASTEXITCODE -ne 0 -or -not $metadata.target_directory) { throw 'Could not resolve the Cargo target directory.' }
    $metadata.target_directory
  } finally { Pop-Location }
}
$binary = Join-Path $targetRoot 'release\forge-core.exe'
Push-Location $root
try {
  cargo build --release --locked --offline -p forge-core-cli --bin forge-core
  if ($LASTEXITCODE -ne 0) { throw 'Could not build the Forge Core for the Desktop package.' }
} finally { Pop-Location }
if (-not (Test-Path -LiteralPath $binary)) { throw 'The built Forge Core executable is missing.' }
$version = & $binary --version
if ($LASTEXITCODE -ne 0 -or $version -ne 'forge-core 0.13.3') {
  throw "The built Forge Core version is not 0.13.3: $version"
}
$destination = Join-Path $desktop 'src-tauri\bundled-core\forge-core.exe'
New-Item -ItemType Directory -Force -Path (Split-Path -Parent $destination) | Out-Null
Copy-Item -LiteralPath $binary -Destination $destination -Force
$sourceRevision = (git -C $root rev-parse --short=12 HEAD).Trim()
Write-Output "Staged Forge Core 0.13.3 from checkout ${sourceRevision}: $(Get-Sha256 $destination)"
