# Forge Desktop 0.1.45 alpha.1

First-time ChatGPT access is easier: the temporary device code now has a
**Copiar código** action beside it. The app confirms success or explains how
to select the code manually if clipboard access fails. The draft stays in
place, and entering the account does not send it automatically.

## Verification and limits

The browser UI suite covered successful and failed copy attempts, the
preserved draft, and the access controls at 360px with enlarged text. A
hidden native Windows WebView test confirmed the code and copy action are
visible, the clipboard API is available, canceling keeps the draft, and no
early Send occurs. It did not copy to the system clipboard or complete a real
provider login. Separately, a hidden native read-only test reopened a real
39-message Codex conversation after a full app-process restart and confirmed
that its previously selected HTML result appeared automatically before
another file click; no Send or project-file edit occurred.

This remains an unsigned Windows x64 alpha without an auto-updater. The
bundled Forge core remains pinned to 0.13.2. Clean-machine setup, physical
mobile access, provider login completion, and manual screen-reader acceptance
remain unverified. Per-model tokens, BRL-equivalent cost, and Pro quota
attribution remain UNKNOWN; no savings are claimed. No manual GitHub CI was
triggered for these changes.

## Candidate and publication

One NSIS candidate was built: `Forge_0.1.45_x64-setup.exe`, 123,142,647
bytes, SHA-256
`001123DC5D143F36C3D96587B605F2525AB1AF992DA1A8FAECF42DBFD97B06D6`.
Silent installation over public 0.1.44 returned exit 0 without changing the
candidate bytes. The installed executable reports ProductVersion 0.1.45,
SHA-256
`E3E71BAC16B63CCA976CF6FBC6C7B4021254B71EE3E78E323DED1E1285416B78`.
Hidden native first-use, real-result/restart, and general shell smokes passed
on that installed candidate. Public-download readback remains to be recorded
after publication; source commit alone does not update the app.
