# Execution Admission v0 fixtures

This fixture family is the executable acceptance corpus for P4a. The test
builder composes existing typed Assurance Case, Operation, Command, Effect,
Claim, and Gate fixtures into one valid commit-time snapshot. Each scenario in
`scenario-matrix.yaml` applies exactly one controlled mutation and asserts the
typed admission verdict. The request binds the canonical SHA-256 tokens of the
Assurance Case and every Operation, Command, and Effect contract so changing a
typed document after attestation cannot reuse the old authority.

The admitted scenario is intentionally narrow: one file-effect transaction,
read-only/offline commands, a complete and current claim/gate snapshot, an
Assurance Case ready for execution, an authorized principal registry result, a
fresh reserved nonce, and verified WAL lock/recovery/commit guarantees.

P4a is a read-only policy decision point. These fixtures prove only that
decision boundary; they do not by themselves prove runtime enforcement. Later
P4b trusted MCP/kernel suites provide the separate observation, preparation,
commit, replay, and recovery evidence.
