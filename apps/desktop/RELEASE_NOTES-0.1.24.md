# Forge Desktop 0.1.24 alpha.1

This Windows x64 alpha makes the first project choice clearer and the project
record quieter while creating. An empty folder field now says **Nenhuma pasta
escolhida** instead of displaying a sample path that could look selected. The
person can browse in the Windows folder picker or paste a path; the app does not
pass the placeholder as a project path. After a project opens, the record keeps
the stage and next step visible while placing the longer agent-recorded
objective behind **Objetivo registrado**. Expanding it still identifies the
agent as the source; it does not imply human approval.

## Verification

- The headless browser suite covers the empty path, folder picker response,
  project revalidation, objective disclosure, conversation and preview states.
- A hidden native debug run used the real Windows folder picker, initialized a
  disposable empty folder, read its honest Forge record, and connected to Codex
  without sending a message.
- A further hidden native debug run sent a real Codex request in a disposable
  project. Codex produced a local HTML page; the app displayed its protected
  preview, sent a change request in the same conversation, refreshed the page,
  and restored the conversation after WebView reload without resending.

## Limits

- This is an unsigned alpha without an automatic updater. Install the new
  package over the previous version to update.
- Fresh-account login completion, clean-machine installation, successful
  default-browser launch, manual accessibility acceptance, mobile, and
  self-contained offline distribution remain unverified or unfinished.
- The protected HTML preview is deliberately non-interactive. Opening a page
  in the normal browser can execute its code and should be reserved for
  trusted projects.
- A reliable per-task Pro cost and controlled Astra/Sol comparison are not
  available from the current telemetry; no savings claim is made.

## Candidate

The single locally tested candidate is `Forge_0.1.24_x64-setup.exe`,
123,068,738 bytes, SHA-256
`EC12F1AC8347FD54163716EDDC91455772EA83003E9CFA4835C60E6CE967B803`.
Silent installation over public 0.1.23 returned exit 0 and left the
candidate bytes unchanged. The installed app reports 0.1.24 and has SHA-256
`13E7E514C1741D6F95E1254BCB50062896A2E87FA3FB0FC848B2243CAA739EAF`.
The installed candidate passed hidden native project selection, Forge
onboarding/readback and an idle authenticated Codex connection. No provider
message was sent by this installed-candidate smoke; the full creation/change
journey was verified in the corresponding source debug build.

The public prerelease contains that one installer and its checksum sidecar.
An unauthenticated download matched the candidate size and SHA-256 exactly;
the downloaded bytes installed silently with exit 0 and produced the same
installed executable hash above. This proves availability and byte continuity
on the tested Windows machine, not a clean-machine installation.
