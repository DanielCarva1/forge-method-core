# Forge Desktop 0.1.51 alpha.1

Links to public websites cited in a completed Codex reply are now usable from
the conversation. They appear as explicit actions rather than raw Markdown.
Before opening one in the system browser, Forge shows its full address and asks
for confirmation. The app does not load the site inside its privileged window;
only HTTP(S) addresses without embedded credentials are eligible. Unsafe or
unsupported link syntax stays inert text. Local project-file citations still
open through the protected project preview.

The project record also uses shorter completion wording, avoiding a repeated
"finished" message beside the status and phase explanation. No Forge lifecycle
or Codex protocol behavior changed in this package.

## Verification and limits

The browser suite tested the new link action, refusal, exact address passed to
the native bridge, unsafe-text handling, keyboard traversal, project navigation,
conversation recovery and responsive states. The Desktop Rust URL validator
passed its focused test; the Desktop crate suite and strict Clippy passed. A
hidden native Windows WebView smoke exercised real Forge project readback and
the existing create, preview and conversation surfaces. The previously
published 0.1.50 also restored a 39-message real Codex conversation across two
app launches without resending, and rendered its local page preview. Opening an
external website through the operating-system browser itself was **NOT_RUN**;
the browser UI used a simulated native call, while Rust validated the address
before that call.

This is an unsigned Windows x64 alpha without an auto-updater. Bundled Forge
core remains pinned to 0.13.2. Clean-machine setup, fresh provider login
completion and manual screen-reader acceptance remain NOT_RUN. Per-model
tokens, BRL-equivalent cost and Pro quota attribution remain UNKNOWN. No manual
GitHub CI was run for each UI edit.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.51_x64-setup.exe`, 123,047,917
bytes, SHA-256
`9B069FD0561BD059719C3EF22159553ABFAA10EBF2CD6D3F35A9B5BE14351F91`.
Its adjacent 93-byte `.sha256` sidecar records the same hash. Silent
installation over 0.1.50 returned exit 0, preserved candidate bytes and
installed ProductVersion 0.1.51, executable SHA-256
`A7949DA7CF6B762C8824DEA14D279A00A2991CB28CB2F3B8E80072BC82668748`.
The installed candidate passed the hidden native smoke. It also restored the
preserved 39-message real Codex conversation across two process launches,
rendered its protected local page preview and made no Send call or artifact
change. In the native smoke that checked unsafe-link rejection, the first
automatic Forge-record read returned the generic lookup error; the explicit
retry succeeded. That occurrence was not traced to a root cause, so first-read
stability is not claimed. Public download verification will be recorded only
after publication.
