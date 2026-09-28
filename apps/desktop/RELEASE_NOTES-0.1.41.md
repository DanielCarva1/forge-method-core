# Forge Desktop 0.1.41 alpha.1

This Windows alpha fixes the project-history explanation when Forge has only
one recorded objective revision. The action now says **Entender esta direção na
conversa** instead of promising changes that do not exist. Its prepared request
asks Codex to explain the current direction and origin, and to say plainly
when there is no earlier version. Projects with multiple revisions retain the
changes action. Preparing the request never sends a message or records a
decision; the user still reviews and sends it.

## Verification and limits

The browser UI suite covers one and multiple objective revisions, draft-only
behavior, navigation, first Send, previews, return to conversation and other
controlled UI states. Eight JavaScript unit tests, Desktop `cargo check`, 51
non-ignored Rust tests, strict Clippy and one NSIS release build passed. A
hidden native Windows debug run used a real existing
Codex conversation and a disposable project with one objective revision.
Codex correctly explained that there was no earlier revision; the project
files and Forge objective/decision history did not change. The same native
conversation then used the preview's **Pedir mudança neste arquivo** action
without replacing its prepared text. One Send changed only the requested
HTML file, refreshed its local preview and restored the conversation after a
full app process restart without replay. This verifies that bounded journey,
not all possible projects or model responses.

The installer is unsigned, Windows x64 only and has no auto-updater. The
bundled Forge core remains pinned to 0.13.2. Protected static-page preview
does not execute JavaScript; using the external browser is a separate explicit
action for trusted projects. Historical decision wording is not reconstructed
from digests. Clean-machine setup, physical mobile-device use, manual
screen-reader acceptance, model-attributed tokens, BRL-equivalent cost and
subscription quota impact remain unverified or unavailable. No manual GitHub
CI run was triggered for this package.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.41_x64-setup.exe`, 123,095,229 bytes,
SHA-256 `3E35663AC600714476DB7737C8A585DA693D0DFAB8D02EE5159E995593B66A6E`.
Silent installation over public 0.1.40 returned exit 0 without changing the
candidate hash. The installed executable reports Windows ProductVersion
0.1.41 and SHA-256
`8EB032BB93F2B9D38AF4B50AB9291EE1F912ECCEB8B515080E07E2DF69472550`.
Hidden native installed-candidate smoke passed against projects with both one
and four real objective revisions, without sending a message. The prerelease
assets then downloaded without authentication. The downloaded installer was
exactly 123,095,229 bytes with SHA-256
`3E35663AC600714476DB7737C8A585DA693D0DFAB8D02EE5159E995593B66A6E`,
matching the candidate and sidecar. Silent installation of the downloaded bytes
over the candidate returned exit 0 without changing the installer file,
produced the same 0.1.41 executable hash, and passed hidden native smoke again.
No candidate rebuild or manual GitHub CI run followed publication.
