import { readAppInfo } from './connection.mjs';
import { prepareProjectSwitch, setProject } from './chat.mjs';
import { setProgressProject } from './progress.mjs';
import { setPreviewProject } from './preview.mjs';
import { rememberProject } from './recent-projects.mjs';
import { projectDisplayName } from './project-display.mjs';
import './navigation.mjs';
import { chooseStarter } from './explore.mjs';

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
const start = document.querySelector('#start-project');
const projectStatus = document.querySelector('#project-status');
const resultPanel = document.querySelector('#project-result');
const projectRoot = document.querySelector('#project-root');
const browse = document.querySelector('#browse-project');
const modeHelp = document.querySelector('#project-mode-help');
const projectTitle = document.querySelector('#project-title');
const setup = document.querySelector('#project-setup');
const setupSummary = document.querySelector('#project-setup-summary');
const projectLocation = document.querySelector('#project-location');
const workspaceTitle = document.querySelector('#workspace-title');
const workspaceIntro = document.querySelector('.workspace-screen .screen-heading .intro');
const workspaceBack = document.querySelector('#workspace-back');
const workspaceBackLabel = document.querySelector('#workspace-back-label');
const projectStep = document.querySelector('#project-step');
const resultLabel = document.querySelector('#project-result-label');
let projectMode = 'new';

function resetWorkspaceHeading() {
  workspaceTitle.textContent = 'Vamos dar vida à sua ideia.';
  workspaceIntro.textContent = 'Escolha onde guardar o projeto. Depois conte sua ideia: a conversa começa quando você enviar.';
  workspaceBack.href = '#explore';
  workspaceBackLabel.textContent = 'Voltar às ideias';
}

function chooseMode(mode, openFolder = false) {
  if (projectRoot.disabled || !['new', 'existing'].includes(mode)) return;
  if (resultPanel.hidden || openFolder) setup.open = true;
  projectMode = mode;
  if (resultPanel.hidden) projectTitle.textContent = mode === 'new' ? 'Onde vamos criar?' : 'Qual projeto vamos abrir?';
  modeHelp.textContent = mode === 'new'
    ? 'A pasta pode estar vazia ou já conter arquivos. Se ainda não usa Forge, vamos prepará-la sem apagar o que existe.'
    : 'Escolha qualquer pasta de projeto. Se ainda não usa Forge, vamos prepará-la sem apagar o que existe.';
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
      chooseMode('new', true);
      if (link.dataset.starter) chooseStarter(link);
      location.hash = '#workspace';
      return;
    }
    if (!link.hasAttribute('data-open-folder')) {
      chooseMode(link.dataset.projectMode);
      return;
    }
    event.preventDefault();
    if (await prepareProjectSwitch()) {
      clearPreviousProjectForFolderChoice();
      chooseMode(link.dataset.projectMode, true);
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
projectRoot.addEventListener('input', () => {
  setProject(null);
  setProgressProject(null);
  setPreviewProject(null);
  resultPanel.hidden = true;
  projectLocation.open = false;
  projectStep.textContent = 'PASSO 1 · SEU PROJETO';
  projectTitle.textContent = projectMode === 'new' ? 'Onde vamos criar?' : 'Qual projeto vamos abrir?';
  setupSummary.textContent = 'Escolher pasta do projeto';
  resetWorkspaceHeading();
  projectStatus.textContent = '';
});
form.addEventListener('submit', async event => {
  event.preventDefault();
  const readOnlyShortcut = event.submitter?.id === 'inspect-project';
  if (inspect.disabled || start.disabled) return;
  inspect.disabled = true;
  start.disabled = true;
  browse.disabled = true;
  setProject(null);
  setProgressProject(null);
  setPreviewProject(null);
  projectRoot.disabled = true;
  resultPanel.hidden = true;
  projectStatus.textContent = readOnlyShortcut ? 'Conferindo o projeto…' : 'Preparando o projeto…';
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
    projectStatus.textContent = 'Projeto pronto. Escreva sua ideia e envie para começar a conversa.';
    if (!document.querySelector('#workspace').hidden) document.querySelector('#message-text').focus();
  } catch (error) {
    projectStatus.textContent = typeof error === 'string' ? error : 'Não foi possível conferir o projeto. Tente novamente.';
  } finally {
    inspect.disabled = false;
    start.disabled = false;
    projectRoot.disabled = false;
    browse.disabled = false;
  }
});
