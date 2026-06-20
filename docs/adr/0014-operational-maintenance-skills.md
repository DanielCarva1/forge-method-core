# ADR 0014: Operational Maintenance Skills

## Status

Accepted

## Context

Forge Method keeps `$forge-method` as the single user-facing product entrypoint. Users should not have to memorize workflow commands or choose between product modes before the runtime has routed their project.

Some actions are not product work. Reloading stale package state and manually updating an installed plugin are operational maintenance tasks. Treating them as product workflows would pollute the phase model, but hiding them behind normal startup makes support harder when a user explicitly wants to repair or update the install.

## Decision

`$forge-method` remains the only product entrypoint.

`$forge-reload` and `$forge-update` are approved Operational Maintenance Skills:

- `$forge-reload` re-anchors the current chat on the active package and filesystem state.
- `$forge-update` runs an explicit Git marketplace update, reads local release notes, and prints a short human summary.

Both skills must avoid mutating project progress. They may call runtime launchers or updater scripts, but they must not create stories, change phases, or edit project artifacts as part of maintenance.

If a package update changes skill instructions, the current chat should remain useful. Opening a new thread may be suggested only as an optional way to reload fresh skill text.

## Consequences

- Product workflows stay inside `$forge-method`, Guidance Engine, workflow refs, and facilitation packs.
- Support and distribution have explicit commands for stale instructions and manual updates.
- Non-Git-marketplace installs are not guessed or patched in place; `$forge-update` explains the required Git marketplace install shape and prints the install command.
