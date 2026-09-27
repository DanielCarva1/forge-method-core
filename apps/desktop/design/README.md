# Approved visual direction

Implementation scope: UI stories #81 and #83. This is the independent desktop
shell, not a claim of a released application or complete design system.

## References and asset inventory

The two original, unmodified user-approved concept images are retained here:
- `references/conversation-approved.png`: conversation, rounded panels, generous
  reading space and contextual information alongside the conversation.
- `references/explore-approved.png`: editorial category illustrations and the
  lavender/plum/coral visual family.

These are reference boards, not executable UI. Do not reproduce fictitious
project progress or previews to match a picture. The eight approved Explore
illustrations are reused as bounded crops from `references/explore-approved.png`
in `../ui/assets/explore-artwork.png`; no replacement artwork was generated.
Mobile-specific reference boards are not yet consolidated here.
The approved C app icon is reused at `../ui/assets/forge.png`; its export source
remains `../src-tauri/icons/source.png`. No new art direction was generated.
Home and Explore reuse decorative *regions* of the approved Explore board in
CSS, without placing its text or fictitious UI inside the app. The existing
full-size `explore-artwork.png` is reused rather than adding another image.

## Implementation rules

- `../ui/styles.css` is the token and component authority. Do not create a second
  token store. Light canvas is lavender, surfaces warm white, text deep plum,
  primary action coral. Dark mode uses plum surfaces and pale text.
- Use system Segoe UI without network fonts; 18px body baseline. Keep text in
  HTML, not baked into illustrations. Small uppercase labels become body-sized
  on narrow screens. Text must wrap when enlarged to 200%.
- Use 12/16/20/24/28/32px spacing, 24px panels and pill-shaped actions. Conversation
  messages have explicit speaker labels and different alignment/shape as well
  as color. Never communicate action state through color alone.
- Main content uses a project/context column and a larger conversation column.
  At 900px and below use natural single-column document order. Do not shrink text
  to squeeze desktop columns onto a phone. Do not trap scrolling in nested panes.
- Respect OS dark, increased-contrast and forced-color preferences. Focus has a
  visible outline; disabled controls use a dashed border and remain readable.
  Every action has visible text and a minimum 48px height.
- Empty conversations are explanatory UI, not simulated assistant messages.
  Connection details and introductory help use native details/summary disclosure.
- Existing native commands and project/agent ownership remain unchanged. No fake
  preview, invented stage progression, new history store or external asset request.

## Verification and gaps

Focused browser checks cover existing event/error behavior, narrow layouts,
200% text enlargement and forced-color controls. Native tests verify the actual
packaged HTML and project lookup. Visual review compares hierarchy, palette,
message shapes and readable spacing against the retained concepts.

Still separate work: backend-driven progress (#86), complete preview (#91), reopening
saved conversations (#93), manual appearance
preferences (#84) and distribution (#97). System-theme support alone does not
establish complete accessibility conformance or mobile remote-agent access.

The local preview now covers bounded project text and raster files in the
native WebView. It is not a browser, a published-artifact checker, or the full
review-and-change loop described by #91. The person can prefill a change
request in the same conversation without an automatic send.

Local Markdown documents can be read as bounded, safely formatted text, with
the original file text one click away in both preview sizes. This reuses the
conversation's non-executing formatter; links in a project document stay
literal rather than becoming privileged app actions.

The same validated file can now be opened in a larger, keyboard-dismissable
modal. This began as an inspection aid for images and literal text; the static
HTML extension below also renders local pages there. Neither view is evidence
that a site was published. The modal's change action returns to the existing
composer and still requires the person to send.

Static HTML files can now be displayed visually in a sandboxed iframe using
the selected page directory's bounded local CSS, raster images and fonts. The
code view remains one click away. The visual preview has no interaction or
script execution, is configured to block remote resources, and cannot stand in for a
deployed-site test or a JavaScript app's actual running state. The native
controlled capture and test limits are in the desktop checkpoint.
The enlarged preview can reveal more of a long page in bounded steps while
remaining non-interactive; the dialog scrolls around the inert frame. This
does not expose the page's own scroll or provide full-site acceptance.

The recorded-direction disclosure uses the existing read-only Forge resume
projection. It distinguishes the current accepted cooperative objective from
durably pending decisions and newly calculated questions. The full historical
decision review requested by #92 remains separate; opaque IDs are not turned
into made-up human-readable decisions.

An opt-in **Histórico de direções** disclosure now reads the existing Forge
report through a bounded native projection. It shows current versus earlier
accepted objective revisions, their recorded origin and date, with details
collapsed until requested. It does not turn opaque decision-audit references
into prose or count suggested questions as accepted decisions. This is one
useful part of #92, not completion of its full decision-history requirement.

The workspace now keeps folder/project switching, preview, and Forge record in
separate panels. Before a project is confirmed, the record stays hidden. After
confirmation, the conversation remains the dominant column and the right side
stacks project, local preview, and recorded context; a loaded preview rises to
the top as in the approved conversation reference. At narrow widths the DOM
order remains project setup, conversation, preview, then record. This fixes the
previous giant project-setup panel without fabricating preview or stage data.

The manual **Abrir conversa** action is explicitly optional because the first
Send can open the conversation; it also lets a person inspect prior messages
without sending. Idle status is subdued, not removed, and the composer grows
with longer drafts before scrolling. These are conversation-entry refinements,
not a change to Codex history or project authority.

The confirmed-project conversation now keeps that optional **Abrir conversa**
action, the explicit new-conversation choice, and the Codex conversation picker
inside one **Conversas e histórico** disclosure. The first visible action is
still writing and sending a message; the history controls are available on
request, not scattered above an empty chat. On connection, focus returns to
the visible status instead of disappearing into the now-hidden disclosure.
The agent badge sits beneath the conversation title, closer to the approved
board. No conversation state, auto-send, or native command changed.
The idle **Interromper** button is hidden until a turn is running. If a send
has unconfirmed delivery, the next attempted Send opens the history disclosure
and focuses **Abrir conversa** so recovery instructions lead to a visible
action. A ready project says **SUA CONVERSA** even before connecting.

Because the current app-server session uses `approvalPolicy: never` and
`danger-full-access`, a plain-language notice remains visible by the composer
after a project is selected. It states that the agent can run commands and
change files without per-action confirmation, including outside the selected
folder. This is disclosure of the current capability, not a safety control or
an approval UI; the transport still rejects interactive requests.

For a confirmed project, the folder path and confirmation explanation now sit
behind a **Ver pasta confirmada** disclosure, while project switching remains
one visible summary away. This keeps the preview/conversation higher in the
viewport and preserves the real path and its status for inspection. The main
navigation says **Explorar** and **Minha conversa**, rather than suggesting
that an already-open project is being newly created. No route or project
authority changed.

The confirmed-project workspace also condenses its title area instead of
repeating first-use instructions above the chat. The conversation card keeps
the composer inside its border even when the contents grow. Initial setup
continues to show the full guidance. This narrows the gap to the approved
conversation-first hierarchy; the later static HTML preview is separately
limited and does not claim pixel fidelity.

Completed Codex responses now receive a small, DOM-only readability treatment
for headings, lists, emphasis, inline code and fenced code. Streaming and
incomplete responses stay literal until a completed item arrives. A single
conversation control can show the exact original text instead. Agent HTML is
never executed. Supported local file references may open the read-only project
preview after native containment and type checks; external URLs do not navigate
the app. Ordinary tables, quoted text and separators also render as safe DOM;
wide tables scroll in a focusable region rather than widening the page. This
is intentionally not a full Markdown engine; unsupported syntax remains
visible as text.

Previous Codex conversations for the confirmed project use a six-entry
read-only page, not a growing list or nested scrolling pane. Long titles are
visually limited to two lines but remain available to assistive technology and
on hover. The real native capture is recorded in the desktop checkpoint; its
conversation titles are private test data, not approved artwork.

## Appearance preferences (#84, incremental)

The native HTML offers System/Light/Dark and an independent increased-contrast
checkbox. `ui/appearance.js` reads a validated `forge.appearance.v1` localStorage
value before styles are loaded to avoid a mismatched initial theme. This is a
device UI preference, not project state or history. OS theme changes apply only
in System mode; OS increased contrast and forced colors remain respected.
Storage errors leave the UI usable and produce an explicit unsaved notice.
No animation is introduced. Status icons supplement readable text without
claiming workflow completion. Focused tests cover initial keyboard traversal,
connected controls, long-conversation focus visibility and reduced motion.
This does not establish screen-reader certification or a published user release.
