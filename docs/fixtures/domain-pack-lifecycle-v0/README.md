# Domain Pack lifecycle v0.2 corpus

This corpus exercises the closed P6b wire boundary. Files in `valid/` are
typed, candidate-only examples used by the contract-family inventory. Files in
`adversarial/` are intentionally invalid and must never be admitted merely
because they are YAML.

The fixtures deliberately contain no private keys, real signatures, trust
claims, executable adapter commands, credentials, or active mutation
authority. Cryptographic verification and lifecycle authority remain inside
the trusted P6b boundary.
