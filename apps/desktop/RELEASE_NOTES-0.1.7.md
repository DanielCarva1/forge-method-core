# Forge Desktop 0.1.7 alpha.1

This Windows x64 alpha makes recovery after an uncertain message delivery
safer. It retains the existing project, Codex conversation and local-result
journey from 0.1.6; it does not claim every Forge capability has a screen.

## Change from 0.1.6

- If the app closes or loses the reply to a Send, the project-scoped warning
  survives restart. Reopening the same Codex conversation no longer treats
  mere resumption as proof that the message was delivered.
- Sending remains disabled until the person inspects the history and chooses
  **Já conferi o envio**. This action never resends the uncertain message.
  If the local warning cannot be cleared, sending stays blocked rather than
  silently losing the protection.
- The review control is visible only in this exceptional recovery state and
  returns keyboard focus to a status message when dismissed.

## Verification and limits

The desktop-only source checks passed: eight Node tests, the controlled
browser suite (including empty resumed history, reload, storage failure and
focus), locked offline desktop `cargo check`, all 44 desktop Rust tests,
formatting, strict desktop Clippy and `git diff --check`. The separate Forge
core workspace and GitHub CI were not run for this UI-only change.

A hidden native WebView test with a controlled Codex bridge restarted the
whole process during a pending Send and confirmed the warning persisted,
automatic replay/new conversation remained blocked, and only explicit review
enabled another Send. The installed recovery screen was visually inspected
from a hidden native capture; the review action, explanation and disabled
Send were visible without fabricated history. A separate hidden test with
**real authenticated Codex**
observed native acceptance of a harmless turn while deliberately withholding
the UI acknowledgement, then killed and restarted the app. The same thread
resumed without replay and the warning still blocked Send. In that run the
accepted prompt was **not visible** in the restored history: native acceptance
alone did not prove durable delivery. The test did not send it again.

The app still depends on separate compatible `forge-core` and authenticated
Codex CLI installations. The installer is unsigned and current-user only;
there is no in-app auto-updater. The local preview is read-only, scriptless,
and cannot prove publication. Manual complete visual/contrast/screen-reader
acceptance and real-world delivery behavior across other Codex versions remain
NOT_RUN. Model-specific token use and BRL cost are UNKNOWN.

One NSIS installer candidate was built at
`D:\forge-method-core-build-cache\main-target\release\bundle\nsis\Forge_0.1.7_x64-setup.exe`
(4,622,466 bytes; SHA-256
`BE0F80E5D0175DA05C90C97FEDC38BE588E9F6E80F00A9064AB0F588F02421B5`).
The release executable passed both hidden restart tests. The exact candidate
was silently installed over 0.1.6 with exit 0; installed version is 0.1.7,
executable SHA-256
`4958CE1F052EBD28703948782C3294CEDCB926FC24BBE83F26352D73786CF203`.
The installed binary restored a real Codex user/reply pair created before
this upgrade without a new Send. It also passed the controlled restart guard
and the real Codex lost-acknowledgement test. The installer was not rebuilt
after hashing.

## Publication readback — 2026-09-27

The prerelease is available at
`https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.7-alpha.1`
from tag `desktop-v0.1.7-alpha.1` at commit
`e2b1d2a7cda0f8bf4fd761162b285855409b356c`. GitHub reported the
installer asset as 4,622,466 bytes with the expected SHA-256. A fresh,
unauthenticated public download of the installer and checksum sidecar matched
the candidate byte-for-byte. That downloaded installer, not a rebuilt file,
silently installed over the local 0.1.7 candidate with exit 0. Installed
version and executable hash matched the previously tested installation.
The downloaded installation restored a pre-upgrade real Codex user/reply pair
without a new Send and passed the hidden native controlled uncertain-Send
restart guard. Real Codex lost-acknowledgement behavior was tested on the
same executable hash before publication; it was not repeated after the
identical public download. GitHub CI was not manually triggered.
