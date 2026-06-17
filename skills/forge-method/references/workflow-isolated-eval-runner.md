# workflow: isolated-eval-runner

trigger:
  - evals must run with stronger isolation than the current shell/session
  - user asks for Docker, sandboxed, reproducible, or untrusted eval execution
  - parity or release evidence needs a repeatable runner contract

inputs:
  - eval target and expected result
  - command set and required tools
  - trust boundary: local, container, remote sandbox, or manual waiver
  - fixture/data setup and cleanup policy
  - evidence path and release impact

steps:
  1. classify the runner need: reproducibility, untrusted code, dependency isolation, or cross-machine proof
  2. choose local, container, remote sandbox, or waiver mode with a reason
  3. define commands, environment inputs, fixture mounts, timeout, cleanup, and artifact outputs
  4. run or dry-run `scripts/forge-eval-runner.ps1`/`.sh` only when the project explicitly opts in
  5. record result, evidence, known nondeterminism, and next workflow

outputs:
  - isolated eval runner contract
  - command/evidence map
  - isolation decision or waiver
  - repeatability risks

done_when:
  - runner mode and trust boundary are explicit
  - commands are reproducible or waiver is justified
  - evidence path and cleanup policy are recorded
  - no always-on hook or background runner is introduced

blocked_when:
  - untrusted execution is required but no approved isolation tool is available
  - required secrets or external services cannot be scoped safely
  - command output cannot be made observable or repeatable

handoff:
  - preserve runner mode, commands, isolation boundary, timeout, cleanup, evidence path, waiver, and next validation command
