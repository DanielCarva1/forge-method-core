# P1.3 Persona And Elicitation Layer Grill

- created_at: 2026-06-14T23:50:00+00:00
- workflow: runtime-builder
- plan: P1.3 Persona and Elicitation Layer

## Objective

Add BMAD-like named role and coach guidance without making Forge's agent runtime verbose or personality-driven.

## Benchmark Finding

The internal benchmark is stronger at human role framing: PM, architect, analyst, UX, QA, game, builder, tech writer, and CIS-style coaches help the human know which kind of thinking is happening. Forge already has compact Agent Profiles and some profile `persona` fields, but those fields are not systematic, routeable, or indexed.

## Grill Questions Resolved

1. Is a named role an Agent Profile?
   Recommended answer: no. Agent Profile remains the compact runtime manifest; named role guidance is a Persona Lens.
   Resolution: accepted.

2. Where does rich human persona text live?
   Recommended answer: in persona overlay metadata and optional facilitation guidance, not in workflow references, state, recovery packs, or default agent summaries.
   Resolution: accepted and recorded in ADR 0010.

3. How does a human get a role lens?
   Recommended answer: through natural language routed by Guidance Engine, not a new slash command.
   Resolution: accepted.

4. How should Council choose participants?
   Recommended answer: explicit `--agent` still wins; otherwise topic/persona lens selects compact Agent Profiles.
   Resolution: accepted.

5. What proves compactness?
   Recommended answer: tests assert default agent recommendations omit persona narration while persona lens output and Capability Index stay compact.
   Resolution: accepted.

## Required Implementation Shape

- Add packaged Persona Lens metadata for PM, Architect, Analyst/Researcher, UX, QA, Game, Builder, Tech Writer, and CIS-style coaches.
- Add compact Elicitation Technique index.
- Add runtime loading/validation and include summaries in generated Capability Index.
- Add Guidance Engine detection and `persona_lens` output.
- Add topic-based Council participant routing.
- Add replay and unit tests for role/persona selection and compact runtime output.

## Boundaries

- Human Experience owns Persona Lens narration.
- Guidance Engine owns lens selection from current human language.
- Agent Runtime owns compact profiles, workflow metadata, technique ids, and generated index summaries.
- Agent Council may display richer persona lines live, but persists only compact decision artifacts.
