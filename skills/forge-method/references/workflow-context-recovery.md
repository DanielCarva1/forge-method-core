# workflow: context-recovery

trigger:
  - context was compacted or reset
  - user asks to continue
  - agent is unsure where the project stands
  - chat, network, or tool context was interrupted
  - stale chat instructions may conflict with durable state

inputs:
  - `.forge-method/state.yaml`
  - `.forge-method/sprint.yaml`
  - context health
  - active story file
  - recent evidence
  - latest context pack
  - latest checkpoint
  - Help Oracle context boundary

steps:
  1. run `preflight`, `start`, and `resume --json` from the active skill or launcher
  2. trust current filesystem and launcher output over prior chat memory
  3. inspect Help Oracle context boundary and read only its `read_first` paths
  4. run `context health`
  5. if health is `compact` or `blocked`, run compact recovery before broader reading
  6. run `audit`
  7. generate or refresh context pack
  8. read only the active workflow reference or returned facilitation pack
  9. if the human supplied new intent, run `guide --question --json`
  10. continue from the authoritative recommended workflow

outputs:
  - context health result
  - context pack
  - current state summary
  - fresh chat boundary
  - compact read order
  - next action

done_when:
  - current phase, workflow, story, and next action are known
  - context health is `ok` or compact recovery exists
  - no broad doc reload is needed
  - stale chat instructions are explicitly discarded
  - the next command or workflow is authoritative

blocked_when:
  - state files contradict each other
  - active story points to a missing file
  - required context cannot fit and no compact recovery can be written
  - launcher output cannot identify a project or route choice

handoff:
  - preserve context health level, context boundary, context pack path, recovery brief, audit result, first command, and next workflow
