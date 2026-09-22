import { readAppInfo } from './connection.mjs';
import { setProject } from './chat.mjs';
import { setProgressProject } from './progress.mjs';
import { rememberProject } from './recent-projects.mjs';
import './navigation.mjs';
import './explore.mjs';

const status = document.querySelector('#native-status');
const retry = document.querySelector('#retry');

async function refresh() {
  retry.disabled = true;
  status.textContent = 'Verificando o aplicativo…';
  const result = await readAppInfo(globalThis.__TAURI__?.core?.invoke);
  status.textContent = result.state === 'ready'
    ? `Aplicativo iniciado · versão ${result.version}`
    : 'Não foi possível acessar a parte nativa. Abra esta tela pelo aplicativo Forge.';
  retry.disabled = false;
}

retry.addEventListener('click', refresh);
refresh();

const form = document.querySelector('#project-form');
const inspect = document.querySelector('#inspect-project');
const projectStatus = document.querySelector('#project-status');
const resultPanel = document.querySelector('#project-result');
const projectRoot = document.querySelector('#project-root');
const browse = document.querySelector('#browse-project');
browse.addEventListener('click', async () => {
  if (browse.disabled || projectRoot.disabled) return;
  browse.disabled = true;
  projectStatus.textContent = 'Abrindo suas pastas…';
  try {
    const invoke = globalThis.__TAURI__?.core?.invoke;
    if (!invoke) throw 'Abra esta tela pelo aplicativo Forge para escolher uma pasta.';
    const path = await invoke('choose_project_folder');
    if (!path || projectRoot.disabled) {
      projectStatus.textContent = path ? '' : 'Seleção cancelada. Nenhum projeto foi alterado.';
      return;
    }
    projectRoot.value = path;
    projectRoot.dispatchEvent(new Event('input', { bubbles: true }));
    form.requestSubmit();
  } catch (error) {
    projectStatus.textContent = typeof error === 'string' ? error : 'Não foi possível abrir a seleção de pastas.';
  } finally {
    if (!projectRoot.disabled) browse.disabled = false;
  }
});
projectRoot.addEventListener('input', () => {
  setProject(null);
  setProgressProject(null);
  resultPanel.hidden = true;
  projectStatus.textContent = '';
});
form.addEventListener('submit', async event => {
  event.preventDefault();
  if (inspect.disabled) return;
  inspect.disabled = true;
  browse.disabled = true;
  setProject(null);
  setProgressProject(null);
  projectRoot.disabled = true;
  resultPanel.hidden = true;
  projectStatus.textContent = 'Conferindo o projeto…';
  try {
    const invoke = globalThis.__TAURI__?.core?.invoke;
    if (!invoke) throw 'Abra esta tela pelo aplicativo Forge para conferir seu projeto.';
    const project = await invoke('inspect_project', {
      projectRoot: projectRoot.value.trim(),
    });
    document.querySelector('#project-name').textContent = project.project_id;
    document.querySelector('#confirmed-root').textContent = project.project_root;
    resultPanel.hidden = false;
    setProject(project);
    setProgressProject(project);
    rememberProject(project);
    projectStatus.textContent = 'Projeto encontrado. Confira se esta é a pasta que você quer usar.';
  } catch (error) {
    projectStatus.textContent = typeof error === 'string' ? error : 'Não foi possível conferir o projeto. Tente novamente.';
  } finally {
    inspect.disabled = false;
    projectRoot.disabled = false;
    browse.disabled = false;
  }
});
