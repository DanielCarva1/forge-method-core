# Forge Desktop 0.1.34 alpha.1

This Windows x64 alpha clarifies two project journeys without changing the
Forge engine or creating another project record.

- A new Codex reply citing two or three distinct local files shows individual
  preview choices immediately. The app does not guess one result. Long lists
  and restored conversations stay folded; closing a list stays respected.
- A project without a recorded next step now has one clear status and a
  **Conversar sobre meu projeto** action. It opens the existing unsent composer,
  including in a narrow window. It does not send or invent project progress.

## Verification

The browser suite covers short cited-file lists, manual folding, absent and
present record states, and the no-Send shortcut. Hidden native Windows smoke
covers real folder onboarding, Forge record readback, the narrow shortcut, and
existing conversation/preview paths. Published 0.1.33 separately passed a real
Codex Send, local page, follow-up change, reload and process restart journey.
This UI-only package does not repeat that costly provider test; its new
cited-file behavior is fixture-tested, not another real-Codex claim.

## Limits

This remains an alpha Windows desktop app. The installer is unsigned and has
no auto-updater; install over the previous alpha. Clean-machine setup,
mobile-device use and manual screen-reader acceptance remain unverified. The
earlier intermittent provider/transport timeout is not claimed resolved.
Per-task model cost and Pro-quota savings remain unknown. The bundled Forge
core remains intentionally pinned to 0.13.2.

## Candidate and public readback

The single tested NSIS candidate is `Forge_0.1.34_x64-setup.exe`, 123,142,636
bytes, SHA-256
`D905C644CF8ECA72C3EC2B6053F51E9567DF06E889643FC0010DC2DA26692E22`.
Silent installation over public 0.1.33 returned exit 0; candidate bytes did
not change. The installed executable reports 0.1.34 and has SHA-256
`9881933D91ACEB74695974EB0081EC94426B4400A0BBF1DA970BAD82E807DB60`.
The installed candidate passed hidden native smoke, including actual Windows
folder selection and Forge onboarding. The published installer and sidecar
were downloaded without authentication; the installer was 123,142,636 bytes
with the same SHA-256 as both the candidate and sidecar. Silent installation
of those downloaded bytes returned exit 0, preserved the downloaded hash and
produced the installed executable hash above. Hidden native smoke passed again
after that installation. No rebuild or manual GitHub CI was run after the
candidate was tested.
