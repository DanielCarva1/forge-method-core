# Forge Desktop 0.1.76 alpha.1

**Use your result without copying technical paths.** Select a project file in the
result area and choose **Mostrar na pasta** to request that Windows Explorer
select that file, without executing it. **Salvar uma cópia** opens the native
Windows save dialog and copies the selected file to a new destination.

The original and message draft are preserved. Cancellation creates no copy;
existing destinations are never overwritten. Source/project authority is
checked again after the dialog. Files too large to render remain selectable as file information, so you can still
copy or reveal them without loading their contents. The copy is streamed rather than loaded into
UI memory. Late responses cannot replace feedback for another project/file.
Nothing is uploaded, published or sent to Codex by these actions.

This copies **one file**, not the entire project or dependencies. For HTML
pages, supporting images/styles are not included; the UI explains this under
**O que será copiado?**. It is not a website-export or publication feature.
A write failure can leave an incomplete new destination; the error says so and
the original is not altered. Core remains 0.13.3.

## Verification and limits

Focused Rust checks cover file containment/type, exact bytes, source preservation,
existing/same destination rejection, invalid destinations and Unicode/UNC Explorer
arguments. Focused browser-double checks cover reveal/copy success, cancellation,
failure, pending controls, stale project feedback, preserved drafts and the
existing result journey/narrow layout. Native save-dialog and installed package
results are recorded in the Desktop README.

Explorer dispatch is mocked in the native journey to keep the active user's
desktop untouched; request/argument validation is not proof of a visible Explorer
selection. No Codex/model is needed for copying. Protected previews remain
scriptless; no new registry, export service, upload or dependency was added.

Unsigned Windows x64 alpha, manual installer updates. Clean-device completed
sign-in, manual accessibility, physical-phone checks and secure remote mobile
access remain unvalidated. The existing Core restriction on initial projects
containing external Windows junctions is unchanged. No workspace/full crate,
broad browser suite or manual GitHub CI, and no monetary savings claim.
