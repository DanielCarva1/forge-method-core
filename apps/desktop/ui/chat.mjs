import { invalidateProgress } from './progress.mjs';
import { readReference, saveReference, readUnconfirmedSend, markUnconfirmedSend, clearUnconfirmedSend } from './conversation-reference.mjs';
import { renderAgentMessage } from './message-format.mjs';
import { previewLinkedFile } from './preview.mjs';
import { projectDisplayName } from './project-display.mjs';
const byId = id => document.getElementById(id);
const status = byId('agent-status');
const connect = byId('connect-agent');
const connectHelp = byId('connect-help');
const disconnect = byId('disconnect-agent');
const send = byId('send-message');
const stop = byId('interrupt-agent');
const input = byId('message-text');
const composerHelp = byId('composer-help');
const accessNote = byId('agent-access-note');
const messages = byId('messages');
const conversationBody = document.querySelector('.conversation-body');
const messageView = byId('message-view');
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
let project = null;
let connected = false;
let busy = false;
let transitioning = false;
let broken = false;
let generation = 0;
let channel;
const items = new Map();
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
const referenceKey = () => JSON.stringify([project.project_id, project.project_root]);
const invoke = (command, args) => globalThis.__TAURI__.core.invoke(command, args);

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

function updateComposerHelp() {
  composerHelp.textContent = !project
    ? 'Para enviar, escolha uma pasta para o projeto.'
    : !connected && unconfirmedSends.has(referenceKey())
      ? 'O último envio não foi confirmado. Abra a conversa e confira o histórico antes de tentar novamente.'
    : !connected
      ? 'Escreva e envie sua ideia. A conversa será aberta antes do envio.'
      : busy
        ? 'Seu agente está trabalhando. Você pode interromper se precisar.'
        : broken
          ? 'A conexão foi encerrada. Desconecte e tente novamente.'
          : 'Pronto para conversar. Você continua no controle das mudanças.';
}

function updateResumeAction() {
  resumeLastConversation.hidden = !project || connected || !hasResumableConversation || messages.childElementCount > 0;
  resumeLastConversation.disabled = transitioning;
  if (project) resumeLastConversation.textContent = unconfirmedSends.has(referenceKey())
    ? 'Conferir envio anterior' : 'Continuar conversa anterior';
}

function controls() {
  const focused = document.activeElement;
  connect.disabled = transitioning || connected || !project;
  newConversation.disabled = transitioning || connected;
  disconnect.disabled = transitioning || !connected;
  send.disabled = transitioning || !project || busy || broken || (connected && unconfirmedSends.has(referenceKey()));
  input.disabled = transitioning || busy || broken;
  stop.disabled = transitioning || !connected || !busy || broken;
  stop.hidden = !connected || !busy;
  connect.hidden = connected;
  connectHelp.hidden = !project || connected;
  disconnect.hidden = !connected;
  newConversationChoice.hidden = connected || !project || !hasPreviousConversation;
  conversationPicker.hidden = !project || connected;
  if (connected) conversationPicker.open = false;
  findConversations.disabled = !project || connected || transitioning || listPending;
  previousConversations.disabled = !project || connected || transitioning || listPending || pageNumber === 0;
  moreConversations.disabled = !project || connected || transitioning || listPending || !listCursor;
  projectStatus.hidden = connected;
  projectResultHint.textContent = 'Pasta confirmada pelo Forge.';
  conversationStep.textContent = project ? 'SUA CONVERSA' : 'PASSO 2 · SUA CONVERSA';
  emptyDescription.textContent = connected
    ? 'Sua conversa está pronta. Conte o que você quer criar ou melhorar.'
    : hasResumableConversation
      ? 'Você pode ler a conversa anterior antes de continuar. Isso não envia mensagens.'
    : 'Conte o que você quer criar ou melhorar. Uma dúvida também é um bom começo.';
  updateResumeAction();
  updateComposerHelp();
  accessNote.hidden = !project;
  byId('project-root').disabled = transitioning || connected;
  byId('browse-project').disabled = transitioning || connected;
  byId('inspect-project').disabled = transitioning || connected;
  byId('start-project').disabled = transitioning || connected;
  if (focused && conversationPicker.hidden && conversationPicker.contains(focused)) status.focus();
  else if (focused?.disabled) (conversationPicker.contains(focused) ? conversationListStatus : status).focus();
}

export function setProject(value) {
  project = value;
  if (!project) {
    messages.replaceChildren(); items.clear();
    rawMessageView = false;
    messageView.hidden = true;
    messageView.textContent = 'Ver texto original';
  }
  conversationStep.textContent = project ? 'SUA CONVERSA' : 'PASSO 2 · SUA CONVERSA';
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
  send.disabled = !project || transitioning || busy || broken || (connected && unconfirmedSends.has(referenceKey()));
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
    : 'Escolha uma pasta para começar. A conversa será aberta quando você enviar sua ideia.');
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
  if (!project || connected || transitioning || listPending || targetPage < 0 || (targetPage > 0 && !cursor)) return;
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
    if (current === listGeneration) conversationListStatus.textContent = typeof error === 'string' ? error : 'Não foi possível buscar as conversas. Tente novamente; a página anterior foi mantida.';
  } finally {
    if (current === listGeneration) { listPending = false; controls(); }
  }
}

findConversations.addEventListener('click', () => loadConversationChoices());
previousConversations.addEventListener('click', () => loadConversationChoices(pageNumber - 1, pageCursors[pageNumber - 1]));
moreConversations.addEventListener('click', () => loadConversationChoices(pageNumber + 1, listCursor));

function paintMessage(item) {
  if (item.isUser || !item.complete) { item.content.textContent = item.raw; return; }
  if (rawMessageView) {
    const raw = document.createElement('pre');
    raw.className = 'message-raw';
    raw.textContent = item.raw;
    item.content.replaceChildren(raw);
  } else renderAgentMessage(item.content, item.raw, previewLinkedFile);
}

function nearLatestMessage() {
  return conversationBody.scrollHeight - conversationBody.clientHeight - conversationBody.scrollTop <= 72;
}

function showLatestMessage() { conversationBody.scrollTop = conversationBody.scrollHeight; }

messageView.addEventListener('click', () => {
  const followLatest = nearLatestMessage();
  rawMessageView = !rawMessageView;
  messageView.textContent = rawMessageView ? 'Ver texto formatado' : 'Ver texto original';
  for (const item of items.values()) paintMessage(item);
  if (followLatest) showLatestMessage();
});

function message(id, role, text, append = false, complete = false) {
  const followLatest = !restoringHistory && (role === 'Você' || !messages.childElementCount || nearLatestMessage());
  let item = items.get(id);
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
  }
  item.raw = append ? item.raw + text : text;
  item.complete = complete;
  paintMessage(item);
  if (complete && !item.isUser) messageView.hidden = false;
  if (followLatest) showLatestMessage();
  return item;
}

function receive(event) {
  if (['running', 'completed', 'interrupted', 'failed', 'disconnected'].includes(event.kind)) invalidateProgress();
  if (event.kind === 'delta' || event.kind === 'message') {
    message(event.id, 'Codex', event.text, event.kind === 'delta', event.kind === 'message');
    return;
  }
  const labels = {
    running: 'O Codex está trabalhando…', activity: 'O Codex está trabalhando…',
    completed: 'Resposta recebida. Confira o resultado e as mudanças feitas.',
    interrupted: 'Interrompido. O que já foi feito não foi desfeito.',
    failed: 'A execução falhou. Confira o que já foi feito antes de tentar novamente.',
    update_required: 'Atualize o Codex CLI: a versão instalada ainda não suporta o modelo escolhido. Depois, desconecte e conecte novamente.',
    disconnected: 'A conexão foi encerrada. Confira o que já foi feito antes de reconectar.',
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
  controls();
}

async function connectCurrent(explicitThreadId = null) {
  if (!project || connected || transitioning) return false;
  const reviewingSend = unconfirmedSends.has(referenceKey());
  if (reviewingSend && newConversation.checked) {
    showStatus('Retome a conversa anterior e confira o envio não confirmado antes de começar outra.', 'error');
    return false;
  }
  ++listGeneration;
  listPending = false;
  const current = ++generation;
  transitioning = true; broken = false; controls();
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
    conversationPicker.open = false;
    messages.replaceChildren(); items.clear(); rawMessageView = false;
    messageView.hidden = true; messageView.textContent = 'Ver texto original';
    restoringHistory = true;
    try {
      for (const item of conversation.messages) message(item.id, item.role === 'user' ? 'Você' : item.incomplete ? 'Codex · resposta incompleta' : 'Codex', item.text, false, item.role === 'agent' && !item.incomplete);
    } finally { restoringHistory = false; }
    sessionReferences.set(referenceKey(), conversation.thread_id);
    hasPreviousConversation = true;
    hasResumableConversation = true;
    let saved = true;
    try { saveReference(localStorage, project, conversation.thread_id); } catch { saved = false; }
    newConversation.checked = false;
    let reviewSaved = true;
    if (reviewingSend && conversation.resumed) {
      unconfirmedSends.delete(referenceKey());
      try { clearUnconfirmedSend(localStorage, project); } catch { reviewSaved = false; }
    }
    showStatus(broken
      ? 'A conexão foi encerrada. Desconecte antes de tentar novamente.'
      : `${conversation.resumed ? 'Conversa retomada. Confira o último registro do Forge e o que já foi feito antes de continuar.' : `Codex conectado ao projeto ${projectDisplayName(project)}. Pode mandar sua ideia.`}${saved ? '' : ' Não foi possível salvar o acesso à conversa. Enquanto este app estiver aberto, você pode reconectar; depois de fechá-lo, pode aparecer a conversa anterior.'}${reviewSaved ? '' : ' A revisão do envio não pôde ser salva; ao reabrir, confira o histórico novamente.'}`, broken ? 'disconnected' : 'connected');
  } catch (error) { if (current !== generation) return; ++generation; showStatus(typeof error === 'string' ? error : 'Não foi possível conectar ao Codex.', 'error'); }
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

connect.addEventListener('click', () => connectCurrent());
resumeLastConversation.addEventListener('click', () => {
  if (!project || connected || transitioning || !hasResumableConversation) return;
  newConversation.checked = false;
  void connectCurrent();
});

byId('message-form').addEventListener('submit', async event => {
  event.preventDefault();
  if (!project || busy || transitioning || broken || !input.value.trim()) return;
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
  const currentThread = sessionReferences.get(referenceKey());
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
    unconfirmedSends.delete(referenceKey());
    try { clearUnconfirmedSend(localStorage, project); } catch {
      showStatus('Envio confirmado, mas não foi possível atualizar o aviso local. Ao reabrir, talvez você precise conferir esta conversa novamente.', 'error');
    }
    controls();
  } catch (error) {
    if (current !== generation) return;
    local.article.dataset.delivery = 'unconfirmed';
    local.title.textContent = 'Você · envio não confirmado';
    transitioning = true; controls();
    try { await invoke('disconnect_agent'); } catch { broken = true; }
    if (current !== generation) return;
    busy = false; connected = broken; transitioning = false; ++generation;
    if (!input.value) input.value = text;
    showStatus(`${typeof error === 'string' ? error : 'Falha ao enviar.'} Abra a conversa e confira o histórico antes de reenviar.`, 'error');
    controls();
  }
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
  if (!connected || transitioning) return false;
  const current = ++generation;
  transitioning = true; controls();
  showStatus('Desconectando…', 'working');
  let disconnected = false;
  try {
    await invoke('disconnect_agent');
    if (current !== generation) return false;
    connected = false; busy = false; broken = false; channel = null;
    disconnected = true;
    showStatus('Desconectado. As alterações já feitas no projeto permanecem.', 'disconnected');
  } catch { if (current !== generation) return false; showStatus('Não foi possível desconectar. Tente novamente.', 'error'); }
  transitioning = false;
  controls();
  return disconnected;
}

export async function prepareProjectSwitch() {
  if (transitioning) return false;
  if (!connected) return !byId('project-root').disabled;
  if (busy && !globalThis.confirm('O Codex ainda está trabalhando. Trocar de projeto vai interromper a resposta; mudanças já feitas podem permanecer. Quer continuar?')) return false;
  return disconnectCurrent();
}

disconnect.addEventListener('click', disconnectCurrent);
