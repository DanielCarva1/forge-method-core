# Final Batch Evidence — v2-015, v2-017, v2-020, v2-021, v2-022, v2-023, v2-024, v2-025
- kind: story-evidence
- created_at: 2026-06-23T16:15:16Z

## v2-015 (research affordance): Added "I don't know — research..." affordance to all 34 facilitation packs. Commit d7f1325.
## v2-017 (partner presence): Added "excited expert friend" presence directive to all 34 packs + openai.yaml. Commit d7f1325.
## v2-020 (GAP-2 type conflicts): _check_type_augmentation_conflicts scans .ts files for declare global/module cross-lane conflicts. Wired into gate. Commit 293f3b6.
## v2-021 (agent-contract): cmd_contract_create writes typed agent-contract artifacts with input/output/verification contracts. Commit cccc26f.
## v2-022 (council standup): cmd_council_standup aggregates lane claims, stories, requests, cross-deps into formatted standup. Commit 293f3b6.
## v2-023 (orchestration spawn): cmd_spawn emits runtime-agnostic spawn directives to spawns/<id>.yaml. No runtime API calls (C3). Commit cccc26f.
## v2-024 (evolve routing fix): is_evolve_reentry_intent classifies new-feature intent in phase 6 as evolve-re-entry ? discovery, not builder. Commit 9231e7e.
## v2-025 (workflow schema): JSON Schema Draft 2020-12 for workflow definitions (id, title, trigger, inputs, steps, outputs, done_when, blocked_when). Commit ba8748f.