# Forge Desktop 0.1.74 alpha.1

**Answer the agent's questions without leaving your project.** Supported Codex
questions now appear in the conversation, with unselected options and a written
answer alternative. Review your choices and click **Enviar respostas**. Your
ordinary message draft stays intact. You can interrupt the execution instead.

Answers go back to the exact pending Codex request, not into a new chat turn.
Already answered, resolved, interrupted and wrong-conversation requests are
rejected. Ending a turn or disconnecting removes its forms. Unsupported requests,
including execution approvals and secret collection, are not automatically
accepted. This is a question interface, not a new permission system or a new
history store. Forge Core remains 0.13.3.

## Verification and limits

Focused browser-double checks passed for options, free text, incomplete answers,
literal question rendering, failed/pending submission, duplicate prevention,
preserved drafts, other-conversation rejection, resolution, interruption and
narrow screens with enlarged text. Reference and folder composer checks also
passed. Rust check and question tests passed; the desktop crate has 56 passing
tests and one existing ignored browser-association test. No core workspace
suite, broad browser suite or manual GitHub CI was run.

Native installation and question/answer protocol results are recorded in the
Desktop README. A simulated Codex server is explicitly identified; it is not
proof of a real model choosing or using this tool. Only the supported
`item/tool/requestUserInput` interface is handled, with up to three bounded
questions per request. Unsupported interactions remain explicit.

Unsigned Windows x64 alpha; manual installer updates. Protected previews remain
local and static. Secure remote mobile access, physical-phone keyboard behavior,
manual screen-reader use and clean-device completed login are not validated.
The older broad browser suite still has outdated first-use expectations.
Installation and publication are separate; the README records their actual
status. No monetary savings or productivity percentage is inferred.
