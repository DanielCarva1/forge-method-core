# Forge Desktop 0.1.39 alpha.1

This Windows alpha improves two everyday screens without changing the Forge engine. **Explorar** now keeps its invitation and search together on desktop-sized windows, while preserving the stacked layout on smaller screens. The decorative artwork no longer sits behind the search label.

In **Onde estamos**, current Forge-suggested questions and their original options start collapsed. A clear **Entender sugestões na conversa** action prepares an unsent request for a plain-Portuguese explanation. The original source wording remains available under **Ver perguntas e opções originais**. A recovered pending choice is still shown separately; suggestions are not presented as the person's decisions. No action sends a message or records a choice without an explicit Send.

## Verification and limits

Browser tests passed the Explore layout at 1536, 1280, 1180 and 1100px, filtering, keyboard/mobile behavior, collapsed suggestions, draft preservation, and no automatic send. Desktop `cargo check`, 51 non-ignored unit tests, strict Clippy and one NSIS release build passed. Hidden native Windows WebView runs on the installed candidate passed a real Forge project record, collapsed mobile view, the unsent suggestion action, actual Windows folder/file/PDF selection, new-project onboarding and local previews. A controlled pending-decision readback also passed without sending or recording a choice.

This remains an unsigned Windows x64 alpha without an auto-updater. The bundled Forge core stays pinned to 0.13.2. Historical decision question-and-choice prose is not reconstructed from references or digests; the agent must consult the original source and admit when it cannot recover it. Actual Codex Send for the new suggestion-explanation action is **NOT_RUN**; the existing real Codex result/change/restart journey was proved separately on installed 0.1.38. Clean-machine setup, physical mobile-device use and manual screen-reader acceptance remain unverified. Model-attributed tokens, BRL-equivalent cost and subscription quota impact remain UNKNOWN.

## Candidate and public readback

The single tested NSIS candidate is `Forge_0.1.39_x64-setup.exe`, 123,163,988 bytes, SHA-256 `E221914913EBA0C2FADFB4567CFFEC4D5FB8A107F36B596B3D0389BAA0C7D1A3`. Silent installation over public 0.1.38 returned exit 0 and left the candidate bytes unchanged. The installed executable reports 0.1.39 and has SHA-256 `542CE6A62A62AD437291EB1D96CDD3F18602C873D07B3D7F6C856AA3A823D9BC`. Installed-candidate hidden native smoke passed.

The published installer and SHA-256 sidecar downloaded without authentication. The downloaded installer matched the tested candidate's byte count and hash. Silent installation of those downloaded bytes returned exit 0 without changing them, produced the same installed executable hash, and passed hidden native smoke again. No candidate rebuild or manual GitHub CI run followed publication.
