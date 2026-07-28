# 10 — Apply a Governed Promotion atomically

**What to build:** Promote one fully admitted isolated change into the canonical repository as a single governed operation, producing authoritative readback and a durable receipt or leaving the canonical destination unchanged.

**Blocked by:** 09 — Preview a Governed Promotion.

**Status:** ready-for-agent

- [ ] A current admitted preview can be applied exactly once.
- [ ] The applied canonical diff exactly matches the previewed diff and declared write set.
- [ ] Destination drift, overlapping ownership, missing evidence, stale authority, and effect-scope expansion fail before mutation.
- [ ] Successful apply performs canonical readback before reporting completion.
- [ ] The durable receipt binds objective, snapshots, diff, evidence, principal, transaction, effects, and resulting canonical state.
- [ ] Retrying an already committed promotion cannot duplicate effects.
