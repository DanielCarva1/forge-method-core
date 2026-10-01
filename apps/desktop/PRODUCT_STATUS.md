# Product closeout — 2026-10-01

This is the accepted journey map, not runtime authority or a claim that the
product is complete. Scope comes from UI epics
[#77](https://github.com/DanielCarva1/forge-method-core/issues/77),
[#78](https://github.com/DanielCarva1/forge-method-core/issues/78),
[#79](https://github.com/DanielCarva1/forge-method-core/issues/79) and
[#80](https://github.com/DanielCarva1/forge-method-core/issues/80).
The Desktop README checkpoints own detailed test/release receipts.
The accepted cinematic/floating UX packages are sequenced in section 2 of
`docs/development-plan.md`; this journey map does not introduce another roadmap.

| Human journey | Existing entry and ownership | Observed evidence | Remaining boundary |
| --- | --- | --- | --- |
| Turn an idea into work | Início/Explorar → main → native project → Codex | Installed real idea/reply and first result, 0.1.66/67 | Not a guarantee of agent output quality |
| Start without choosing a folder | Default Documents/Projetos Forge, unique name | Installed native default creation and first-result checkpoints | Clean-device completed sign-in not verified |
| Use an existing folder | Native chooser → Core start/init | Real folder selection; installed 0.1.78 opening/record/results with pre-existing external junction | Windows cooperative observation only; protected effects/completion remain strict |
| Return to work | Meus projetos → inspect → saved conversation | Native saved reopening, no message replay | Device-local shortcuts (50), not global discovery |
| Converse and answer questions | Minha conversa → Codex stdio adapter | Real text turns/history; native simulated question protocol | Real-model invocation of question tool not verified |
| Understand progress | Andamento → read-only Core record | Installed record/navigation checks | Record is not a complete automatic transcript of agent activity |
| Review agreements and changes | Recorded objective history; 0.1.79 literal revision comparison | Focused comparison tests; native receipt in latest checkpoint | #92 partial: opaque legacy decision resolutions lack recoverable original wording |
| Inspect and change a result | Resultado → bounded file list/preview → draft | Installed native discovery, preview and change-draft checks | Protected HTML is intentionally noninteractive |
| Use a result outside Forge | Reveal/copy file; explicit trusted browser action | Native copy/cancel, exact bytes; browser dispatch partly simulated | Single-file copy is not complete website export or publishing |
| Read and navigate comfortably | Appearance, responsive panes, keyboard controls; floating workspace | Installed 0.1.81 focused gestures/snap/focus/compose-fit with no Send; 360px/200% simple-layout evidence | Physical phone/screen reader and complete cinematic polish not performed |
| Install and update | Published Windows NSIS, manual update instructions | Exact public download hashes and installed upgrades | No automatic update checker; unsigned alpha |
| Access securely from a phone | #96 options assessed; alpha stays local-only | Pinned CLI/IPC/origin assessment; remote desktop disabled on this host | No phone connection tested; optional host-access choice remains explicit |

## Work that remains, without inventing new scope

1. Keep the 0.1.78 external-junction fix within its verified boundary: Windows
   cooperative initialization and common observations, no target traversal or
   external files. Protected promotion/completion and other-platform symlinks
   retain strict capture. Do not infer those paths are supported from onboarding
   success. Native project start, record consultation and results passed with
   the junction present; Codex simulated, external file unchanged.
2. Prioritize visual polish of the existing screens. Native account reuse and
   simulated first-login/cancellation are covered by the 0.1.80 connection
   package. Real provider-side first login by a new human on a clean device and
   physical accessibility remain evidence limits, not a new Forge login system
   or a reason to block visual work.
3. **Deferred by maintainer, 2026-09-30:**
   [#96](https://github.com/DanielCarva1/forge-method-core/issues/96), mobile.
   the supported-path comparison and local-only alpha scope are recorded in
   `docs/development-plan.md`. A real remote connection still needs an explicit
   host-access choice and controlled validation.
   The issue explicitly delivers an **integration decision**, not authorization
   for additional infrastructure, hosting costs or an exposed network server.
4. Reconcile the remaining issue acceptance items with these receipts. Open
   issues and published installers are neither proof of absence nor proof of
   complete acceptance. Do not close untested criteria by inference.
   In particular, #92's legacy decision-history wording is not supplied by the
   current report. Do not create a parallel decision store or revive strict
   authority ceremony in the normal cooperative journey to fill that gap.

## Keyboard closeout package

The 0.1.77 source corrects the skip link returning to Início, retains the
focused file when background discovery repaints the list, marks pending
questions in the visible workspace navigation without forcing a pane change,
and restores focus after answer/interrupt/resolution. Selecting Conversa takes
the reader to the pending question. Late events do not steal focus from a
draft. These are UI-only changes; Core snapshot restrictions remain unchanged.

No requirement to expose every Core command, introduce another project
registry, add process gates to ordinary agent work, or rebuild chat/history
again is inferred from this map.
