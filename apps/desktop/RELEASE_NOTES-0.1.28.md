# Forge Desktop 0.1.28 alpha.1

This Windows x64 alpha makes generated files easier to find and discuss even
when Forge cannot display their contents. Selecting a non-previewable file
inside the current project, such as a PDF, now shows a local file card instead
of a dead-end preview error. The card identifies the verified project file and
offers **Conversar sobre este arquivo**, which prepares a draft in the same
conversation without sending it. Common document, archive and media file
references in Codex replies can also be selected. The app does not read or
execute unsupported file contents, and files outside the project remain
rejected.

The preview of supported text, images and protected local HTML remains
unchanged. A local file card is not a rendered document or proof of internet
publication. Forge still owns project state; Codex still owns conversation
history. No new registry, provider or governance engine was added.

## Verification

- Desktop preview tests and the complete Desktop crate passed (50 tests
  passed, one optional Windows browser-association test ignored), as did the
  frontend unit and browser UI suites.
- The source-built Windows app passed hidden native onboarding and preview
  smoke. A controlled local PDF was selected with the actual Windows file
  dialog on an isolated desktop; the app showed the file card and prepared a
  draft without a Send or external file launch.
- A separate hidden native read-only test reopened an existing real
  33-message Codex conversation, displayed its actual HTML result, prepared
  a change request and survived a full process restart without sending or
  editing the file.

## Limits

- PDF, Office, media and other unsupported files are identified, **not
  rendered or opened**. The user can ask Codex about them in the same chat.
- The controlled PDF fixture is not a new real Codex response. A fresh
  provider reply citing one of these formats has not been tested.
- The external-browser action, fresh-account login, clean-machine
  installation and manual screen-reader acceptance have not been verified
  end-to-end. Mobile and self-contained offline distribution remain future
  work.
- A previously observed hidden native timeout when both folder and file
  dialogs were exercised in one run has not been explained. Separate real
  dialog checks passed; this release does not claim the intermittent cause
  is fixed.
- There is no automatic updater or code signature. Install the NSIS package
  over 0.1.27. The package includes pinned Forge core 0.13.2 and Codex CLI;
  WebView2 can be downloaded by the installer if missing.
- Per-task subscription cost and an Astra/Sol/Luna savings comparison remain
  unknown because attributable model usage counters are unavailable.

## Tested candidate

The single NSIS candidate is `Forge_0.1.28_x64-setup.exe`, 123,102,085 bytes,
SHA-256 `C8ED4DD0F4B87027FA3C66DCD2715E44C7CEA8D0390A6D58E107ED2B66DDC5B5`.
Silent installation over public 0.1.27 returned exit code 0. The installed
0.1.28 executable has SHA-256
`F03422B1172DE05AE433BF9FEDD5FE6FFC5C40AAEE2CCF557C0751D0EB71750B`.
The installed candidate passed hidden native PDF-picker and read-only
real-result/restart tests. Public download continuity is checked separately
after publication; these local results alone do not prove that the release
asset is available.
