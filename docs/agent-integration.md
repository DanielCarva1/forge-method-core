# Agent integration contract

Forge is not a conversational model. It is a local typed governance boundary
driven by a host agent. A correct integration keeps the human in chat while
preserving the distinction between advice, evidence, and admitted authority.

## Required loop

1. Resolve the user-selected project root.
2. Run `forge-core start --root <root> --json` once per chat.
3. Execute only structured argv returned by Forge.
4. Initialize or resume workflow governance.
5. Check the durable release and perform only an exact returned upgrade.
6. Call `workflow next` without caller-selected phase, policy, bundle, or
   readiness target.
7. Perform the highest-ranked feasible action.
8. Collect evidence from the tool/runtime/human named by the evaluator; never
   self-upgrade artifact presence into representative proof.
9. Record observations through an authorized surface and call `workflow next`.
10. Stop and explain typed gaps when authority or capability is unavailable.

The canonical bootstrap procedure is
[`skill/start-forge/SKILL.md`](../skill/start-forge/SKILL.md). The generated
[command surface](generated/command-surface.md) is the flag-level reference.

## JSON handling

- Treat `CliEnvelope.ok` as the result, not process exit alone.
- Preserve argv boundaries. Never shell-evaluate display command strings.
- Bind mutating follow-up to returned snapshot/head/CAS digests.
- Do not cache guidance across mutation; ask Forge again.
- Redact private keys and secret material; audit projections are not authority.

The normal `workflow next` response embeds `authorization.action_packets`,
registry setup state, and typed setup gaps. The standalone
`workflow action-packets` command exposes the same packets and registry status
for read-only diagnostics. Authority-bearing observations use those
Forge-derived packets and the external origin-broker bridge described in the
[operator guide](operator-guide.md). The host signs a minimal closed answer
bound to the returned packet; `workflow action apply` derives, verifies, and
records the exact request without exposing an intermediate attestation. Never
manufacture request, registry, ledger, or receipt documents in the host.

`workflow action authorize` is a cooperative local one-call lane only for a
packet marked `operator_credential_broker`. Forge rejects that lane before
signing for human, independent-reviewer, and trusted-runtime broker packets.

The local signing bridge proves key possession only inside Forge's cooperative
same-OS-principal model. It does not prove human presence or reviewer
independence. An agent must never self-provision or use a `human`, `reviewer`, or
`runtime` local profile as evidence of a distinct actor. The external broker
vouches for the signed origin subject and separation domain; Forge does not
infer physical presence from those labels.

## Human attention

Ask the human only for an irreducible decision returned by the admitted
workflow, consent to an operator-owned trust ceremony, or information the agent
cannot obtain. Translate the decision into natural chat. Never ask the human to
select a workflow or edit internal YAML.

## Evidence discipline

- A file is not automatically working behavior.
- A self-report is not an independent review.
- A mocked execution is not a representative session.
- A second agent is independent only when principal and evidence are distinct.
- External/runtime capability must be verified by corresponding authority.

If authority is not provisioned, the correct result is a blocked gap. Never
hand-author a registry, signature, receipt, or ledger record to advance.

## Replacement agents

A replacement begins from `start` and `workflow resume`. It must not require
prior chat context. If durable state cannot reconstruct release, effective
Domain Pack generation, obligations, blockers, and next action, fail closed.

## Compatibility surfaces

`guide describe`, `guide status`, and `guide decide` are diagnostics. They do
not select authoritative P5/P6 workflow. New integrations use
`workflow init|resume|next`.

## Integration acceptance checklist

- Fresh and existing projects bootstrap idempotently.
- Paths with spaces remain one argv element.
- A stale snapshot/head is rejected and retried from new guidance.
- A consumed host event is idempotent and cannot authorize another packet.
- Broker absence/revocation blocks without falling back to a local human label.
- Missing evidence cannot complete a policy.
- Human questions appear only after prerequisite claims are verified.
- Replacement process returns the same durable epoch and next action.
- No private key, opaque capability, or operator anchor is exposed in chat.
