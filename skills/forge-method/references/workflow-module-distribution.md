# workflow: module-distribution

trigger:
  - a Forge module is ready to distribute, publish, install, or upgrade
  - module-builder output needs setup, registry, or install proof hardened

inputs:
  - module-builder artifact
  - module manifest and catalog entries
  - skill/plugin/package files
  - setup and configuration decisions
  - install, smoke, and upgrade commands

steps:
  1. identify distribution target: local, team, plugin, public, or standalone
  2. separate shared config, local/user config, and project override behavior
  3. verify capability/help registration and generated path ownership
  4. define install, reinstall, upgrade, and legacy cleanup checks
  5. record smoke commands, waivers, release note, and validation handoff

outputs:
  - distribution contract
  - setup/config registration map
  - install and upgrade proof
  - validation handoff

done_when:
  - distribution target and install path are explicit
  - config, capability, setup, and cleanup behavior are documented
  - install or smoke proof is recorded or waived
  - next workflow is module-validate or release check

blocked_when:
  - distribution target is unknown
  - install command cannot be run or cred/access is missing
  - generated package files contradict catalog/module metadata

handoff:
  - preserve distribution target, install command, smoke result, config boundary, cleanup policy, waivers, and next validation command
