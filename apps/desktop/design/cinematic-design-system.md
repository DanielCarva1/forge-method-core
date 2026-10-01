# Forge — cinematic, immersive UI with restrained glassmorphism

Accepted direction: 2026-09-30. This is a visual implementation specification,
not a new workflow, runtime policy or claim that the installed UI implements it.
The companion `cinematic-board.html` is a local component board, not the app.

### Refinement — 2026-10-01

The maintainer requested a stronger cinematic/glass treatment. Study 02 replaces
the explanatory component dashboard with a scene-led workspace: a dominant
smoked-glass conversation, compact top-right appearance controls, bottom floating
icon dock, quiet scene caption and on-demand design information. This composition
and the new scene are proposals, not an assertion of final visual approval.

The board now uses `../ui/assets/cinematic-atelier.png`, a newly generated
full-resolution background, not an enlarged category-card crop. Existing approved
reference images remain untouched. This is a manually authored local design
asset; no runtime image generation, service or recurring request is introduced.
The reference board and production share this single image, not duplicated PNGs.

The reference explores lower glass opacity (56% chrome, 79% night conversation)
with 24px blur and a local darkening veil; day reading stays 91%. These are
compositional experiments, not a replacement for final composited contrast checks.
For production, raise reading opacity as needed for real long-form messages or
uncontrolled scenery; solid/high-contrast fallback always takes precedence.
Do not make the illustrative navigation or fictional dialogue a production state.

## Intent

The person works inside an inviting creative scene. The interface floats above
it, quiet until needed. Conversation and the result are the protagonists; folder
paths, diagnostics and secondary tools do not dominate the screen.

Keep the approved Forge identity: plum, lavender, warm coral, botanical artwork
and approachable language. Evolve the existing references rather than importing
a generic neon dashboard, sci-fi HUD or unrelated image style.

## Layers and layout

1. **Scene:** one static, local thematic background per workspace. Art is decorative;
   it does not contain interface text or claim anything about project progress.
   Keep detail at the edges, darken/soften behind reading areas. No autoplay video,
   parallax, automatic generated-image requests or provider calls for decoration.
2. **Navigation:** a compact floating rail with quiet secondary icon controls.
   The selected destination is unmistakable. First-use primary choices retain
   short text; do not hide the only way to start a project behind an unknown icon.
3. **Work surface:** one dominant conversation panel. The result may share the
   stage when it exists. Progress/project details are contextual drawers or the
   existing panel choices, not permanent equal-weight cards beside an empty chat.
4. **Temporary layer:** anchored tooltips/popovers; modal only when focus isolation
   is useful. Avoid floating clutter and permanent explanatory boxes.

**Interaction expansion accepted 2026-10-01:** the four existing workspace panels
are internal movable/resizable windows on sufficiently wide fine-pointer screens.
Moving is optional: useful defaults, click-to-snap, keyboard alternatives,
minimize/reopen, focus and one-click organize keep the experience usable without
window management. Keep small/coarse screens and enlarged text in the simple
layout. Save only device-local normalized geometry; no OS window per panel,
new project/agent state or docking framework. Use current DOM owners, routing
and data; preserve keyboard/focus and preview security boundaries. The roadmap
sequence is in `docs/development-plan.md`, not duplicated here.

## Token targets

These values are starting targets for migration into `../ui/styles.css`, the
single production token authority. Do not add a second runtime token store.

| Role | Night / cinematic | Day / soft studio |
| --- | --- | --- |
| Canvas | `#171320` | `#EEE9F4` |
| Reading surface | `#252030` at 94–98% opacity | `#FFF9F3` at 94–98% |
| Navigation / glass | `#252030` at 80–88% | `#FFF9F3` at 82–90% |
| Primary text | `#F1E8DD` | `#302337` |
| Secondary text | `#C4B9CE` | `#63556D` |
| Primary action | `#FF9987`, ink `#302028` | `#B84336`, ink `#FFF9F3` |
| Focus / selection | `#D0BCFF` | `#674096` |
| Success / caution / error | `#B2D7B5` / `#E7C58D` / `#FFB0AC` | `#326B49` / `#795112` / `#A73636` |

- Use warm off-white for night reading, not pure white. Warm plum ink for day.
- Transparency belongs mainly to chrome. Reading areas are nearly opaque.
- Glass blur: 16–20px on a small number of top-level surfaces; no stacked blurs.
- Border: 1px, subtle warm light at night / plum at day. Shadow broad and soft,
  no glow outlines. Radius: panel 24px, composer 18px, controls 12px.
- Spacing scale: 4 / 8 / 12 / 16 / 24 / 32 / 48px. Panel padding 24–32px.
- System Segoe UI; no remote fonts. Body 18px / 1.65, supporting text 15–16px,
  headings 24–36px. Reading measure 60–72 characters. Code remains monospace.
- Preserve device-level system/light/dark and stronger-contrast preferences.
  Do not force a night theme merely because the scene is cinematic.

## Components and interactions

**Icon control:** consistent outlined SVG family, 18–20px glyph, minimum 40px
desktop hit area (44px for touch/coarse pointer). Visible label only in tooltip
on hover or keyboard focus; accessible name always present. Tooltip after about
300ms hover, immediate focus, dismissible with Escape, never clipped by panels.
No essential instructions only in a tooltip. Touch has no hover: keep primary
navigation recognizable and expose secondary controls through a labeled menu.

**Primary action:** one clear, named action in the active work area. “Enviar”,
“Começar” and access-required “Conectar ao Codex” may retain text. Never make
Send, irreversible confirmation or first-use entry an ambiguous icon-only act.

**Conversation:** clean reading plane; distinguish human/agent without a box
around every paragraph. Calm message tint, restrained avatar, generous gaps.
Composer sits inside the same surface; attachment/options are icon controls.
Remove repeated boilerplate descriptions, not real warnings or needed decisions.

**Result:** genuine file/image/page only. Empty state is short and honest; no fake
website, fictional agent reply, stage bar or fabricated progress to fill space.
Existing explicit copy/open/modify actions keep their semantics and boundaries.

**Context drawer:** project, recorded agreements/history and connection details
remain available on demand. Plain-language summaries first, technical detail
collapsed. Do not invent a new project registry or replace backend authority.

**States:** hover = subtle fill; selected = tint plus marker; focus = 2px visible
ring with 3px offset; disabled = muted plus semantic disabled; busy = compact
activity indicator with a useful status. Success/error never rely on color alone.
Do not remove meaningful connection, permission or unconfirmed-send warnings.

**Motion:** 140–180ms opacity/transform, maximum 4px travel. No constant movement.
Respect reduced motion. Support solid surfaces when transparency/blur is disabled
or unsupported, and explicit forced-colors/high-contrast presentation.

## Scene selection, without complexity

Use existing approved local art first. The current illustration board contains
UI/text, so never stretch the entire board as a wallpaper: use only reviewed
decorative crops. Future category scenes can be local assets selected by an
explicit category choice; fall back to the Forge scene for unknown/general work.
No agent guessing, mandatory classification, background-generation service or
new scene settings screen is required for the first implementation.

## Screen composition

- **Início:** scene, one invitation, idea input and a clear start action. Existing
  folder selection stays optional; quieter route to open a previous project.
- **Explorar:** illustrated choices with short titles, not paragraphs or a wall
  of equally prominent buttons. Preserve user-approved category imagery.
- **Meus projetos:** readable project names and useful real recency/context;
  opening is obvious, management tools secondary. Never invent recency data.
- **Minha conversa:** reading/composer dominant; result available alongside or
  via existing navigation. Connection visible only when needed. Context on demand.

## Implementation and acceptance

The 0.1.82 presentation package extends the shared atelier scene and warm tokens
to the four existing routes. A compact header rail keeps the active desktop
destination named; all destinations retain accessible names and hover/focus
tooltips. Narrow screens use the same route links, without a second router.
Comfort controls live in one anchored, opaque menu with Escape/outside dismissal.
Hero/project/category surfaces replace the legacy decorative crop backgrounds;
original category artwork and project-list ownership are unchanged. Reading
opacity stays deliberately higher than the reference study to protect long sessions.
Secondary selected-file tools use small named-on-focus icons; the change request
stays text-labeled and drafts into the existing conversation, never auto-sending.

Apply a coherent shell + conversation/composer slice first, then projects/explore
and result/context surfaces, through existing modules. Replace superseded CSS
rules rather than piling permanent overrides on the legacy stylesheet. No backend
changes, new UI framework, window system or icon dependency required by this spec.

Check the changed views only: native screenshot inspected at normal window size,
small desktop, 200% zoom, keyboard navigation, tooltip/focus, day/night and opaque
fallback. Measure final composited text contrast against both bright and dark
background regions: target 4.5:1 for normal text, 3:1 for large text and controls.
Targets are not a compliance claim before the actual implementation is measured.
Hidden native checks must not steal focus. Do not rerun unrelated chat/backend
journeys to approve purely visual changes. No full Rust suite for CSS.

Done means recognizable Forge art, comfortable long-form reading, clear start/
send/resume, quiet secondary tools and no lost functionality—not a screenshot
alone and not a generic set of translucent cards.

### Reading and context package — 0.1.83

Agent prose is an open reading column rather than another shadowed card; human
messages retain a quiet inset surface. Warm text uses 1.75 line spacing, bounded
measure and solid contrasting code/table/quote surfaces. The scene canvas must
never be reused as the background of light-theme readable content.

Conversation options use the existing details owner as a small anchored menu,
with Escape, outside/focus dismissal and focus restoration after an action.
Draft height is maintained by the chat owner for text and width changes; no
second draft store or autosizing framework. Andamento gives objective and next
step priority, leaving original records/phase/history behind existing disclosures.
Refresh is secondary and named on hover/focus. No progress percentage is invented.

Questions keep visible primary submission and interruption actions; selecting an
option changes only the local form. Login and confirmation retain explicit prose,
solid text surfaces and real authorization boundaries. Glass belongs around these
surfaces, not over sensitive instructions. Presentation samples in acceptance
captures do not imply a model turn, recorded decision or authenticated sign-in.
