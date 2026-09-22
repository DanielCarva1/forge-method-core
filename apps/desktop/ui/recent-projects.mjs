// Local shortcuts only. Forge remains the authority for project identity and state.
const storageKey = 'forge.projects.v1';
const list = document.querySelector('#recent-projects');
const empty = document.querySelector('#projects-empty');
const status = document.querySelector('#projects-status');
const projectRoot = document.querySelector('#project-root');
const projectForm = document.querySelector('#project-form');

function validProject(value) {
  return value && typeof value === 'object'
    && typeof value.project_id === 'string' && value.project_id.length > 0 && value.project_id.length <= 200
    && typeof value.project_root === 'string' && value.project_root.length > 0 && value.project_root.length <= 1024;
}

function readProjects() {
  try {
    const raw = localStorage.getItem(storageKey);
    if (!raw) return [];
    if (raw.length > 12000) throw new Error('Too large');
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) throw new Error('Invalid list');
    return parsed.filter(validProject).slice(0, 8);
  } catch {
    status.textContent = 'Não foi possível ler os atalhos deste dispositivo. Você ainda pode abrir um projeto.';
    return [];
  }
}

let projects = readProjects();

function saveProjects() {
  try {
    localStorage.setItem(storageKey, JSON.stringify(projects));
    status.textContent = '';
  } catch {
    status.textContent = 'Não foi possível guardar esta lista neste dispositivo. Seus projetos não foram alterados.';
  }
}

function renderProjects() {
  list.replaceChildren();
  empty.hidden = projects.length > 0;
  for (const project of projects) {
    const card = document.createElement('article');
    card.className = 'recent-project panel';
    const icon = document.createElement('span');
    icon.className = 'project-icon';
    icon.setAttribute('aria-hidden', 'true');
    icon.textContent = '✦';
    const details = document.createElement('div');
    details.className = 'recent-project-details';
    const title = document.createElement('h2');
    title.textContent = project.project_id;
    const path = document.createElement('p');
    path.textContent = project.project_root;
    details.append(title, path);
    const actions = document.createElement('div');
    actions.className = 'recent-project-actions';
    const open = document.createElement('button');
    open.type = 'button';
    open.className = 'primary';
    open.textContent = 'Abrir';
    open.setAttribute('aria-label', `Abrir ${project.project_id}`);
    open.addEventListener('click', () => {
      if (projectRoot.disabled) {
        if (projectRoot.value === project.project_root) location.hash = '#workspace';
        else status.textContent = 'Encerre a conversa atual antes de abrir outro projeto.';
        return;
      }
      projectRoot.value = project.project_root;
      projectRoot.dispatchEvent(new Event('input', { bubbles: true }));
      location.hash = '#workspace';
      projectForm.requestSubmit(); // The native resolver validates the shortcut again.
    });
    const remove = document.createElement('button');
    remove.type = 'button';
    remove.textContent = 'Remover da lista';
    remove.setAttribute('aria-label', `Remover ${project.project_id} da lista`);
    remove.addEventListener('click', () => {
      projects = projects.filter(item => item.project_root !== project.project_root);
      saveProjects();
      renderProjects();
    });
    actions.append(open, remove);
    card.append(icon, details, actions);
    list.append(card);
  }
}

export function rememberProject(project) {
  if (!validProject(project)) return;
  projects = [
    { project_id: project.project_id, project_root: project.project_root },
    ...projects.filter(item => item.project_root !== project.project_root),
  ].slice(0, 8);
  saveProjects();
  renderProjects();
}

renderProjects();
