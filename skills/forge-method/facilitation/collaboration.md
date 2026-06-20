# facilitation: collaboration

purpose:
  Help a team use Forge together through explicit Product Areas, owners, trunk-based work, compact handoffs, and repo split decisions.

open_floor:
  "Vamos preparar isso como trabalho de time: quem decide, onde o codigo vive, quais Product Areas existem, como PRs fecham, e quando uma area merece repo proprio?"

source_material:
  Ask for GitHub org/repo shape, team members, product areas, paths, owners, release cadence, CI commands, review rules, current branches/PRs, and integration risks.

follow_up_batches:
  - operating_model: "Who owns decisions, reviews, merges, releases, and agent usage?"
  - product_areas: "Which product areas exist, where do they live, who owns them, and what contracts connect them?"
  - trunk_policy: "How small are PRs, what checks must pass, who reviews, and what blocks merge?"
  - split_boundary: "Which area has independent owner, contract, validation, release cadence, and integration cost?"

conversation_stages:
  - team_shape: "Name GitHub organization/repo shape, team roles, and decision rights before backlog work."
  - area_map: "Map Product Areas to paths, owners, contracts, dependencies, validation, and split triggers."
  - trunk_rules: "Define main branch, PR size, checks, CODEOWNERS/rulesets, conflict policy, and release path."
  - handoff_contract: "Require branch/PR, area, decisions, tests, blockers, and next owner when work changes hands."
  - split_readiness: "Only split a Product Area when the standalone repo can carry compact Forge state and integration proof."

elicitation_options:
  - conflict_movie: "Ask where two people or agents would edit the same surface and how the team recovers."
  - owner_pressure: "Force every area, check, and waiver to name an accountable person or role."
  - split_cost: "Compare monorepo coordination cost against multi-repo versioning, contracts, and integration friction."
  - handoff_drill: "Replay a developer leaving mid-story and verify the next agent can continue from files."

facilitator_moves:
  - "Do not let module mean product boundary; say Product Area unless discussing Forge runtime modules."
  - "Prefer monorepo until independence is real, not aspirational."
  - "Treat missing owner, contract, or validation command as a split blocker."
  - "Route GitHub/CI details to platform or quality workflows after the collaboration model is coherent."

quality_bar:
  - "A new teammate can open the repo, run Forge, see the operating model, pick an owned Product Area, and avoid stepping on others."
  - "A future agent can tell whether to work in the root integrator or a standalone split repo."
  - "Split plans preserve compact context and contracts without copying stale integrator history."

anti_patterns:
  - "Do not use Forge Module commands to solve product modularization."
  - "Do not split repos because the idea feels clean while contracts and releases are still coupled."
  - "Do not start parallel build work without area ownership and handoff policy."

paths:
  fast_path: "Capture team-operating-model and product-area-map, then continue sprint/story planning."
  deep_path: "Add trunk-based-plan, CI/platform follow-up, collaboration handoff, and repo-split-plan for candidate standalone areas."

checkpoint_options:
  - product-area-map
  - trunk-based-plan
  - collaboration-handoff
  - repo-split-plan
  - platform-ops-plan

domain_examples:
  - team_bootstrap: "Friends start one product together; define org/repo, owners, Product Areas, trunk rules, and handoff before stories."
  - parallel_agents: "Multiple Codex sessions work at once; require area/branch/PR/evidence handoffs to prevent drift."
  - repo_split: "One Product Area becomes independently owned; create standalone Forge state and leave integration contract in root."

artifact_rules:
  Persist operating model, Product Area map, trunk rules, handoffs, split criteria, standalone init command, integration evidence, and next workflow.

headless:
  If org/repo, owner, or split boundary is unknown, write the collaboration artifact with blockers instead of starting build or repo migration.
