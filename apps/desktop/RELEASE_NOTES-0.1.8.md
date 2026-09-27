# Forge Desktop 0.1.8 alpha.1

This Windows x64 alpha makes the project workspace clearer before and after
there is a file to preview. It keeps the same Forge project authority and
Codex conversation behavior as 0.1.7.

## Change from 0.1.7

- Before any file is chosen, the real Forge project record appears beside the
  conversation, ahead of the compact empty preview. Its recorded phase,
  activity and next step are easier to find without scrolling past a large
  placeholder.
- After the person chooses a file or opens one from a completed Codex reply,
  the actual local preview moves ahead of the record. Visual, keyboard and
  reading order move together. Choosing a file focuses the preview heading;
  the file is still validated by the existing native project-bound command.
- The record header puts **Consultar registro** beside **Onde estamos** and
  removes duplicate introductory text. The status continues to say that the
  Forge record may lag behind the conversation. No phase or progress is
  invented by the UI.

## Verification and limits

The browser suite first reproduced the prior off-screen recorded activity at
1180 × 820, then passed after the change. It covers empty/loaded order,
keyboard focus, narrow layout, safe local preview and the existing conversation
journey. Eight Node tests, desktop-only locked offline `cargo check`, all 44
desktop Rust tests, formatting, strict desktop Clippy and `git diff --check`
passed. The release executable and installed candidate passed hidden native
WebView smokes with real Forge project and record readback; an inspected
1180 × 820 screenshot shows the actual recorded activity and the start of
the next step beside the empty conversation. Native file inspection,
message-to-preview behavior and selection in the actual Windows file dialog
passed. An existing real Codex user/reply pair resumed across installation
without a new Send. A fresh real Codex artifact turn was not repeated for
this UI package; it passed in 0.1.6 and the 0.1.7 safeguard was separately
verified. GitHub CI was not manually run.

The app still depends on separate compatible `forge-core` and authenticated
Codex CLI installations. The installer is unsigned and current-user only;
there is no in-app auto-updater. The local preview is read-only and does not
prove publication or interactive-site behavior. Full manual visual, contrast
and screen-reader acceptance remains unfinished. Model-specific tokens and
BRL cost are UNKNOWN.

One NSIS candidate was built at
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.8_x64-setup.exe`
(4,624,036 bytes; SHA-256
`733CA93C5D0389BAC67C7213BF5CB71B0AF8A3C384E5A59CFAE9578842F93CDC`).
Its `.sha256` sidecar sits beside it. The exact candidate installed silently
over 0.1.7 with exit 0. Installed version is 0.1.8; executable SHA-256 is
`B3BFB9CC7C13AEF235143A88EC958C4F81949B0EDA6CB09CB35BF00A70FF1BF0`.
It was not rebuilt after hashing.

## Publication readback — 2026-09-27

The prerelease is available at
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.8-alpha.1`
from tag `desktop-v0.1.8-alpha.1` at commit
`5769918876484d9aab0d573c9b578c7757249785`. GitHub reported the
installer asset as 4,624,036 bytes with the expected SHA-256. A fresh,
unauthenticated public download of the installer and checksum sidecar matched
the candidate byte-for-byte. That downloaded installer, not a rebuilt file,
silently installed over the local candidate with exit 0. Installed version
and executable hash matched the tested installation. The downloaded install
restored the pre-upgrade real Codex user/reply pair without a new Send. The
hidden native record, preview and actual Windows file-picker checks were run
on the same executable hash before public download, not redundantly rerun
after the identical reinstall. GitHub CI was not manually triggered.
