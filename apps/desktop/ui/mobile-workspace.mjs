// The mobile switch changes only which existing panel is visible; it never
// copies project, preview or conversation state.
const navigation = document.getElementById('mobile-workspace-nav');
const workspace = document.querySelector('.workspace');
const buttons = [...navigation.querySelectorAll('[data-mobile-pane-button]')];
const panels = {
  conversation: document.querySelector('.workspace .conversation'),
  preview: document.getElementById('project-preview'),
  progress: document.getElementById('project-record'),
};

export function showWorkspacePane(pane, focus = false) {
  if (!Object.hasOwn(panels, pane)) return;
  workspace.dataset.mobilePane = pane;
  for (const button of buttons) {
    button.setAttribute('aria-pressed', String(button.dataset.mobilePaneButton === pane));
  }
  if (focus && matchMedia('(max-width: 700px)').matches) {
    const heading = panels[pane].querySelector('h2, h3');
    heading?.focus({ preventScroll: true });
    navigation.scrollIntoView({ block: 'start' });
  }
}

export function setMobileWorkspaceProject(project) {
  navigation.hidden = !project;
  showWorkspacePane('conversation');
}

for (const button of buttons) {
  button.addEventListener('click', () => {
    showWorkspacePane(button.dataset.mobilePaneButton);
    if (matchMedia('(max-width: 700px)').matches) navigation.scrollIntoView({ block: 'start' });
  });
}
showWorkspacePane('conversation');
