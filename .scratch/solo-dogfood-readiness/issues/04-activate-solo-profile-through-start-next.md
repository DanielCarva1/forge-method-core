# 04 — Activate Solo Cooperative through the start and next journey

**What to build:** Let an ordinary single-owner project activate and durably resume the `solo_cooperative` readiness profile through the existing start and workflow-next journey, with one start invocation applying to the entire current chat.

**Blocked by:** 02 — Rebase the canonical roadmap to Solo Dogfood Ready.

**Status:** ready-for-agent

- [ ] A fresh solo project activates `solo_cooperative` without external broker configuration.
- [ ] Start is idempotent and never creates a new authority epoch merely because it is invoked again.
- [ ] Task, phase, retry, and internal agent-handoff changes do not require another start invocation in the same chat.
- [ ] The active profile is durable and visible in the governed next-action projection.
- [ ] Strict external mode remains available as a separate profile and is never silently downgraded.
- [ ] Solo activation does not claim independent human presence, reviewer separation, or enterprise compliance.
