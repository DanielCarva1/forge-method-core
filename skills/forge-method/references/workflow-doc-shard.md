# workflow: doc-shard

trigger:
  - user asks to shard or split a large document
  - agent context would be overloaded by one large doc

inputs:
  - source document
  - target audience
  - desired shard structure
  - cross-link rules
  - original document handling decision

steps:
  1. identify natural boundaries and source-of-truth risks
  2. split markdown on stable section boundaries into focused shards
  3. create or update shard index links and ownership notes
  4. decide whether the original document is deleted, archived, or kept with waiver
  5. record source fingerprint, precedence rule, orphan/staleness notes, and validation command
  6. run `artifact doc-shard --path <document-utility-artifact>` to write/register the handoff
  7. run `artifact doc-check --path <document-utility-artifact>`

outputs:
  - sharded docs
  - index updates
  - generated shard handoff artifact
  - orphan/staleness notes
  - original document decision
  - source fingerprint and stale-check proof

done_when:
  - shards are independently useful
  - index connects the shard set
  - generated artifact is registered
  - source-of-truth risk is documented
  - original document handling avoids duplicate-source ambiguity
  - stale-check proof passes or a waiver is explicit

blocked_when:
  - source document cannot be edited
  - splitting would destroy required context
  - original and shards both remain authoritative without a precedence rule

handoff:
  - preserve source doc, shard paths, index path, original handling, precedence rule, fingerprint, and orphan/staleness notes
