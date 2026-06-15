# ADR 0013: Two-Phase Traceability Gates

Forge traceability gates use two phases: design-time coverage mapping before or during build, and release-time gate decision after evidence exists. This keeps early quality planning useful without pretending it proves release readiness, and it forces release gates to name pass, concerns, fail, missing evidence, or explicit waiver with owner, rationale, and revisit trigger.
