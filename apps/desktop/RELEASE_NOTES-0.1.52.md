# Forge Desktop 0.1.52 alpha.1

Consequential choices now stay inside Forge's interface instead of showing a
generic browser prompt. Opening a public website cited by Codex displays the
full destination and offers a clear cancel action. Switching projects while
Codex is answering explains that the response will be interrupted and that
changes already made may remain. Escape or Cancel keeps the current state.

Removing a project shortcut now confirms that files remain in the folder. If
the device refuses to save the changed list, the shortcut stays visible rather
than disappearing only until restart. This is a local shortcut, not deletion
of a Forge project.

## Verification and limits

The controlled browser suite exercised accept, cancel, Escape, narrow layout
with enlarged text, project switching and failed local storage. A hidden native
Windows WebView smoke exercised real Forge project/record readback, onboarding
and safe preview. It did not send a new real Codex message or launch a website
in the system browser for this package. The installed 0.1.51 previously
restored a 39-message real Codex conversation across app launches; this
package does not change that protocol.

This remains an unsigned Windows x64 alpha without automatic updates. The
user installs the newer package over the earlier one. Clean-machine setup,
fresh provider login completion, actual external-browser launch, and manual
screen-reader acceptance remain NOT_RUN. A prior occasional first-read Forge
record error has not been isolated; explicit retry works, but first-read
stability is not claimed. Per-model token use, BRL-equivalent cost and Pro
quota attribution remain UNKNOWN. No manual GitHub CI was run per UI edit.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.52_x64-setup.exe`, 123,119,675
bytes, SHA-256
`4EC4CE228E19C76B02AC161FBD5493AB359FF50E5042666863FFB7CC7FEC6B37`.
Its adjacent 93-byte `.sha256` sidecar records the same hash. Silent
installation over 0.1.51 returned exit 0 without changing candidate bytes.
The installed executable reports ProductVersion 0.1.52, SHA-256
`1A3CE7FE741B14ED873791A49C138BDCB0D474F769B27EBDACD888ECE2AA3E39`,
and passed the hidden native smoke. The prerelease was published at
https://github.com/DanielCarva1/forge-method-core/releases/tag/desktop-v0.1.52-alpha.1.
An unauthenticated download of the installer and sidecar matched the exact
candidate size and SHA-256. Silent installation of the downloaded file exited
0, preserved its bytes, installed ProductVersion 0.1.52 with the same
executable hash, and passed the hidden native smoke. This is same-machine
evidence, not clean-machine installation.
