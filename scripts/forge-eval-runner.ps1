param(
  [string]$Root = ".",
  [string]$Mode = "local",
  [string]$Command,
  [int]$TimeoutSeconds = 600,
  [switch]$AllowDocker,
  [switch]$DryRun
)

$ErrorActionPreference = "Stop"
$resolvedRoot = (Resolve-Path -LiteralPath $Root).Path

if (-not $Command) {
  throw "Command is required. Pass -Command '<eval command>'."
}

if ($Mode -notin @("local", "docker")) {
  throw "Mode must be 'local' or 'docker'."
}

$contract = [ordered]@{
  root = $resolvedRoot
  mode = $Mode
  command = $Command
  timeout_seconds = $TimeoutSeconds
  dry_run = [bool]$DryRun
  allow_docker = [bool]$AllowDocker
}

if ($DryRun) {
  $contract | ConvertTo-Json -Depth 4
  exit 0
}

if ($Mode -eq "docker" -and -not $AllowDocker) {
  throw "Docker mode is opt-in. Re-run with -AllowDocker after recording the isolation contract."
}

Push-Location $resolvedRoot
try {
  $scriptBlock = if ($Mode -eq "local") {
    {
      param($cmd, $cwd)
      Set-Location -LiteralPath $cwd
      powershell -NoProfile -ExecutionPolicy Bypass -Command $cmd
      if ($LASTEXITCODE -ne 0) {
        throw "Eval command exited with code $LASTEXITCODE."
      }
    }
  } else {
    {
      param($cmd, $cwd)
      docker run --rm -v "${cwd}:/workspace" -w /workspace mcr.microsoft.com/powershell:latest pwsh -NoProfile -Command $cmd
      if ($LASTEXITCODE -ne 0) {
        throw "Docker eval command exited with code $LASTEXITCODE."
      }
    }
  }

  $job = Start-Job -ScriptBlock $scriptBlock -ArgumentList $Command, $resolvedRoot
  if (-not (Wait-Job $job -Timeout $TimeoutSeconds)) {
    Stop-Job $job | Out-Null
    Remove-Job $job -Force
    throw "Eval command timed out after $TimeoutSeconds seconds."
  }

  Receive-Job $job
  $exitCode = if ($job.State -eq "Failed") { 1 } else { 0 }
  Remove-Job $job -Force
  exit $exitCode
}
finally {
  Pop-Location
}
