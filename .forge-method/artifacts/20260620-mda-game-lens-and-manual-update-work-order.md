# MDA Game Lens And Manual Update Work Order

workflow: guideline-audit
mode: work-order
scope: Forge Method Core 1.33.0
problem: Game guidance still preserves fantasy, loop, and proof, but lacks a compact MDA trace that forces agents to connect desired player experience to dynamics, mechanics, UI feedback, and playtest proof. Update guidance exists only as startup self-update, so humans do not have an explicit maintenance command for existing installs.
guideline: Add MDA Lens as an integrated Game Studio lens, not a parallel workflow. Add MDA Trace as compact agent-facing artifact data. Add forge-update as an Operational Maintenance Skill, not a product workflow.
canonical_terms: MDA Lens, MDA Trace, Operational Maintenance Skill, Manual Update
guardrails: DDE and RMDA may inform internal critique but must not become formal artifact fields. Product entrypoint remains forge-method; forge-reload and forge-update are operational maintenance exceptions.
acceptance_evidence: generated game brief includes complete mda_trace; game-check warns for legacy game artifacts without MDA and fails incomplete MDA blocks; guidance routes MDA/player-feel/fun/playtest questions to Game Studio flows; forge-update can run manual marketplace upgrade and summarize patch notes.
validation: test-runner, smoke-runtime, smoke-install, verify-fast
next_workflow: runtime-builder
