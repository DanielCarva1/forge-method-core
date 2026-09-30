# Forge Desktop 0.1.79 alpha

## What changes

- Andamento shows each recorded objective directly in its history card.
- Expanding a later revision shows exactly which objective wording, agreements
  and precautions were added or removed compared with the preceding visible
  revision. This needs no model request and does not send a chat message.
- The first visible revision is never compared with an invented predecessor.
  Empty history and truncated history retain their explicit limits.

## Limits

The comparison is literal text, not an explanation of why a choice was made or
proof of a new human approval. It reuses the existing read-only objective
history; opaque legacy decision resolutions still cannot be shown as invented
plain-language agreements (#92 remains partial).

Core remains 0.13.4. Secure phone access is not implemented: the app still uses
local IPC and Codex stdio, not a network server. Manual unsigned Windows alpha
installation and existing preview limits remain unchanged.

## Verification

Focused comparison unit tests pass. Native installed-app verification and
installer/public-download receipts are recorded in the session checkpoint.
Controlled historical wording is identified as such, not a real model choice.
