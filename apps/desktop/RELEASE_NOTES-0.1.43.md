# Forge Desktop 0.1.43 alpha.1

This Windows alpha makes the project progress panel easier to scan. The real
Forge stage and recorded next step remain visible. The unmodified activity
record, which can be technical and lengthy, is now available under **Ver
atividade registrada** instead of occupying the first view. Refreshing the
record or changing projects closes that detail so stale activity is not left
open. On narrow windows, the Alpha label shares the brand row when space
allows; at enlarged text sizes it may wrap rather than overlap navigation.

## Verification and limits

The complete browser UI suite passed, including real-text preservation in a
controlled record, detail closure after refresh, narrow header layout, and
360px/200%-text reflow. Hidden native Windows WebView smoke read this
repository's actual Forge record, displayed its stage and next step, opened
and closed the activity disclosure, and checked the narrow layout. The native
comparison screenshot is `D:/Temp/User/forge-progress-compact-header-mobile.png`.
No Codex turn or project-file edit was needed for this read-only UI package.
The exact tested installer and public downloaded-file readback are recorded
below after verification.

The installer is unsigned, Windows x64 only and has no auto-updater. The
bundled Forge core remains pinned to 0.13.2. The app displays the record's
next-step wording rather than inventing a simpler interpretation; **Entender
isto na conversa** prepares an unsent request for a plain-language explanation.
Clean-machine setup, physical mobile-device use, manual screen-reader
acceptance, per-model token counts, BRL-equivalent cost and Pro-quota
attribution remain unverified or unavailable. No manual GitHub CI was
triggered for this package.

## Candidate and publication

One NSIS candidate was built and installed silently over 0.1.42. It is
`Forge_0.1.43_x64-setup.exe`, 123,064,514 bytes, SHA-256
`67C4CDEDCD7DE175AFB8553427301AB396780B414913898CD1D42A10233793C3`.
The installed executable reports ProductVersion 0.1.43 and SHA-256
`29123D1FFC2246F6AD8DBAD30C0FE2B82AB1CFC59B48C11B5174F96736225F62`.
The installed-package hidden native smoke passed, including actual Forge
record readback. It did not send a real Codex turn. Package commit `64c7387a`
was pushed and tagged `desktop-v0.1.43-alpha.1`. An unauthenticated public
download returned the same 123,064,514 bytes and SHA-256 as the tested
candidate and published sidecar. Silent installation of those downloaded
bytes over the candidate returned exit 0, preserved the downloaded file, and
installed the same 0.1.43 executable hash. The downloaded installation passed
hidden native smoke again, including real Forge record readback. A source
commit alone does not update the installed app.
