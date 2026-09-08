# Forge Core — Development Plan

**Status:** ACTIVE — the single development entry document for this repository.
**Last updated:** 2026-09-07 (B1 probe found executed — plan corrected; maintainer product principle recorded)
**Kind:** navigation and sequencing only. **Not runtime authority.**

---

## 0. Read me first — how every agent must use this document

This is the **only** document an agent needs to open to know what to do next.
It exists to prevent duplicated plans, divergent guidance, and lost agents.

**This document owns:**

1. The work sequence (what to do, in what order, what may run in parallel).
2. Story routing (which authority owns each story's full requirements).
3. Per-story done definitions, test strategy, and review gates.
4. The current status snapshot of every work stream.

**This document does NOT own truth.** It summarizes and routes. The owning
authorities listed in §3 remain authoritative. If anything here contradicts a
typed contract, an open issue, or runtime receipts, **the authority wins and
this document must be corrected in the same change.**

### Non-negotiable agent rules

1. **One story at a time.** Use the existing Forge Current Work flow,
   isolation, and promotion for implementation work (delivery rule from #73).
2. **Open the owning authority before implementing.** The per-story summaries
   in §5 are routing aids, not full requirements. Never implement from this
   document's summary alone when the authority file or issue exists.
3. **Never create a second plan, PRD, or status document.** If sequencing must
   change, edit this document. If a contract must change, change the contract
   through its own review path, then update §5 here.
4. **Never relabel evidence.** `solo_cooperative` evidence never proves
   human origin, independent review, trusted-runtime separation, publication,
   or field use. Retained history is never solo completion proof.
5. **Do not repeat recorded work.** v0.12.0 publication, its CI, the packaged
   Windows download verification, the alpha.49 → 0.12.0 update test, and the
   three consecutive packaged-journey runs are already recorded (see §2).
   Do not rerun them to "be sure".
6. **Publication rules** (repository AGENTS.md): no issue, PR, or release is
   published without maintainer acceptance of its content; human maintainer
   stays the author; no AI attribution trailers.
7. **Definition of done is global** (§8) and applies to every story unless its
   authority states a stricter one.
8. **Product principle (maintainer, 2026-09-07): Forge is a companion, not a
   sargeant.** The agent may act. Hard gates exist only to protect durable
   user effects (authority, promotion boundaries, irreversible/external
   actions) — never as friction that prevents delivery or degrades agent
   capability. Fraud-proof human-origin attestation (C1) is out of the solo
   product path; honest documented limits replace it.
9. **Communication rule (maintainer, 2026-09-07): every report, comment, or
   summary written for a human starts with a plain-language answer** — what
   happened, what it means, what is next, in sentences a non-auditor reads
   once and understands. Details, hashes, and evidence boundaries come after,
   clearly separated. Audit precision never replaces a human-readable
   conclusion; if a report cannot be summarized plainly, the work is not
   understood well enough yet.

---

## 1. Product promise (PRD digest)

> Forge accompanies people through their agents while they build and evolve
> software. It preserves accepted intent, decisions and work continuity;
> exposes useful practices; and protects project effects without scripting the
> agent's reasoning. Discovery and planning involve substantial human
> conversation. Technical execution becomes more autonomous as intent settles.

**Full PRD authority: issue #73** (accepted by the maintainer on 2026-09-06).
Requirement digest — every story must be traceable to at least one of these:

| # | Requirement |
|---|---|
| R1 | Ordinary human chat is enough: no workflow IDs, no YAML edits, no binding reconstruction. |
| R2 | An agreed need stays traceable through plan, implementation, acceptance; rejections/non-goals survive restart. |
| R3 | Guidance is consulted at material events; a practice may be declined; no recurring catalogue scans or durable writes on ordinary progress. |
| R4 | Bounded change finishes compactly; material uncertainty expands only the relevant stage; no forced tiny templates on large projects. |
| R5 | A replacement agent recovers accepted work, why it matters, limits, next step — from public interfaces, without old chat. |
| R6 | Delivered behavior satisfies the agreed scenario, not merely internal protocol checks. New functionality starts a new user-directed discovery cycle. |
| R7 | Retain consolidated decisions and necessary evidence, not all brainstorming. Measure owned temp files, elapsed time, repeated reads/writes before inventing budgets. |
| R8 | Current guidance never describes implemented capabilities as missing and never routes Solo Cooperative users into enterprise requirements. |

**Non-goals (do not build these):** new orchestrator, memory database,
classifier, mandatory research quota, identity broker, signature layer,
catalogue rewrite, host-specific core, full UI testing, or any universal
guarantee that an LLM always follows guidance.

---

## 2. Current state snapshot (2026-09-07)

| Fact | State |
|---|---|
| Stable release | **v0.12.0** published (tag = commit `4fc9bd80`), Release + CI green |
| Main machine install | `forge-core 0.12.0` at `C:\Users\User\AppData\Local\Programs\forge-core\bin` (upgraded from alpha.49 on 2026-09-07; rollback: `forge-core.exe.alpha49.bak`) |
| Host skill | `start-forge` identical to the packaged canonical skill |
| Milestone | **Solo Dogfood Ready — QUALIFIED** (`milestone_qualified: true`, authority revision 5, flipped 2026-09-08 through governed promotion) |
| SD items (solo milestone) | SD-00 … SD-08 **all completed**; what remains is qualification evidence (§5 WS10) |
| **B1 probe (issue #75)** | **Executed 2026-09-06; CLOSED 2026-09-07.** 16 turns, real Codex CLI 0.153.4 + isolated human simulator, real delivered app through public promotions. Verdict: **bounded PARTIAL** — R1/R2/R5/R6/R8 PASS; R3/R4/R7 NOT OBSERVED in full. Repairs committed during the probe (7b8aa6e7, 3c123198, 6e890357, 648b23e3). Sanitized summary published and issue closed as completed with the PARTIAL verdict: [#75 comment](https://github.com/DanielCarva1/forge-method-core/issues/75#issuecomment-5578622376) |
| Human-origin trust boundary (C1) | **PARKED by maintainer product principle (2026-09-07):** Forge must not strangle the agent with hard fraud-proof barriers. Codex/pi/OpenCode rejected; no further host hunting for now; record honest limits instead (see §5 WS3) |
| Qualified platform | Native Windows only. Linux/macOS artifacts exist but are **not qualified** |
| ZCode host | **Recognized working initial solo host (maintainer decision 2026-09-07).** Retained authenticated journey completed all eight solo capabilities on Forge alpha.49 (21 assertions); capability outcomes are judged on observed cooperative behavior; host-native authenticity attestation is out of scope for the solo profile. Support matrix corrected to the retained evidence with current digests |
| C1.1 host selection | **None.** Codex 0.143.0, pi 0.80.7, OpenCode 1.14.33 all rejected for the human-origin signer boundary (see `contracts/spec/C1.1-*-host-capability-decision.yaml`) |
| cosign | Not installed on this machine; cryptographic local verification outstanding |
| Open issues | #73 (PRD umbrella), #75 (B1, `ready-for-agent`), #19 (docs audit), #22/#23/#24 (platform tests, `needs-triage`) |
| Closed and settled | #74 (A1 guidance reconciliation), #70, #64, #41–#46 — do not reopen or recreate |

Recorded evidence (already collected — cite, don't regenerate):
`D:\Temp\User\forge-companion-b1-20260906\evidence\` — in particular
`stable-0.12.0-verification.json`, `stable-0.12.0-packaged-journey.json`,
`stable-release-closeout.json`, `terminal-plan-*.json`.

---

## 3. Authority map

| Owns | Authority | Where |
|---|---|---|
| Milestone, readiness profiles, claims, evidence boundaries | `solo-dogfood-readiness-v0` | `contracts/spec/solo-dogfood-readiness-v0.yaml` |
| Product constitution | agent-native product constitution | `contracts/policies/agent-native-product-constitution.yaml` |
| Assurance architecture | agent-native assurance architecture | `contracts/spec/agent-native-assurance-architecture.yaml` |
| Campaign definitions C0–C7, statuses, exit criteria | gap closure plan | `contracts/plan/product-gap-closure-plan.yaml` |
| Story identity, dispositions, evidence-boundary projection | story inventory v1 | `contracts/plan/product-gap-closure-story-inventory-v1.yaml` |
| Campaign execution records | campaign v1 | `contracts/plan/product-gap-closure-campaign-v1.yaml` |
| C2.2 continuity | c2.2 campaign continuity | `contracts/plan/c2.2-campaign-continuity-v1.yaml` |
| PRD R1–R8, epics A/B, delivery rule | issue #73 | GitHub Issues |
| Story B1 full text | issue #75 | GitHub Issues |
| Work-item publication + authorship rules | repo agent instructions | `AGENTS.md`, `docs/agents/` |
| Navigation aid (committed) | context map | `CONTEXT.md` |
| **Sequence + status routing (this file)** | development plan | `docs/development-plan.md` |

**Conflict order:** runtime receipts → typed contracts → issues → this document.

---

## 4. Roadmap — sequence and dependencies

```
WS1  #75 closeout: publish sanitized PARTIAL report + close issue   [DONE 2026-09-07]
WS2  B1 follow-ups (R3 accounting, R7 hygiene check)                small; mostly delivered
WS3  C1 trust boundary — PARKED (rule §0.8)                         resumes only by maintainer decision
WS4  C2 state-loss + install lifecycle                              P0 hardening, independent
WS5  C3 release-assurance debts                                     P0 hardening; v0.12.0 covered much of it
WS6  Platform triage #22/23/24 → C4 breadth                         independent; unblocks Linux/macOS
WS7  Docs audit #19                                                 independent, low priority
WS8  C5 evidence closure (post-BuildVerify)                         independent
WS9  C6.2/C7 evidence closure (domain packs)                        independent
WS10 Milestone qualification closeout                               [READY — next up]
```

Rules for this sequence:

- **WS1 is closed.** The sanitized summary is published and #75 is closed with
  the honest PARTIAL verdict.
- **WS10 is next:** the claim-by-claim milestone audit uses existing evidence
  only — no new tests, no reruns.
- **WS3 is parked, not abandoned.** The C1.1 rejection decisions and frozen
  adapter contract stay in `contracts/spec/` for future reuse. Solo evidence
  never claims human-origin attestation, so nothing downstream is blocked.
- **WS5 no longer waits on WS3:** release assurance proceeds on the
  cooperative evidence path; only independent-review items keep their
  own strict requirements.
- **WS10 closes the milestone** only with explicit claim-by-claim evidence and
  honest remaining limits (delivery rule from #73).
- Sequencing changes are made **only** by editing this section.

---

## 5. Work streams — stories, acceptance, tests, reviews

Statuses: `ready` | `blocked` | `in_progress` | `implemented_pending_evidence` | `done`.

### WS1 — B1 product-conversation probe (issue #75) — `done — closed 2026-09-07 as PARTIAL`

- **Authority:** issue #75 (closed). Evidence root:
  `D:\Temp\User\forge-companion-b1-20260906\evidence\` (`b1-partial-report.md`
  is the consolidated report; `turn1…turn16` are raw runs; `host-conformance/`
  holds the adapter bundle).
- **What ran:** isolated human simulator + real product agents
  (Codex CLI 0.153.4, native Windows) building a small task-board app through
  public Forge interfaces; two deliveries via governed promotion with verified
  receipts; replacement-agent recovery without old chat; human-requested
  extension; installed-runtime compact task (turns 15/16) where the product
  agent accepted a Quick Cycle, created its own worktree, promoted, and closed
  its own Work Focus with five stage closeouts.
- **Verdict (bounded to the scenario):** R1/R2/R5/R6/R8 **PASS**;
  R3/R4/R7 **NOT OBSERVED in full**. Fully hands-off environment setup NOT
  achieved (disclosed coordinator rescues); installed-only boundary for
  repaired replay NOT achieved.
- **Closeout (2026-09-07):** maintainer authorized the sanitized summary;
  published as
  [#75 comment](https://github.com/DanielCarva1/forge-method-core/issues/75#issuecomment-5578622376);
  issue closed as completed with the PARTIAL verdict. Raw transcripts remain
  out of the issue, retained as evidence.
- **Do not:** rerun the simulation, extend with more tiny scenarios, or
  backfill the terminal record. The observation is closed.

### WS2 — B2 targeted repairs — `largely delivered during the probe`

- **Authority:** #73 Epic B (B2): repair only reproduced gaps.
- **Already delivered from B1 findings:** `7b8aa6e7` (retained Windows file
  creation/recovery), `3c123198` (claim attachment to existing isolation),
  `6e890357` (kernel test statement formatting), `648b23e3` (start-forge skill
  guidance for compact-task stage continuity — the guidance gap the probe
  exposed).
- **Remaining small follow-ups:** R3 exact redundant read/write accounting
  (mechanism-level evidence exists: one guide-status event at activation, no
  recurring scans observed; full 181-event classification optional) and
  confirming the final R7 fixture hygiene stays clean. If neither reproduces a
  defect, no repair story is created.

### WS3 — C1 first-use authority vertical slice — `PARKED (maintainer product principle, 2026-09-07)`

- **Authority:** plan C1 (GAP-001, GAP-003) — but see rule §0.8: Forge is a
  companion, not a sargeant. Host screening already ran to an honest verdict:
  Codex 0.143.0, pi 0.80.7, OpenCode 1.14.33 all **rejected** for the
  human-origin signer boundary (decision files in `contracts/spec/C1.1-*`).
  Selected host: **none** — and no further host hunting is scheduled.
- **What parking means:** no broker build (C1.2), no adapter (C1.3), no
  clean-path campaign (C1.4) for the current solo milestone. The frozen
  threat-model and adapter contract stay in `contracts/spec/` as reusable
  history. Solo evidence never claims human-origin attestation, so nothing in
  WS4–WS10 depends on this.
- **What is NOT parked:** the principle behind it in softened form — Forge
  keeps protecting durable user effects (admission, promotion, receipts) and
  asks the human only for irreducible or irreversible decisions (CONTEXT.md
  operating model). Barriers stay out of the agent's ordinary working path.
- **Resume condition:** the maintainer decides the product needs external
  trust (enterprise profile). Then C1.1 restarts from the retained decision
  files, not from zero.

### WS4 — C2 state-loss and install lifecycle — `planned` (C2.3 `in_progress`)

- **Authority:** plan C2 (GAP-002, GAP-004); continuity ref
  `contracts/plan/c2.2-campaign-continuity-v1.yaml`.
- **Stories:** C2.1 fail closed on linked-sidecar loss (typed state-loss
  diagnostic, never implicit recreation); C2.2 complete-state backup/restore
  contracts (reject partial/stale/cross-project/tampered restores; private
  broker keys stay in owner-specific backup); C2.3 owned product lifecycle
  (`in_progress`: idempotent setup/diagnostic surface, verified immutable
  asset updates with rollback, uninstall that preserves consumer projects);
  C2.4 interrupted and mixed-version states (partial setup, interrupted update,
  downgrade refusal, wrapper mismatch, restore after replacement machine).
- **Tests:** every failure mode in C2.4 as an explicit case; crash-recoverable
  apply; backup verification receipts.
- **Exit:** missing sidecar never becomes fresh authoritative state; normal
  user repairs installation without YAML; uninstall/update preserve durable
  authority.

### WS5 — C3 release assurance and publication — `planned`

- **Authority:** plan C3 (GAP-005, GAP-006).
- **Stories:** C3.1 release-control debts (locked Cargo resolution, declared-MSRV
  lane, Linux ARM64 install smoke, POSIX wrapper without `readlink -f`, SBOM
  bound to the exact archive set); C3.2 cumulative source + hosted gates;
  C3.3 publish and independently verify (tag only the gate-passing commit;
  verify version, tag/commit, manifest, checksum, signature, SBOM binding,
  wrapper, clean install from downloaded assets); C3.4 real-host evidence with
  the exact published integration + independent semantic/actor-separation review.
- **Machine note:** install cosign before C3.3 verification work starts.
- **Tests:** consecutive CI timing runs with retained command/duration/timeout
  artifacts; failure injection on verification paths.
- **Exit:** every supported archive immutable and independently verifiable; the
  exact published assets complete one clean real-host journey.

### WS6 — Platform triage → C4 host breadth — `planned`

- **Step 1 — triage #22/#23/#24** (Linux `unshare`/`CAP_SYS_ADMIN` tests,
  macOS symlink tests, domain-pack CLI e2e). Triage any concrete overlap with
  the tested Windows path. Outcome per issue: re-enable, environment-fix, or
  documented platform boundary — no silent relabeling as Windows blockers.
- **Step 2 — C4.1/C4.2:** apply the frozen conformance suite to Claude, Cursor,
  OpenCode; mark targets unsupported when host APIs cannot preserve the origin
  boundary; publish a supported-host matrix separating manifest recognition,
  installability, read-only MCP, human-origin assurance, governed mutation, and
  field evidence per host/version.
- **Exit:** every advertised target has exact versions, assets, conformance
  evidence, known limits. Linux/macOS qualification claims only after their
  platform gates actually run.

### WS7 — Docs audit (#19) — `planned`, low priority

- **Authority:** issue #19 owns tracked Markdown classification and the
  owner-review rule. Not a prerequisite for WS1. Do not expand its scope
  silently; keep product+usage docs shipped, move development diaries out.

### WS8 — C5 post-BuildVerify episodes — `implemented_pending_evidence`

- **Authority:** plan C5 (GAP-007). Implementation exists (C5.1–C5.3). What is
  missing is **evidence**, not code: a real release + rollback baseline with
  durable receipts; operational evidence/feedback that can disprove readiness
  after release; a replacement agent resuming the exact evolution episode
  across fresh processes without hidden chat.
- **Work:** design and run the evidence journey; admit claims through the
  existing readiness/claim machinery; convert each `implemented_pending_evidence`
  to closed or reopen the specific defect.

### WS9 — C6.2 / C7 domain-pack lifecycle and SDK — `implemented_pending_evidence`

- **Authority:** plan C6 (C6.2 immutable remote acquisition) and C7 (C7.1/C7.2
  authoring + publish/sign/review/revoke workflows). Both implemented pending
  evidence. Exit: a third-party author completes the full lifecycle without
  core changes; incompatible, shadowing, tampered, and revoked packs fail
  closed. Evidence journey design follows the WS8 pattern.

### WS10 — Solo Dogfood Ready qualification closeout — `done — milestone_qualified flipped 2026-09-08`

- **Authority:** `contracts/spec/solo-dogfood-readiness-v0.yaml`
  (`readiness_profiles.solo_cooperative`, `release_evidence.rule`).
- **Audit result:** SD-00…SD-05 and SD-07 fully evidenced. SD-06 resolved by
  maintainer decision: capability outcomes judged on observed cooperative
  behavior; authenticity attestation out of scope; host matrix corrected to
  the retained evidence. SD-08 covered by the B1-era governed repairs.
- **Gate closed 2026-09-08:** the released-path delivery (development plan +
  host-matrix correction) was promoted through governed Forge 0.12.0 with receipt
  `sha256:763bf02f…` (canonical commit `5e00219b`), and the four authority files
  flipped to `milestone_qualified: true` (revision 4 → 5) as a second governed
  delivery. Honest limits stand: Linux/macOS unqualified; authenticity attestation
  out of scope; strict_external closed.
- **Honest limits that must survive into the closeout:** Linux/macOS
  unqualified; ZCode authenticity attestation out of scope (recorded, not
  hidden); strict_external profile explicitly out of this milestone's scope.

---

## 6. Test strategy

| Layer | What it proves | When it runs |
|---|---|---|
| Focused unit/integration (`cargo test -p …`) | The story's own logic, including its negative cases | Every story, before review |
| Workspace tests | No cross-crate regression | Before every PR/merge |
| Failure injection | Fail-closed behavior (state loss, tamper, forgery, interruption) | C1/C2/C3 stories — mandatory there |
| Host-conformance adapters | The 8 host capabilities, protocol-level | Host-facing changes; evidence runs |
| Packaged journeys (3 consecutive, Windows) | The shipped artifact works end-to-end | Release candidates (already done for 0.12.0 — do not rerun) |
| `scripts/smoke-release-install.py` + `scripts/check-release-archive.py` | Downloaded assets verify and install clean | C3 verification work |
| Readiness claim admission | Evidence is claim-bound, fresh, honest | WS8–WS10 |

Standing rules: every new behavior gets a test (`feature => test`); build/lint/
typecheck after every change; a green CI never substitutes for the
requirement-level R1–R8 audit.

---

## 7. Review and delivery process (per story)

1. **Prepare:** open the story's owning authority; record a Forge Current Work
   entry; create isolation for mutation work.
2. **Implement:** smallest change that satisfies the authority's acceptance;
   reuse existing routes; no new governance layer unless the authority says so.
3. **Verify:** §6 layers applicable to the story; evidence retained with
   command, duration, and artifacts.
4. **Two-axis review:** (a) *Standards* — repo conventions, AGENTS.md,
   domain docs/ADRs; (b) *Spec* — against the owning issue/contract, not
   against this document's summary.
5. **Close:** promote through the existing governed flow; update the story row
   status in §5 and the §2 snapshot in the same change; clean owned scratch.
6. **Publish:** issues/PRs/releases only after maintainer acceptance of content;
   human authorship preserved; no AI attribution.

---

## 8. Global definition of done

A story is done when **all** of these hold:

- [ ] Tests written and passing (§6 layers for its kind).
- [ ] Errors handled, not swallowed — fail closed where the authority requires.
- [ ] Verified, not asserted — evidence run and retained, not claimed.
- [ ] Two-axis review completed (Standards + Spec).
- [ ] Evidence boundary honest (no solo→strict_external relabeling, ever).
- [ ] §5 row + §2 snapshot updated here; scratch/temp removed.
- [ ] For claims: admitted through the claim machinery, not prose.

---

## 9. Change control for this document

- Any agent may update §2 and §5 **statuses** as part of completing a story.
- Only the maintainer (or an approved story whose scope is this document)
  may change §4 sequencing or §0 rules.
- This document must never gain authority over contracts, issues, or runtime
  receipts. When it drifts, fix this document.
- Staleness check: at story start, re-read §2 and the story's authority; if the
  snapshot is older than the last merged change on `master`, refresh §2 first.

## 10. Document history

- 2026-09-08 — v4: **Solo Dogfood Ready qualified.** Released-path dogfood gate closed
  by governed promotion receipt; four authority files flipped to
  `milestone_qualified: true` (revision 5). WS10 done.
- 2026-09-07 — v3: WS1 closed. Sanitized B1 summary published on #75
  (comment 5578622376) and issue closed as completed with the PARTIAL verdict,
  under the maintainer's plain-language draft. WS10 (milestone qualification)
  is now the next item.
- 2026-09-07 — v2 correction: B1 probe found already executed (16 turns,
  PARTIAL verdict, repairs already committed); WS1 re-scoped to issue-#75
  closeout; WS2 marked largely delivered; WS3 (C1) parked under the maintainer
  product principle "companion, not sargeant" (rule §0.8) and plain-language
  communication rule added (§0.9).
- 2026-09-07 — v1 created: consolidated #73/#75, gap-closure plan C0–C7,
  story inventory, readiness spec, and the 2026-09-06 handoff evidence into a
  single entry document after v0.12.0 stable publication and main-install
  upgrade to 0.12.0.
