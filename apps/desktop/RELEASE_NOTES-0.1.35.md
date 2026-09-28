# Forge Desktop 0.1.35 alpha.1

This Windows x64 alpha makes a confirmed project easier to navigate in a
narrow window. **Conversa**, **Prévia**, **Andamento** and **Projeto** now each
show only their own existing panel. The project folder and connection controls
remain one choice away, rather than lengthening the progress screen. On very
narrow widths the four choices form a balanced two-column layout. The project
name is not repeated in the narrow Project card; the wider desktop layout and
first-use folder selection are unchanged. No project or conversation state is
copied or created by switching views.

## Verification

Browser tests cover panel switching by mouse and keyboard, preserving the
loaded local result and unsent draft, access to the folder picker, 200% text
without clipped controls, and return to the wide layout. Hidden native Windows
smoke covers a real Forge project/record, the separate narrow panels, the
folder controls and isolated preview. Native progress and project screenshots
were reviewed. Actual Codex Send in this UI-only package is NOT_RUN; the
separate installed 0.1.33 real-Codex journey remains documented in the desktop
checkpoint.

## Limits

This is responsive Windows desktop UI, not mobile-device distribution or
remote-agent access. Actual mobile virtual-keyboard and manual screen-reader
acceptance remain unverified. The installer is unsigned and has no auto-updater;
install it over the previous alpha. Clean-machine setup is unverified. The
bundled Forge core remains intentionally pinned to 0.13.2. No per-task model
cost or Pro-quota savings can be attributed reliably.

## Candidate and public readback

The single tested NSIS candidate is `Forge_0.1.35_x64-setup.exe`, 123,135,285
bytes, SHA-256
`B2D24C149529E5DE7FC77C47F10796DB2067F0E58240587B23E2B98652259384`.
Silent installation over public 0.1.34 returned exit 0 without changing the
candidate bytes. The installed executable reports 0.1.35 and has SHA-256
`16FF5987A7138A17E612F87EF4BC512F27E7CFBD9568233F3C8C379FFFF349E7`.
Installed-candidate hidden native smoke passed with actual folder selection,
new-project Forge onboarding and record readback, narrow panel switching and
isolated preview. Public download readback remains pending; do not call the
version available until that passes.
