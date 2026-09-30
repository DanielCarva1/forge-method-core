# Forge Desktop 0.1.72 alpha candidate

**Move around your project without hunting below the conversation.** A visible
project navigation bar now offers Conversa, Resultado, Andamento and Projeto on
desktop as well as narrow screens. Desktop shortcuts scroll to the existing
panel and focus its heading; narrow screens show the chosen panel. Selecting
another area leaves the expanded-conversation view so the requested content is
not hidden. Navigation stays available while scrolling and adjusts its space
when larger text wraps. It does not send messages, reconnect Codex, create
project state or refresh the engine.

Forge Core remains 0.13.3. No engine or Codex protocol behavior changed.

## Verification and limits

Focused browser-double checks passed for the four areas, keyboard activation,
expanded-conversation exit, narrow layouts, 200% text and no additional backend
operations. Preview-preservation and project-record checks also passed. The
preview test's setup was updated to expand the existing optional folder choice;
that earlier timeout was an outdated test setup, not a preview failure.

Native candidate and installation results are recorded in the Desktop README.
No new real Codex turn is required or claimed for navigation-only changes.
Viewport and larger-text checks are not physical-phone or screen-reader proof.
The broader browser suite remains known to have outdated first-use expectations;
it was not rerun or called passing for this package.

Windows x64, unsigned alpha, manual installer updates. Protected local preview
and explicit normal-browser use retain their earlier limitations. A candidate
or local installation does not mean a new public download is available.
