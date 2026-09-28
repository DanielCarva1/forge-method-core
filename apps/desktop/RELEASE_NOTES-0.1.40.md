# Forge Desktop 0.1.40 alpha.1

This Windows alpha makes the project record easier to understand without changing the Forge engine. When Forge shows suggested questions but no pending human choice, **Onde estamos** no longer claims that a pending choice was lost. The shortcut now distinguishes **Ver perguntas**, **Ver escolhas**, and **Ver escolhas e perguntas** according to the actual readback. A pending-choice count does not pretend that the original question text was recovered.

The existing objective-history panel adds **Entender mudanças na conversa**. It prepares a request for a plain-Portuguese explanation of recorded objective revisions in the current Codex conversation. The user must still review and send it. The button does not record a decision, treat suggestions as agreements, or assert independent human approval of an agent's record.

## Verification and limits

Browser tests cover suggestion-only, pending-only, mixed, revised objective and unsent-history-action states. Desktop `cargo check`, 51 non-ignored unit tests, strict Clippy, eight frontend unit tests and one NSIS release build passed. A hidden native Windows WebView run on the installed 0.1.40 candidate read four real Forge objective revisions and prepared the history explanation without sending. A separate installed-0.1.39 run sent a real first message to Codex in a disposable project and recovered its reply after WebView reload; that verifies the general Send/reply/recovery path, **not** an actual Codex reply to the new history or suggestion explanation prompts. The bundled Forge core remains pinned to 0.13.2. No manual GitHub CI run was triggered for this package.

This remains an unsigned Windows x64 alpha without an auto-updater. Historical decision question-and-choice prose is not reconstructed from references or digests; the agent must consult the original source and admit when it cannot recover it. Clean-machine setup, physical mobile-device use, and manual screen-reader acceptance remain unverified. Model-attributed tokens, BRL-equivalent cost and subscription quota impact remain UNKNOWN.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.40_x64-setup.exe`, 123,058,432 bytes, SHA-256 `44B1DC739437C6C8E2E9B363674C9CB1DE5EBF44289477FD3437BE5B0D06A13A`. Silent installation over public 0.1.39 returned exit 0 without changing the candidate hash. The installed executable reports 0.1.40 in its Windows product version and has SHA-256 `53823B2296F733CC45472FFF3EDD7F2A5E241B447CADE56825ECB28ECCD170A6`. Hidden native candidate smoke passed with actual Forge project readback, objective-history draft, onboarding and previews; the file/PDF dialog response was simulated in this run.

Public download and installation of those downloaded bytes are still pending. A code commit alone does not make this installer available.
