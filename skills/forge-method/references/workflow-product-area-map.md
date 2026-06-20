# workflow: product-area-map

trigger:
  - user asks to modularize product work
  - team needs ownership by area, path, repo, package, service, or domain
  - multiple people or agents may work in parallel

inputs:
  - product scope and architecture notes
  - current paths/packages/services
  - owners and responsibilities
  - contracts, dependencies, checks, and release boundaries

steps:
  1. name Product Areas without using Forge Module terminology
  2. map each area to paths, owner, purpose, contract, dependencies, checks, and evidence
  3. mark shared surfaces and conflict-prone files
  4. record split criteria for areas that may become standalone repos
  5. route trunk-based-plan, story-creation, architecture, or repo-split-plan

outputs:
  - product area map artifact
  - owner and contract map
  - split candidate list

done_when:
  - every planned parallel work surface has area, owner, dependency, validation, and handoff expectation
  - split candidates have explicit criteria instead of vague preference

blocked_when:
  - product boundaries are unknown
  - owner or contract is missing for high-risk parallel work

handoff:
  - preserve Product Area ids, paths, owners, contracts, dependencies, checks, split criteria, and next workflow
