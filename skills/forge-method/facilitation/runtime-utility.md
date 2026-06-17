# facilitation: runtime-utility

purpose:
  Shape opt-in runtime utilities such as isolated eval runners, hook/event dispatchers, and reusable API/browser helpers without adding always-on Codex overhead.

open_floor:
  "What repeated or risky action needs a utility, what must never run silently, and what evidence proves the utility helped instead of adding ceremony?"

source_material:
  Ask for command examples, failing manual steps, trust boundary, side effects, required tools, secrets/auth constraints, timeout needs, cleanup policy, and validation evidence.

follow_up_batches:
  - utility_kind: "Is this an eval runner, hook/event dispatcher, API/browser helper, or project-local test utility?"
  - human_experience: "What should become easier for the human, and what warning or choice must stay visible?"
  - agent_contract: "What compact state/workflow/template/script contract should future agents consume?"
  - side_effects: "What can this utility read, write, execute, delete, publish, or never touch?"
  - proof: "Which replay, smoke, test, or dry-run proves the utility route and command contract?"

conversation_stages:
  - repeated_pain: "Start from the repeated manual pain or risky action, not from a tool idea."
  - boundary: "Choose local, container, remote, provider, or manual-waiver mode."
  - opt_in_contract: "Make the command explicit; no hidden startup, no background mutation."
  - proof_design: "Define dry-run, timeout, fixture, cleanup, and evidence requirements."
  - handoff: "Write the compact artifact and route the next workflow."

elicitation_options:
  - dry_run_first: "Ask what dry-run output would prove the route before executing anything."
  - side_effect_grill: "List every side effect and force an allow/deny decision."
  - repeatability_ladder: "Rank proof from local shell to container to remote sandbox to release evidence."
  - utility_or_project: "Challenge whether this belongs in Forge core, a project test helper, or a module."

facilitator_moves:
  - "Reject hidden always-on hooks unless there is a project-approved lifecycle contract."
  - "Prefer opt-in scripts and compact artifacts over background services."
  - "Make untrusted execution a trust-boundary decision, not a casual command."
  - "Tie API/browser helpers to visible outcomes, stable contracts, fixtures, and cleanup."

quality_bar:
  - "The human sees why the utility exists and when it will run."
  - "Future agents can execute or waive it from a compact artifact without reading a long transcript."
  - "The utility reduces repeated risk without adding normal Forge startup overhead."

anti_patterns:
  - "Do not install background hooks as part of normal Forge startup."
  - "Do not run untrusted code without an isolation decision or waiver."
  - "Do not create generic API/browser helpers that duplicate an existing project framework."
  - "Do not hide secrets, destructive operations, or publish steps behind a convenience script."

paths:
  fast_path: "Write the opt-in contract, add replay coverage, and validate workflow/catalog references."
  deep_path: "Prototype project-local scripts, run dry-run evidence, then promote only repeated patterns."

checkpoint_options:
  - isolated-eval-runner
  - hook-event-plan
  - api-browser-utility
  - workflow-validate
  - eval-design

domain_examples:
  - isolated-eval-runner: "A release needs reproducible eval output; choose local/container/waiver, record commands, timeout, cleanup, and evidence."
  - hook-event-plan: "A project wants post-story validation; define event, payload, dispatcher command, side effects, and disabled-by-default policy."
  - api-browser-utility: "Three E2E checks repeat login/setup; define provider helper, auth boundary, fixture cleanup, visible assertions, and command map."

artifact_rules:
  Persist utility kind, trigger, opt-in command, side-effect boundary, trust/isolation decision, validation evidence, waiver, and next workflow.

headless:
  Prefer dry-run contracts over execution when side effects, trust boundary, or required tools are unclear.
