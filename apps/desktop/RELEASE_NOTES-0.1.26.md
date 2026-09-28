# Forge Desktop 0.1.26 alpha.1

This Windows x64 alpha improves reading the first screens with enlarged text.
At narrow widths, the Home and Explore headings reflow without clipped letters,
and the appearance controls no longer cover the active screen. The Home
introduction stays readable at 200% text size. No Forge engine or conversation
protocol behavior changed.

## Verification

- The full browser UI suite passed, including a new 360px / 200% text check
  for Home, Explore, My Projects and My Conversation. It checks heading
  clipping, horizontal overflow and overlap with appearance controls.
- A source-built Windows WebView passed the same layout checks on an isolated
  desktop. Its existing no-project smoke also passed first-use draft retention,
  missing-folder guidance, appearance reload and frontend-to-Rust identity.
- A separate opt-in native read-only test reopened an existing real 33-message
  Codex conversation and its generated local HTML result after a full process
  restart. The test prepared, but did not send, a change request and verified
  that the result file was unchanged. This test was performed against installed
  0.1.25; the test harness is included in the source, not the installer.

## Limits

- This alpha does not claim complete accessibility conformance. Manual screen
  reader and accessibility acceptance remain unverified. The enlarged-text
  regression covers the stated four screens and layout conditions, not every
  possible OS scaling combination.
- The **Usar no navegador** button launching a normal external browser remains
  unverified end-to-end. The app avoids launching one during hidden tests to
  protect the user's active desktop.
- Fresh-account login with the real provider, clean-machine installation,
  mobile, and self-contained offline distribution remain unverified or
  unfinished. No automatic updater or signing is included; install the new
  NSIS package over 0.1.25.
- This release does not establish a per-task Pro subscription cost or a
  controlled Astra/Sol/Luna savings comparison.

## Candidate

The single locally tested candidate is `Forge_0.1.26_x64-setup.exe`,
123,088,230 bytes, SHA-256
`A5739B90347393B2F44F53D358622E9E5E0AF8E3A7D15248F32B95F257267DBA`.
Silent installation over 0.1.25 returned exit code 0 and did not change the
candidate bytes. The installed executable reports 0.1.26 with SHA-256
`AAC88FEAE37B81E6ACCBBAFC07B142E0B69DBEA0645F115BBBF50E2F8DEB85A9`.
The installed app passed the hidden native first-use/layout smoke; no real
provider turn was sent. A public download has not yet been checked.
