$ErrorActionPreference = 'Stop'

# The Desktop installer uses the proven public 0.13.2 Windows artifact. Do not
# silently switch to the current source-tree version or a machine installation.
$expectedArchive = '27976049225D8650758D2593B5CB06C0FC20870216383C16C7FBC70478AD23BB'
$expectedBinary = 'CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF'
$expectedSkill = '10581E17D5DBB98BDA3E0F3BC0B6A152736499451E1424E093DBECFAFD8F0B06'
function Get-Sha256([string]$path) {
  $stream = [System.IO.File]::OpenRead($path)
  $sha = [System.Security.Cryptography.SHA256]::Create()
  try { return [System.BitConverter]::ToString($sha.ComputeHash($stream)).Replace('-', '') }
  finally { $sha.Dispose(); $stream.Dispose() }
}
$desktop = Split-Path -Parent $PSScriptRoot
$skill = Join-Path $desktop 'third-party\start-forge-0.13.2\SKILL.md'
if (-not (Test-Path -LiteralPath $skill) -or (Get-Sha256 $skill) -ne $expectedSkill) {
  throw 'The bundled Start Forge guidance does not match the pinned 0.13.2 release.'
}
$targetDir = Join-Path $desktop 'src-tauri\bundled-core'
$target = Join-Path $targetDir 'forge-core.exe'
if ((Test-Path -LiteralPath $target) -and
    (Get-Sha256 $target) -eq $expectedBinary) {
  Write-Output 'Verified cached forge-core 0.13.2 binary.'
  exit 0
}

New-Item -ItemType Directory -Force -Path $targetDir | Out-Null
$archive = Join-Path $env:TEMP ("forge-core-0.13.2-$([guid]::NewGuid().ToString('N')).zip")
$staged = Join-Path $targetDir ("forge-core-$([guid]::NewGuid().ToString('N')).exe")
try {
  $url = 'https://github.com/DanielCarva1/forge-method-core/releases/download/v0.13.2/forge-core-x86_64-windows.zip'
  [System.Net.ServicePointManager]::SecurityProtocol = [System.Net.SecurityProtocolType]::Tls12
  $client = [System.Net.WebClient]::new()
  try { $client.DownloadFile($url, $archive) }
  finally { $client.Dispose() }
  if ((Get-Sha256 $archive) -ne $expectedArchive) {
    throw 'Downloaded forge-core archive has an unexpected SHA-256.'
  }
  Add-Type -AssemblyName System.IO.Compression.FileSystem
  $zip = [System.IO.Compression.ZipFile]::OpenRead($archive)
  try {
    $entry = $zip.GetEntry('forge-core.exe')
    if ($null -eq $entry) { throw 'The pinned archive has no forge-core.exe.' }
    $inputStream = $entry.Open()
    $outputStream = [System.IO.File]::Create($staged)
    try { $inputStream.CopyTo($outputStream) }
    finally { $outputStream.Dispose(); $inputStream.Dispose() }
  } finally { $zip.Dispose() }
  if ((Get-Sha256 $staged) -ne $expectedBinary) {
    throw 'Extracted forge-core executable has an unexpected SHA-256.'
  }
  Move-Item -LiteralPath $staged -Destination $target -Force
  Write-Output 'Staged verified forge-core 0.13.2 for the Windows installer.'
} finally {
  Remove-Item -LiteralPath $archive -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $staged -ErrorAction SilentlyContinue
}
