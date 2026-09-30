# Forge Desktop 0.1.73 alpha.1

**Bring your own reference and move easily around your project.** The conversation
now offers **Adicionar arquivo à mensagem**. Choose a local image, text or other
document in the Windows dialog: its path is added visibly to the existing draft,
with a request to use it as a reference without executing it. Review or remove
that text before sending. Choosing a file does not create a project, read/copy
the file or send anything. When you explicitly send, Codex receives the ordinary
message and can use its existing local-file tools. This is not a file upload,
image-input API or guarantee that every document format can be interpreted.

The writing area grows for longer messages, with bounded scrolling. Enter adds
a new line; Ctrl+Enter sends when Send is available. Pending selection, an active
turn or a disconnected/uncertain send still prevents another Send. Canceled,
failed, duplicate or late file choices do not overwrite the draft. Navigation
away or a project change discards a pending selection.

This package also includes 0.1.72's visible Conversa, Resultado, Andamento and
Projeto shortcuts. Desktop scrolls to the existing panels; narrow screens show
the selected area. Keyboard focus and unsent drafts are preserved. No parallel
project state, new governance engine or model-routing service was introduced.
Forge Core remains 0.13.3.

## Verification and limits

Focused browser-double checks passed for reference selection/cancellation/error,
late-result safety, multiline writing, Enter, one exact Ctrl+Enter first Send,
default/custom project setup, draft changes during preparation, and project-area
navigation with narrow layouts/200% text. Desktop Rust check and focused reference
tests passed; the affected desktop crate has 54 passing tests and one existing
ignored default-browser association test. No core workspace suite was run.

Native installer, actual Windows reference-dialog and published download
readback results are recorded in the Desktop README. No new real Codex file-read
turn is claimed: the reference is ordinary visible text, and native selection
proof does not prove a model has read or understood the selected content.
Physical phones, manual screen-reader use and clean-device login remain unverified.
The broad browser suite still has outdated first-use expectations and was not
rerun or represented as passing. A crate-wide formatting check also identifies
pre-existing formatting differences; the new reference module is formatted.

Unsigned Windows x64 alpha; updates use the installer manually. Protected preview
is local and static. Normal-browser use remains an explicit trust-sensitive
action. Local installation and publication are distinct; consult the README for
the exact package/download status. No subscription-cost or productivity savings
are inferred from these checks.
