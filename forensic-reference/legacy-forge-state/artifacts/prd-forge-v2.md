# PRD — Forge Method v2.0: Flock Coordination

- kind: prd
- version: "2.0"
- status: specification-locked
- created_at: 2026-06-23T08:00:00Z
- inputs: [forge-flock-coordination-rfc-v3.md, 20260623-phase-b-poc-gap-report.md, 20260623-phase-b-poc-acceptance.md, forge-runtime-audit.md]
- constraints: [C1-preserve-fsm, C2-backward-compatible, C3-runtime-agnostic, C4-model-agnostic, C5-opt-in-facilitated, C6-commit-safe, C7-quality-packaging, C8-partner-experience]

## 1. Vision (one paragraph)

Forge Method v2.0 turns the single-agent state-machine runtime into a **runtime-agnostic coordination protocol for human+agent flocks**. Multiple humans, each operating multiple agents across Pi/Codex/Claude/OpenCode/the Forge App, work on one repo without corrupting shared state, without rogue commits, without anyone being a bottleneck. The `.forge-method/` directory becomes to human+agent coordination what `.git/` is to code coordination. **Commit-safe by construction; autonomy progressive and cyclic.**

## 2. Problem (what v1.34.1 cannot do)

| Blocker | ID | Impact |
|---|---|---|
| `write_flat_yaml` = full overwrite, no locking, no version | **G1** | Two concurrent agents on `state.yaml` = silent data loss |
| `handoff`/`checkpoint` mutate `state.next_action` | **G2** | Workers clobber the driver's integration state |
| No agent registry / per-agent identity | **G3** | No way to attribute work, detect collisions, or coordinate flocks |
| No lane claim primitive | **G5/G6** | Two agents can write the same files simultaneously |
| No integration verification in gate | **GAP-5** | Cross-lane type conflicts not caught before phase advance |
| Git add sweeps cross-lane files | **GAP-1** | Lane claims enforced at Forge level, not VCS level |
| Shared-type divergence undetected | **GAP-2** | Conflicting global type augmentations break project-wide build |

## 3. Solution (the v2 mechanisms, mapped to principles)

**Layer 1 — Concurrency-safe state (G1 fix):** `state.yaml` gains `version` field. `write_state` accepts `expected_version` + `agent_id`. On mismatch → `VersionConflict` (typed, retryable). Default `None` = v1.34.1 clobber (C2 backward compat). ✅ **IMPLEMENTED** (commit f0d1abe).

**Layer 2 — Append-only handoffs (G2 fix):** In fleet mode (`agents/registry.yaml` present), `handoff`/`checkpoint` do NOT mutate state. They append a request to `requests.ndjson`. The driver polls and applies. `--update-state` flag overrides for single-agent legacy. ✅ **IMPLEMENTED** (this session).

**Layer 3 — Fleet registry + agent identity (G3 fix):** `agents/registry.yaml` declares flocks + agents + lanes. `is_fleet_mode()` detects presence. `--agent-id` flag + `FORGE_AGENT_ID` env attribute every ledger entry. ✅ **HELPERS EXIST** (`is_fleet_mode`, `append_request`); CLI threading pending.

**Layer 4 — Lane claims (Principle 5, 18):** `claims/<lane>.lock` files with agent_id + TTL (30min) + heartbeat. Claim/release/heartbeat CLI commands. Lane boundary = write boundary.

**Layer 5 — Guidance safety extensions (Principle D2):** Multi-agent + autonomy anti-patterns encoded as write-time guidance safety checks: "do not write integration state without holding driver claim," "do not write without expected_version in fleet mode," "do not act outside your claimed lane."

**Layer 6 — Partner experience (Principle 14, 15, 16):** Research-always-on affordance in every facilitation pack. Grill-as-default at every decision-close point. Partner-grade presence directive (excited expert friend, matches energy).

**Layer 7 — Integration quality (GAP-1, 2, 5):** Integration gate runs `tsc --noEmit` + smoke test. `forge commit` wrapper respects lane claims (stages only claimed-lane files). Shared-types contract lane.

## 4. Constraints (non-negotiable)

- **C1:** Preserve each agent's state machine (FSM is the soul).
- **C2:** Backward compatible (no opt-in = v1.34.1 behavior, byte-compatible).
- **C3:** Runtime-agnostic (pure files, no runtime API calls in coordination layer).
- **C4:** Model-agnostic (facilitation is behavior, not model features).
- **C5:** Opt-in and facilitated (multi-agent surfaced through dialogue, never automatic).
- **C6:** Commit-safe by construction (claims + branch policy + reviewer gate + verification gates).
- **C7:** Quality packaging intact (gates, evals, decision-source traceability).
- **C8:** Partner-grade experience (excited expert friend, not a form).

## 5. Epics + Story Map (9 epics, 26 stories)

### Epic 1: Concurrency-Safe State (CRITICAL — G1 fix)
| Story | Title | Status | Priority |
|---|---|---|---|
| v2-001 | Version field + optimistic concurrency in write_state | ✅ done | CRITICAL |
| v2-002 | Auto-migration: add version:"0" to existing projects on first read | planned | HIGH |
| v2-003 | --expected-version + --agent-id CLI flags on advance/handoff/checkpoint/state commands | planned | HIGH |

### Epic 2: Append-Only Handoffs (CRITICAL — G2 fix)
| Story | Title | Status | Priority |
|---|---|---|---|
| v2-004 | Fleet-mode append-only handoff/checkoff (no state mutation) | ✅ done | CRITICAL |
| v2-005 | `requests poll` command — driver reads pending requests from requests.ndjson | planned | HIGH |
| v2-006 | `requests apply <id>` command — driver applies a request with version check | planned | HIGH |

### Epic 3: Fleet Registry + Agent Identity (CRITICAL — G3 fix)
| Story | Title | Status | Priority |
|---|---|---|---|
| v2-007 | agents/registry.yaml support + is_fleet_mode() detection | ✅ done | CRITICAL |
| v2-008 | --agent-id flag + FORGE_AGENT_ID env threaded through all mutating commands | planned | HIGH |
| v2-009 | Agent attribution in every ledger entry (agent_id field) | planned | HIGH |

### Epic 4: Lane Claims (HIGH — Principle 5, 18)
| Story | Title | Status | Priority |
|---|---|---|---|
| v2-010 | claims/<lane>.lock with TTL (30min) + heartbeat + claim/release commands | planned | HIGH |
| v2-011 | `lanes` command — show all lane claim statuses | planned | MED |
| v2-012 | Claim enforcement in guidance safety (anti-pattern: write file in unclaimed lane) | planned | MED |

### Epic 5: Guidance Safety Extensions (HIGH — Principle D2)
| Story | Title | Status | Priority |
|---|---|---|---|
| v2-013 | Multi-agent anti-patterns: write-without-driver-claim, write-without-version, act-outside-lane | planned | HIGH |
| v2-014 | Autonomy anti-patterns: approve-during-autopilot, spec-write-without-gate | planned | MED |

### Epic 6: Partner Experience (HIGH — Principle 14, 15, 16)
| Story | Title | Status | Priority |
|---|---|---|---|
| v2-015 | Research-always-on affordance added to all facilitation pack open_floor prompts | planned | HIGH |
| v2-016 | Grill-as-default trigger at every decision-close point (handoff, phase transition, spec lock) | planned | HIGH |
| v2-017 | Partner-grade presence directive in system prompt template + pack tone | planned | MED |

### Epic 7: Integration Quality (MED — POC gaps)
| Story | Title | Status | Priority |
|---|---|---|---|
| v2-018 | GAP-5: Integration gate (tsc --noEmit + smoke test added to gate command) | planned | MED |
| v2-019 | GAP-1: `forge commit` wrapper (stages only files in claimed lanes) | planned | MED |
| v2-020 | GAP-2: Shared-types contract lane + type-conflict detection in gate | planned | MED |

### Epic 8: Advanced Coordination (MED/HIGH — Principle G5, D4, 12)
| Story | Title | Status | Priority |
|---|---|---|---|
| v2-021 | Typed agent-contract artifact + contract-check eval | planned | MED |
| v2-022 | Council standup mode (status + cross-dep sync + hard-problem sharing) | planned | MED |
| v2-023 | Orchestration spawning (agent-team/parallel/subagent actually spawn) | planned | HIGH |
| v2-024 | Evolve Loop wiring: fix evolve-reentry-routing-gap (evolve → discovery, not builder) | planned | HIGH |

### Epic 9: Extensibility (MED/LOW — Principle D1, 13)
| Story | Title | Status | Priority |
|---|---|---|---|
| v2-025 | JSON schema for workflows/templates | planned | MED |
| v2-026 | AGENTS.md emitter for Claude Code/OpenCode integration | planned | MED |

## 6. Success Criteria

1. **G1/G2/G3 fixed:** two concurrent agents on state.yaml = VersionConflict (not silent loss); worker handoff = append-only (not clobber); agent identity attributed in every ledger entry.
2. **Lane claims work:** two agents cannot write the same lane; claims auto-expire (TTL); heartbeat renews.
3. **Backward compatible:** existing single-agent projects (no registry.yaml) behave byte-identically to v1.34.1. All 24 evals pass. All existing tests pass.
4. **POC reproducible:** the hot-take POC patterns (forge_v2_poc.py) are now native runtime commands, not a separate script.
5. **Gate catches integration issues:** tsc --noEmit + smoke in gate; type conflicts detected before phase advance.
6. **Partner experience:** every facilitation pack carries research affordance; grill fires at every decision-close.

## 7. Implementation Order (priority × dependency)

```
Phase 1 (foundation):  Epic 1 (v2-002, v2-003) → Epic 2 (v2-005, v2-006) → Epic 3 (v2-008, v2-009)
Phase 2 (coordination): Epic 4 (v2-010, v2-011, v2-012) → Epic 5 (v2-013, v2-014)
Phase 3 (experience):   Epic 6 (v2-015, v2-016, v2-017)
Phase 4 (quality):      Epic 7 (v2-018, v2-019, v2-020)
Phase 5 (advanced):     Epic 8 (v2-021..v2-024)
Phase 6 (extensibility): Epic 9 (v2-025, v2-026)
```

## 8. Non-Goals (explicitly out of scope for v2.0)

- Human-human governance protocol (explicit out-of-scope per RFC v3 §2.5).
- Forge App native implementation (the App consumes the protocol; it doesn't change the protocol).
- CRDT production implementation (simplified re-read-from-log suffices for v2.0; full CRDT is a v2.1+ concern).
- Council decision-mode orchestration beyond standup (v2.1).
