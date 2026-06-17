param(
  [string]$Root = ".",
  [Parameter(Mandatory=$true)][string]$Event,
  [string]$Payload = "{}",
  [int]$TimeoutSeconds = 120,
  [switch]$Run
)

$ErrorActionPreference = "Stop"
$resolvedRoot = (Resolve-Path -LiteralPath $Root).Path
$hookDir = Join-Path $resolvedRoot ".forge-method\hooks"
$hookPath = Join-Path $hookDir "$Event.ps1"

$contract = [ordered]@{
  root = $resolvedRoot
  event = $Event
  hook_path = $hookPath
  payload = $Payload
  timeout_seconds = $TimeoutSeconds
  run = [bool]$Run
  exists = Test-Path -LiteralPath $hookPath
}

if (-not $Run) {
  $contract | ConvertTo-Json -Depth 4
  exit 0
}

if (-not (Test-Path -LiteralPath $hookPath)) {
  throw "Hook not found: $hookPath"
}

$job = Start-Job -ScriptBlock {
  param($path, $root, $eventName, $payload)
  powershell -NoProfile -ExecutionPolicy Bypass -File $path -Root $root -Event $eventName -Payload $payload
  if ($LASTEXITCODE -ne 0) {
    throw "Hook '$eventName' exited with code $LASTEXITCODE."
  }
} -ArgumentList $hookPath, $resolvedRoot, $Event, $Payload

if (-not (Wait-Job $job -Timeout $TimeoutSeconds)) {
  Stop-Job $job | Out-Null
  throw "Hook '$Event' timed out after $TimeoutSeconds seconds."
}

Receive-Job $job
$exitCode = if ($job.State -eq "Failed") { 1 } else { 0 }
Remove-Job $job -Force
exit $exitCode
