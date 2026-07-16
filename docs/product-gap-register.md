# Product gap register

Status: current Rust source checkpoint `0.12.0` at the repository commit being
reviewed. This register records product gaps; it is not proof that a future tag,
asset, host integration, or field run exists.

## How to read this register

A gap appears here only when source tracing or direct public-interface evidence
showed that a promised user outcome is absent, incomplete, or still lacks the
external evidence needed to claim it. Internal primitives, a public reachable
surface, a shipped release asset, and a real-host result are different facts.

Closed status vocabulary:

- `implementation_absent`: the required product behavior is not implemented;
- `partial_public_surface`: useful primitives or expert interfaces exist, but the
  ordinary supported journey is not complete;
- `external_evidence_pending`: source may contain the control, but only a release,
  hosted run, or field review can close the claim;
- `engineering_hardening_debt`: a specific implementation or release-control
  weakness remains before the affected claim is safe.

Priority is outcome-based: `P0` blocks or can invalidate the first supported
production journey; `P1` blocks the complete operated-product lifecycle; `P2`
blocks ecosystem breadth but not the first core journey.

## Confirmed gaps

| ID | Priority | Classification | Real gap | Current evidence and boundary | Closure evidence |
|---|---:|---|---|---|---|
| **GAP-001** | P0 | `partial_public_surface` | **Ordinary first-use authority bootstrap is not self-contained.** A clean project reaches an action that requires a human-origin broker. Forge ships the closed external-origin event protocol, public-key registry, verification, and `workflow action apply` path, plus a deliberately lower-authority local cooperative signer. It does not ship a host-owned authenticator/signer and bridge that can attest a chat event outside agent control. | `workflow next` can report missing principal/broker registries and require `human_approval_broker`. `workflow action authorize` correctly rejects that authority class. `skill/start-forge/SKILL.md` delegates creation of the signed origin event to the host. This security boundary must not be weakened by letting the agent mint human authority or invoke a signer as an oracle. | From one ordinary chat message, a selected host integration authenticates a native human interaction, keeps private authority and signer invocation outside agent reach, binds opaque host-event provenance, and completes the public action path. Adversarial tests prove the agent alone cannot manufacture or cause approval. |
| **GAP-002** | P0 | `partial_public_surface` | **Complete-state backup, verification, restore, and durable reinitialize-as-new apply are not product-owned.** Linked sidecar loss fails closed and now exposes explicit, non-conflated bootstrap recovery choices, but operators still lack verified complete-authority restore and new-authority plan/apply operations. | `start` returns the versioned `forge_bootstrap_state_loss_v1` diagnosis with a deterministic correlation digest, typed loss cause and identity, and explicit inspect/restore/reinitialize-as-new choices. Inspection is read-only; restore and reinitialize are deferred with no apply argv, and reinitialize declares abandonment plus distinct identity/location requirements. `project init` rejects linked loss, preexisting unlinked state, and substituted targets; fresh target directories are atomically reserved, Project Link publication is create-if-absent, and authority markers are populated only after publication wins. | Typed backup/verify/restore binds the Project Link, complete sidecar, external anchors/registries, and exact release identity. A separate durable reinitialize plan/apply path requires explicit confirmation and unrelated authority identity/location. Omission, loss, rollback, stale diagnosis, interruption, and cross-project tests pass. |
| **GAP-003** | P0 | `implementation_absent` | **No concrete reference-host adoption package is shipped.** | The generic host manifest/projection recognizes Codex, Claude, Cursor, and OpenCode, and MCP readiness can generate a client configuration. Those are adapter-neutral primitives. The release payload contains no named-host plugin/extension installer or host conformance suite. Default MCP is intentionally read-only and is not a substitute for the separate governed workflow CLI/authority path. | One selected host passes a source-level clean-install, setup, chat-only authority, conflict, and replacement-session conformance suite; GAP-006 separately requires that exact integration to ship and pass production-host evidence. |
| **GAP-004** | P0 | `partial_public_surface` | **Installation is documented, but product lifecycle operations are not turnkey.** There is no public setup/diagnostics/update/uninstall workflow that owns host configuration, verifies the selected installed release, reports partial state, and rolls back safely. | Source installation with Cargo and verified archive extraction are documented. Distribution admission and exact workflow-release upgrade semantics exist. Direct CLI probes for setup-, doctor-, install-, update-, and uninstall-style top-level commands return unknown-command errors. The exact command names are not required; the user outcomes are. | One idempotent public lifecycle surface installs/configures, diagnoses, updates, rolls back, and uninstalls product-owned files without deleting project authority. Tests cover partial install, stale host config, interrupted update, downgrade refusal, and uninstall with retained state. |
| **GAP-005** | P0 | `engineering_hardening_debt` | **Release reproducibility and platform evidence have specific open controls.** | Release builds omit Cargo `--locked`; declared MSRV `1.85` has no dedicated CI lane; Linux ARM64 is cross-built without native smoke; the POSIX wrapper depends on non-portable `readlink -f`; and the separately generated SBOM is not checksum/signature-bound to each release archive. | Locked release builds, an MSRV lane, native/emulated ARM64 install smoke with a disclosed boundary, portable wrapper tests, and a signed/checksummed release manifest bind each archive and its SBOM. |
| **GAP-006** | P0 | `external_evidence_pending` | **There is no verified public `0.12.0` release and no production-host P7F result in the evidence reviewed here.** | Source version is `0.12.0`, but no matching tag was found during the audit; `v0.4.0` is only a historical Rust predecessor. Source workflow and P7F bundle checks prove control structure, not publication, host behavior, semantics, or actor independence. | A matching immutable tag run publishes the exact assets and sidecars; independent verification checks identity and installation; one supported production host produces and receives independent review of the bounded P7F journey; consecutive hosted CI timing evidence closes P7G. |
| **GAP-007** | P1 | `implementation_absent` | **The normal governed lifecycle stops at BuildVerify.** ReadyOperate and Evolve exist in typed phase vocabulary and policy/gate material but are not reachable through normal runtime phase advancement. | `plan_phase_advance` in the workflow-governance adapter maps Discovery through BuildVerify and returns no successor for BuildVerify, ReadyOperate, or Evolve. The promise audit therefore marks idea-to-operated-product incomplete. | Public runtime transitions durably bind release, rollback baseline, operational observations, feedback, incident/bug intake, and repeated evolution episodes. Replacement agents resume the exact post-release episode. |
| **GAP-008** | P2 | `partial_public_surface` | **Domain Pack acquisition/lifecycle remains host-expert-heavy, and public acquisition is local rather than a complete remote ecosystem.** | Clean installation has a joined acquisition apply path that derives important resolver/composer/trust/preflight internals, but the host still prepares accepted-intent, discovery, catalog, trust, review, capability, and sandbox inputs. Upgrade, rollback, remove, and rebase semantics exist through typed lifecycle documents, state, and recovery; those operations are not absent. Initialized-state intent derivation and public remote catalog/download productization remain open. | Chat intent derives first acquisition and each initialized-state operation without human YAML editing, preserves explicit approval and trust, and proves crash recovery. A signed/revocable catalog path downloads by immutable digest without silently activating content. |
| **GAP-009** | P2 | `partial_public_surface` | **The Domain Pack external-author ecosystem is not complete.** | The core already validates, resolves, composes, and consumes typed packs; includes compatibility/adversarial material and a governed reference pack; and exposes learning capture, evaluation, conflict, reviewer/trust registry, and promotion primitives. Missing work is a coherent external-author SDK plus packaging, signing, publishing, complete revocation, and evolution product workflows. | An external author can complete the full SDK lifecycle without modifying domain-specific Rust or universal-core authority, and hostile/revoked/incompatible pack tests fail closed. |
| **GAP-010** | P1 | `implementation_absent` | **Named-host support breadth is not implemented or certified.** | Recognizing target enum values does not establish installability, authenticated human-origin events, governed mutation, or field support for Codex, Claude, Cursor, and OpenCode. | After the reference integration freezes the contract, each intended target passes the same versioned adapter suite or is explicitly marked unsupported with the failed capability named. |

## Explicit non-gaps and non-solutions

- The Rust/YAML product is authoritative. The unrelated legacy Python/plugin
  history is forensic input only and must not be restored or copied as runtime.
- Manual source installation and verified-archive installation exist; the gap is
  the owned lifecycle and host setup around them, not the ability to build a CLI.
- Generic host manifests, read-only MCP, hardened MCP readiness, action packets,
  broker registries, claims, handoffs, worktree-isolation proposals, readiness
  policies, and Domain Pack lifecycle operations exist. They must be reused, not
  reimplemented under new names.
- Read-only MCP is a deliberate authority boundary. Closing host adoption must
  not expose mutating workflow authority through a transport merely for
  convenience.
- Same-OS-principal hostile isolation and exhaustive discovery of every unknown
  unknown remain disclosed boundaries, not silently promoted promises.

## Ordering rule

The canonical implementation sequence is
[`contracts/plan/product-gap-closure-plan.yaml`](../contracts/plan/product-gap-closure-plan.yaml).
Work starts with **GAP-001** because an ordinary user currently cannot cross the
first required human-authority boundary. Any implementation must preserve that
boundary rather than replacing it with agent-controlled signing. GAP-002 and
GAP-005 are also release blockers and must close before publishing the resulting
supported journey.
