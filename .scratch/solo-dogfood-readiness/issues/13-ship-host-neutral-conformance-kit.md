# 13 — Ship the Host Capability Conformance kit

**What to build:** Provide an open-world, versioned conformance contract and reusable journey runner through which any host can prove individual Forge capabilities without host-name policy or a Forge core release.

**Blocked by:** 12 — Resume with a replacement agent.

**Status:** ready-for-agent

- [ ] The contract tests activation, project-root resolution, read-only guidance, conversation-derived intent, cooperative evidence, isolated work, governed promotion, and replacement-agent recovery independently.
- [ ] Canonical project-root resolution is global; the Windows-to-WSL bridge is required only when a Windows host targets a project whose canonical root is inside WSL.
- [ ] Results use the closed outcomes `supported`, `partially_supported`, and `unsupported` for each capability.
- [ ] Conformance binds the exact host version, adapter version, Forge package, platform, environment, and evidence digests.
- [ ] A new host adapter can run the public corpus without adding host-name branches to Forge core.
- [ ] Missing host APIs produce typed gaps and cannot weaken the conformance requirements.
- [ ] The evidence bundle is deterministic, redacted, independently inspectable, and rejects fabricated or incomplete results.
