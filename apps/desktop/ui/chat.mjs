import { invalidateProgress } from './progress.mjs';
const byId = id => document.getElementById(id);
const status = byId('agent-status');
const connect = byId('connect-agent');
const disconnect = byId('disconnect-agent');
const send = byId('send-message');
const stop = byId('interrupt-agent');
const input = byId('message-text');
const messages = byId('messages');
let project = null;
let connected = false;
let busy = false;
let transitioning = false;
let broken = false;
let generation = 0;
let channel;
const items = new Map();
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

function controls() {
  const focused = document.activeElement;
  connect.disabled = transitioning || connected || !project;
  disconnect.disabled = transitioning || !connected;
  send.disabled = transitioning || !connected || busy || broken;
  input.disabled = transitioning || !connected || busy || broken;
  stop.disabled = transitioning || !connected || !busy || broken;
  byId('project-root').disabled = transitioning || connected;
  byId('inspect-project').disabled = transitioning || connected;
  if (focused?.disabled) status.focus();
}

export function setProject(value) {
  project = value;
  connect.disabled = !project || connected;
}

function message(id, role, text, append = false) {
  let node = items.get(id);
  if (!node) {
    const article = document.createElement('article');
    article.dataset.role = role === 'Você' ? 'user' : 'agent';
    const title = document.createElement('strong');
    title.textContent = role;
    node = document.createElement('p');
    article.append(title, node); messages.append(article); items.set(id, node);
  }
  node.textContent = append ? node.textContent + text : text;
}

function receive(event) {
  if (['running', 'completed', 'interrupted', 'failed', 'disconnected'].includes(event.kind)) invalidateProgress();
  if (event.kind === 'delta' || event.kind === 'message') {
    message(event.id, 'Codex', event.text, event.kind === 'delta');
    return;
  }
  const labels = {
    running: 'O Codex está trabalhando…', activity: 'O Codex está trabalhando…',
    completed: 'Resposta recebida. Confira o resultado antes de considerar a tarefa concluída.',
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

connect.addEventListener('click', async () => {
  const current = ++generation;
  transitioning = true; broken = false; controls();
  showStatus('Conectando ao Codex com seu login…', 'working');
  try {
    channel = new globalThis.__TAURI__.core.Channel();
    channel.onmessage = event => { if (current === generation) receive(event); };
    await invoke('connect_agent', { projectRoot: project.project_root, events: channel });
    if (current !== generation) return;
    connected = true; busy = false;
    messages.replaceChildren(); items.clear();
    showStatus(broken
      ? 'A conexão foi encerrada. Desconecte antes de tentar novamente.'
      : `Codex conectado ao projeto ${project.project_id}. Pode mandar sua ideia.`, broken ? 'disconnected' : 'connected');
  } catch (error) { if (current !== generation) return; ++generation; showStatus(typeof error === 'string' ? error : 'Não foi possível conectar ao Codex.', 'error'); }
  transitioning = false;
  controls();
});

byId('message-form').addEventListener('submit', async event => {
  event.preventDefault();
  if (!connected || busy || transitioning || broken || !input.value.trim()) return;
  const text = input.value;
  if (new TextEncoder().encode(text).length > 64000) {
    showStatus('A mensagem está muito longa. Divida em partes menores; sua conexão continua ativa.', 'error');
    return;
  }
  const current = generation;
  input.value = '';
  busy = true; controls(); stop.disabled = true;
  showStatus('Enviando sua mensagem…', 'working');
  const id = `user-${Date.now()}`;
  message(id, 'Você', text);
  try {
    await invoke('send_message', { text });
    if (current !== generation) return;
    controls();
  } catch (error) {
    if (current !== generation) return;
    transitioning = true; controls();
    try { await invoke('disconnect_agent'); } catch { broken = true; }
    if (current !== generation) return;
    busy = false; connected = broken; transitioning = false; ++generation;
    if (!input.value) input.value = text;
    showStatus(`${typeof error === 'string' ? error : 'Falha ao enviar.'} Confira possíveis efeitos antes de reconectar.`, 'error');
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

disconnect.addEventListener('click', async () => {
  const current = ++generation;
  transitioning = true; controls();
  showStatus('Desconectando…', 'working');
  try {
    await invoke('disconnect_agent');
    if (current !== generation) return;
    connected = false; busy = false; broken = false; channel = null;
    showStatus('Desconectado. As alterações já feitas no projeto permanecem.', 'disconnected');
  } catch { if (current !== generation) return; showStatus('Não foi possível desconectar. Tente novamente.', 'error'); }
  transitioning = false;
  controls();
});
