# Forge Desktop 0.1.75 alpha.1

**Find your project's files, inspect them and ask for a change.** The result
area now lists actual pages, images, documents and other supported files from
the project folder. Search by name/path, filter by type and select a file to
use the existing protected preview. Change requests preserve your draft and
are never sent automatically. Refreshing the list also happens after an agent
turn; failed reads keep the previous list with an explicit warning.

**Try a locally running app.** Loopback addresses cited in completed agent
messages appear beside the result. An explicit confirmation opens the address
in the browser. Receiving a link never starts a server, executes a command or
publishes anything. The app must already be running. You can prepare a change
request tied to that address without sending it.

No second project registry, file-content index, server manager or permissions
system was added. Discovery is metadata-only and bounded: 100 files, four
subdirectory levels and 2,000 inspected entries. Hidden/dependency/build
folders and symlinks/Windows junctions are skipped. A partial-list warning
points to the existing file chooser. Files appearing here are not proof of
completion or publication. Core stays at 0.13.3.

## Verification and limits

Focused Rust discovery checks and headless browser checks passed. Browser
checks cover search/filter, preview selection, preserved change drafts,
failed refresh, missing files, stale project responses, explicit app opening
and narrow layouts with enlarged text. Native installation/journey results
and tested hashes are recorded in the Desktop README.

The native journey uses a simulated Codex server, real project/file authority
and a real interactive loopback page. Native confirmation/cancellation is checked without dispatching the OS
browser in the final run; accepting/opening is covered by a browser double.
The actual local page is exercised in a separate headless browser. This does not prove a real model starts an app or that an
old cited address is still live. Protected Forge previews remain scriptless.

Unsigned Windows x64 alpha; manual installer updates. Secure remote mobile
access, physical-phone keyboard behavior, manual screen-reader use and
clean-device completed login remain unvalidated. No workspace/broad browser
suite or manual GitHub CI, and no claimed percentage of productivity or
subscription savings.
