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

Still separate work: backend-driven progress (#86), preview (#91), reopening
saved conversations (#93), manual appearance
preferences (#84) and distribution (#97). System-theme support alone does not
establish complete accessibility conformance or mobile remote-agent access.

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
