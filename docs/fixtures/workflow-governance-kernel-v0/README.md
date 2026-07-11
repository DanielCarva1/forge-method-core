# Workflow governance kernel v0 fixtures

The canonical policy bundle is
`contracts/workflow-governance/kernel-v0.yaml`. These caller-authored
evaluation documents exercise the simulation/authority boundary. Every result
is candidate guidance with `authority: simulation_only`:

- `complete.yaml`: qualifying evidence, capability, prerequisite, and resolved
  decision state are all present;
- `active.yaml`: evidence collection is in progress;
- `missing-evidence.yaml`: no claim may be inferred from artifact intent or
  playbook text;
- `missing-capability.yaml`: the simulation reports a candidate blocker instead
  of pretending an absent environment is available;
- `human-decision.yaml`: an explicitly observed irreducible product choice
  stays with the human; starting the workflow alone does not trigger contact;
- `contradictory-evidence.yaml`: current disproof defeats a completion claim;
- `invented-completion.yaml`: an assertion without qualifying proposed evidence is
  reported explicitly;
- `invalid-cycle-bundle.yaml` and `invalid-dangling-bundle.yaml`: invalid policy
  graphs fail closed.

Playbooks are advisory. Tests deliberately delete and replace their text while
checking that candidate eligibility, progression, completion, claims,
obligations, capability gaps, decisions, and ranked next actions remain derived
exclusively from typed policy and caller-proposed observations. Only the opaque
kernel lane can turn a trusted Project Snapshot into consumable authority.
