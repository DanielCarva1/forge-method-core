# workflow: trunk-based-plan

trigger:
  - user asks for trunk-based development
  - team needs branch, PR, review, CODEOWNERS, rulesets, or merge policy
  - CI/check rules must be settled before build loops

inputs:
  - team operating model
  - Product Area map
  - current repo and CI commands
  - review/ruleset preferences

steps:
  1. choose main/trunk branch and short-lived branch naming
  2. define PR size, review, required checks, CODEOWNERS/rulesets, and conflict policy
  3. decide whether merge queue is needed now or deferred
  4. record CI/platform follow-up when checks or GitHub setup must be implemented
  5. link collaboration-handoff expectations for every branch/PR

outputs:
  - trunk-based plan artifact
  - branch and PR policy
  - CI/platform follow-up

done_when:
  - team can merge small PRs into trunk without guessing checks, reviewers, or conflict handling
  - CI/platform follow-up is explicit when required

blocked_when:
  - required checks are unknown
  - branch protection/ruleset authority is missing

handoff:
  - preserve trunk branch, PR size, review/check policy, CODEOWNERS/ruleset stance, merge queue stance, and next workflow
