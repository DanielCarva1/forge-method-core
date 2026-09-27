# Forge Desktop 0.1.14 alpha.1

This Windows x64 alpha makes the existing create-and-continue journey easier to
understand. The project panel now shows “Atualizar andamento”, “Agora” and
“Próximo passo” instead of repeatedly exposing the internal word “registro”.
Loading, empty, slow and error messages explain what Forge actually knows and
what it has not assumed. First-use ChatGPT access now makes it clearer that a
draft was not sent, removes the redundant sign-in button after a code appears,
and leaves sending under the person's control. No Forge or Codex authority was
changed. The installer still bundles Forge core 0.13.2 and Codex CLI 0.157.1.

## Verified scope

- PASS: browser suite covers project states, first-use sign-in simulation,
  preservation of drafts, history, preview and responsive/keyboard behavior.
- PASS: hidden native WebView with isolated signed-out Codex home shows a real
  device challenge, preserves the draft, prevents early sending and cancels.
- PASS: hidden native debug journey used real Windows folder and file dialogs,
  created a real local HTML file through the authenticated Codex, previewed it,
  changed it in the same conversation and restored ordered messages after
  WebView reload and full process restart without resending.
- PASS: one NSIS candidate upgraded installed 0.1.13 to 0.1.14. On those
  installed bytes, with core/Codex executable overrides unset, hidden native
  tests repeated signed-out device-code start/cancel and the complete real
  folder → Codex chat → local HTML preview → follow-up change → process-restart
  journey. Both Windows folder and preview-file dialogs were exercised.

## Limits

**Completing a new ChatGPT login in the browser remains unverified.** The
signed-out native test cancels the challenge; fixture completion does not prove
provider authorization. The default-browser button and a genuinely fresh
Windows account/VM have not been tested. Installation uses the new NSIS package;
there is no automatic updater or signing. Manual accessibility acceptance and
mobile remain open. This is an alpha building block, not a complete-product
claim. Model-specific token use and BRL cost are UNKNOWN.

## Candidate and publication

The tested NSIS candidate is `Forge_0.1.14_x64-setup.exe`, 123,057,736 bytes,
SHA-256 `BA7E26638A91224AC9DB939EF4BA2C844B74DB9C1F7BECE3D2D93FAF0BF2970C`.
Public readback and download verification remain pending until publication.
Do not treat a commit as a published installer.
