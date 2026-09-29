import { readAppInfo } from './connection.mjs';
import { cancelPendingFirstSend, finishPendingFirstSend, prepareProjectSwitch, resumeSavedConversation, setProject } from './chat.mjs';
import { setProgressProject } from './progress.mjs';
import { setPreviewProject } from './preview.mjs';
import { rememberProject } from './recent-projects.mjs';
import { projectDisplayName } from './project-display.mjs';
import './navigation.mjs';
import { chooseStarter, clearStarterHandoff, showStarterHandoffError } from './explore.mjs';

const status = document.querySelector('#native-status');
const retry = document.querySelector('#retry');
const appVersion = document.querySelector('#app-version');
const openUpdates = document.querySelector('#open-updates');
const updatesStatus = document.querySelector('#updates-status');
const updatesUrl = document.querySelector('#updates-url');

async function refresh() {
  retry.disabled = true;
  status.textContent = 'Verificando o aplicativo…';
  const result = await readAppInfo(globalThis.__TAURI__?.core?.invoke);
  status.textContent = result.state === 'ready'
    ? `Aplicativo iniciado · versão ${result.version}`
    : 'Não foi possível acessar a parte nativa. Abra esta tela pelo aplicativo Forge.';
  appVersion.textContent = result.state === 'ready'
    ? `Versão instalada: ${result.version}`
    : 'Não foi possível consultar a versão instalada.';
  retry.disabled = false;
}

retry.addEventListener('click', refresh);
openUpdates.addEventListener('click', async () => {
  openUpdates.disabled = true;
  updatesStatus.textContent = 'Abrindo a página de versões no navegador…';
  updatesUrl.hidden = true;
  try {
    const invoke = globalThis.__TAURI__?.core?.invoke;
    if (typeof invoke !== 'function') throw new Error('Native bridge unavailable');
    await invoke('open_updates_page');
    updatesStatus.textContent = 'Página de versões solicitada ao navegador.';
  } catch {
    updatesStatus.textContent = 'Não foi possível abrir o navegador.';
    updatesUrl.hidden = false;
  } finally { openUpdates.disabled = false; }
});
refresh();

const form = document.querySelector('#project-form');
const inspect = document.querySelector('#inspect-project');
const start = document.querySelector('#start-project');
const projectStatus = document.querySelector('#project-status');
const resultPanel = document.querySelector('#project-result');
const projectRoot = document.querySelector('#project-root');
const browse = document.querySelector('#browse-project');
const createDefaultProject = document.querySelector('#create-default-project');
const defaultProjectStatus = document.querySelector('#default-project-status');
const modeHelp = document.querySelector('#project-mode-help');
const projectTitle = document.querySelector('#project-title');
const setup = document.querySelector('#project-setup');
const customFolderOption = document.querySelector('#custom-folder-option');
const setupSummary = document.querySelector('#project-setup-summary');
const projectLocation = document.querySelector('#project-location');
const workspaceTitle = document.querySelector('#workspace-title');
const workspaceIntro = document.querySelector('.workspace-screen .screen-heading .intro');
const workspaceBack = document.querySelector('#workspace-back');
const workspaceBackLabel = document.querySelector('#workspace-back-label');
const projectStep = document.querySelector('#project-step');
const resultLabel = document.querySelector('#project-result-label');
let projectMode = 'new';
let createdDefaultPath = '';
let startReturnTo = 'explore';

function resetWorkspaceHeading() {
  workspaceTitle.textContent = 'Vamos dar vida à sua ideia.';
  workspaceIntro.textContent = 'Escreva sua ideia e comece. O Forge pode criar o espaço do projeto para você; nada é enviado sem sua ação.';
  workspaceBack.href = startReturnTo === 'projects' ? '#projects' : '#explore';
  workspaceBackLabel.textContent = startReturnTo === 'projects' ? 'Voltar aos projetos' : 'Voltar às ideias';
}

function chooseMode(mode, openFolder = false, returnTo = 'explore') {
  if (projectRoot.disabled || !['new', 'existing'].includes(mode)) return;
  if (resultPanel.hidden || openFolder) setup.open = true;
  customFolderOption.open = openFolder;
  projectMode = mode;
  startReturnTo = returnTo;
  if (resultPanel.hidden) resetWorkspaceHeading();
  if (resultPanel.hidden) projectTitle.textContent = mode === 'new' ? 'Onde vamos criar?' : 'Qual projeto vamos abrir?';
  modeHelp.textContent = 'Você pode abrir uma pasta existente ou criar outra no seletor do Windows. O Forge prepara o projeto sem apagar seus arquivos.';
}

function clearPreviousProjectForFolderChoice() {
  projectRoot.value = '';
  projectRoot.dispatchEvent(new Event('input', { bubbles: true }));
}

for (const link of document.querySelectorAll('[data-project-mode]')) {
  link.addEventListener('click', async event => {
    if (link.hasAttribute('data-new-project') && !resultPanel.hidden) {
      // A new Explore idea must not inherit the previous project's conversation.
      event.preventDefault();
      event.stopImmediatePropagation();
      if (!await prepareProjectSwitch()) return;
      clearPreviousProjectForFolderChoice();
      chooseMode('new', true, link.dataset.returnTo);
      if (link.dataset.starter) chooseStarter(link);
      location.hash = '#workspace';
      return;
    }
    if (!link.hasAttribute('data-open-folder')) {
      chooseMode(link.dataset.projectMode, false, link.dataset.returnTo);
      return;
    }
    event.preventDefault();
    if (await prepareProjectSwitch()) {
      clearPreviousProjectForFolderChoice();
      chooseMode(link.dataset.projectMode, true, link.dataset.returnTo);
      location.hash = '#workspace';
    }
  }, { capture: true });
}
setup.querySelector('summary').addEventListener('click', async event => {
  if (!projectRoot.disabled) return;
  event.preventDefault();
  if (await prepareProjectSwitch()) {
    setup.open = true;
    projectRoot.focus();
  }
});

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
    projectStatus.textContent = 'Pasta escolhida. Clique em “Continuar nesta pasta” para abrir o projeto.';
  } catch (error) {
    projectStatus.textContent = typeof error === 'string' ? error : 'Não foi possível abrir a seleção de pastas.';
  } finally {
    if (!projectRoot.disabled) browse.disabled = false;
  }
});
createDefaultProject.addEventListener('click', async () => {
  if (createDefaultProject.disabled || projectRoot.disabled) return;
  if (!resultPanel.hidden && !await prepareProjectSwitch()) return;
  const invoke = globalThis.__TAURI__?.core?.invoke;
  if (!invoke) {
    cancelPendingFirstSend();
    defaultProjectStatus.textContent = 'Abra esta tela pelo aplicativo Forge para começar.';
    return;
  }
  createDefaultProject.disabled = true;
  projectRoot.disabled = true;
  browse.disabled = true;
  start.disabled = true;
  inspect.disabled = true;
  defaultProjectStatus.textContent = 'Criando seu espaço em Documentos…';
  let createdPath;
  try {
    createdPath = await invoke('create_default_project', { idea: document.querySelector('#message-text').value.trim() });
    defaultProjectStatus.textContent = 'Pasta criada. Preparando o projeto no Forge…';
  } catch (error) {
    cancelPendingFirstSend();
    defaultProjectStatus.textContent = typeof error === 'string' ? error : 'Não foi possível criar o projeto. Nada foi iniciado.';
  } finally {
    createDefaultProject.disabled = false;
    projectRoot.disabled = false;
    browse.disabled = false;
    start.disabled = false;
    inspect.disabled = false;
  }
  if (!createdPath) { cancelPendingFirstSend(); return; }
  createdDefaultPath = createdPath;
  projectRoot.value = createdPath;
  projectRoot.dispatchEvent(new Event('input', { bubbles: true }));
  form.requestSubmit(start);
});
projectRoot.addEventListener('input', () => {
  if (projectRoot.value.trim() !== createdDefaultPath) {
    createdDefaultPath = '';
    defaultProjectStatus.textContent = '';
  }
  projectRoot.removeAttribute('aria-invalid');
  setProject(null);
  setProgressProject(null);
  setPreviewProject(null);
  resultPanel.hidden = true;
  projectLocation.open = false;
  projectStep.textContent = 'SEU PROJETO';
  projectTitle.textContent = projectMode === 'new' ? 'Onde vamos criar?' : 'Qual projeto vamos abrir?';
  setupSummary.textContent = 'Começar ou abrir projeto';
  resetWorkspaceHeading();
  projectStatus.textContent = '';
});
form.addEventListener('submit', async event => {
  event.preventDefault();
  let openedProject = null;
  const readOnlyShortcut = event.submitter?.id === 'inspect-project';
  if (inspect.disabled || start.disabled) { cancelPendingFirstSend(); return; }
  if (!projectRoot.value.trim()) {
    cancelPendingFirstSend();
    projectRoot.setAttribute('aria-invalid', 'true');
    projectStatus.textContent = 'Escolha uma pasta para este projeto. Você pode usar “Escolher pasta…” ou informar o caminho acima.';
    setup.open = true;
    customFolderOption.open = true;
    projectRoot.focus();
    return;
  }
  inspect.disabled = true;
  start.disabled = true;
  browse.disabled = true;
  createDefaultProject.disabled = true;
  setProject(null);
  setProgressProject(null);
  setPreviewProject(null);
  projectRoot.disabled = true;
  resultPanel.hidden = true;
  projectStatus.textContent = readOnlyShortcut ? 'Conferindo o projeto…' : 'Preparando o projeto… Na primeira vez, isso pode levar alguns instantes.';
  try {
    const invoke = globalThis.__TAURI__?.core?.invoke;
    if (!invoke) throw 'Abra esta tela pelo aplicativo Forge para conferir seu projeto.';
    const project = await invoke(readOnlyShortcut ? 'inspect_project' : 'start_project', {
      projectRoot: projectRoot.value.trim(),
    });
    document.querySelector('#project-name').textContent = projectDisplayName(project);
    document.querySelector('#confirmed-project-id').textContent = project.project_id;
    document.querySelector('#confirmed-root').textContent = project.project_root;
    resultLabel.textContent = readOnlyShortcut ? 'PROJETO ENCONTRADO' : 'PROJETO PRONTO';
    resultPanel.hidden = false;
    projectLocation.open = false;
    setProject(project);
    setProgressProject(project);
    setPreviewProject(project);
    rememberProject(project);
    projectTitle.textContent = 'Seu projeto';
    projectStep.textContent = 'PROJETO EM USO';
    setupSummary.textContent = 'Trocar de projeto';
    workspaceTitle.textContent = projectDisplayName(project);
    workspaceIntro.textContent = 'Converse com seu agente e consulte o que ficou registrado sobre este projeto.';
    workspaceBack.href = '#projects';
    workspaceBackLabel.textContent = 'Voltar aos projetos';
    setup.open = false;
    projectStatus.textContent = 'Projeto pronto. Escreva para começar ou continuar a conversa; nada foi enviado.';
    if (projectRoot.value.trim() === createdDefaultPath) defaultProjectStatus.textContent = 'Projeto criado e pronto.';
    clearStarterHandoff();
    openedProject = project;
    if (!document.querySelector('#workspace').hidden) document.querySelector('#message-text').focus();
  } catch (error) {
    cancelPendingFirstSend();
    customFolderOption.open = true;
    setup.open = true;
    projectStatus.textContent = typeof error === 'string' ? error : 'Não foi possível conferir o projeto. Tente novamente.';
    if (projectRoot.value.trim() === createdDefaultPath) defaultProjectStatus.textContent = 'A pasta foi criada, mas o projeto ainda não ficou pronto. Tente “Continuar nesta pasta” novamente.';
    showStarterHandoffError(projectStatus.textContent);
  } finally {
    inspect.disabled = false;
    start.disabled = false;
    projectRoot.disabled = false;
    browse.disabled = false;
    createDefaultProject.disabled = false;
  }
  if (openedProject) {
    await resumeSavedConversation();
    finishPendingFirstSend(openedProject);
  }
});
