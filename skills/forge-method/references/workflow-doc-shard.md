# workflow: doc-shard

trigger:
  - user asks to shard or split a large document
  - agent context would be overloaded by one large doc

inputs:
  - source document
  - target audience
  - desired shard structure
  - cross-link rules

steps:
  1. identify natural boundaries and source-of-truth risks
  2. split content into focused shards
  3. add index links and ownership notes
  4. verify no critical context was orphaned

outputs:
  - sharded docs
  - index updates
  - orphan/staleness notes

done_when:
  - shards are independently useful
  - index connects the shard set
  - source-of-truth risk is documented

blocked_when:
  - source document cannot be edited
  - splitting would destroy required context

handoff:
  - preserve source doc, shard paths, index path, and orphan/staleness notes
