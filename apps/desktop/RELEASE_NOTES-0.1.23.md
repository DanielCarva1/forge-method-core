# Forge Desktop 0.1.23 alpha.1

This Windows x64 alpha makes it easier to find and resume a project. **Meus projetos** now shows up to 50 recently opened folders instead of eight, with a search by project name or folder path. The search ignores accents and letter case, shows how many shortcuts matched, and explains when none did. Choosing a result checks the actual folder again; this list is only a set of shortcuts stored on this device, not a second project registry.

For a new or returning project, the app now says that the person can start **or continue** the conversation without implying that a message was already sent. When Forge has no recorded progress, the record says so rather than implying that files or a conversation already exist.

## Verification

- The browser UI suite covers 60 seeded shortcuts capped at 50, search by name and path, duplicate names in distinct folders, an empty result, narrow-window layout, and opening a result through project revalidation.
- A hidden native debug run used a real disposable Forge project, found its shortcut and reopened it through native inspection without sending a message. The first-use wording was also checked in an isolated native unsigned-in profile.
- PASS: eight frontend unit tests, the browser UI suite, Desktop `cargo check`, all 50 Desktop crate tests, strict Clippy, and one NSIS release build.
- The exact candidate installed silently over public 0.1.22 with exit 0 and unchanged installer hash. The **installed** 0.1.23 passed hidden native smoke, real project-shortcut search and revalidation with zero sends, and a simulated first-use completion that checked the bundled Start Forge path. The real account login completion was not attempted.

## Limits

- The installer remains unsigned and has no automatic updater. A person updates by installing the newer package.
- Real fresh-account login completion, clean-machine installation, a successful default-browser launch, manual accessibility acceptance, and mobile remain unverified.
- Search only covers up to 50 shortcuts stored locally by this app. It does not discover every project on disk or search Codex conversations. Project contents remain in their own folders.
- A controlled model-cost comparison and reliable per-task Pro cost are not available from the current usage evidence.

## Candidate

The single tested candidate is `Forge_0.1.23_x64-setup.exe`, 123,073,611 bytes, SHA-256 `EA27A6081F8F3A4F4318487833C08F624FDE606F2A5A9E94DA08F11EA1C1818A`. The installed executable SHA-256 is `F362B88D44F9DDDB0DD7D6B8CB4CCF0CF91D90AF98DCFBF880E260CF3EEB2076`; the bundled Forge core remains pinned at 0.13.2, SHA-256 `CFD6F81B1710D0469A53D12B374258CC122676EA7865F26926B1CDB4C7541EDF`. The public installer was downloaded without authentication; its bytes and checksum sidecar matched this candidate, and those downloaded bytes installed successfully.
