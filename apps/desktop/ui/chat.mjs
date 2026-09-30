import { invalidateProgress, refreshProgressAfterTurn } from './progress.mjs';
import { readReference, saveReference, readUnconfirmedSend, markUnconfirmedSend, clearUnconfirmedSend } from './conversation-reference.mjs';
import { renderAgentMessage } from './message-format.mjs';
import { previewLinkedFile, refreshPreviewAfterTurn } from './preview.mjs';
import { projectDisplayName } from './project-display.mjs';
import { setConversationFocus } from './conversation-focus.mjs';
import { createQuestions } from './questions.mjs';
import { localAppLinks } from './local-app-links.mjs';
import { showWorkspacePane, setWorkspaceQuestionPending } from './mobile-workspace.mjs';
const byId = id => document.getElementById(id);
const status = byId('agent-status');
const updateAction = byId('agent-update-action');
const loginPanel = byId('login-panel');
const loginChallenge = byId('login-challenge');
const loginStatus = byId('login-status');
const loginCode = byId('login-code');
const copyLoginCodeButton = byId('copy-login-code');
const copyLoginStatus = byId('copy-login-status');
const loginUrl = byId('login-url');
const startLoginButton = byId('start-login');
const finishLoginButton = byId('finish-login');
const cancelLoginButton = byId('cancel-login');
const openLoginPageButton = byId('open-login-page');
const connect = byId('connect-agent');
const connectHelp = byId('connect-help');
const disconnect = byId('disconnect-agent');
const send = byId('send-message');
const stop = byId('interrupt-agent');
const input = byId('message-text');
const composerHelp = byId('composer-help');
const draftNote = byId('draft-note');
const accessNote = byId('agent-access-note');
const addReference = byId('add-reference');
const referenceStatus = byId('reference-status');
let referencePending = false;
let referenceRequest = 0;
const messages = byId('messages');
const conversationBody = document.querySelector('.conversation-body');
const jumpLatest = byId('jump-latest');
const messageView = byId('message-view');
const conversationOptions = byId('conversation-options');
const newConversation = byId('new-conversation');
const newConversationChoice = byId('new-conversation-choice');
const conversationPicker = byId('conversation-picker');
const findConversations = byId('find-conversations');
const previousConversations = byId('previous-conversations');
const moreConversations = byId('more-conversations');
const conversationList = byId('conversation-list');
const conversationListStatus = byId('conversation-list-status');
const projectStatus = byId('project-status');
const projectSetup = byId('project-setup');
const projectResultHint = byId('project-result-hint');
const conversationStep = byId('conversation-step');
const emptyDescription = byId('empty-conversation-description');
const resumeLastConversation = byId('resume-last-conversation');
const reopenConversation = byId('reopen-conversation');
const previewLastResult = byId('preview-last-result');
const previewIntro = byId('preview-intro');
const emptyPreviewText = 'Confira arquivos citados na conversa ou escolha um da pasta do projeto.';
const confirmReviewedSend = byId('confirm-reviewed-send');
let project = null;
let connected = false;
let busy = false;
let transitioning = false;
let loginActive = false;
let loginPending = false;
let loginCompletedEarly = false;
let broken = false;
let reconnectable = false;
let generation = 0;
let channel;
let activeThreadId = null; // A new empty thread is not yet durable Codex history.
const items = new Map();
let latestItem = null;
let lastResultPath = null;
const citedFiles = document.getElementById('preview-cited-files');
const citedFilesList = document.getElementById('preview-cited-files-list');
const citedFilesMore = document.getElementById('preview-cited-files-more');
const citedFilesSummary = document.getElementById('preview-cited-files-summary');
let rawMessageView = false;
let restoringHistory = false;
let listGeneration = 0;
let listPending = false;
let listCursor = null;
let pageNumber = 0;
let pageCursors = [null];
let hasPreviousConversation = false;
let hasResumableConversation = false;
const sessionReferences = new Map();
const unconfirmedSends = new Map(); // Project key -> Codex thread ID; no message copy.
const projectDrafts = new Map(); // Immediate in-session copy; persisted drafts stay local to this device.
let draftOrigin = null; // Survives a same-folder validation attempt that temporarily clears `project`.
let draftStorageFailed = false;
let pendingFirstSend = null; // One explicit Send while the first project is prepared; never persisted or retried.
const projectKey = value => JSON.stringify([value.project_id, value.project_root]);
const draftKey = value => `forge.draft.v1:${projectKey(value)}`;
const referenceKey = () => JSON.stringify([project.project_id, project.project_root]);
const invoke = (command, args) => globalThis.__TAURI__.core.invoke(command, args);
const questions = createQuestions(byId('agent-questions'), args => invoke('answer_questions', args), () => invoke('interrupt_agent'), () => activeThreadId, {
  onPendingChange: setWorkspaceQuestionPending,
  restoreFocus: () => status.focus({ preventScroll: true }),
});
function readStoredDraft(value) {
  const text = localStorage.getItem(draftKey(value));
  if (text !== null && text.length > 100000) throw new Error('Stored draft is too large');
  return text;
}
function storeDraft(value, text) {
  try {
    if (text.length > 100000) {
      localStorage.removeItem(draftKey(value));
      draftStorageFailed = true;
      return false;
    }
    if (text) localStorage.setItem(draftKey(value), text);
    else localStorage.removeItem(draftKey(value));
    draftStorageFailed = false;
    return true;
  } catch {
    draftStorageFailed = true;
    return false;
  }
}

// Icons supplement readable text; these are connection/turn states, not workflow progress.
const statusIcons = { idle: '○', working: '◷', connected: '↔', completed: '✓', interrupted: 'Ⅱ', error: '!', disconnected: '○' };
function showStatus(text, kind = 'idle') {
  const icon = document.createElement('span');
  icon.className = 'status-icon';
  icon.setAttribute('aria-hidden', 'true');
  icon.textContent = statusIcons[kind] ?? statusIcons.idle;
  const label = document.createElement('span');
  label.textContent = text;
  status.replaceChildren(icon, label);
  status.dataset.state = kind;
}
showStatus(status.textContent);

function offerLogin(error) {
  if (typeof error !== 'string' || !error.includes('Entre na sua conta ChatGPT pelo Forge')) return false;
  loginPanel.hidden = false;
  loginStatus.textContent = '';
  return true;
}

function updateComposerHelp() {
  const help = !project
    ? 'Ao enviar, o Forge prepara o projeto e só então envia sua ideia. Você também pode preparar o projeto primeiro.'
    : unconfirmedSends.has(referenceKey())
      ? connected
        ? 'O último envio não foi confirmado. Confira as mensagens e escolha “Já conferi o envio” antes de enviar outra.'
        : 'O último envio não foi confirmado. Abra a conversa e confira o histórico antes de tentar novamente.'
    : !connected
      ? loginPanel.hidden ? 'Escreva e envie sua ideia. A conversa será aberta antes do envio.' : 'Entre no ChatGPT para enviar. Seu texto está preservado.'
      : busy
        ? 'O agente está trabalhando. Você pode preparar a próxima mensagem; o envio fica disponível quando ele terminar.'
        : broken
          ? reconnectable ? 'Use “Reabrir conversa” para continuar sem reenviar sua mensagem.' : 'Atualize o Codex antes de continuar. Sua conversa não foi apagada.'
          : 'Pronto para conversar. Você continua no controle das mudanças.';
  if (composerHelp.textContent !== help) composerHelp.textContent = help;
  const note = project && draftStorageFailed
    ? 'Não foi possível guardar este rascunho. Ele pode se perder ou reaparecer ao fechar o aplicativo.'
    : project && input.value
      ? 'Rascunho salvo neste dispositivo. Nada foi enviado.'
      : '';
  draftNote.hidden = !note;
  if (draftNote.textContent !== note) draftNote.textContent = note;
}

function updateResumeAction() {
  resumeLastConversation.hidden = !project || connected || !hasResumableConversation;
  resumeLastConversation.disabled = transitioning || !loginPanel.hidden;
  if (project) resumeLastConversation.textContent = unconfirmedSends.has(referenceKey())
    ? 'Conferir envio anterior' : 'Continuar conversa anterior';
}

function updateSendControl() {
  byId('send-label').textContent = project ? 'Enviar'
    : byId('project-root').value.trim() ? 'Enviar e abrir projeto' : 'Enviar e criar projeto';
  send.disabled = referencePending || transitioning || loginPending || !loginPanel.hidden || busy || broken ||
    !input.value.trim() || (connected && unconfirmedSends.has(referenceKey()));
}

function controls() {
  const focused = document.activeElement;
  connect.disabled = transitioning || connected || !project || !loginPanel.hidden;
  newConversation.disabled = transitioning || connected;
  disconnect.disabled = transitioning || !connected;
  updateSendControl();
  // A running turn or lost connection blocks Send, not the user's next local draft.
  input.disabled = transitioning;
  addReference.disabled = transitioning || referencePending;
  if (input.getClientRects().length) {
    input.style.height = 'auto';
    input.style.height = `${input.scrollHeight}px`;
  }
  stop.disabled = transitioning || !connected || !busy || broken;
  stop.hidden = !connected || !busy;
  startLoginButton.disabled = loginPending || loginActive || connected;
  startLoginButton.hidden = loginActive;
  finishLoginButton.disabled = loginPending || !loginActive;
  cancelLoginButton.disabled = loginPending || !loginActive;
  openLoginPageButton.disabled = loginPending || !loginActive;
  copyLoginCodeButton.disabled = loginPending || !loginActive;
  connect.hidden = connected;
  connectHelp.hidden = !project || connected;
  disconnect.hidden = !connected;
  conversationOptions.hidden = !project && messageView.hidden;
  if (conversationOptions.hidden) conversationOptions.open = false;
  reopenConversation.hidden = !project || !connected || !broken || !reconnectable;
  reopenConversation.disabled = transitioning;
  newConversationChoice.hidden = connected || !project || !hasPreviousConversation;
  conversationPicker.hidden = !project || connected;
  if (connected) conversationPicker.open = false;
  findConversations.disabled = !project || connected || transitioning || listPending || !loginPanel.hidden;
  previousConversations.disabled = !project || connected || transitioning || listPending || !loginPanel.hidden || pageNumber === 0;
  moreConversations.disabled = !project || connected || transitioning || listPending || !loginPanel.hidden || !listCursor;
  projectStatus.hidden = connected;
  projectResultHint.textContent = 'Pasta confirmada pelo Forge.';
  conversationStep.textContent = 'SUA CONVERSA';
  emptyDescription.textContent = connected
    ? 'Sua conversa está pronta. Conte o que você quer criar ou melhorar.'
    : hasResumableConversation
      ? 'Você pode ler a conversa anterior antes de continuar. Isso não envia mensagens.'
    : 'Conte o que você quer criar ou melhorar. Uma dúvida também é um bom começo.';
  updateResumeAction();
  confirmReviewedSend.hidden = !project || !connected || !unconfirmedSends.has(referenceKey());
  confirmReviewedSend.disabled = transitioning || busy || broken;
  updateComposerHelp();
  accessNote.hidden = !project;
  byId('project-root').disabled = transitioning || connected;
  byId('browse-project').disabled = transitioning || connected;
  byId('create-default-project').disabled = transitioning || connected;
  byId('inspect-project').disabled = transitioning || connected;
  byId('start-project').disabled = transitioning || connected;
  if (focused && conversationPicker.hidden && conversationPicker.contains(focused)) status.focus();
  else if (focused?.disabled) (conversationPicker.contains(focused) ? conversationListStatus : status).focus();
}

export function setProject(value) {
  byId('preview-local-apps').hidden = true;
  byId('preview-local-apps-list').replaceChildren();
  questions.clear();
  referenceRequest++;
  referenceStatus.hidden = true;
  referenceStatus.textContent = '';
  if (!value) setConversationFocus(false);
  reconnectable = false;
  const previous = project ?? draftOrigin;
  if (previous) {
    const previousKey = projectKey(previous);
    const leavingProject = value
      ? projectKey(value) !== previousKey
      : byId('project-root').value.trim() !== previous.project_root;
    if (leavingProject) {
      if (input.value) projectDrafts.set(previousKey, input.value);
      else projectDrafts.delete(previousKey);
      storeDraft(previous, input.value);
      input.value = '';
      draftOrigin = null;
    }
  }
  project = value;
  byId('conversation-focus').hidden = !project;
  conversationOptions.hidden = !project && messageView.hidden;
  if (!connected) activeThreadId = null;
  if (project) {
    draftOrigin = project;
    if (!input.value) {
      const key = projectKey(project);
      if (projectDrafts.has(key)) input.value = projectDrafts.get(key);
      else {
        try {
          input.value = readStoredDraft(project) ?? '';
          draftStorageFailed = false;
        } catch { draftStorageFailed = true; }
      }
    }
    if (input.value) {
      projectDrafts.set(projectKey(project), input.value);
      storeDraft(project, input.value);
    }
  }
  lastResultPath = null;
  previewLastResult.hidden = true;
  previewIntro.textContent = emptyPreviewText;
  citedFiles.hidden = true;
  citedFiles.open = false;
  citedFilesList.replaceChildren();
  citedFilesMore.hidden = true;
  if (!project) {
    updateAction.hidden = true;
    messages.replaceChildren(); items.clear(); latestItem = null;
    jumpLatest.hidden = true;
    rawMessageView = false;
    messageView.hidden = true;
    messageView.textContent = 'Ver texto original';
    conversationOptions.hidden = true;
    conversationOptions.open = false;
    if (!loginActive) loginPanel.hidden = true;
  }
  conversationStep.textContent = 'SUA CONVERSA';
  hasPreviousConversation = false;
  hasResumableConversation = false;
  newConversation.checked = false;
  if (project) {
    try {
      hasResumableConversation = !!(sessionReferences.get(referenceKey()) ?? readReference(localStorage, project));
      hasPreviousConversation = hasResumableConversation;
    }
    catch { hasPreviousConversation = true; } // Keep the explicit new-conversation escape hatch.
    try {
      const uncertainThread = readUnconfirmedSend(localStorage, project);
      if (uncertainThread) unconfirmedSends.set(referenceKey(), uncertainThread);
    } catch { if (!unconfirmedSends.has(referenceKey())) unconfirmedSends.set(referenceKey(), null); } // Unreadable marker fails closed.
    if (unconfirmedSends.has(referenceKey())) hasPreviousConversation = true;
    if (unconfirmedSends.get(referenceKey())) hasResumableConversation = true;
  }
  listGeneration++;
  listPending = false;
  listCursor = null;
  pageNumber = 0;
  pageCursors = [null];
  conversationList.replaceChildren();
  conversationListStatus.textContent = 'A busca começa quando você pedir.';
  conversationPicker.hidden = !project;
  conversationPicker.open = false;
  moreConversations.hidden = true;
  previousConversations.hidden = true;
  connect.disabled = !project || connected;
  connectHelp.hidden = !project || connected;
  updateSendControl();
  findConversations.disabled = !project || connected || transitioning;
  moreConversations.disabled = true;
  previousConversations.disabled = true;
  newConversationChoice.hidden = connected || !project || !hasPreviousConversation;
  updateComposerHelp();
  emptyDescription.textContent = hasResumableConversation
    ? 'Você pode ler a conversa anterior antes de continuar. Isso não envia mensagens.'
    : 'Conte o que você quer criar ou melhorar. Uma dúvida também é um bom começo.';
  updateResumeAction();
  accessNote.hidden = !project;
  if (!connected && !transitioning) showStatus(project
    ? 'Projeto pronto. Escreva sua ideia; a conversa abre quando você enviar.'
    : 'Comece um projeto para conversar. Sua ideia só será enviada quando você clicar em Enviar.');
}

function validConversationPage(page) {
  return page && Array.isArray(page.conversations) && page.conversations.length <= 50
    && (page.next_cursor === null || (typeof page.next_cursor === 'string' && page.next_cursor.length > 0 && page.next_cursor.length <= 1024))
    && page.conversations.every(choice => choice && typeof choice.id === 'string' && choice.id.length > 0 && choice.id.length <= 200
      && typeof choice.title === 'string' && choice.title.length > 0 && [...choice.title].length <= 160
      && Number.isSafeInteger(choice.updated_at) && choice.updated_at >= 0
      && typeof choice.active === 'boolean');
}

async function loadConversationChoices(targetPage = 0, cursor = null) {
  if (!project || connected || transitioning || listPending || !loginPanel.hidden || targetPage < 0 || (targetPage > 0 && !cursor)) return;
  const current = ++listGeneration;
  const root = project.project_root;
  listPending = true;
  conversationListStatus.textContent = 'Buscando conversas no Codex…';
  controls();
  try {
    const page = await invoke('list_conversations', { projectRoot: root, cursor });
    if (current !== listGeneration) return;
    if (!validConversationPage(page)) throw new Error('Invalid conversation list');
    if (page.next_cursor && (page.next_cursor === cursor || pageCursors.slice(0, targetPage + 1).includes(page.next_cursor))) throw new Error('Repeated conversation cursor');
    const rows = document.createDocumentFragment();
    const listedIds = new Set();
    for (const choice of page.conversations) {
      if (listedIds.has(choice.id)) continue;
      listedIds.add(choice.id);
      const row = document.createElement('div');
      row.className = 'conversation-choice';
      const description = document.createElement('div');
      description.className = 'conversation-choice-description';
      const title = document.createElement('strong');
      title.textContent = choice.title;
      title.title = choice.title;
      const date = document.createElement('span');
      date.textContent = `Atualizada em ${new Date(choice.updated_at * 1000).toLocaleDateString('pt-BR')}`;
      description.append(title, date);
      const open = document.createElement('button');
      open.type = 'button';
      open.textContent = choice.active ? 'Em andamento' : 'Retomar';
      open.disabled = choice.active;
      open.setAttribute('aria-label', `${choice.active ? 'Em andamento' : 'Retomar'}: ${choice.title}`);
      open.addEventListener('click', async () => {
        if (!project || connected || transitioning || listPending) return;
        newConversation.checked = false;
        conversationPicker.open = false;
        status.focus();
        await connectCurrent(choice.id);
      });
      row.append(description, open);
      rows.append(row);
    }
    conversationList.replaceChildren(rows);
    pageNumber = targetPage;
    pageCursors = pageCursors.slice(0, targetPage + 1);
    pageCursors[targetPage] = cursor;
    listCursor = page.next_cursor;
    previousConversations.hidden = pageNumber === 0;
    moreConversations.hidden = !listCursor;
    conversationListStatus.textContent = listedIds.size
      ? `Página ${pageNumber + 1}: ${listedIds.size} ${listedIds.size === 1 ? 'conversa' : 'conversas'} do índice deste dispositivo para este projeto.`
      : listCursor ? `Página ${pageNumber + 1}: nenhuma conversa nesta página. Você pode avançar.` : 'Nenhuma conversa encontrada no índice deste dispositivo.';
  } catch (error) {
    if (current === listGeneration) {
      const needsLogin = offerLogin(error);
      conversationListStatus.textContent = needsLogin
        ? 'Entre no ChatGPT para ver suas conversas. A lista anterior foi mantida.'
        : typeof error === 'string' ? error : 'Não foi possível buscar as conversas. Tente novamente; a página anterior foi mantida.';
    }
  } finally {
    if (current === listGeneration) { listPending = false; controls(); }
  }
}

findConversations.addEventListener('click', () => loadConversationChoices());
previousConversations.addEventListener('click', () => loadConversationChoices(pageNumber - 1, pageCursors[pageNumber - 1]));
moreConversations.addEventListener('click', () => loadConversationChoices(pageNumber + 1, listCursor));

const confirmation = byId('action-confirmation');
const confirmationTitle = byId('action-confirmation-title');
const confirmationCopy = byId('action-confirmation-copy');
const confirmationAddress = byId('action-confirmation-address');
const confirmationCancel = byId('action-confirmation-cancel');
const confirmationAccept = byId('action-confirmation-accept');
let resolveConfirmation = null;
function finishConfirmation(accepted) {
  if (!resolveConfirmation) return;
  const resolve = resolveConfirmation;
  resolveConfirmation = null;
  if (confirmation.open) confirmation.close();
  resolve(accepted);
}
confirmationCancel.addEventListener('click', () => finishConfirmation(false));
confirmationAccept.addEventListener('click', () => finishConfirmation(true));
confirmation.addEventListener('close', () => finishConfirmation(false));
function confirmAction({ title, copy, address = '', accept, cancel = 'Cancelar' }) {
  if (resolveConfirmation) return Promise.resolve(false);
  confirmationTitle.textContent = title;
  confirmationCopy.textContent = copy;
  confirmationAddress.textContent = address;
  confirmationAddress.hidden = !address;
  confirmation.setAttribute('aria-describedby', address ? 'action-confirmation-copy action-confirmation-address' : 'action-confirmation-copy');
  confirmationAccept.textContent = accept;
  confirmationCancel.textContent = cancel;
  return new Promise(resolve => {
    resolveConfirmation = resolve;
    confirmation.showModal();
    confirmationCancel.focus();
  });
}

async function openExternalUrl(url, button) {
  const root = project?.project_root;
  const local = localAppLinks(url).length > 0;
  if (!await confirmAction({ title: local ? 'Experimentar app no navegador?' : 'Abrir site no navegador?', copy: local ? 'Este endereço foi citado pelo agente. O app precisa estar rodando e pode executar código no navegador. Nada será publicado ou ligado automaticamente.' : 'Este endereço foi citado na resposta do Codex. Confira antes de sair do Forge.', address: url, accept: 'Abrir no navegador' })) return;
  if (project?.project_root !== root) return;
  let feedback = button.nextElementSibling;
  if (!feedback?.classList.contains('message-web-status')) {
    feedback = document.createElement('span');
    feedback.className = 'message-web-status';
    feedback.setAttribute('role', 'status');
    button.after(feedback);
  }
  button.disabled = true;
  feedback.textContent = 'Solicitando abertura…';
  try {
    await invoke('open_external_link', { url });
    feedback.textContent = 'Abertura solicitada ao navegador.';
  } catch {
    feedback.textContent = 'Não foi possível abrir. Confira o navegador padrão.';
  } finally {
    button.disabled = false;
  }
}

function paintMessage(item) {
  if (item.isUser || !item.complete) {
    item.previewPaths = [];
    if (item.fileAction) item.fileAction.hidden = true;
    if (item.fileChoices) item.fileChoices.hidden = true;
    item.content.textContent = item.raw;
    return;
  }
  const formatted = document.createElement('div');
  renderAgentMessage(formatted, item.raw, previewLinkedFile, openExternalUrl);
  item.previewPaths = [...new Set([...formatted.querySelectorAll('.message-file-link')].map(button => button.dataset.previewPath))];
  const distinctPaths = [...new Map(item.previewPaths.map(path => [path.replace(/\\/g, '/').toLowerCase(), path])).values()];
  if (distinctPaths.length === 1) {
    if (!item.fileAction) {
      item.fileAction = document.createElement('button');
      item.fileAction.type = 'button';
      item.fileAction.className = 'message-result-action';
      item.fileAction.addEventListener('click', () => { void previewLinkedFile(item.fileAction.dataset.previewPath); });
      item.content.after(item.fileAction);
    }
    item.fileAction.dataset.previewPath = distinctPaths[0];
    item.fileAction.textContent = 'Conferir arquivo citado →';
    item.fileAction.setAttribute('aria-label', `Conferir arquivo citado na resposta: ${distinctPaths[0]}`);
    item.fileAction.hidden = false;
  } else if (item.fileAction) item.fileAction.hidden = true;
  if (distinctPaths.length > 1) {
    if (!item.fileChoices) {
      const choices = document.createElement('details');
      choices.className = 'message-file-choices';
      const summary = document.createElement('summary');
      const list = document.createElement('div');
      list.className = 'message-file-choices-list';
      const more = document.createElement('p');
      more.className = 'hint';
      more.textContent = 'Outros arquivos continuam disponíveis no texto da resposta.';
      choices.append(summary, list, more);
      item.content.after(choices);
      item.fileChoices = choices;
      // A short, newly completed list should be discoverable without another
      // click. Restored history and long lists stay folded; later repaints keep
      // the person's own open/closed choice.
      choices.open = !restoringHistory && distinctPaths.length <= 3;
    }
    item.fileChoices.hidden = false;
    item.fileChoices.querySelector('summary').textContent = `Conferir ${distinctPaths.length} arquivos da resposta`;
    item.fileChoices.querySelector('.message-file-choices-list').replaceChildren(...distinctPaths.slice(0, 20).map(path => {
      const button = document.createElement('button');
      button.type = 'button';
      button.textContent = path;
      button.setAttribute('aria-label', `Conferir arquivo da resposta: ${path}`);
      button.addEventListener('click', () => { void previewLinkedFile(path); });
      return button;
    }));
    item.fileChoices.querySelector('.hint').hidden = distinctPaths.length <= 20;
  } else if (item.fileChoices) item.fileChoices.hidden = true;
  if (rawMessageView) {
    const raw = document.createElement('pre');
    raw.className = 'message-raw';
    raw.textContent = item.raw;
    item.content.replaceChildren(raw);
  } else item.content.replaceChildren(...formatted.childNodes);
}

function updateLastResultAction() {
  const localSection = byId('preview-local-apps');
  const localList = byId('preview-local-apps-list');
  const urls = project ? [...new Set([...items.values()].reverse().filter(item => !item.isUser && item.complete).flatMap(item => localAppLinks(item.raw)))].slice(0, 5) : [];
  localSection.hidden = !urls.length;
  localList.replaceChildren(...urls.map(url => {
    const card = document.createElement('div'); card.className = 'local-app-card';
    const address = document.createElement('strong'); address.textContent = url;
    const button = document.createElement('button'); button.type = 'button'; button.className = 'primary'; button.textContent = 'Experimentar ↗';
    button.setAttribute('aria-label', `Experimentar app local: ${url}`);
    button.addEventListener('click', () => { void openExternalUrl(url, button); });
    const change = document.createElement('button'); change.type = 'button'; change.textContent = 'Pedir mudança';
    change.setAttribute('aria-label', `Pedir mudança no app local: ${url}`);
    change.addEventListener('click', () => {
      if (!project || input.disabled) return;
      const request = `Quero mudar o resultado em ${url}: `;
      input.value = input.value.trim() ? `${input.value.trimEnd()}\n${request}` : request;
      input.dispatchEvent(new Event('input', { bubbles: true }));
      showWorkspacePane('conversation'); input.focus();
      showStatus('Pedido preparado. Escreva a mudança e envie quando quiser.', busy ? 'working' : 'ready');
    });
    const actions = document.createElement('div'); actions.className = 'local-app-actions'; actions.append(button, change);
    card.append(address, actions); return card;
  }));
  const previousResult = project && latestItem
    ? [...items.values()].reverse().find(item => !item.isUser && item.complete && item.previewPaths?.length)
    : null;
  const paths = previousResult
    ? [...new Map(previousResult.previewPaths.map(path => [path.replace(/\\/g, '/').toLowerCase(), path])).values()]
    : [];
  const citedEarlier = !!previousResult && previousResult !== latestItem;
  lastResultPath = paths.length === 1 ? paths[0] : null;
  previewLastResult.hidden = !lastResultPath;
  previewIntro.textContent = lastResultPath
    ? citedEarlier ? 'Um resultado anterior tem um arquivo. Confira-o aqui antes de pedir mudanças.' : 'A resposta cita um arquivo. Confira-o aqui antes de pedir mudanças.'
    : emptyPreviewText;
  citedFiles.hidden = paths.length < 2;
  citedFilesSummary.textContent = `${paths.length} arquivos citados na resposta`;
  citedFilesMore.hidden = paths.length <= 20;
  citedFilesList.replaceChildren(...paths.slice(0, 20).map(path => {
    const button = document.createElement('button');
    button.type = 'button';
    button.textContent = path;
    button.setAttribute('aria-label', `Conferir arquivo citado: ${path}`);
    button.addEventListener('click', () => { void previewLinkedFile(path); });
    return button;
  }));
  if (paths.length < 2) citedFiles.open = false;
}

previewLastResult.addEventListener('click', () => {
  if (lastResultPath) void previewLinkedFile(lastResultPath);
});

function nearLatestMessage() {
  return conversationBody.scrollHeight - conversationBody.clientHeight - conversationBody.scrollTop <= 72;
}

function updateLatestAction() { jumpLatest.hidden = !messages.childElementCount || nearLatestMessage(); }
function showLatestMessage() { conversationBody.scrollTop = conversationBody.scrollHeight; updateLatestAction(); }
conversationBody.addEventListener('scroll', updateLatestAction);
jumpLatest.addEventListener('click', () => {
  showLatestMessage();
  conversationBody.focus({ preventScroll: true });
});

messageView.addEventListener('click', () => {
  const followLatest = nearLatestMessage();
  rawMessageView = !rawMessageView;
  messageView.textContent = rawMessageView ? 'Ver texto formatado' : 'Ver texto original';
  for (const item of items.values()) paintMessage(item);
  updateLastResultAction();
  if (followLatest) showLatestMessage();
});

function message(id, role, text, append = false, complete = false) {
  const followLatest = !restoringHistory && (role === 'Você' || !messages.childElementCount || nearLatestMessage());
  let item = items.get(id);
  const created = !item;
  if (!item) {
    const article = document.createElement('article');
    const isUser = role === 'Você';
    article.dataset.role = isUser ? 'user' : 'agent';
    const avatar = document.createElement('span');
    avatar.className = 'message-avatar';
    avatar.setAttribute('aria-hidden', 'true');
    if (isUser) avatar.textContent = 'V';
    else {
      const logo = document.createElement('img');
      logo.src = 'assets/forge.png';
      logo.alt = '';
      avatar.append(logo);
    }
    const bubble = document.createElement('div');
    bubble.className = 'message-bubble';
    const title = document.createElement('strong');
    title.textContent = role;
    const content = document.createElement('div');
    content.className = 'message-content';
    item = { article, title, content, raw: '', isUser, complete: false };
    bubble.append(title, content);
    article.append(avatar, bubble); messages.append(article); items.set(id, item);
    latestItem = item;
  }
  item.raw = append ? item.raw + text : text;
  item.complete = complete;
  paintMessage(item);
  if (!restoringHistory && (created || complete)) updateLastResultAction();
  if (complete && !item.isUser) {
    messageView.hidden = false;
    conversationOptions.hidden = false;
  }
  if (followLatest) showLatestMessage();
  else updateLatestAction();
  return item;
}

function receive(event) {
  if (['completed', 'interrupted', 'failed', 'disconnected', 'update_required'].includes(event.kind)) questions.clear();
  if (['questions', 'questions_resolved'].includes(event.kind)) {
    questions.receive(event);
    if (event.kind === 'questions') showStatus('O Codex tem uma pergunta para você. Responda abaixo para continuar.', 'working');
    return;
  }
  if (event.kind === 'update_required') updateAction.hidden = false;
  else if (['running', 'completed', 'interrupted', 'failed'].includes(event.kind)) updateAction.hidden = true;
  if (['running', 'completed', 'interrupted', 'failed', 'disconnected', 'update_required'].includes(event.kind)) invalidateProgress();
  if (['completed', 'interrupted', 'failed', 'disconnected', 'update_required'].includes(event.kind)) refreshProgressAfterTurn();
  if (['completed', 'interrupted', 'failed', 'disconnected', 'update_required'].includes(event.kind)) void refreshPreviewAfterTurn();
  if (event.kind === 'delta' || event.kind === 'message') {
    message(event.id, 'Codex', event.text, event.kind === 'delta', event.kind === 'message');
    return;
  }
  const labels = {
    running: 'O Codex começou. Você pode preparar a próxima mensagem enquanto ele trabalha.',
    activity: ({
      checking: 'O Codex está conferindo o projeto…',
      editing: 'O Codex está alterando arquivos…',
      replying: 'O Codex está preparando a resposta…',
    })[event.text] || 'O Codex está trabalhando…',
    completed: 'Resposta recebida. Confira o resultado e as mudanças feitas.',
    interrupted: 'Interrompido. O que já foi feito não foi desfeito.',
    failed: 'A execução falhou. Confira o que já foi feito antes de tentar novamente.',
    update_required: 'O Codex usado pelo Forge não aceita o modelo solicitado. Confira se há uma versão mais nova do Forge Desktop em Início → Como funciona → Versão e atualizações. Se você configurou um Codex externo, atualize-o também. A conversa não foi apagada.',
    disconnected: 'A conexão foi encerrada. Reabra a conversa para conferir o histórico; nada será reenviado.',
    interaction_required: 'O Codex pediu uma interação que esta tela ainda não oferece. Nada foi aprovado automaticamente.',
  };
  if (labels[event.kind] && !transitioning) {
    const kind = { running: 'working', activity: 'working', completed: 'completed', interrupted: 'interrupted', failed: 'error', update_required: 'error', disconnected: 'disconnected', interaction_required: 'error' }[event.kind];
    showStatus(labels[event.kind], kind);
  }
  if (event.kind === 'running') busy = true;
  if (['completed', 'interrupted', 'failed', 'disconnected', 'update_required'].includes(event.kind)) busy = false;
  // Keep disconnect available after a connection failure to release the native session.
  if (['disconnected', 'update_required'].includes(event.kind)) broken = true;
  if (event.kind === 'disconnected') reconnectable = true;
  if (event.kind === 'update_required') reconnectable = false;
  controls();
}

async function connectCurrent(explicitThreadId = null) {
  if (!project || connected || transitioning || !loginPanel.hidden) return false;
  updateAction.hidden = true;
  const reviewingSend = unconfirmedSends.has(referenceKey());
  if (reviewingSend && newConversation.checked) {
    showStatus('Retome a conversa anterior e confira o envio não confirmado antes de começar outra.', 'error');
    return false;
  }
  ++listGeneration;
  listPending = false;
  const current = ++generation;
  transitioning = true; broken = false; reconnectable = false; controls();
  showStatus('Conectando ao Codex com seu login…', 'working');
  try {
    let threadId = explicitThreadId;
    if (!threadId && !newConversation.checked) {
      try { threadId = unconfirmedSends.get(referenceKey()) || sessionReferences.get(referenceKey()) || readReference(localStorage, project); }
      catch { throw 'Não foi possível consultar a conversa salva neste dispositivo. Para seguir sem retomá-la, marque “Começar outra conversa”.'; }
    }
    if (reviewingSend) {
      const expected = unconfirmedSends.get(referenceKey());
      if (!expected || threadId !== expected) throw 'Retome a conversa do envio não confirmado antes de reenviar. Se ela não estiver disponível, confira o histórico pelo Codex.';
    }
    channel = new globalThis.__TAURI__.core.Channel();
    channel.onmessage = event => { if (current === generation) receive(event); };
    const conversation = await invoke('connect_agent', { projectRoot: project.project_root, threadId, events: channel });
    if (current !== generation) return;
    if (reviewingSend && !conversation.resumed) {
      try { await invoke('disconnect_agent'); } catch { /* Keep the send guard even if cleanup fails. */ }
      throw 'O Codex não confirmou a retomada da conversa anterior. Confira o histórico antes de reenviar.';
    }
    connected = true; busy = false;
    loginPanel.hidden = true;
    conversationPicker.open = false;
    messages.replaceChildren(); items.clear(); latestItem = null; rawMessageView = false;
    jumpLatest.hidden = true;
    updateLastResultAction();
    messageView.hidden = true; messageView.textContent = 'Ver texto original';
    restoringHistory = true;
    try {
      for (const item of conversation.messages) message(item.id, item.role === 'user' ? 'Você' : item.incomplete ? 'Codex · resposta incompleta' : 'Codex', item.text, false, item.role === 'agent' && !item.incomplete);
    } finally { restoringHistory = false; updateLastResultAction(); }
    activeThreadId = conversation.thread_id;
    // Codex may not persist a thread until it receives a turn. Do not replace
    // a real bookmark with a newly opened, empty conversation.
    const durableHistory = conversation.resumed || conversation.messages.length > 0;
    if (durableHistory) {
      sessionReferences.set(referenceKey(), conversation.thread_id);
      hasPreviousConversation = true;
      hasResumableConversation = true;
    }
    let saved = true;
    if (durableHistory) {
      try { saveReference(localStorage, project, conversation.thread_id); } catch { saved = false; }
    }
    newConversation.checked = false;
    const latestReplyIncomplete = conversation.messages.at(-1)?.role === 'agent' && conversation.messages.at(-1).incomplete;
    showStatus(broken
      ? reconnectable
        ? 'A conexão foi encerrada. Use “Reabrir conversa” para conferir o histórico; nada será reenviado.'
        : 'O Codex precisa ser atualizado antes de continuar. Sua conversa não foi apagada.'
      : `${reviewingSend ? 'Conversa retomada. O envio anterior ainda não foi confirmado. Confira as mensagens e o que foi feito; depois escolha “Já conferi o envio” para continuar. Nada foi reenviado.' : latestReplyIncomplete ? 'Conversa retomada. Última resposta incompleta: confira mensagens e arquivos antes de continuar. Mudanças podem permanecer; nada foi reenviado.' : conversation.resumed ? 'Conversa retomada.' : `Codex conectado ao projeto ${projectDisplayName(project)}. Pode mandar sua ideia.`}${saved ? '' : ' Não foi possível salvar o acesso à conversa. Enquanto este app estiver aberto, você pode reconectar; depois de fechá-lo, pode aparecer a conversa anterior.'}`, broken ? 'disconnected' : 'connected');
  } catch (error) {
    if (current !== generation) return;
    activeThreadId = null;
    ++generation;
    const needsLogin = offerLogin(error);
    showStatus(needsLogin ? 'Entre no ChatGPT para continuar. Sua mensagem não foi enviada.'
      : typeof error === 'string' ? error : 'Não foi possível conectar ao Codex.', 'error');
  }
  transitioning = false;
  controls();
  if (connected && !broken) showLatestMessage();
  if (connected && !broken && !byId('workspace').hidden) {
    requestAnimationFrame(() => {
      if (current === generation && connected && !byId('workspace').hidden) send.scrollIntoView({ block: 'nearest' });
    });
  }
  return connected && !broken;
}

export function resumeSavedConversation() {
  if (!project || !hasResumableConversation || unconfirmedSends.has(referenceKey()) || connected || transitioning) return Promise.resolve(false);
  return connectCurrent();
}

export function cancelPendingFirstSend() { pendingFirstSend = null; }

export function finishPendingFirstSend(openedProject) {
  const pending = pendingFirstSend;
  pendingFirstSend = null;
  if (!pending || !project || projectKey(project) !== projectKey(openedProject)
    || byId('workspace').hidden || input.value !== pending.text
    || (pending.path && pending.path.toLowerCase() !== project.project_root.toLowerCase())
    || (hasResumableConversation && !connected)) return false;
  // A second submit during the original form's submit dispatch is ignored by
  // Chromium. Resume on the next task, and recheck that the draft still owns it.
  setTimeout(() => {
    if (project && projectKey(project) === projectKey(openedProject)
      && !byId('workspace').hidden && input.value === pending.text) byId('message-form').requestSubmit();
  }, 0);
  return true;
}

addEventListener('hashchange', () => {
  if (location.hash !== '#workspace') cancelPendingFirstSend();
});

connect.addEventListener('click', () => connectCurrent());
async function finishLogin() {
  if (!loginActive || loginPending) return;
  loginPending = true; controls();
  loginStatus.textContent = 'Conferindo o acesso…';
  try {
    if (await invoke('finish_login')) {
      loginActive = false;
      loginChallenge.hidden = true;
      loginPanel.hidden = true;
      loginStatus.textContent = '';
      showStatus('Conta conectada. Sua ideia está pronta para enviar.', 'completed');
      input.focus();
    } else loginStatus.textContent = 'Aguardando a confirmação no navegador. Depois, escolha “Já entrei · verificar”.';
  } catch (error) {
    loginStatus.textContent = typeof error === 'string' ? error : 'Não foi possível conferir o acesso. Tente novamente.';
  } finally { loginPending = false; controls(); }
}

startLoginButton.addEventListener('click', async () => {
  if (loginActive || loginPending || connected) return;
  loginPending = true; loginCompletedEarly = false; controls();
  copyLoginStatus.hidden = true;
  copyLoginStatus.textContent = '';
  loginStatus.textContent = 'Preparando o acesso seguro pelo Codex…';
  try {
    const loginEvents = new globalThis.__TAURI__.core.Channel();
    loginEvents.onmessage = event => {
      if (!event || typeof event.success !== 'boolean') return;
      if (event.success) {
        if (loginPending) loginCompletedEarly = true;
        else void finishLogin();
      } else loginStatus.textContent = 'O acesso não foi concluído. Cancele e tente novamente.';
    };
    const challenge = await invoke('start_login', { events: loginEvents });
    if (typeof challenge.user_code !== 'string' || typeof challenge.verification_url !== 'string') throw new Error('Resposta de acesso incompatível.');
    loginActive = true;
    loginCode.textContent = challenge.user_code;
    loginUrl.textContent = challenge.verification_url;
    loginChallenge.hidden = false;
    loginStatus.textContent = 'O Forge nunca pede sua senha. Quando terminar no navegador, volte aqui para continuar.';
  } catch (error) {
    loginStatus.textContent = typeof error === 'string' ? error : 'Não foi possível iniciar o acesso. Tente novamente.';
  } finally {
    loginPending = false; controls();
    if (loginCompletedEarly && loginActive) void finishLogin();
  }
});

finishLoginButton.addEventListener('click', () => void finishLogin());
copyLoginCodeButton.addEventListener('click', async () => {
  if (!loginActive || loginPending || !loginCode.textContent) return;
  try {
    await navigator.clipboard.writeText(loginCode.textContent);
    copyLoginStatus.textContent = 'Código copiado. Cole-o na página de acesso.';
  } catch {
    copyLoginStatus.textContent = 'Não foi possível copiar. Selecione o código acima e copie manualmente.';
  }
  copyLoginStatus.hidden = false;
});
cancelLoginButton.addEventListener('click', async () => {
  if (!loginActive || loginPending) return;
  loginPending = true; controls();
  try {
    await invoke('cancel_login');
    loginActive = false;
    loginChallenge.hidden = true;
    loginCode.textContent = '';
    copyLoginStatus.hidden = true;
    copyLoginStatus.textContent = '';
    loginStatus.textContent = 'Acesso cancelado. Seu texto continua aqui.';
  } catch (error) {
    loginStatus.textContent = typeof error === 'string' ? error : 'Não foi possível cancelar. Tente novamente.';
  } finally { loginPending = false; controls(); }
});
openLoginPageButton.addEventListener('click', async () => {
  if (!loginActive || loginPending) return;
  try { await invoke('open_login_page'); }
  catch (error) { loginStatus.textContent = typeof error === 'string' ? error : 'Abra o endereço mostrado em seu navegador.'; }
});
resumeLastConversation.addEventListener('click', () => {
  if (!project || connected || transitioning || !hasResumableConversation) return;
  newConversation.checked = false;
  void connectCurrent();
});
reopenConversation.addEventListener('click', async () => {
  if (!project || !connected || !broken || !reconnectable || transitioning) return;
  if (await disconnectCurrent()) void connectCurrent();
});

confirmReviewedSend.addEventListener('click', () => {
  if (!project || !connected || busy || transitioning || broken || !unconfirmedSends.has(referenceKey())) return;
  try { clearUnconfirmedSend(localStorage, project); }
  catch {
    showStatus('Não foi possível salvar sua revisão neste dispositivo. O envio continua protegido; tente novamente.', 'error');
    return;
  }
  unconfirmedSends.delete(referenceKey());
  showStatus('Revisão concluída neste dispositivo. Nada foi reenviado. Você pode escrever a próxima mensagem.', 'connected');
  status.focus(); // The review button is about to become hidden.
  controls();
});

byId('message-form').addEventListener('submit', async event => {
  event.preventDefault();
  if (!input.value.trim() || referencePending || busy || transitioning || broken) return;
  if (!project) {
    pendingFirstSend = { text: input.value, path: byId('project-root').value.trim() };
    const setup = byId('project-setup');
    setup.open = true;
    setup.scrollIntoView({ block: 'start' });
    if (byId('project-root').value.trim()) {
      byId('project-form').requestSubmit(byId('start-project'));
    } else byId('create-default-project').click();
    return;
  }
  if (unconfirmedSends.has(referenceKey())) {
    showStatus('O envio anterior não foi confirmado. Abra a conversa sem enviar e confira o histórico antes de tentar novamente.', 'error');
    if (!connected) { conversationPicker.open = true; connect.focus(); }
    return;
  }
  const text = input.value;
  if (new TextEncoder().encode(text).length > 64000) {
    showStatus('A mensagem está muito longa. Divida em partes menores; sua conexão continua ativa.', 'error');
    return;
  }
  if (!connected && !await connectCurrent()) return;
  const current = generation;
  const currentThread = activeThreadId;
  if (!currentThread) {
    showStatus('Não foi possível identificar a conversa atual. Abra o histórico antes de enviar.', 'error');
    return;
  }
  try { markUnconfirmedSend(localStorage, project, currentThread); }
  catch {
    showStatus('Não foi possível guardar a proteção deste envio. Sua mensagem não foi enviada; verifique o armazenamento do aplicativo antes de tentar novamente.', 'error');
    return;
  }
  input.value = '';
  projectDrafts.delete(referenceKey());
  referenceStatus.hidden = true;
  referenceStatus.textContent = '';
  busy = true; controls(); stop.disabled = true;
  showStatus('Enviando sua mensagem…', 'working');
  const id = `user-${Date.now()}`;
  const local = message(id, 'Você', text);
  local.article.dataset.delivery = 'pending';
  local.title.textContent = 'Você · enviando';
  unconfirmedSends.set(referenceKey(), currentThread);
  try {
    await invoke('send_message', { text });
    if (current !== generation) return;
    local.article.dataset.delivery = 'accepted';
    local.title.textContent = 'Você';
    sessionReferences.set(referenceKey(), currentThread);
    hasPreviousConversation = true;
    hasResumableConversation = true;
    try { saveReference(localStorage, project, currentThread); }
    catch {
      showStatus('Mensagem enviada, mas não foi possível guardar a conversa neste dispositivo. Confira o histórico antes de tentar outra mensagem.', 'error');
      controls();
      return;
    }
    if (!storeDraft(project, '')) {
      showStatus('Envio confirmado, mas não foi possível remover o rascunho salvo. Confira esta conversa antes de tentar enviar novamente.', 'error');
      controls();
      return;
    }
    try {
      clearUnconfirmedSend(localStorage, project);
      unconfirmedSends.delete(referenceKey());
    } catch { showStatus('Envio confirmado, mas não foi possível atualizar o aviso local. Confira esta conversa antes de tentar enviar novamente.', 'error'); }
    controls();
  } catch (error) {
    if (current !== generation) return;
    local.article.dataset.delivery = 'unconfirmed';
    local.title.textContent = 'Você · envio não confirmado';
    transitioning = true; controls();
    try { await invoke('disconnect_agent'); } catch { broken = true; }
    if (current !== generation) return;
    busy = false; connected = broken; transitioning = false; ++generation;
    if (!input.value) {
      input.value = text;
      projectDrafts.set(referenceKey(), text);
      storeDraft(project, text);
    }
    showStatus(`${typeof error === 'string' ? error : 'Falha ao enviar.'} Abra a conversa e confira o histórico antes de reenviar.`, 'error');
    controls();
  }
});

input.addEventListener('input', () => {
  const owner = project ?? draftOrigin;
  if (owner) {
    if (input.value) projectDrafts.set(projectKey(owner), input.value);
    else projectDrafts.delete(projectKey(owner));
    storeDraft(owner, input.value);
  }
  controls();
});

addReference.addEventListener('click', async () => {
  if (addReference.disabled) return;
  const owner = generation;
  const selection = ++referenceRequest;
  referencePending = true;
  referenceStatus.hidden = false;
  referenceStatus.textContent = 'Escolha uma imagem, um texto ou outro arquivo. Nada será enviado agora.';
  controls();
  try {
    const file = await invoke('choose_reference_file');
    if (owner !== generation || selection !== referenceRequest || byId('workspace').hidden) return;
    if (!file) { referenceStatus.textContent = 'Seleção cancelada. Sua mensagem continua como estava.'; return; }
    const reference = `Use como referência este arquivo local, sem executá-lo: ${JSON.stringify(file)}`;
    if (input.value.includes(reference)) { referenceStatus.textContent = 'Este arquivo já está indicado na mensagem.'; return; }
    const next = `${input.value.trimEnd()}${input.value.trim() ? '\n\n' : ''}${reference}`;
    if (new TextEncoder().encode(next).length > 64000) {
      referenceStatus.textContent = 'A mensagem está muito longa. Reduza o texto antes de adicionar este arquivo.';
      return;
    }
    input.value = next;
    input.dispatchEvent(new Event('input', { bubbles: true }));
    referenceStatus.textContent = 'Arquivo indicado no rascunho. Ao enviar, o agente poderá lê-lo neste computador; o Forge não cria uma cópia.';
    input.focus();
  } catch {
    if (owner === generation && selection === referenceRequest) referenceStatus.textContent = 'Não foi possível escolher o arquivo. Sua mensagem não foi alterada.';
  } finally {
    referencePending = false;
    controls();
  }
});

input.addEventListener('keydown', event => {
  if (event.key === 'Enter' && (event.ctrlKey || event.metaKey) && !event.isComposing) {
    event.preventDefault();
    if (!send.disabled) byId('message-form').requestSubmit();
  }
});
addEventListener('resize', () => controls());
addEventListener('hashchange', () => {
  if (location.hash !== '#workspace') referenceRequest++;
  requestAnimationFrame(() => controls());
});

stop.addEventListener('click', async () => {
  const current = generation;
  const hadFocus = document.activeElement === stop;
  stop.disabled = true;
  if (hadFocus) status.focus();
  showStatus('Pedindo ao Codex para interromper…', 'working');
  try { await invoke('interrupt_agent'); }
  catch (error) { if (current !== generation || !busy) return; showStatus(typeof error === 'string' ? error : 'Não foi possível interromper.', 'error'); controls(); }
});

async function disconnectCurrent() {
  questions.clear();
  if (!connected || transitioning) return false;
  const current = ++generation;
  transitioning = true; controls();
  showStatus('Desconectando…', 'working');
  let disconnected = false;
  try {
    await invoke('disconnect_agent');
    if (current !== generation) return false;
    connected = false; busy = false; broken = false; reconnectable = false; channel = null;
    activeThreadId = null;
    conversationOptions.open = false;
    disconnected = true;
    showStatus('Desconectado. As alterações já feitas no projeto permanecem.', 'disconnected');
  } catch { if (current !== generation) return false; showStatus('Não foi possível desconectar. Tente novamente.', 'error'); }
  transitioning = false;
  controls();
  return disconnected;
}

export async function prepareProjectSwitch() {
  if (transitioning) return false;
  if (loginActive || loginPending) {
    showStatus('Cancele o acesso em andamento antes de trocar de projeto.', 'error');
    return false;
  }
  if (!connected) return !byId('project-root').disabled;
  if (busy && !await confirmAction({ title: 'Trocar de projeto?', copy: 'O Codex ainda está trabalhando. Trocar agora interrompe a resposta; mudanças já feitas podem permanecer.', accept: 'Interromper e trocar', cancel: 'Ficar neste projeto' })) return false;
  return disconnectCurrent();
}

disconnect.addEventListener('click', disconnectCurrent);
