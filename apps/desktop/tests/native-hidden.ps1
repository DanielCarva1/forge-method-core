param()
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$temp = [System.IO.Path]::GetTempPath()
$id = [guid]::NewGuid().ToString('N')
$runner = Join-Path $temp "forge-native-hidden-$id.ps1"
$log = Join-Path $temp "forge-native-hidden-$id.log"
$testScript = if ($env:FORGE_NATIVE_TEST_SCRIPT -in @('upgrade-continuity.cjs', 'native-restart-guard.cjs', 'native-real-inflight.cjs', 'artifact-resume.cjs', 'native-result-shortcut.cjs', 'native-auth.cjs')) { $env:FORGE_NATIVE_TEST_SCRIPT } else { 'native.cjs' }
$runnerBody = @'
Set-Location '__REPO__'
& node apps/desktop/tests/__SCRIPT__ *> '__LOG__'
exit $LASTEXITCODE
'@.Replace('__REPO__', $repo.Replace("'", "''")).Replace('__LOG__', $log.Replace("'", "''")).Replace('__SCRIPT__', $testScript)
$runnerBody | Set-Content -LiteralPath $runner -Encoding UTF8

Add-Type -TypeDefinition @'
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class ForgeHiddenNativeTest {
  [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)]
  public struct STARTUPINFO {
    public int cb; public string lpReserved; public string lpDesktop; public string lpTitle;
    public int dwX; public int dwY; public int dwXSize; public int dwYSize;
    public int dwXCountChars; public int dwYCountChars; public int dwFillAttribute;
    public int dwFlags; public short wShowWindow; public short cbReserved2;
    public IntPtr lpReserved2; public IntPtr hStdInput; public IntPtr hStdOutput; public IntPtr hStdError;
  }
  [StructLayout(LayoutKind.Sequential)]
  public struct PROCESS_INFORMATION {
    public IntPtr hProcess; public IntPtr hThread; public int dwProcessId; public int dwThreadId;
  }
  [DllImport("user32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  public static extern IntPtr CreateDesktop(string name, string device, IntPtr mode, int flags, int access, IntPtr security);
  [DllImport("user32.dll", SetLastError=true)] public static extern bool CloseDesktop(IntPtr desktop);
  [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)]
  public static extern bool CreateProcess(string app, StringBuilder cmd, IntPtr processAttrs, IntPtr threadAttrs,
    bool inherit, int flags, IntPtr env, string dir, ref STARTUPINFO startup, out PROCESS_INFORMATION info);
  [DllImport("kernel32.dll")] public static extern uint WaitForSingleObject(IntPtr handle, uint milliseconds);
  [DllImport("kernel32.dll")] public static extern bool GetExitCodeProcess(IntPtr handle, out uint code);
  [DllImport("kernel32.dll")] public static extern bool CloseHandle(IntPtr handle);
  public static uint Run(string exe, string runner, uint timeoutMs) {
    var name = "ForgeHidden-" + Guid.NewGuid().ToString("N");
    var desktop = CreateDesktop(name, null, IntPtr.Zero, 0, 0x10000000, IntPtr.Zero);
    if (desktop == IntPtr.Zero) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    try {
      var startup = new STARTUPINFO(); startup.cb = Marshal.SizeOf(typeof(STARTUPINFO)); startup.lpDesktop = name;
      PROCESS_INFORMATION process;
      var cmd = new StringBuilder("\"" + exe + "\" -NoProfile -NonInteractive -ExecutionPolicy Bypass -File \"" + runner + "\"");
      if (!CreateProcess(exe, cmd, IntPtr.Zero, IntPtr.Zero, false, 0, IntPtr.Zero,
          System.IO.Path.GetDirectoryName(exe), ref startup, out process))
        throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
      CloseHandle(process.hThread);
      try {
        if (WaitForSingleObject(process.hProcess, timeoutMs) != 0) throw new Exception("Hidden native test timed out");
        uint code; GetExitCodeProcess(process.hProcess, out code); return code;
      } finally { CloseHandle(process.hProcess); }
    } finally { CloseDesktop(desktop); }
  }
}
'@

try {
  $powershell = Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe'
  $timeoutMs = if ($env:FORGE_TEST_ARTIFACT_JOURNEY -eq '1' -or $testScript -eq 'native-real-inflight.cjs') { [uint32]480000 } else { [uint32]300000 }
  $code = [ForgeHiddenNativeTest]::Run($powershell, $runner, $timeoutMs)
  Get-Content -LiteralPath $log
  if ($code -ne 0) { throw "Native hidden smoke failed with exit code $code" }
} finally {
  Remove-Item -LiteralPath $runner -ErrorAction SilentlyContinue
  Remove-Item -LiteralPath $log -ErrorAction SilentlyContinue
}
