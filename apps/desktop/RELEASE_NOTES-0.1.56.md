# Forge Desktop 0.1.56 alpha.1

This update makes returning to work easier. When this device has a recent
project shortcut, Home offers **Continuar último projeto** in the main action
and shows the project's name and folder in the first card. The action
revalidates the folder through Forge before opening it. It does not send a
message. With no shortcut, Home retains the folder-opening route; removing a
shortcut restores that route. The project list remains local shortcuts, not a
second Forge project registry.

## Verification and limits

A focused headless-browser check covered invalid shortcuts, removal, and no
Send. A hidden Windows build prepared and reopened a disposable real Forge
project from Home using native `inspect_project`, without a Codex Send. A
second focused hidden-native check sent one real read-only Codex message,
closed and reopened the app, then resumed the validated project and saved
reply from Home without a second Send. No broad suite or manual GitHub CI ran.

This is still an unsigned Windows x64 alpha with manual installer updates. It
bundles pinned Forge core 0.13.2, not source-tree core 0.13.3. Clean-machine
installation, first-time ChatGPT login completion, actual external browser
launch, and manual screen-reader acceptance remain NOT_RUN. Model-specific
token use and BRL-equivalent cost remain UNKNOWN. This release does not prove
every kind of interrupted/incomplete conversation can resume automatically.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.56_x64-setup.exe`, 123,061,847
bytes, SHA-256
`9FE5E9A87E0D7A8642C83BA4E3E63B9CBE4374771C079A0575273D0E444D775C`.
Its 93-byte `.sha256` sidecar records the same hash. Silent installation over
0.1.55 exited 0 without changing the candidate. The installed executable
reports ProductVersion 0.1.56, SHA-256
`D8E62767536AB93932540B901CEB098DA1BBD617B74A266524D776E31CD92F83`.
The installed candidate passed the focused hidden-native real Codex
reply/restart/Home-resume check without a second Send. Publication, anonymous
download verification, and downloaded-installer readback remain pending.
