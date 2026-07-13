# Verification guide

Verification should be proportional while editing and cumulative at a coherent
publication boundary. Re-running the full Rust workspace for every small change
wastes time without increasing coverage.

## Tier 0: documentation and static hygiene

```bash
cargo fmt --all -- --check
git diff --check
python scripts/generate-workspace-layout.py --check
cargo run -p forge-core-command-surface --example generate_command_surface_docs -- --check
```

For documentation-only changes, also verify local Markdown links and inspect
all command snippets against generated command help. A simple local-link check:

```bash
python - <<'PY'
from pathlib import Path
import re
from urllib.parse import unquote
for document in Path('.').rglob('*.md'):
    if any(part in {'.git', 'target', 'target-test'} for part in document.parts):
        continue
    text = document.read_text(encoding='utf-8-sig')
    for target in re.findall(r'\[[^]]*\]\(([^)]+)\)', text):
        path = target.split('#', 1)[0]
        if not path or '://' in path or path.startswith('mailto:'):
            continue
        assert (document.parent / unquote(path)).resolve().exists(), (document, target)
PY
```

## Tier 1: focused boundary

Run the package and integration tests that own the change:

```bash
cargo test -p <package>
cargo test -p <package> --test <integration-test>
cargo clippy -p <package> --all-targets -- -D clippy::pedantic
```

Contract changes normally require the contract crate, semantic decision crate,
consumer/kernel tests, CLI adapter test, and aggregate validator. Persistence or
authority changes require recovery, stale-CAS/replay, tamper, and zero-write
failure cases.

## Tier 2: generated release subjects

When embedded workflow release material changes, run the relevant examples from
`crates/forge-core-decisions/examples/` with `--check`. CI currently checks the
foundation, registry, core-assurance, assurance-operations,
agent-native-continuity, and retirement generators. Use
`.github/workflows/ci.yml` as the executable list.

Generated equality proves reproducibility, not behavioral sufficiency. Review
the changed subject, scenario outcomes, independent authorization, and
compatibility boundary.

## Tier 3: coherent slice gate

Before committing a medium/large slice, run the normal workspace gate. The
expensive P6d real-process journey is feature-gated and is not repeated here:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D clippy::pedantic
cargo run -p forge-core-cli -- validate --root .
cargo test --workspace
```

This gate covers the default workspace. Do not describe it as covering the
feature-gated P6d journey.

## Tier 4: publication and release

Publication additionally runs the expensive cumulative journey once, after
all coherent slices are integrated:

```bash
cargo test -p forge-core-cli --test domain_pack_cli_e2e \
  --features expensive-p6d-e2e \
  p6d_workflow_journey::p6d_reference_pack_real_journey -- --exact
```

This is additive to the Tier 3 workspace gate. When validating from scratch,
`cargo test --workspace --features expensive-p6d-e2e` is the equivalent single
combined run. CI uses the exact filtered command so it does not repeat the
default workspace tests. Publication also requires:

- generated files clean;
- aggregate validation anchors at their declared values;
- supported Linux/Windows checks;
- packaged-binary smoke tests;
- archive content inspection;
- checksum and Sigstore verification;
- version/tag/changelog/docs agreement;
- current installation path tested from a clean location;
- residual risks documented.

## Evidence report template

```text
Scope:
Files/contracts:
Focused commands:
Focused results:
Generated checks:
Cumulative gate:
Platform/release smoke:
Residual risks:
```

A green command is evidence only for the surface it actually covers. The final
claim must match the union of executed gates.
