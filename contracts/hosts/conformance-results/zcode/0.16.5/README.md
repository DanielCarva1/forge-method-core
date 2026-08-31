# ZCode CLI 0.16.5 on native Windows

This is the current native-Windows ZCode CLI result recorded for SD-06. It
covers ZCode Desktop `3.10.1` (file version `3.10.1.6272`), its bundled CLI
`0.16.5`, Forge `0.12.0-alpha.47`, and Windows `10.0.26200` on x86_64. It does
not claim coverage of the ZCode GUI or another version.

## Plain result

The real public CLI was started with a disposable project and isolated ZCode
storage. It accepted the prompt path and reached the model service, but the
request was rejected as unauthorized before the first agent turn. Because no
agent turn began, none of the eight Forge capabilities could be observed.

The honest result is therefore eight `unsupported` capabilities with the typed
gap `zcode_cli_unauthorized_before_first_turn`. This describes the tested CLI
journey only. ZCode Desktop continuing to work in its normal window does not
prove that an isolated CLI session can reuse the Desktop login.

The retained Desktop identity comes from the live Windows executable metadata.
It corrects the earlier planned `3.9.2` identity before publication; the
installed product and file versions are `3.10.1` and `3.10.1.6272`.

## Safety and retention boundaries

- No `computer use`, GUI automation, or private ZCode test hook was used.
- No token, credential, raw HTTP output, or CLI transcript is retained.
- The bridge and closed observation were temporary. Their hashes remain bound
  in the verified bundle, but their source files are not retained.
- Cleanup removed the disposable project, isolated storage, temporary home,
  bridge, and logs: four temporary roots and 38,097,147 bytes in total.
- A discarded setup attempt touched the normal ZCode CLI config before any
  ZCode test command ran. It was immediately restored to the exact known
  151-byte content. The altered file was never used by a ZCode execution.
- Cleanup did not copy, delete, or rewrite the normal per-user `.zcode` state.

## Product meaning

The generic Forge protocol represented the failure without ZCode-specific Rust
code, a permanent adapter, or a new result format. The next product task is to
establish a safe public CLI login and rerun this same corpus. Until then, ZCode
does not qualify as a working initial-release host.

Verify the retained bundle from the repository root:

```text
forge-core host-conformance verify --bundle-dir contracts/hosts/conformance-results/zcode/0.16.5/bundle --json
```

Passing that command proves the files and derived result still match. It does
not turn the failed CLI authentication into host support.
