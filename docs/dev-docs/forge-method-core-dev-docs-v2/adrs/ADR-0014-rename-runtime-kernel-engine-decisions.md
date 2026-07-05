# ADR-0014 - Rename `runtime`→`kernel`, `engine`→`decisions`

- **Status**: Accepted (V0 implemented — crates renamed; CONTEXT.md canonical terms already
  point to the new names; docstrings corrected)
- **Date**: 2026-07-02
- **Track**: V0 — naming-match-roles
- **Supersedes**: none
- **Superseded by**: none

## Context

Two crate names were at odds with their roles, causing navigation friction at every
architecture review.

1. **The crate that mutates state** was called `forge-core-runtime`. But ADR-0001, `CONTEXT.md`
   and every other ADR already called it **"the kernel"** — the single source of truth for
   mutation, deterministic and auditable (ADR-0001), the only PDP for mutation (ADR-0024). The
   name "runtime" is generic (any execution crate can be a "runtime"); "kernel" names the
   role. The divergence between name and role was corrected verbally in every conversation and
   re-suggested in every review.

2. **The `forge-core-engine` crate** had a docstring claiming it "sits above the
   runtime executor" — but it has **no dependency whatsoever** on the runtime. They are sibling
   crates, not stacked layers. Its modules (`phase_transition`,
   `claim_engine`, `isolation`, `autonomy_router`, `catalog`, `coordination_eval`,
   `guide_validation`) are **pure decision functions**: they take data, return a verdict,
   with no IO, no mutable state, no dependency on the mutation kernel. Describing it as
   "above" the runtime induced callers to depend on a layering relationship that does not exist.

## Decision

1. Rename **`forge-core-runtime` → `forge-core-kernel`** — the mutation crate, owner of
   `execute_operation` and the WAL append. Every state-bearing mutant path flows through it.

2. Rename **`forge-core-engine` → `forge-core-decisions`** — a library of pure, deterministic
   decision functions: claim lifecycle, worktree isolation, phase-transition gates, autonomy
   routing, workflow catalog, coordination evaluation, guide validation. It takes data and
   returns a verdict; **no IO, no mutable state, no dependency on the Kernel.** It only
   *decides* what should be allowed; the Kernel performs the mutation.

3. **Fix the docstrings.** The crate docstring of `forge-core-decisions` now states
   explicitly: "no IO, no mutable state, and **no dependency on the mutation kernel**.
   The only crate-level dependency is the typed `forge_core_contracts` layer". And: "The two
   are sibling crates, not stacked layers — do not describe Decisions as 'sitting above' the
   Kernel."

4. **Document the canonical terms** in `CONTEXT.md` (sections "Kernel (the mutation crate)"
   and "Decisions (the pure-function library)") — already added in V0, pointing to the new
   names and citing this ADR.

## Rationale (the real trade-off)

The alternative — leaving the old names and correcting verbally each time — was rejected for
its recurring cost: every architecture review would re-suggest the rename, every newcomer had
to be corrected that "runtime == kernel" and "engine is not above anything". The rename is a
mechanical migration cost paid once; the benefit is that names and roles coincide forever. The
dependency of `decisions` on `contracts` (and on nothing else at the crate level)
empirically confirms the role: it is a decision library over contract types, not a layer over
the kernel.

## Consequences

**Positive:**

- **Names match roles.** `forge-core-kernel` is the crate that mutates;
  `forge-core-decisions` is the library of pure functions that decide. There is no more
  name-vs-role divergence for a reviewer to verbalize.
- **Future architecture reviews no longer re-suggest the rename.** The mechanical cost was
  paid once in V0.
- **Decisions is correctly described as a sibling library, not a layer.** The crate docstring
  prevents the "sits above" confusion that induced misguided dependencies.
- The canonical terms in `CONTEXT.md` anchor the vocabulary for the rest of the documentation.

**Negative:**

- Mechanical migration: import paths, `Cargo.toml` deps, `use` statements, refs in docs. Paid
  in V0; downstream crates already point to the new names.
- Any pre-V0 branch/PR that still references the old names needs the rename applied. Accepted
  as the normal cost of a rename.

## Anti-goals

- **Does not** change crate responsibilities — only names and docstrings. What mutates keeps
  mutating; what is a pure decision keeps being a pure decision.
- **Does not** create a layering relationship between `decisions` and `kernel` — they remain
  siblings. `decisions` does not depend on `kernel`.

## References

- In-repo: ADR-0001 (deterministic kernel), ADR-0024 (kernel is the only PDP for mutation),
  `CONTEXT.md` (sections "Kernel (the mutation crate)", "Decisions (the pure-function
  library)"), `crates/forge-core-kernel/src/lib.rs`,
  `crates/forge-core-decisions/src/lib.rs`.
