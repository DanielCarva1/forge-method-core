# 03 — Make mandatory CI failures visible

**What to build:** Make the overall CI verdict fail whenever a mandatory platform, package, or reference-journey gate fails, while keeping genuinely experimental observations explicitly separate from required readiness.

**Blocked by:** None — can start immediately.

**Status:** implemented-pending-two-phase-rollout

- [x] A failed mandatory child job makes the overall workflow fail.
- [x] Required readiness jobs cannot use allowed-failure semantics.
- [x] Experimental or informational jobs are labeled separately and cannot satisfy a readiness claim.
- [x] A regression scenario demonstrates that the prior hidden-failure shape now returns a failing overall verdict.
- [x] The final summary lists each mandatory job and its actual terminal state.

## Evidence — 2026-07-27

- python3 -I scripts/test-check-ci-verdict.py: 6/6 PASS.
- python3 -I scripts/test-msrv.py: 41/41 PASS, including YAML topology mutations for no-op, if: false, || true, removed tests, mandatory/informational classification, and bootstrap states.
- python3 -I scripts/check-msrv.py: PASS for 23 workspace packages and the protected workflow topology.
- Rollout is intentionally split by contracts/migration/msrv-policy-v2-rollout.yaml: phase 1 lands the trusted base checker/policy/tests through an explicitly audited administrative landing; phase 2 changes only ci.yml and is validated by the immutable phase-1 base.
- Hosted GitHub Actions and remote required-check configuration remain unverified; no readiness claim is made from local evidence alone.
- Two independent adversarial reviewers completed before a separate fix agent.
- No stage, commit, push, hosted CI, or external mutation occurred.
