# Help Oracle Guidance Safety Guard

- created_at: 2026-06-16T02:12:00+00:00
- status: help-oracle-guidance-safety-guard
- workflow: runtime-builder
- lifecycle: durable

## Problem

The workflow guidance guard protected compact `workflow-*.md` files, but Help Oracle and runtime outputs could still carry the same unsafe agent instructions through `next_action`, `human_next_step`, context boundaries, or recorded ledger payloads.

That left a gap between durable workflow validation and the runtime surface agents actually consume during fresh chats, reloads, resumes, and post-command guidance.

## Decision

- Reuse the misleading-guidance detector for arbitrary runtime guidance text.
- Make the detector position-aware so safe guidance like "use durable state instead of chat memory" passes while unsafe guidance like "use chat memory instead of durable state" fails.
- Add Help Oracle safety validation over structured oracle payloads, excluding executable command strings.
- Run Help Oracle safety from `audit`, so unsafe `next_action` output fails normal project validation.
- Rephrase an ambiguous recovery trigger so it describes the stale-chat condition without sounding like an instruction to rely on chat memory.

## Result

- `snapshot` and `resume --json` Help Oracle payloads now have a deterministic safety contract.
- `audit` rejects unsafe Help Oracle output generated from project state.
- Workflow guidance validation and runtime oracle validation share the same core rule.

## Validation

- focused Help Oracle and workflow safety tests passed
- `python skills\forge-method\scripts\forge_method_runtime.py audit --root .`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow validate`
- `python skills\forge-method\scripts\forge_method_runtime.py workflow compactness`
- `python skills\forge-method\scripts\forge_method_runtime.py parity replay --json`

## Next Gap

Run the full runtime validation batch, then keep auditing agent-facing runtime surfaces for stale or misleading route instructions without flattening human-facing guidance.
