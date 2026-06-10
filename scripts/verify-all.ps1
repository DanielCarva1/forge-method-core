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

Run python -m unittest discover -s tests
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1
if ($LASTEXITCODE -ne 0) {
  throw "smoke-runtime.ps1 failed"
}
powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1
if ($LASTEXITCODE -ne 0) {
  throw "smoke-install.ps1 failed"
}
Run python skills\forge-method\scripts\forge_method_runtime.py workflow validate

Write-Host "All verification checks passed."

