# 20 — Gate the packaged Solo Dogfood journey

**What to build:** Make the packaged host-neutral Solo Dogfood journey a mandatory hard-fail release gate and require each advertised host-support row to reference a complete valid conformance evidence bundle.

**Blocked by:** 01 — Repair the authoritative static validator; 03 — Make mandatory CI failures visible; 14 — Conform the Codex host; 15 — Conform the ZCode host; 16 — Conform the OpenCode host; 17 — Conform the Claude host; 18 — Conform the Pi.dev host; 19 — Conform the Cursor host.

**Status:** ready-for-agent

- [ ] Packaged install, activation, objective binding, evidence admission, promotion, and recovery run as one mandatory reference journey.
- [ ] Any failed mandatory step makes the release gate fail.
- [ ] Candidate or unsupported hosts do not block the core release unless a capability is advertised as supported.
- [ ] Every advertised support claim is rejected when its exact evidence bundle is missing, stale, incomplete, or inconsistent.
- [ ] Platform package identity and runtime readback are retained rather than inferred from source tests.
- [ ] At least three consecutive clean gate executions are retained with duration and result evidence.
