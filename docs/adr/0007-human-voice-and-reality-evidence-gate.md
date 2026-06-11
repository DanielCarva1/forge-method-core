# ADR 0007: Human Voice And Reality/Evidence Gate

## Status

Accepted

## Context

Forge Method has two surfaces with different jobs. The Human Experience must feel guided, useful, candid, and alive. The Agent Runtime must stay compact, deterministic, and cheap to reload after context loss. If human personality leaks into runtime artifacts, future agents inherit noise. If runtime language leaks into first-run guidance, the product feels cold and procedural.

Forge also needs to challenge ideas before turning them into market or implementation work. Existing Grill Gate, innovation, risk, and evidence workflows cover parts of this, but they do not provide one canonical check for physical possibility, technical feasibility, ethics, safety, legal risk, alternatives, and minimum evidence.

## Decision

Forge Method separates a Human Voice Layer from Agent Runtime artifacts. Non-JSON human guidance may be warm, funny, blunt, opinionated, and adaptive to the user's energy. The agent may criticize ideas, assumptions, process, bugs, or product direction directly, including calling an idea bad, dumb, impossible, unsafe, or not worth building when evidence supports that. It must not humiliate or attack the human.

Forge Method also adds Reality/Evidence Gate as a canonical discovery and planning check. New product ideas, risky claims, and market opportunities must be checked for basic reality before market attractiveness or implementation planning. Market, Domain, and Technical Feasibility scans support the gate when evidence is needed.

## Consequences

- First-run and guide output can feel more human without polluting JSON, state, evidence, workflows, or recovery artifacts.
- Weak or impossible ideas can be blocked early instead of receiving inflated viability scores.
- Market scarcity is not treated as viability when the underlying idea is impossible, unsafe, cruel, illegal, or incoherent.
- Workflows remain compact state machines; full human debate or flavor is not persisted by default.
