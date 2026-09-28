# Forge Desktop 0.1.25 alpha.1

This Windows x64 alpha makes a generated local page easier to use and the
project record less misleading. **Usar no navegador** now asks Windows for
the default HTTPS browser instead of opening the app associated with `.html`,
which could be an editor. The selected HTML file must still belong to the
opened project. A completed work item now says **Esta parte foi concluída**;
the app no longer suggests that the whole project is finished.

## Verification

- Focused Desktop Rust check, preview tests, all Desktop crate tests, and
  strict Clippy passed for the browser handoff change. The Windows association
  probe found an installed browser executable without launching it.
- The full browser UI suite passed. A hidden native debug run opened the real
  project's Forge record and displayed the revised completed-work wording.
- The registered browser executable opened the project's real local HTML page
  in an isolated headless session; its JavaScript saved an idea and preserved
  it across page reload.
- A hidden native first-use fixture completed login without using the
  maintainer's account, retained the draft, and passed the bundled Start Forge
  guidance to the first Codex send.
- The exact installer below silently upgraded the locally installed 0.1.24
  with exit code 0. The installed 0.1.25 passed hidden native Windows folder
  selection, Forge onboarding and record readback, protected preview, one
  simple real Codex reply in a disposable project, and restoration of a
  separate existing 33-message real conversation after WebView reload.

## Limits

- The **Forge button itself launching a browser** has not been exercised
  end-to-end: a normal browser could surface on the maintainer's active
  desktop. Browser association lookup plus headless rendering are narrower
  checks, not proof of that action.
- Fresh-account login with the real provider, clean-machine installation,
  manual accessibility acceptance, mobile, and self-contained offline
  distribution remain unverified or unfinished.
- The in-app HTML preview remains deliberately non-interactive. Opening a
  trusted page in a normal browser can execute its code and access the network.
- This is an unsigned alpha without an automatic updater. Install this
  package over 0.1.24 to update. A reliable per-task Pro cost or controlled
  model-savings comparison is not available from current telemetry.

## Candidate

The single locally tested candidate is `Forge_0.1.25_x64-setup.exe`,
123,042,265 bytes, SHA-256
`F76671E32C235F614C059DD3C6CA7537B6C6C55B22E00BF4E471AC096B930CC3`.
Silent installation over 0.1.24 left those installer bytes unchanged, and
the installed executable reports 0.1.25 with SHA-256
`A6DCDA319C241CDD089D3FF33A44204C8ACB6CC5424F842537AA268EE6CCAC39`.
The public prerelease contains this installer and its checksum sidecar. An
unauthenticated download matched the tested candidate's size and SHA-256
exactly. Installing those downloaded bytes silently returned exit code 0;
the installed executable reported 0.1.25 and retained the hash above.
This test machine is not a clean-machine oracle.
