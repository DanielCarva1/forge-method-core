# ZCode CLI 0.16.5 on native Windows

This is the current native-Windows ZCode CLI result recorded for SD-06. It
covers ZCode Desktop `3.10.1` (file version `3.10.1.6272`), its bundled public
CLI `0.16.5`, Forge `0.12.0-alpha.49`, and Windows `10.0.26200` on x86_64. It
does not claim coverage of the ZCode GUI or another version.

## Plain result

The real public CLI completed one authenticated Solo Cooperative journey in a
disposable project. ZCode ran in `yolo` mode, so host permission buttons were
not part of the journey. Forge still governed accepted intent, evidence,
isolation ownership, promotion, readback, and replacement-agent recovery.

All eight capabilities are `partially_supported`: 21 assertions passed, two
were not exercised, and the native-Windows bridge assertion was not applicable.
The unexercised checks were ambiguous-root rejection and rejection when no
isolation exists. A wrong-owner claim was exercised and rejected before Forge
saved an isolation.

The result remains a same-owner candidate, not official ZCode support. The
temporary bridge reports closed observations, and this Forge build has no
ZCode-native verifier. Forge can verify the bundle but cannot prove that the
host itself produced every reported action.

## Journey observed

- Start Forge activated once in one continued authenticated CLI chat and kept
  versioned JSON readback.
- Read-only guidance left project and Forge state unchanged.
- Human intent and the agent proposal stayed separate and the accepted intent
  was read back.
- Cooperative evidence was admitted and read back; a tampered binding stayed
  rejected.
- ZCode edited only its linked Git worktree. A claim owned by another agent was
  rejected before isolation persistence.
- Promotion preview covered only the intended README. Apply was read back and
  an exact retry returned `already_committed` without a second mutation.
- A fresh ZCode CLI session, without transcript continuation, reconstructed the
  objective, Phase, completed Work Focus, delivered change, and next safe step.

## Safety and retention boundaries

- No `computer use`, GUI automation, or private ZCode test hook was used.
- No token, credential, raw HTTP output, raw CLI output, or transcript is
  retained.
- The bridge and closed observation were temporary. Their hashes remain bound
  in the verified bundle, but their source files are not retained.
- The normal per-user ZCode state was used by the authenticated public CLI but
  was never copied into retained material and is outside cleanup scope.
- The disposable project, Forge sidecar, worktree, and generation files were
  cleanup-only test material, not product state.

## Product meaning

ZCode can complete the current Solo Cooperative journey through its public CLI
on native Windows without permission-button friction. This does not make ZCode
an officially supported or field-verified host. It provides a strong candidate
result and leaves the two unexercised checks visible instead of guessing.

Verify the retained bundle from the repository root:

```text
forge-core host-conformance verify --bundle-dir contracts/hosts/conformance-results/zcode/0.16.5/bundle --json
```

Passing that command proves the files and derived result still match. It does
not prove host-native authenticity.
