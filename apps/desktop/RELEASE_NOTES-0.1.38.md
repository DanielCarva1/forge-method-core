# Forge Desktop 0.1.38 alpha.1

This Windows alpha makes the working screen easier to use before a result exists. The compact **Prévia do resultado** card and **Escolher arquivo** action now appear above the project record. The Forge stage and current activity remain visible below, and the conversation and **Enviar** stay in view. When a file is opened, the same preview expands in place; there is no separate project store or automatic file launch.

When Forge reports a recovered pending decision, **Entender escolhas em aberto** prepares an unsent question in the conversation. It asks the agent to verify the original source and admit when the historical wording cannot be recovered. Suggestions are not displayed as decisions and this action neither sends a message nor records a choice.

## Verification and limits

The browser UI suite passed navigation order, empty and loaded preview, keyboard focus, narrow layout, long conversation, project switching, and pending-decision source/failure paths. Desktop `cargo check`, 51 non-ignored unit tests, strict Clippy and the release build passed. A hidden native Windows WebView run on the installed candidate passed real project/new-project onboarding, the actual Windows folder and PDF picker dialogs, Forge record readback, local preview and the revised layout. The pending-decision action was also exercised with a controlled native IPC response; this does not prove a real project has an available historical question. Actual Codex Send for this action is **NOT_RUN**.

This remains an unsigned Windows x64 alpha without an auto-updater; install it over the previous alpha. The bundled Forge core remains pinned to 0.13.2. The core's historical decision audit exposes references and digests, not verified original question-and-choice prose; the UI does not reconstruct it from a current policy. Clean-machine setup, mobile-device use and manual screen-reader acceptance remain unverified. Model-attributed tokens, BRL-equivalent cost and subscription quota impact remain UNKNOWN.

## Candidate and public readback

The single tested NSIS candidate is `Forge_0.1.38_x64-setup.exe`, 123,097,865 bytes, SHA-256 `5506BA4E61DA833A8FA381559E22DA8D3F9F4C645212EB0D16B02A0B2AB05797`. Silent installation over public 0.1.37 returned exit 0 and left the candidate bytes unchanged. The installed executable reports 0.1.38 and has SHA-256 `EDA191ACC39A624AF77EFC78CF1881D8751F0184879F819EC6F9B4CBC5B4AA82`; installed-candidate hidden native smoke passed. Public download verification is pending; do not call the installer available before those checks pass.
