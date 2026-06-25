# Forge Method Runtime Audit — Multi-Agent Readiness & Quality

- audit_at: 2026-06-22
- auditor: review subagent (read-only)
- scope: `skills/forge-method/` runtime v1.34.1 (byte-identical on Pi and Codex)
- goal: assess readiness for safe multi-agent orchestration, human-guided experience, and agent-facing doc quality

---

## STRENGTHS — preserve these

### S1. Append-safe event log is the right backbone
`append_ledger` (scripts/forge_method_runtime.py:2775) and `append_artifact_index` (scripts/forge_method_runtime.py:2944) both use `open("a", encoding="utf-8")` with one JSON line per write. POSIX append is atomic for lines under the pipe buffer. This is the canonical "one JSONL file written by many, edited by none" pattern — the exact substrate distributed-systems literature (munderdifflin, ESAA, rezzed.ai) recommends for multi-agent coordination. **This is your multi-agent foundation. Do not break it.**

### S2. Guidance-safety guardrails are sophisticated and enforced
`guidance_safety_errors` (scripts/forge_method_runtime.py:4697) + `WORKFLOW_MISLEADING_GUIDANCE_PATTERNS` (scripts/forge_method_runtime.py:207) enforce 4 anti-patterns on write: no relying on chat memory, no following stale state, no procedural continue-confirmations, no catalog dumps. `validate_state_guidance_safety` (scripts/forge_method_runtime.py:2645) runs this against `state.yaml` fields (`guide_summary`, `last_route_reason`, `next_action`) on **every** `write_state`. `require_record_guidance_safety` (scripts/forge_method_runtime.py:2667) extends this to stories, inputs, review findings, artifact index. This is a rare and valuable feature — most agent frameworks have no write-time guardrails.

### S3. Facilitation packs are genuinely strong human-UX
All 34 facilitation packs (facilitation/*.md) follow a rich schema: `purpose`, `open_floor` (PT-PT bilingual hook), `source_material`, `follow_up_batches`, `conversation_stages`, `elicitation_options`, `facilitator_moves`, `quality_bar`, `anti_patterns`, `paths` (fast/deep), `checkpoint_options`, `domain_examples`, `artifact_rules`, `headless`. Example: `facilitation/discover-intent.md` elicitation_options include `example_mining`, `anti_goal`, `first_user`, `evidence_probe`, `closeout_probe` — these are real facilitation techniques, not generic prompts. This is the runtime's strongest identity asset. Preserve it.

### S4. Workflow structure is machine-validated with compactness limits
`validate_workflow_file` (scripts/forge_method_runtime.py:4821) enforces 7 required sections (`WORKFLOW_REQUIRED_SECTIONS` L167: trigger, inputs, steps, outputs, done_when, blocked_when, handoff) plus compactness caps (`validate_workflow_compactness` L4647: max lines, words, bullets). Workflows that grow too verbose are rejected by the gate. This is rare discipline — most "spec" formats have no size enforcement.

### S5. Collaboration workflows already exist as concepts
The catalog already registers `team-operating-model`, `product-area-map`, `trunk-based-plan`, `collaboration-handoff`, `repo-split-plan`, `council-decision` (catalog/workflows.json). `council-decision` even declares modes `["debate", "decision", "parallel", "agent-team", "subagent", ...]`. `build-story-work-order.md` template (templates/) is the most contract-like artifact: it has `owner`, `branch`, `pr`, `product_area`, `dependencies`, `do_not_prompt`, `stop_only_when`. The conceptual vocabulary for multi-agent is present.

### S6. Decision-source traceability
Stories carry `decision_sources` (scripts/forge_method_runtime.py:7039 `cmd_story_add` → `prepare_story_decision_sources`), and `cmd_gate` blocks done-stories that lack an explicit decision source. This is strong provenance — every build traces back to a spec artifact.

---

## GAPS for multi-agent

### G1. CRITICAL — `state.yaml` write is a full overwrite with zero concurrency control
`write_flat_yaml` (scripts/forge_method_runtime.py:857) does `path.write_text("\n".join(lines))`. No `flock`, no `fcntl`, no `FileLock`, no version field, no optimistic-concurrency check (confirmed: grep for `flock|FileLock|lockfile|semaphore|threading.` = empty across the 16,851-line runtime). `write_state` (scripts/forge_method_runtime.py:2689) calls this. **Two agents calling `transition`/`story_start`/`handoff`/`checkpoint`/`council_run`/`correct_course`/`input_answer` concurrently → last-writer-wins, silent data loss.** Evidence: every state-mutating command follows `load_state_or_fail` → mutate dict → `write_state` (transition L7026, story_start L7198, handoff L15264, checkpoint L14990, council_run L13369, correct_course L13448, input_answer L7392). Severity: **BLOCKER for any multi-agent use.**

### G2. CRITICAL — `handoff` and `checkpoint` are NOT append-safe (they mutate state)
`cmd_handoff` (scripts/forge_method_runtime.py:15245): after writing the handoff `.md`, it does `state["next_action"] = ...; write_state(root, state)`. `cmd_checkpoint` (scripts/forge_method_runtime.py:14973): same pattern — `state["next_action"] = next_action; write_state(root, state)`. A worker agent writing a handoff to signal another agent will **clobber** any concurrent state write from the driver. This defeats the intended "workers emit handoffs, driver owns state" model. Severity: **BLOCKER.**

### G3. CRITICAL — no agent registry / no per-agent state
The `agents/` directory contains only `openai.yaml` (display adapter) and `profiles/` (role descriptions: facilitator, implementer, planner, etc.). There is **no registry of running agents, no agent identity, no per-agent state file**. Confirmed: grep for `multi.?agent|concurren|file.?lock|claim.?lock|agent.?registry|agent.?owner|worktree|optimistic|version.?number|principal` across all references/templates/facilitation/agents/catalog = **zero matches**. Each agent must share the single `state.yaml`, forcing serialization. Severity: **BLOCKER.**

### G4. HIGH — no owner/claimant attribution anywhere
`set_story_status` (scripts/forge_method_runtime.py:7170) has no `owner`/`claimant`/`assigned_to` field. `append_ledger` (scripts/forge_method_runtime.py:2775) writes `{"ts", "event", "payload"}` — **no `agent_id`, no `author`, no `principal`**. When two agents both run `story_start` on the same story, the second silently overwrites the first's status. The event log cannot answer "who did this?" — a multi-agent forensics gap (jatinbansal: "turns every shared-memory bug into a forensics nightmare"). Severity: HIGH.

### G5. HIGH — council "orchestration" is descriptive, not executable
`council_orchestration_plan` (scripts/forge_method_runtime.py:13284) returns a dict describing workers, but `runtime_policy` is just a string: "Use real Codex subagents only when available and the jobs are independent; otherwise run the same roles serially." The runtime **does not spawn workers**. `cmd_council_run` (L13335) prints a transcript and writes an artifact, but executes nothing in parallel. This is "orchestration theater" — the `agent-team`/`parallel`/`subagent` modes are labels, not behaviors. Severity: HIGH (limits the flagship collaboration feature).

### G6. MEDIUM — no claim/lock mechanism for Product Areas
`product-area-map` workflow exists, but there is no runtime primitive to *claim* an area. Two agents working different areas still share `state.yaml` and `sprint.yaml`, so the area boundary is documentary, not enforced. The file-based-lock pattern (Anthropic case study: `current_tasks/<id>.lock` + git push rejection) is not implemented. Severity: MEDIUM.

### G7. MEDIUM — `sprint.yaml` is read-modify-write, shared across all agents
`update_sprint` (called from `set_story_status` L7187) recomputes story counts from the `stories/` directory and overwrites `sprint.yaml`. Two agents finishing stories concurrently → count race. Severity: MEDIUM.

---

## GAPS for human-guided experience

### H1. MEDIUM — research happens *in the agent*, not *via* the runtime
`routed_research_workflow` (scripts/forge_method_runtime.py:9522) is a keyword-matching router that *names* which research workflow to run, but the runtime has **no web_search/deep_research/fetch tools** (confirmed: grep for tool-defining patterns = empty). Research is delegated to whatever hosting agent (Pi/Codex) provides. This is fine for identity, but means the facilitation packs cannot *guarantee* "do research before asking." A research step is advisory, not enforced. Severity: MEDIUM.

### H2. MEDIUM — clarifying-question UX is not a first-class runtime primitive
`human_input_add` (scripts/forge_method_runtime.py:7338) writes a durable input file, but the *phrasing* of the question is left entirely to the agent reading the facilitation pack. There is no "ask exactly N clarifying questions, then batch" mechanic, no question-quality gate, no "did the human feel heard?" check. The facilitation packs describe elicitation options richly, but nothing enforces them. Severity: MEDIUM.

### H3. LOW — no "teach/explain" workflow beyond `teach-testing`
Only `teach-testing` (catalog) exists as a dedicated learning workflow. The 2026 literature (Amazon Science, design@tive "interrogability") emphasizes that the best agentic UX *teaches while it works* and exposes reasoning as inspectable. Forge has the bones (council transcript, evidence), but no general "explain my reasoning / teach this concept" workflow across domains. Severity: LOW (opportunity, not defect).

### H4. LOW — progress-visibility is pull-based, not push
There is no "what changed since I last looked" diff view. `checkpoint-preview` and `context-recovery` exist but require explicit invocation. The 2026 AX literature (mer.vin "Agent Experience") stresses ambient progress visibility for long-running work. Severity: LOW.

---

## GAPS for agent-facing docs

### D1. HIGH — no JSON schema / typed contract layer
Workflows are validated structurally (7 required sections) but **not typed**. Templates are loose `key:` lists with no schema file. There are zero `*.schema.json` / `*.jsonschema` files (confirmed by find). The 2026 literature is converging on machine-readable contracts (GitHub Spec Kit `main.md`, PBC `.pbc.md`, MDA frontmatter, derive-spec). Forge's workflow refs are human+agent-readable Markdown but lack the validation-on-write that typed contracts enable. Severity: HIGH.

### D2. HIGH — anti-patterns are only 4 regex patterns
`WORKFLOW_MISLEADING_GUIDANCE_PATTERNS` (scripts/forge_method_runtime.py:207) catches: chat-memory reliance, stale-state following, procedural confirms, catalog dumps. These are excellent but **minimal**. Multi-agent introduces a whole new anti-pattern class (clobbering state, acting without a claim, persisting to a shared file without attribution) that is not yet encoded. Severity: HIGH.

### D3. MEDIUM — workflow `handoff:` section is unstructured prose
The `handoff:` section of each workflow (e.g. workflow-build-story.md) is a bullet list of free text. There is no required schema for it (unlike `done_when`/`blocked_when`). A better contract would be `handoff: { preserve: [...], do_not: [...] }`. Severity: MEDIUM.

### D4. MEDIUM — no explicit "contract" artifact type for inter-agent interfaces
`build-story-work-order.md` is the closest thing to a typed contract (has owner/branch/pr/deps/do_not_prompt), but it is a build artifact, not a reusable interface-definition type. Multi-agent needs a `agent-contract` artifact that declares: agent_id, permitted_write_paths, permitted_commands, product_area, dependencies. Severity: MEDIUM.

---

## REFACTOR OPPORTUNITIES — minimal, additive, backward-compatible

All changes below are **opt-in**: a project without the new files behaves exactly as v1.34.1. This is the key to not breaking existing users.

### R1. Add `version` field to `state.yaml` + optimistic-concurrency check
- In `write_state` (L2689): add `state.setdefault("version", "0")` then before writing, if `args.expected_version` was passed and differs from disk → reject with a typed conflict error.
- Backward compat: if no `expected_version` passed, behave as today (clobber). Existing callers unaffected.
- New callers (multi-agent) pass `--expected-version` to detect races.
- Impact: **unlocks safe multi-writer detection** with ~15 lines.

### R2. Add `owner`/`agent_id` attribution to `append_ledger`
- Change `append_ledger` (L2775) signature: `entry = {"ts", "event", "agent_id": agent_id or "default", "payload"}`.
- Add `--agent-id` flag to all mutating commands; default to `"default"` when absent.
- Backward compat: existing callers omit the flag → `"default"` → identical ledger.
- Impact: **enables forensics + per-agent audit** with ~20 lines. Zero breakage.

### R3. Make `handoff` and `checkpoint` append-only (stop mutating state)
- `cmd_handoff` (L15245): remove the `state["next_action"] = ...; write_state(...)` block. Instead, append the next_action into the handoff `.md` only. Add an optional `--request-state-change` that writes a `handoff-request` entry to a new append-only `requests.ndjson` that the driver polls.
- `cmd_checkpoint` (L14973): same — checkpoint should be a durable memory write, not a state mutation.
- Backward compat: add `--update-state` flag (default false in multi-agent mode, true in single-agent legacy mode).
- Impact: **makes workers safe**; removes the biggest clobber vector.

### R4. Add `agents/registry.yaml` + per-agent state files
- New file `.forge-method/agents/registry.yaml`: lists active agents `{agent_id, runtime (pi|codex), product_area, role, state_file}`.
- New dir `.forge-method/agents/<id>.yaml`: per-agent local FSM snapshot (phase, status, next_action for *that agent's* work).
- The global `state.yaml` becomes the *integration* state (root integrator). Workers read it but write their own.
- Backward compat: if `agents/registry.yaml` absent → single-agent legacy mode (unchanged).
- Impact: **isolates agent state**; the core idea from actor model + repo-split-plan.

### R5. Add `claims/<area>.lock` file-based coordination
- New command `forge claim --area <id> --agent-id <id>` writes `claims/<area>.lock` with `{agent_id, ts}`; `forge release --area <id>` removes it.
- `story_start`/`transition` check: if a registry exists and the calling `--agent-id` doesn't own the relevant product area → reject with a typed error.
- Backward compat: no registry → no claim check.
- Impact: **prevents two agents on the same story** (Anthropic file-lock pattern).

### R6. Encode multi-agent anti-patterns into guidance safety
- Extend `WORKFLOW_MISLEADING_GUIDANCE_PATTERNS` (L207) with: "do not write shared state without a claim", "do not clobber state without checking version", "do not persist worker transcript as integration memory", "do not act outside your product area".
- Impact: **write-time enforcement** of multi-agent discipline using the existing guardrail engine.

### R7. Add a typed `agent-contract` artifact + JSON schema
- New template `agent-contract-artifact.md` with typed fields: `agent_id`, `product_area`, `permitted_write_paths[]`, `permitted_commands[]`, `dependencies[]`, `merge_contract`, `merge_owner`.
- New eval kind `contract-check` that validates an agent's actions against its contract.
- Impact: **machine-checkable boundaries**; aligns with 2026 PBC/MDA/derive-spec direction.

---

## RISKS — what could break existing users

### RISK-1 (HIGH): Adding a `version` field changes `state.yaml` shape
Any external tool that parses `state.yaml` and does strict key checking could break. **Mitigation:** additive only — new key, no removed keys. Existing readers ignore unknown keys. Test: the runtime's own `read_flat_yaml` (L841) ignores any key it doesn't expect.

### RISK-2 (HIGH): Making `handoff`/`checkpoint` not mutate state changes behavior
Users (and the current guidance-engine) may rely on `handoff` updating `next_action`. **Mitigation:** default `--update-state=true` preserves legacy behavior; only multi-agent mode sets it false. Document the flag clearly.

### RISK-3 (MEDIUM): `--agent-id` on every command is noisy for single-agent users
**Mitigation:** make it optional everywhere with `"default"` fallback. A wrapper script or env var `FORGE_AGENT_ID` can inject it for multi-agent setups without touching single-agent users.

### RISK-4 (MEDIUM): Per-agent state files fragment the "one source of truth"
If agents diverge from the integration state, reconciliation cost rises. **Mitigation:** keep `state.yaml` as the authoritative integration state; per-agent files are work-in-progress snapshots. The driver reconciles on merge (repo-split-plan already models this as "root integration contract").

### RISK-5 (LOW): Pi ↔ Codex compatibility
The core is byte-identical (v1.34.1 on both). Any runtime change must land in forge-method-core (the shared repo) so both consume it. **Mitigation:** develop against the shared core; test in both adapters. The risk is low because there is only one runtime to change.

### RISK-6 (LOW): Ledger grows unbounded
`ledger.ndjson` is already append-only and unbounded (munderdifflin notes rotation as a concern). Multi-agent increases write rate. **Mitigation:** add rotation/snapshot as a separate, non-breaking follow-up.

---

## TOP 10 RECOMMENDATIONS (ranked impact × 1/effort)

| # | Recommendation | Impact | Effort | Why |
|---|---|---|---|---|
| 1 | **R2: add `agent_id` attribution to ledger + `--agent-id` flag** | HIGH | LOW | Enables forensics, zero breakage, foundation for all multi-agent features |
| 2 | **R1: add `version` field + optimistic-concurrency opt-in** | CRITICAL | LOW | Unlocks safe multi-writer detection; the single highest-leverage change |
| 3 | **R3: make `handoff`/`checkpoint` append-only (`--update-state` flag)** | CRITICAL | MED | Removes the biggest clobber vector; makes workers safe |
| 4 | **R6: encode multi-agent anti-patterns in guidance safety** | HIGH | LOW | Leverages your existing, excellent guardrail engine; write-time enforcement |
| 5 | **R4: add `agents/registry.yaml` + per-agent state** | CRITICAL | MED | The core isolation primitive; converts single-writer to multi-worker |
| 6 | **R5: add `claims/` file-lock + claim check** | HIGH | MED | Prevents duplicate work; Anthropic-proven pattern |
| 7 | **D2/R7: typed `agent-contract` artifact + eval kind** | HIGH | MED | Machine-checkable boundaries; aligns with 2026 contract trends |
| 8 | **G5: make council `agent-team`/`parallel` actually spawn workers** | HIGH | HIGH | Turns orchestration theater into real orchestration; flagship feature |
| 9 | **D1: add JSON schema for workflow refs + templates** | MED | MED | Enables external tooling and stricter validation; future-proofing |
| 10 | **H1: add an enforced "research-before-asking" step option** | MED | MED | Strengthens guided identity; research is currently advisory |

**Recommended sequencing:** 1 → 2 → 3 → 4 first (the "safe multi-writer" minimum, all LOW effort). Then 5 → 6 (isolation). Then 7 → 8 (contracts + real orchestration). 9 → 10 are polish. This ordering means you can ship multi-agent safety without changing the human experience, then layer richer orchestration on top.

---

## APPENDIX — collaboration workflow summary (one-line each)

| Workflow | Trigger → Outputs |
|---|---|
| `team-operating-model` | multiple agents use Forge → team operating model, owner/review policy, agent-use conventions |
| `product-area-map` | modularize/parallel work → Product Area map, owner+contract map, split candidate list |
| `trunk-based-plan` | need branch policy → trunk policy, PR/review/check rules, CODEOWNERS stance |
| `collaboration-handoff` | work moves between actors → branch/PR handoff, area status, owner+next actor, blockers |
| `repo-split-plan` | area needs standalone repo → split plan, standalone init contract, root integration handoff |
| `council-decision` | high-risk/taste decision → live debate, compact decision, orchestration plan, dissent map |
