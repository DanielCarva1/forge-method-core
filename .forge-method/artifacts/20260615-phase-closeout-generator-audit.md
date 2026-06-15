# Phase Closeout Generator Audit

created_at: 2026-06-15
phase: 6-evolve
workflow: runtime-builder
status: spec-kernel-generator-added

## Purpose

Audit phase-closing workflows for the same pattern proven by discovery closeout: rich human guidance collects fields, compact workflow refs name the state-machine step, and a deterministic artifact command writes and validates the handoff artifact.

## Current Coverage

| Boundary | Workflow | Template | Validator | Generator | Status |
| --- | --- | --- | --- | --- | --- |
| discovery -> specification | discover-intent | discovery-closeout-artifact | artifact discovery-check | artifact discovery-closeout | covered |
| specification -> planning/build | write-spec | spec-kernel-artifact | artifact spec-check | artifact spec-kernel | covered in this increment |
| research -> decision | market/domain/technical scan | research-scan-artifact | artifact research-check | none | remaining |
| game discovery/planning | game-brief, game-sprint-planning | game brief/sprint artifacts | artifact game-check | none | remaining |
| test/quality planning | test-framework, test-automation, game E2E | test artifacts | artifact test-check | none | remaining |
| enterprise readiness | security/privacy/risk/release artifacts | enterprise artifacts | artifact enterprise-check | none | remaining |
| document utilities | doc-index, doc-shard | document-utility-artifact | artifact doc-check | none | remaining |

## Decision

Implement `artifact spec-kernel` next because `write-spec` is the central phase-2 closeout. It sits directly after accepted discovery and before architecture, sprint planning, product requirements, or quick-dev. Without a generator, agents still have to hand-write the most important compact WHAT contract.

## Contract

- Human-facing `write-spec` guidance asks for source artifacts, why, capabilities with stable CAP IDs, constraints, non-goals, success signal, preservation map, validation verdict, and next workflow.
- Agent-facing `workflow-write-spec.md` remains compact and routes through `artifact spec-kernel`, then `artifact spec-check`.
- `artifact spec-kernel` writes, validates, registers, and optionally creates an eval for a spec kernel artifact.

## Next Gap

Research closeout is the next likely generator family because market/domain/technical scans already share one validator and template, and research is a common benchmark-style facilitator boundary before PRD/spec commitment.
