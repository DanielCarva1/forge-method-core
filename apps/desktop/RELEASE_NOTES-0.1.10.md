# Forge Desktop 0.1.10 alpha.1

This Windows x64 alpha makes the local-result review clearer when a Codex
answer cites several files. It does not change Forge's project authority,
Codex's conversation history or the preview's security boundary.

## Change from 0.1.9

- A completed answer citing two or more distinct supported local files now
  offers a compact list beside the preview. The person chooses; the app does
  not guess which file is the result. Each choice still goes through native
  project-bound validation. Lists of more than 20 point back to the reply for
  additional citations.
- The list remains available after viewing a file and clears when another
  conversation or project opens. The existing single-file shortcut remains.
- The enlarged preview shows site-specific restrictions only for local HTML,
  not for image, text or Markdown files. Main navigation and two project
  disclosures now keep the design's 48px minimum hit target.

## Verification and limits

The controlled browser suite, eight Node tests and focused hidden native
multi-file/full-process-restart test passed on this UI source. A separate
hidden native journey with authenticated Codex CLI 0.157.1 created local
HTML, opened it in the isolated preview, changed it in the same conversation
and restored the conversation after a WebView reload. That real journey did
not test multiple citations or a full process restart; those cases used a
controlled Codex-history fixture with real native file validation. The
native screenshot of the multi-file state was visually reviewed.

The app still depends on separate compatible `forge-core` and authenticated
Codex CLI installations. The default CLI found on the maintainer's machine
is older than the verified 0.157.1, so this is not a clean-machine setup
claim. The installer is unsigned and current-user only, with no in-app
auto-updater. Local preview does not execute scripts or verify publication.
Complete manual screen-reader/contrast acceptance, self-contained setup and
mobile remain unfinished. Model-specific tokens and BRL cost are UNKNOWN.
GitHub CI is not manually run for each UI change.

## Candidate and installed upgrade

One NSIS installer candidate was built at
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.10_x64-setup.exe`
(4,621,292 bytes; SHA-256
`4327DC375F0374E14A65531E0E36335CAA4CC8551962178B83BD3A44B474F8C6`).
Its checksum sidecar is beside it. The exact release executable passed the
focused hidden native full-process restart/multi-file test. The same candidate
silently upgraded the local 0.1.9 installation with exit 0. Installed version
is 0.1.10 and executable SHA-256 is
`2B1A87BE769DAF86A2ABD255A6A8C2089B76C86B1B7E5E0FBBFBD8254E049533`.
The installed candidate passed the same focused hidden native test. No
installer rebuild followed the hash. A fresh real Codex turn and real-history
resume on this installed binary were not run; the real create/change journey
was run on the same UI source before the metadata bump.

## Publication readback — 2026-09-27

The prerelease is available at
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.10-alpha.1`
from annotated tag `desktop-v0.1.10-alpha.1`, which resolves to source commit
`53c0baafb5fe206219945bc6042a362f6247f4a0`. GitHub reported the
installer asset as 4,621,292 bytes with the expected SHA-256. A fresh
unauthenticated public download to
`C:/ForgeFast/forge-alpha-0110-public-20260927` matched the candidate's size,
hash and checksum sidecar. That downloaded file silently reinstalled with
exit 0; installed version and executable hash matched the tested candidate.
The installed downloaded bytes passed the focused hidden native full-process
restart, local-file preview, multiple-citation selection, prepared change and
outside-path rejection test. A real Codex create/change journey was run on
the same UI source before the version bump, not repeated on the downloaded
installer. GitHub CI was not manually triggered.
