# Forge Desktop 0.1.12 alpha.1

This Windows x64 alpha includes both the pinned Codex CLI 0.157.1 and a
verified, released `forge-core 0.13.2` executable. A person can choose a local
folder, start its Forge record, converse, preview a local result and request a
change without separately installing either CLI. Existing global installations
are not replaced. The core still owns project state; Codex still owns chat
history. This package does not silently upgrade either bundled runtime.

## Verification

- PASS: the build stages the public core release only after checking the
  Windows archive SHA-256 and the extracted executable SHA-256. A fresh staging
  pass was also tested after moving aside the cached copy. The NSIS archive
  contains both native runtimes and their notices/license.
- PASS: focused Desktop check and project tests, all 46 Desktop Rust tests,
  strict Desktop Clippy, eight Node tests, browser suite and release build. No
  core workspace build or manual GitHub CI run was needed for this package.
- PASS: the exact candidate silently upgraded local 0.1.11 with exit 0. The
  installed binary reports Desktop 0.1.12, bundled `forge-core 0.13.2` and
  `codex-cli 0.157.1` with the expected executable hashes.
- PASS: on a hidden Windows desktop with `FORGE_CORE_EXE` and `FORGE_CODEX_EXE`
  unset and an empty alternate `LOCALAPPDATA`, the installed app initialized a
  new folder, read its Forge record, sent to real authenticated Codex, created
  actual local HTML, opened the isolated preview, changed the result in the
  same conversation, reloaded the WebView and then restored the real messages
  in order after a full app-process restart without re-sending a turn.

## Limits

The alternate `LOCALAPPDATA` check is not a clean-machine installation: it
reused this Windows account, its existing Codex authentication and installed
WebView2. In-app ChatGPT sign-in, automatic updates, installer signing, manual
screen-reader/contrast acceptance and mobile are still unfinished. The bundle
uses the last verified released core 0.13.2, not the repository's 0.13.3 source
line; future protocol or provider changes require deliberate updates and new
native compatibility testing. The native folder dialog was not exercised in
the fresh-profile run; its response was simulated there. Model-specific tokens
and BRL cost remain UNKNOWN.

## Candidate

`D:/forge-method-core-build-cache/main-target/release/bundle/nsis/Forge_0.1.12_x64-setup.exe`
is 123,060,687 bytes, SHA-256
`34003537959E05A97377C2C55FD2A788B89BB3707BCAE6A18843B74101B107AC`.
Installed app SHA-256 is
`EA432B88E2FE58C26ED413757B90DF7098008E59E55C1CE93593ED93C19E3D21`;
bundled core SHA-256 is
`CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`;
bundled Codex SHA-256 is
`8CB0E69E99FF2A158C54815DB82D0F2E524D8F301BC30184722CFD1AE5973574`.
Do not rebuild or replace this tested candidate after hashing. It is not
public until the exact tagged asset is published, downloaded and checked.
