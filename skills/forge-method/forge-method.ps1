$ErrorActionPreference = "Stop"

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

$python = Resolve-Python
$updater = Join-Path $PSScriptRoot "scripts\forge_method_updater.py"
if (Test-Path -LiteralPath $updater) {
  & $python $updater --skill-dir $PSScriptRoot -- @args
}
$runtime = Join-Path $PSScriptRoot "scripts\forge_method_runtime.py"
& $python $runtime @args
exit $LASTEXITCODE
