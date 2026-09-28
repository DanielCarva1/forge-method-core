# Forge Desktop 0.1.37 alpha.1

This Windows x64 alpha makes the project PDF action easier to find. **Abrir PDF no navegador** is now the first, highlighted action on a selected PDF; **Conversar sobre este arquivo** remains available below it. Keyboard order matches the visible order. Other project files retain their existing actions. Nothing opens outside Forge automatically.

The narrow project navigation now adapts to enlarged text: it keeps two columns at normal size and gives each destination a full row when 200% text needs the space. This prevents labels such as **Conversa** and **Andamento** from breaking in the middle of a word. No backend, project record or conversation state changed in this package.

## Verification and limits

The browser UI suite passed action order, narrow navigation, PDF state retention and 200% text checks. A hidden native Windows WebView run passed real project/new-project onboarding, actual folder and PDF picker selection, project record readback and isolated HTML preview. Its enlarged-text PDF screenshot was reviewed; navigation labels were intact. Actual external PDF browser launch and actual Codex Send were **NOT_RUN** for this UI-only package. Earlier real-Codex evidence is separate.

This remains an unsigned alpha installer without an auto-updater; install it over the previous alpha. Clean-machine setup, mobile-device use and manual screen-reader acceptance remain unverified. The bundled Forge core stays deliberately pinned to 0.13.2. Per-model token costs and subscription quota impact remain UNKNOWN.

## Candidate and public readback

The single tested NSIS candidate is `Forge_0.1.37_x64-setup.exe`, 123,159,645 bytes, SHA-256 `4A0A4C688B22E624EFE6166DDB6053D63037AFDE8C0F067C73C9C1513CA93C3D`. Silent installation over public 0.1.36 returned exit 0 without changing the candidate bytes. The installed executable reports 0.1.37 and has SHA-256 `21DBE1B874DD5D887CB701D8A1A264A6B5C862E0172EF9EFC4FC51DE8A48005B`. Installed-candidate hidden native smoke passed. The published installer and SHA-256 sidecar downloaded without authentication; the download matched the candidate's size and hash. Silent installation of those downloaded bytes returned exit 0 without changing them, produced the same installed executable hash and passed hidden native smoke again. No candidate rebuild or manual GitHub CI run followed publication.
