# BMAD / Forge guided experience stress audit

- created_at: 2026-06-17
- project: forge-method-core
- workflow: correct-course -> runtime-builder
- scope: human guidance experience, Help Oracle / Guidance Engine parity, drift recovery, energy matching

## Benchmark observed

BMAD Help acts as a small orientation router: it tells the human where they are, the next recommended skill, how to invoke it, and offers a quick start without dumping the whole catalog.

The richer BMAD flows then carry the human experience:

- product brief opens with a brain dump, source material, "anything else?", then fast path vs coaching path.
- brainstorming keeps the user in exploration mode and delays convergence.
- PRFAQ is intentionally provocative and customer-first.
- correct-course stops the route, scans impact, and proposes a repair path.

The important pattern is not verbosity by itself. The pattern is: small router, rich facilitation pack, explicit mode/pace, and durable output only after the human picture is on the table.

## Forge stress replay before patch

Source/runtime stress cases exposed real gaps:

- Broad game idea routed to `game-brief`, but the first visible question still collapsed to a field checklist.
- "nao sei / perdido / destravar a ideia" routed to `brainstorming` because singular `ideia` was treated as brainstorm.
- Explicit "brainstorm de jogo" with no state routed to `game-brief` before divergent setup.
- "pesquisar Foundry Fantasy Grounds e VTTs" did not classify as research.
- "isso ta frio..." and "nao e isso..." with no state did not reliably enter guidance repair.
- The policy said "match energy", but there was no compact observable style contract to test pace/energy.

## Patch decisions

- No-state Guidance Engine precedence now routes:
  - correction/frustration before project creation
  - confusion before brainstorm/product/game
  - research before game/product
  - explicit brainstorm before game brief
- Removed singular `ideia` as a brainstorm keyword.
- Added VTT, Foundry, Fantasy Grounds, `pesquisar`, benchmark, and drift/correction phrases.
- Updated first questions for `game-brief`, `brainstorming`, and discovery to ask for the full picture before narrowing.
- Updated initial game project facilitation prompt to ask for brain dump plus fast path vs coaching path.
- Added compact `style_contract` in human experience payload:
  - `coaching`
  - `fast-path`
  - `diagnostic`
  - `divergent`
  - `evidence-first`
  - `repair`
  - `mechanical`
- Updated facilitation packs with "anything else", mode choice, no acceleration unless explicit urgency, and energy matching rules.

## Stress coverage now

The new tests and replay cases cover:

- broad game idea -> `game-brief`, coaching, brain dump first
- rushed simple request -> `quick-dev`, fast-path
- lost/confused user -> `problem-solving`, diagnostic
- explicit brainstorm -> `brainstorming`, divergent
- VTT benchmark/research -> `market-scan`, evidence-first
- frustrated/cold guidance -> `correct-course`, repair
- misleading correction -> `correct-course`, repair

## Current verdict

Forge is now closer to the BMAD human-guidance behavior in the first-run and correction cases that were failing in real transcripts.

This does not prove every long multi-turn facilitation run is better than BMAD. It proves the router, first prompts, packs, and regression fixtures now enforce the missing pattern: do not accelerate broad first ideas, ask for brain dump, adapt pace when the human is rushed, and recover course when the human or model drifts.

## Proof

- focused first-question test passed
- focused stress/style test passed
- focused project-create prompt test passed
- parity replay passed 96/96
- full unit suite passed: 126 tests
- verify-fast `-SkipUnit` passed
- smoke-runtime passed
- install-plugin-local passed
- smoke-install passed
- installed launcher stress passed:
  - broad game idea -> `game-brief`, `coaching`, brain dump first
  - confused/lost user -> `problem-solving`, `diagnostic`
  - explicit brainstorm -> `brainstorming`, `divergent`
  - VTT benchmark/research -> `market-scan`, `evidence-first`
  - frustrated/cold guidance -> `correct-course`, `repair`

Validation evidence: `.forge-method/evidence/20260617-014953-validation-guided-experience-stress-audit.md`.
