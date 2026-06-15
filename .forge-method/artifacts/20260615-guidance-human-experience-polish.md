# Guidance Human Experience Polish

- kind: runtime-polish
- created_at: 2026-06-15
- scope: Guidance Engine human lede, runtime-builder routing, Reality/Evidence Gate filtering

## Observed Symptoms

- `guide` for correction/frustration printed route metadata first and felt like a catalog rather than a guided response.
- Runtime-builder polish requests that mentioned docs could route through `research-needed` because `docs` was treated as an evidence signal.
- Existing Forge projects showed Reality/Evidence Gate noise for correction/runtime polish requests, even when the question was not a new product idea or research decision.

## Decision

- Keep workflow references and JSON contracts compact for agents.
- Add a short contextual human lede to non-JSON `guide` output for existing Forge projects.
- Enrich `human_experience` with compact fields: `decision_summary`, `next_move`, `human_question`, `guardrail`, `compact_contract`, and `contract_split`.
- Treat "human experience + agent docs/state machine" wording as runtime-builder work.
- Show Reality/Evidence Gate on existing projects only when the final classification is idea/research-shaped: `research-needed`, `product-flow`, `creative-flow`, or `game-flow`.

## Proof

- Added `test_guidance_human_lede_and_runtime_builder_contract`.
- Targeted checks passed:
  - `python -m unittest tests.test_runtime.RuntimeTests.test_guidance_human_lede_and_runtime_builder_contract`
  - `python -m unittest tests.test_runtime.RuntimeTests.test_guidance_engine_routes_transcript_fixtures tests.test_runtime.RuntimeTests.test_parity_replay_command_validates_fixture_matrix`
- Full checks passed:
  - `python -m unittest discover -s tests` (70 tests)
  - `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-runtime.ps1`
  - `powershell -ExecutionPolicy Bypass -File .\scripts\verify-fast.ps1`
  - `powershell -ExecutionPolicy Bypass -File .\scripts\smoke-install.ps1`

## Files

- `skills/forge-method/scripts/forge_method_runtime.py`
- `tests/test_runtime.py`
- `docs/adr/0008-guidance-engine.md`
- `CHANGELOG.md`

## Next

- Review remaining post-parity polish surface and decide the next release/version batch.
