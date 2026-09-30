// Desktop shortcuts and the narrow-screen switch share the existing panels;
// neither copies state nor performs project or conversation operations.
import { setConversationFocus } from './conversation-focus.mjs';
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
    else panels[pane].scrollIntoView({ block: 'start' });
  }
}

export function setMobileWorkspaceProject(project) {
  navigation.hidden = !project;
  showWorkspacePane('conversation');
}

for (const button of buttons) {
  button.addEventListener('click', () => {
    showWorkspacePane(button.dataset.mobilePaneButton, true);
  });
}
showWorkspacePane('conversation');
