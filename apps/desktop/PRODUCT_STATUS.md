# Product closeout — 2026-09-30

This is the accepted journey map, not runtime authority or a claim that the
product is complete. Scope comes from UI epics
[#77](https://github.com/DanielCarva1/forge-method-core/issues/77),
[#78](https://github.com/DanielCarva1/forge-method-core/issues/78),
[#79](https://github.com/DanielCarva1/forge-method-core/issues/79) and
[#80](https://github.com/DanielCarva1/forge-method-core/issues/80).
The Desktop README checkpoints own detailed test/release receipts.

| Human journey | Existing entry and ownership | Observed evidence | Remaining boundary |
| --- | --- | --- | --- |
| Turn an idea into work | Início/Explorar → main → native project → Codex | Installed real idea/reply and first result, 0.1.66/67 | Not a guarantee of agent output quality |
| Start without choosing a folder | Default Documents/Projetos Forge, unique name | Installed native default creation and first-result checkpoints | Clean-device completed sign-in not verified |
| Use an existing folder | Native chooser → Core start/init | Real folder selection and existing-file preservation | External junction can block workflow initialization |
| Return to work | Meus projetos → inspect → saved conversation | Native saved reopening, no message replay | Device-local shortcuts (50), not global discovery |
| Converse and answer questions | Minha conversa → Codex stdio adapter | Real text turns/history; native simulated question protocol | Real-model invocation of question tool not verified |
| Understand progress | Andamento → read-only Core record | Installed record/navigation checks | Record is not a complete automatic transcript of agent activity |
| Inspect and change a result | Resultado → bounded file list/preview → draft | Installed native discovery, preview and change-draft checks | Protected HTML is intentionally noninteractive |
| Use a result outside Forge | Reveal/copy file; explicit trusted browser action | Native copy/cancel, exact bytes; browser dispatch partly simulated | Single-file copy is not complete website export or publishing |
| Read and navigate comfortably | Appearance, responsive panes, keyboard controls | 360px/200% browser checks and native Windows resizing | Physical phone and screen-reader checks not performed |
| Install and update | Published Windows NSIS, manual update instructions | Exact public download hashes and installed upgrades | No automatic update checker; unsigned alpha |
| Access securely from a phone | Integration decision #96, not a server already delivered | No supported remote connection selected or tested | Requires bounded connection decision and controlled validation |

## Work that remains, without inventing new scope

1. Remove the external-junction onboarding bottleneck with an explicit
   cooperative-observation design. Current retained-tree capture fails closed
   on relevant links. Do not follow links, rewrite the user's ignore rules,
   silently omit security evidence, or weaken strict promotion snapshots.
   Core `start` succeeding is not proof that `workflow init` succeeded.
2. Complete clean-device sign-in with a real human account, and physical
   accessibility checks. Existing authenticated-host and simulated-provider
   evidence cannot close those checks.
3. Finish [#96](https://github.com/DanielCarva1/forge-method-core/issues/96):
   compare supported connection paths, select and validate one bounded path.
   The issue explicitly delivers an **integration decision**, not authorization
   for additional infrastructure, hosting costs or an exposed network server.
4. Reconcile the remaining issue acceptance items with these receipts. Open
   issues and published installers are neither proof of absence nor proof of
   complete acceptance. Do not close untested criteria by inference.

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
