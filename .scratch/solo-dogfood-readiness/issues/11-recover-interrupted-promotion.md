# 11 — Recover an interrupted Governed Promotion

**What to build:** Reconcile interrupted promotions deterministically across the prepared, write-ahead, effect-commit, readback, replay-consume, and receipt boundaries without duplicating or concealing canonical effects.

**Blocked by:** 10 — Apply a Governed Promotion atomically.

**Status:** ready-for-agent

- [ ] A crash before the first canonical effect leaves the destination unchanged and safely retryable.
- [ ] A crash after an effect commit recovers the committed result rather than applying it again.
- [ ] Recovery verifies the actual canonical destination before completing the receipt.
- [ ] Corrupt, ambiguous, rolled-back, or mismatched recovery state fails closed with an actionable diagnostic.
- [ ] Repeated reconciliation converges to one terminal result.
- [ ] Failure-injection evidence covers every authority and durability boundary in the promotion sequence.
