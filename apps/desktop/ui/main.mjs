import { readAppInfo } from './connection.mjs';
import { setProject } from './chat.mjs';
import { setProgressProject } from './progress.mjs';

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
    projectStatus.textContent = 'Projeto encontrado. Confira se esta é a pasta que você quer usar.';
  } catch (error) {
    projectStatus.textContent = typeof error === 'string' ? error : 'Não foi possível conferir o projeto. Tente novamente.';
  } finally {
    inspect.disabled = false;
    projectRoot.disabled = false;
  }
});
