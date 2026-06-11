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

Run $pythonExe -m unittest discover -s tests
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
if ($LASTEXITCODE -ne 0) {
  throw "smoke-runtime.ps1 failed"
}
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
if ($LASTEXITCODE -ne 0) {
  throw "smoke-install.ps1 failed"
}
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-plugin-local.ps1
if ($LASTEXITCODE -ne 0) {
  throw "smoke-plugin-local.ps1 failed"
}
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-fixtures.ps1
if ($LASTEXITCODE -ne 0) {
  throw "smoke-fixtures.ps1 failed"
}
Run $pythonExe skills\forge-method\scripts\forge_method_runtime.py workflow validate

Write-Host "All verification checks passed."
