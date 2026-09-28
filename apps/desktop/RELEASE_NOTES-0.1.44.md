# Forge Desktop 0.1.44 alpha.1

Returning to a project now restores the last file selected in **Prévia**.
Forge keeps only that file path as a local shortcut, not a copy of its content.
Every return reads the file again through the native, project-bound preview
check. A missing or rejected file shows a clear empty state instead of an old
result, and removing a project from the local shortcuts also forgets its
preview shortcut. Another project's result cannot appear during a switch.

## Verification and limits

The complete browser UI suite covered a fresh page, project switch, rejected
remembered path, and shortcut removal. A hidden native Windows WebView test
reopened a temporary project after both a reload and a full app process
restart, then re-read its selected result from the project folder. No real
Codex turn or user project file was changed in this package. The final
installed-package and public-download checks are recorded below when complete.

This is a Windows x64 unsigned alpha, without an auto-updater. The bundled
Forge core remains pinned to 0.13.2. Clean-machine setup, physical mobile
access, and manual screen-reader acceptance remain unverified. The app does
not automatically reopen a preview window or infer which file the agent
meant; it restores only a file the user actually opened in that project.
Per-model tokens, BRL-equivalent cost and Pro quota attribution remain
UNKNOWN; no savings are claimed. No manual GitHub CI was triggered.

## Candidate and publication

One NSIS candidate was built and installed silently over public 0.1.43:
`Forge_0.1.44_x64-setup.exe`, 123,091,825 bytes, SHA-256
`60873230D44C0D04A71C3E6BF46E265BF2F591D6104FC2E79516BE124344D83D`.
The installed executable reports ProductVersion 0.1.44, SHA-256
`72BFB98A1EEC5432E61CD7E455BDBB7DA5112A76BCE33ECFC14D57C1D4F91C9D`.
The candidate remained byte-identical after installation. Hidden native smoke
of the installed package passed, including a full process restart and a new
project-bound read of the selected result. Package commit `fb86808a` was
pushed and tagged `desktop-v0.1.44-alpha.1`. An unauthenticated public
download returned the same 123,091,825 bytes and SHA-256 as the tested
candidate and published sidecar. Silent installation of those downloaded
bytes over the candidate returned exit 0, preserved the downloaded file, and
installed the same 0.1.44 executable hash. The downloaded installation
passed hidden native smoke again, including the full process restart and
fresh file read. A source commit alone does not update the app.
