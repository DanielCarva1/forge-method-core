# workflow: investigation

trigger:
  - user asks to investigate, diagnose, triage, root-cause, or explain a failure
  - direct fixing would be premature because symptoms and cause are not separated

inputs:
  - symptom report
  - expected behavior
  - current state or failing artifact
  - logs, tests, evidence, and recent changes when available

steps:
  1. restate the observable symptom and expected behavior
  2. list likely hypotheses and what would prove or disprove each one
  3. choose the cheapest probe first
  4. record findings, confidence, and remaining uncertainty
  5. route to correct-course, build-story, research-closeout, or problem-solving

outputs:
  - investigation frame
  - hypotheses
  - probe plan and results
  - next reversible action

done_when:
  - cause is known or the next probe is explicit
  - confidence and uncertainty are recorded
  - next repair or research workflow is clear

blocked_when:
  - symptom cannot be reproduced or observed
  - required evidence or logs are inaccessible
  - investigation would require destructive action without approval

handoff:
  - preserve symptom, hypotheses, probes, findings, confidence, uncertainty, and next workflow
