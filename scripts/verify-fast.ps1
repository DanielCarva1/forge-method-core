param(
  [string[]]$Test = @(),
  [string[]]$Match = @(),
  [switch]$SkipUnit,
  [switch]$Debug,
  [switch]$NoReport,
  [string]$ReportPath = "",
  [string]$JunitPath = "",
  [int]$Workers = 0,
  [int]$TimeoutSeconds = 90
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
  throw "Python not found. Set PYTHON to a Python executable."
}

$pythonExe = Resolve-Python

function Split-List {
  param([string[]]$Values)
  $items = @()
  foreach ($value in $Values) {
    foreach ($item in ($value -split ",")) {
      $trimmed = $item.Trim()
      if ($trimmed) {
        $items += $trimmed
      }
    }
  }
  return $items
}

$normalizedTests = Split-List $Test
$normalizedMatches = Split-List $Match

if (-not $NoReport -and -not $ReportPath) {
  $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
  $ReportPath = Join-Path ".forge-method\test-runs" "verify-fast-$stamp.json"
}

if ($Workers -le 0) {
  if ($Debug) {
    $Workers = 1
  } else {
    $Workers = [Math]::Min(4, [Math]::Max(1, [Environment]::ProcessorCount))
  }
}

function Runner-Args {
  $args = @("scripts\test-runner.py", "--workers", "$Workers", "--timeout", "$TimeoutSeconds")
  if ($Debug) {
    $args += "--debug"
  }
  if ($ReportPath) {
    $args += @("--report", $ReportPath)
  }
  if ($JunitPath) {
    $args += @("--junit", $JunitPath)
  }
  foreach ($case in $normalizedTests) {
    $args += @("--test", $case)
  }
  foreach ($match in $normalizedMatches) {
    $args += @("--match", $match)
  }
  return $args
}

if (-not $SkipUnit) {
  $runnerArgs = Runner-Args
  Run $pythonExe @runnerArgs
}
Run $pythonExe scripts\verify-onboarding-assets.py
Run $pythonExe skills\forge-method\scripts\forge_method_runtime.py workflow validate
Run $pythonExe skills\forge-method\scripts\forge_method_runtime.py agent validate

Write-Host "Fast verification checks passed."
