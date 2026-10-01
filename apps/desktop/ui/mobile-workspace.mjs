// Desktop shortcuts and the narrow-screen switch share the existing panels;
// neither copies state nor performs project or conversation operations.
import { setConversationFocus } from './conversation-focus.mjs';
import { clampRect, snapRect, defaultLayout, encodeLayout, decodeLayout, panelNames } from './workspace-layout.mjs';
const navigation = document.getElementById('mobile-workspace-nav');
const workspace = document.querySelector('.workspace');
const buttons = [...navigation.querySelectorAll('[data-mobile-pane-button]')];
const panels = {
  conversation: document.querySelector('.workspace .conversation'),
  preview: document.getElementById('project-preview'),
  progress: document.getElementById('project-record'),
  project: document.getElementById('project-panel'),
};
const narrow = matchMedia('(max-width: 700px)');
const floatingScreen = matchMedia('(min-width: 901px) and (pointer: fine)');
const layoutKey = 'forge.workspace-layout.v1';
const labels = { conversation: 'Conversa', preview: 'Resultado', progress: 'Andamento', project: 'Projeto' };
let layout = null;
let bounds = null;
let stored = null;
let gesture = null;
let stack = [...panelNames];
try { stored = JSON.parse(localStorage.getItem(layoutKey)); } catch { /* Layout storage is optional. */ }
const status = document.getElementById('workspace-layout-status');
const organize = document.getElementById('workspace-organize');

function floating() { return workspace.classList.contains('floating-workspace'); }
function saveLayout() {
  if (!layout || !bounds) return;
  stored = encodeLayout(layout, bounds);
  try { localStorage.setItem(layoutKey, JSON.stringify(stored)); }
  catch { status.textContent = 'A disposição mudou, mas não foi salva neste aparelho.'; }
}
function front(pane) {
  stack = [...stack.filter(name => name !== pane), pane];
  panelNames.forEach(name => { panels[name].style.zIndex = String(stack.indexOf(name) + 1); });
}
function renderWindows() {
  if (!floating() || !layout) return;
  const focus = workspace.dataset.windowFocus;
  panelNames.forEach(name => {
    const panel = panels[name], r = focus === name ? { x: 0, y: 0, ...bounds } : layout[name];
    panel.toggleAttribute('data-window-minimized', layout[name].minimized);
    panel.toggleAttribute('data-window-obscured', !!focus && focus !== name);
    Object.assign(panel.style, { left: `${r.x}px`, top: `${r.y}px`, width: `${r.width}px`, height: `${r.height}px` });
    panel.querySelector('[data-window-action="focus"]').setAttribute('aria-label', focus === name ? `Restaurar ${labels[name]}` : `Ampliar ${labels[name]}`);
  });
}
function syncWindows() {
  // Large text and small/coarse screens use the established simple layout.
  const enabled = !navigation.hidden && floatingScreen.matches
    && parseFloat(getComputedStyle(document.documentElement).fontSize) <= 24;
  workspace.classList.toggle('floating-workspace', enabled);
  organize.hidden = !enabled;
  if (!enabled) {
    cancelGesture();
    for (const panel of Object.values(panels)) {
      ['left', 'top', 'width', 'height', 'z-index'].forEach(property => panel.style.removeProperty(property));
      panel.removeAttribute('data-window-minimized'); panel.removeAttribute('data-window-obscured');
    }
    return;
  }
  const next = { width: workspace.clientWidth, height: Math.max(620, innerHeight - 240) };
  if (!next.width) return;
  if (!bounds || next.width !== bounds.width || next.height !== bounds.height) {
    const previous = layout && bounds ? encodeLayout(layout, bounds) : stored;
    bounds = next; layout = decodeLayout(previous, bounds);
  }
  workspace.style.setProperty('--window-stage-height', `${bounds.height}px`);
  renderWindows();
}
function setWindowFocus(pane) {
  setConversationFocus(pane === 'conversation');
  workspace.dataset.windowFocus = pane || '';
  renderWindows();
}
workspace.addEventListener('forge:workspace-focus', event => {
  workspace.dataset.windowFocus = event.detail ? 'conversation' : '';
  if (event.detail && layout) layout.conversation.minimized = false;
  renderWindows();
});
function alter(pane, action) {
  if (!floating()) return;
  if (action === 'focus') {
    setWindowFocus(workspace.dataset.windowFocus === pane ? null : pane);
  } else if (action === 'minimize') {
    if (workspace.dataset.windowFocus) setWindowFocus(null);
    layout[pane].minimized = true;
    const next = buttons.find(button => button.dataset.mobilePaneButton === pane);
    next?.focus({ preventScroll: true });
    status.textContent = `${labels[pane]} recolhido. Abra novamente pela barra; nenhuma ação do agente foi interrompida.`;
    renderWindows(); saveLayout();
  } else {
    setWindowFocus(null);
    layout[pane] = { ...snapRect(action, bounds), minimized: false };
    front(pane); renderWindows(); saveLayout();
    status.textContent = `${labels[pane]} encaixado à ${action === 'left' ? 'esquerda' : 'direita'}.`;
  }
}
function cancelGesture() {
  if (!gesture) return;
  const { pane, original, control, pointerId } = gesture;
  layout[pane] = original; gesture = null;
  if (control.hasPointerCapture(pointerId)) control.releasePointerCapture(pointerId);
  workspace.removeAttribute('data-snapping');
  renderWindows();
}
function bindGeometry(control, pane, resize) {
  control.addEventListener('pointerdown', event => {
    if (control.classList.contains('window-chrome') && event.target.closest('button')) return;
    if (!floating() || workspace.dataset.windowFocus || event.button !== 0 || gesture) return;
    event.preventDefault(); front(pane); control.focus({ preventScroll: true });
    gesture = { pane, resize, control, pointerId: event.pointerId,
      startX: event.clientX, startY: event.clientY, original: { ...layout[pane] } };
    control.setPointerCapture(event.pointerId);
  });
  control.addEventListener('pointermove', event => {
    if (!gesture || gesture.control !== control || gesture.pointerId !== event.pointerId) return;
    const dx = event.clientX - gesture.startX, dy = event.clientY - gesture.startY, r = gesture.original;
    layout[pane] = { ...clampRect(resize ? { ...r, width: r.width + dx, height: r.height + dy }
      : { ...r, x: r.x + dx, y: r.y + dy }, bounds), minimized: false };
    const edge = workspace.getBoundingClientRect();
    const side = !resize && (event.clientX - edge.left < 36 ? 'left' : edge.right - event.clientX < 36 ? 'right' : '');
    workspace.dataset.snapping = side;
    renderWindows();
  });
  control.addEventListener('pointerup', event => {
    if (!gesture || gesture.control !== control || gesture.pointerId !== event.pointerId) return;
    const side = workspace.dataset.snapping;
    if (side) layout[pane] = { ...snapRect(side, bounds), minimized: false };
    gesture = null; workspace.removeAttribute('data-snapping');
    control.releasePointerCapture(event.pointerId);
    renderWindows(); saveLayout();
  });
  control.addEventListener('pointercancel', cancelGesture);
  control.addEventListener('lostpointercapture', () => { if (gesture?.control === control) cancelGesture(); });
  control.addEventListener('keydown', event => {
    if (event.target !== control) return;
    if (event.key === 'Escape') { cancelGesture(); return; }
    if (!floating() || workspace.dataset.windowFocus || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(event.key)) return;
    event.preventDefault(); front(pane);
    const step = event.shiftKey ? 40 : 12;
    const dx = event.key === 'ArrowLeft' ? -step : event.key === 'ArrowRight' ? step : 0;
    const dy = event.key === 'ArrowUp' ? -step : event.key === 'ArrowDown' ? step : 0;
    const r = layout[pane];
    layout[pane] = { ...clampRect(resize ? { ...r, width: r.width + dx, height: r.height + dy }
      : { ...r, x: r.x + dx, y: r.y + dy }, bounds), minimized: false };
    renderWindows(); saveLayout();
  });
}
// Only presentation controls are added; panels are never cloned or reparented.
const glyphs = { move: 'M5 9 2 12l3 3m4-10 3-3 3 3m4 4 3 3-3 3m-4 4-3 3-3-3M2 12h20M12 2v20',
  left: 'M3 4h18v16H3zM12 4v16M6 8h3v8H6z', right: 'M3 4h18v16H3zM12 4v16M15 8h3v8h-3z',
  focus: 'M4 9V4h5m6 0h5v5M4 15v5h5m6 0h5v-5', minimize: 'M5 12h14', resize: 'M8 20 20 8M14 20l6-6',
  reference: 'm8 13 7-7a3 3 0 0 1 4 4l-9 9a5 5 0 0 1-7-7l10-10M7 14l8-8' };
function iconButton(action, label) {
  const button = document.createElement('button'); button.type = 'button';
  button.className = 'window-icon'; button.dataset.windowAction = action;
  button.setAttribute('aria-label', label);
  button.addEventListener('keydown', event => { if (event.key === 'Escape') button.toggleAttribute('data-tooltip-dismissed', true); });
  for (const event of ['blur', 'pointerleave']) button.addEventListener(event, () => button.removeAttribute('data-tooltip-dismissed'));
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.setAttribute('viewBox', '0 0 24 24'); svg.setAttribute('aria-hidden', 'true');
  const path = document.createElementNS(svg.namespaceURI, 'path'); path.setAttribute('d', glyphs[action]);
  svg.append(path); button.append(svg); return button;
}
// Keep the existing reference action/listeners; only its secondary presentation changes.
const reference = document.getElementById('add-reference');
const referenceLabel = document.createElement('span'); referenceLabel.className = 'reference-label';
referenceLabel.textContent = reference.textContent;
reference.setAttribute('aria-label', reference.textContent);
reference.classList.add('composer-reference');
reference.replaceChildren(iconButton('reference', reference.textContent).firstChild, referenceLabel);
reference.addEventListener('keydown', event => { if (event.key === 'Escape') reference.toggleAttribute('data-tooltip-dismissed', true); });
for (const event of ['blur', 'pointerleave']) reference.addEventListener(event, () => reference.removeAttribute('data-tooltip-dismissed'));
document.getElementById('connect-agent').setAttribute('aria-describedby', 'connect-help');
for (const pane of panelNames) {
  const panel = panels[pane]; panel.dataset.windowPane = pane;
  const chrome = document.createElement('div'); chrome.className = 'window-chrome';
  chrome.tabIndex = -1; bindGeometry(chrome, pane, false);
  const move = iconButton('move', `Mover ${labels[pane]}; use as setas do teclado`);
  const title = document.createElement('span'); title.className = 'window-caption'; title.textContent = labels[pane];
  chrome.append(move, title); bindGeometry(move, pane, false);
  for (const [action, label] of [['left', 'Encaixar à esquerda'], ['right', 'Encaixar à direita'], ['focus', 'Ampliar'], ['minimize', 'Recolher']]) {
    const button = iconButton(action, `${label} ${labels[pane]}`);
    button.addEventListener('click', () => alter(pane, action)); chrome.append(button);
  }
  const resize = iconButton('resize', `Redimensionar ${labels[pane]}; use as setas do teclado`);
  resize.classList.add('window-resize'); bindGeometry(resize, pane, true);
  panel.prepend(chrome); panel.append(resize);
  panel.addEventListener('pointerdown', () => { if (floating()) front(pane); });
  panel.addEventListener('focusin', () => { if (floating()) front(pane); });
}
organize.addEventListener('click', () => {
  cancelGesture(); setWindowFocus(null); layout = defaultLayout(bounds);
  if (workspace.classList.contains('preview-loaded')) {
    layout.conversation = { ...snapRect('left', bounds), minimized: false };
    layout.preview = { ...snapRect('right', bounds), minimized: false };
  }
  renderWindows(); saveLayout(); status.textContent = 'Espaço organizado. Conversa e rascunho continuam aqui.';
});
floatingScreen.addEventListener('change', syncWindows);
addEventListener('resize', syncWindows);
new ResizeObserver(syncWindows).observe(workspace);
new MutationObserver(syncWindows).observe(document.documentElement, { attributes: true, attributeFilter: ['style'] });
export function setWorkspaceQuestionPending(pending) {
  document.getElementById('workspace-question-cue').textContent = pending ? ' · pergunta' : '';
}

// Keep headings and the sticky conversation below the navigation even when
// larger text wraps its buttons onto another row.
new ResizeObserver(() => {
  workspace.style.setProperty('--workspace-nav-height', `${navigation.hidden ? 0 : navigation.getBoundingClientRect().height}px`);
}).observe(navigation);

export function showWorkspacePane(pane, focus = false) {
  if (!Object.hasOwn(panels, pane)) return;
  if (floating() && layout) {
    if (workspace.dataset.windowFocus && workspace.dataset.windowFocus !== pane) setWindowFocus(null);
    layout[pane].minimized = false; front(pane); renderWindows(); saveLayout();
  }
  workspace.dataset.mobilePane = pane;
  for (const button of buttons) {
    button.setAttribute('aria-pressed', String(button.dataset.mobilePaneButton === pane));
  }
  if (focus) {
    if (pane !== 'conversation') setConversationFocus(false);
    const questions = document.getElementById('agent-questions');
    const heading = pane === 'conversation' && !questions.hidden
      ? questions.querySelector('h3') : panels[pane].querySelector('h2, h3');
    heading?.focus({ preventScroll: true });
    if (narrow.matches) navigation.scrollIntoView({ block: 'start' });
    else if (floating()) navigation.scrollIntoView({ block: 'start' });
    else panels[pane].scrollIntoView({ block: 'start' });
  }
}

export function setMobileWorkspaceProject(project) {
  navigation.hidden = !project;
  syncWindows();
  showWorkspacePane('conversation');
}

for (const button of buttons) {
  button.addEventListener('click', () => {
    showWorkspacePane(button.dataset.mobilePaneButton, true);
  });
}
showWorkspacePane('conversation');
