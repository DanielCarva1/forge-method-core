// Local shortcuts only. Forge remains the authority for project identity and state.
import { prepareProjectSwitch } from './chat.mjs';
import { projectDisplayName } from './project-display.mjs';
import { forgetPreviewProject } from './preview.mjs';
const storageKey = 'forge.projects.v1';
const maxRecent = 50;
const list = document.querySelector('#recent-projects');
const empty = document.querySelector('#projects-empty');
const filterBox = document.querySelector('#projects-filter-box');
const filter = document.querySelector('#project-filter');
const filterStatus = document.querySelector('#projects-filter-status');
const noResults = document.querySelector('#projects-no-results');
const openFolderLabel = document.querySelector('#projects-open-folder-label');
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
    if (raw.length > 100000) throw new Error('Too large');
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) throw new Error('Invalid list');
    return parsed.filter(validProject).slice(0, maxRecent);
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
  filterBox.hidden = projects.length === 0;
  const query = filter.value.trim().normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase('pt-BR');
  const shown = query ? projects.filter(project =>
    `${projectDisplayName(project)} ${project.project_root}`.normalize('NFD').replace(/\p{M}/gu, '').toLocaleLowerCase('pt-BR').includes(query)) : projects;
  filterStatus.hidden = !query;
  filterStatus.textContent = query ? `${shown.length} ${shown.length === 1 ? 'projeto encontrado' : 'projetos encontrados'} nesta lista.` : '';
  noResults.hidden = projects.length === 0 || shown.length > 0;
  openFolderLabel.textContent = projects.length ? 'Abrir outro projeto' : 'Escolher uma pasta';
  for (const project of shown) {
    const name = projectDisplayName(project);
    const card = document.createElement('article');
    card.className = 'recent-project panel';
    const icon = document.createElement('span');
    icon.className = 'project-icon';
    icon.setAttribute('aria-hidden', 'true');
    const brand = document.createElement('img');
    brand.src = 'assets/forge.png';
    brand.alt = '';
    icon.append(brand);
    const details = document.createElement('div');
    details.className = 'recent-project-details';
    const title = document.createElement('h2');
    title.textContent = name;
    const path = document.createElement('p');
    path.textContent = project.project_root;
    details.append(title, path);
    const actions = document.createElement('div');
    actions.className = 'recent-project-actions';
    const open = document.createElement('button');
    open.type = 'button';
    open.className = 'primary';
    open.textContent = 'Abrir';
    open.setAttribute('aria-label', `Abrir ${name} na pasta ${project.project_root}`);
    open.addEventListener('click', async () => {
      if (projectRoot.disabled && projectRoot.value === project.project_root) {
        location.hash = '#workspace';
        return;
      }
      if (projectRoot.disabled && (!await prepareProjectSwitch() || projectRoot.disabled)) {
        status.textContent = 'Não foi possível trocar de projeto agora. Confira a conversa atual e tente novamente.';
        return;
      }
      projectRoot.value = project.project_root;
      projectRoot.dispatchEvent(new Event('input', { bubbles: true }));
      document.querySelector('#project-setup').open = true;
      location.hash = '#workspace';
      projectForm.requestSubmit(document.querySelector('#inspect-project')); // The native resolver validates the shortcut again.
    });
    const remove = document.createElement('button');
    remove.type = 'button';
    remove.textContent = 'Remover da lista';
    remove.setAttribute('aria-label', `Remover ${name} da lista de projetos`);
    remove.addEventListener('click', () => {
      projects = projects.filter(item => item.project_root !== project.project_root);
      forgetPreviewProject(project.project_root);
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
  filter.value = '';
  projects = [
    { project_id: project.project_id, project_root: project.project_root },
    ...projects.filter(item => item.project_root !== project.project_root),
  ].slice(0, maxRecent);
  saveProjects();
  renderProjects();
}

filter.addEventListener('input', renderProjects);
renderProjects();
