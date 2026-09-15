# Forge desktop shell

Independent Tauri application, currently a development shell, not a published
user release. It does not require Codex Desktop. Agent integration is not yet
implemented in this shell; the native identity check is not an agent connection.

## Run and test

From the repository root (Rust and Windows WebView2 prerequisites required):

```powershell
cargo run --manifest-path apps/desktop/src-tauri/Cargo.toml
node --test apps/desktop/tests/connection.test.mjs
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
```

Optional browser checks use an existing Playwright installation (no automatic
downloads): set `PLAYWRIGHT_MODULE` to its module directory if it is not on the
Node resolution path, then run `node apps/desktop/tests/browser.cjs`. This starts
a temporary loopback-only test server and closes it afterward. Browser test
doubles do not prove native IPC or an agent connection.

On Windows, set `FORGE_DESKTOP_EXE` to the built development executable and run
`node apps/desktop/tests/native.cjs` to check the actual WebView-to-Rust identity
call. This test enables loopback WebView debugging only for its child process,
then closes the app and removes its temporary WebView profile.
The normal application does not enable a debugging port.

The app has an independent Cargo workspace and lockfile so desktop dependencies
do not expand core builds. Static frontend assets need no npm install, dev
server, CDN or runtime download. This is a small first slice, not a commitment
against using a frontend framework when component complexity warrants it.

## Boundaries

- Rust exposes only `app_info`, an identity read with no project access.
- No shell/filesystem plugins, network listener, credentials or project database.
- Forge retains project-state ownership; an agent adapter will be added separately.
- Preview content must never share privileged application IPC access.
- The approved C direction uses lavender, deep plum, coral and editorial art.
  This shell establishes layout and typography, not the final illustration assets.
  The application icon uses approved direction C: coral/lavender petals on a plum
  rounded tile. `src-tauri/icons/source.png` is the retained raster master;
  `src-tauri/icons/export.ps1` exports the PNG and multi-resolution Windows ICO
  (16, 20, 24, 32, 48, 64, 128 and 256 pixels) using Windows System.Drawing.
  The built-in image tool produced the isolated asset from the approved C board,
  then removed the baked checkerboard to obtain actual alpha transparency.
  Prompt intent: preserve direction C, isolate one icon, no wordmark or labels,
  transparent corners. This icon change does not publish an installer.
- Responsive styling does not establish mobile agent execution or remote access.
- Documentation is English; the initial user-facing UI is Portuguese.

Work tracking: GitHub issues #81, #82 and #83. Full agent conversation is #85.
