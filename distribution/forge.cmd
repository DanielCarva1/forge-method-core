@echo off
REM Forge wrapper (Windows) - delegates to forge-core.exe in the same directory.
REM
REM The Forge Method skill (`start-forge`) looks for a `forge` command on PATH
REM first, falling back to `forge-core`. This wrapper lets releases ship a
REM single archive containing both names, so users do not have to set up an
REM alias or doskey themselves.
REM
REM Install (or extract) this file alongside `forge-core.exe`, e.g. under
REM %LOCALAPPDATA%\Programs\forge-core\, then add that directory to PATH.
REM
REM `%~dp0` resolves to the directory of this script (trailing backslash), so
REM the wrapper keeps working regardless of the current working directory.
"%~dp0forge-core.exe" %*
