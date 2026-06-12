# Independent app research and experiment reference

- kind: research-reference
- created_at: 2026-06-12
- scope: Forge Method experiments, Codex plugin boundary, future independent app direction
- status: archived-reference

## Current Decision

Keep refining Forge Method as a Codex plugin for now.

The Codex performance issue is not a Forge-specific defect. Codex can become slow, stop responding, or crash even without Forge. Therefore, the experiment branches should not be treated as fixes for the current plugin surface.

Preserve the research as reference only. Delete the active experiment worktrees and branches.

## What Was Tested

### Hooked runtime prototype

- Branch: `codex/experiment-hooked-runtime`
- Commit: `5aaa6a9`
- Shape: Python hook wrapper around Forge runtime commands.
- Finding: useful future idea for deterministic lifecycle events, but it adds complexity before there is a concrete plugin need.
- Keep: event concepts such as pre-command, post-command, guide-classified, state-written, artifact-written, gate-failed.
- Do not keep now: active branch/worktree or hook surface in the plugin.

### TypeScript transcript harness

- Branch: `codex/experiment-ts-transcript-harness`
- Commit: `6bd8364`
- Shape: TypeScript replay harness for `tests/fixtures/guidance_transcripts.json`.
- Result: `npm run replay` passed `13/13`.
- Finding: good for future evaluation and UI/operator tooling, but it does not make Codex lighter.
- Keep: the idea of transcript replay as a product-quality proof for human guidance.
- Do not keep now: Node/npm dependency in the plugin runtime or normal Codex workflow.

### Rust inspector/bench prototype

- Branch: `codex/experiment-rust-app`
- Commit: `9f9fda6`
- Shape: Rust CLI that inspects `.forge-method/state.yaml` and benchmarks Python runtime cold start.
- Result: `cargo test` passed; Python runtime cold-start benchmark median was about `275 ms` in the experiment run.
- Finding: Rust is promising for a future native core, but a Rust inspector does not solve Codex surface instability today.
- Keep: Rust as likely future core direction for an independent Forge app.
- Do not keep now: Rust rewrite pressure inside the Codex plugin.

## Language And Architecture Research

Best future app shape:

```txt
Forge app outside Codex
- UI / human experience: TypeScript with React or Svelte
- Desktop shell: Tauri 2
- Native core: Rust
- State/artifacts: durable files first; SQLite only when query/index needs justify it
- Providers: user-configurable provider/model adapters
- Codex: one supported surface, not the only product surface
```

Why Rust for the future core:

- Rust is strongest where Forge needs local reliability: filesystem state, indexing, provider routing, process control, concurrency, and long-running local services.
- The main advantage is not raw speed alone. It is native performance plus memory/thread safety without a garbage collector.
- Rust should own core/runtime boundaries, not necessarily the visual UI.

Why not TypeScript as the plugin core:

- TypeScript is excellent for UI, replay tooling, and developer/operator interfaces.
- It does not reduce Codex crashes or context pressure when used as another harness.
- Adding Node/npm to the normal plugin path would increase operational surface area.

Why not rewrite the current plugin now:

- The current Forge plugin still needs refinement as a Codex-native runtime.
- Codex instability is outside the plugin boundary.
- A language rewrite would not fix the current surface if the same context volume and chat constraints remain.

## References

- Rust: https://www.rust-lang.org/
- Tauri: https://v2.tauri.app/start/
- Tauri sidecars: https://v2.tauri.app/develop/sidecar/
- Tauri updater: https://v2.tauri.app/plugin/updater/
- Electron: https://electronjs.org/
- Bun docs: https://bun.com/docs
- Bun Rust rewrite reporting: https://www.theregister.com/devops/2026/05/14/anthropics-bun-rust-rewrite-merged-at-speed-of-ai/5240381

## Archived Decision

Delete active experiment forks/worktrees:

- `codex/experiment-hooked-runtime`
- `codex/experiment-ts-transcript-harness`
- `codex/experiment-rust-app`

Continue with the main Forge path:

- refine Forge as a Codex plugin;
- keep agent-facing docs compact;
- keep human guidance rich through runtime routing and facilitation packs;
- revisit independent app work later as a separate product track with Rust core and TypeScript UI.
