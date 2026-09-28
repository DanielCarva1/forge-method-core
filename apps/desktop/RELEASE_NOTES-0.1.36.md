# Forge Desktop 0.1.36 alpha.1

This Windows x64 alpha closes a result-reading gap: a PDF selected from the
current project now has an explicit **Abrir PDF no navegador** action. The
Forge card still identifies the local file and offers a conversation about it.
Selecting or refreshing the PDF never opens another app automatically.

The PDF is not rendered inside Forge's protected preview. On click, the
desktop command checks the currently confirmed project, canonical file
containment, `.pdf` extension and `%PDF-` header, then asks the Windows
default browser to open the local file. This is an external handoff, not a
promise that the browser rendered the document or that the file is safe.
Other unsupported file types retain the prior read-only card; the isolated
HTML preview and its explicit browser action remain unchanged.

## Verification and limits

Focused Rust validation and the full Desktop crate suite passed. The browser
UI suite passed the PDF action, no-automatic-open, failure message, other-file
and existing HTML paths. A hidden native Windows WebView run passed real
project and new-project onboarding, actual file-picker selection of a PDF,
visible PDF action, conversation draft, and isolated HTML preview. The native
screenshot was reviewed. **Actual external browser launch was NOT_RUN** to
avoid opening a visible browser on the maintainer's desktop; browser launch is
covered only by the native command's validation test and simulated UI invoke.
Actual Codex Send was NOT_RUN in this result-view package; prior release
evidence remains separate.

This remains an unsigned alpha installer without an auto-updater. Install it
over the previous alpha. Clean-machine setup, mobile-device use and manual
screen-reader acceptance remain unverified. The bundled Forge core stays
intentionally pinned to 0.13.2. Per-model token costs and subscription quota
impact cannot be attributed reliably; no savings claim is made.

## Candidate and public readback

The single NSIS candidate is `Forge_0.1.36_x64-setup.exe`, 123,068,768
bytes, SHA-256
`DAC51CBF4B8653A4DF0223510CDECA4AF9B7D61752926473F0F40DCB1357B575`.
Silent installation over public 0.1.35 returned exit 0 and left the candidate
unchanged. The installed executable reports 0.1.36 and has SHA-256
`CCAF472DB21B0DDB9B2D51B9B2609ABA8982559084389768A0EA479ACB85CF98`.
Installed-candidate hidden native smoke passed with the real Windows PDF
picker and project readback. The published installer and sidecar were
downloaded without authentication. Both installer copies and the sidecar
matched the SHA-256 above and the download had the same 123,068,768 bytes.
Silent installation of the downloaded file returned exit 0, preserved its
hash and produced the installed executable hash above. Hidden native smoke
passed again after installing those downloaded bytes. No candidate rebuild
or manual GitHub CI run followed publication.
