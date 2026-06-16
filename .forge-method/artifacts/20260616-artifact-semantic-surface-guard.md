# Artifact Semantic Surface Guard

## Gap

Specialized artifact checks existed for spec, research, game, test, document, discovery, and enterprise artifacts, but the shared artifact surface only checked active file existence, stale summaries, and stale guidance markers.

That allowed a malformed artifact with `workflow: write-spec` to fail `artifact spec-check` while still passing `artifact verify`, `snapshot.quality.artifacts`, and `gate`.

## Change

`artifact_findings` now routes active artifacts that declare a known artifact workflow through the matching semantic validator.

Covered workflow families:

- discovery closeout
- document utility
- spec kernel
- research scan
- test utility
- game artifacts
- enterprise artifacts

Narrative or historical artifacts without a recognized `workflow` field remain valid legacy artifacts. This keeps old project records readable while enforcing semantic checks for generated or workflow-declared artifacts.

## Proof

Added a regression fixture where `.forge-method/artifacts/bad-spec.md` declares `workflow: write-spec` but omits required spec-kernel fields.

Expected behavior:

- `artifact spec-check` fails.
- `artifact verify` fails through the shared artifact surface.
- `snapshot.quality.artifacts.errors` exposes the same failure.
- `gate` fails with the same artifact error.

Also added a game-brief verification fixture to prove `kind: game-brief` resolves to `game-check`, not discovery closeout validation, when both families mention `workflow: game-brief`.

Validation passed:

- `python -m unittest discover -s tests`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
- `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`
- `python skills\forge-method\scripts\forge_method_runtime.py artifact verify --root .`
- `python skills\forge-method\scripts\forge_method_runtime.py gate --root . --require-evals`
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --json`
- `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`
