import { showWorkspacePane } from './mobile-workspace.mjs';
const button = document.getElementById('refresh-progress');
const status = document.getElementById('progress-status');
const result = document.getElementById('progress-result');
const questionsShortcut = document.getElementById('record-questions-shortcut');
const explainPending = document.getElementById('explain-pending');
const startConversation = document.getElementById('record-start-conversation');
const recordPanel = document.getElementById('project-record');
const workspacePhase = document.getElementById('workspace-phase');
const historyPanel = document.getElementById('direction-history');
const historyButton = document.getElementById('refresh-direction-history');
const historyStatus = document.getElementById('direction-history-status');
const historyList = document.getElementById('direction-history-list');
let project;
let generation = 0;
let pending = false;
let historyPending = false;
let historyLoaded = false;
let slowNotice;
let historySlowNotice;
const labels = { absent: 'Ainda não há um próximo passo registrado para este projeto.', current: 'Acompanhamento do Forge disponível. Pode não incluir a conversa mais recente.', stale: 'O acompanhamento do Forge está desatualizado.', blocked: 'O trabalho acompanhado pelo Forge tem uma pendência.', completed: 'Esta parte do trabalho foi concluída. O projeto pode continuar.', abandoned: 'O trabalho acompanhado pelo Forge foi encerrado sem conclusão.' };
const stateLabels = { absent: 'Próximo passo ainda não registrado', current: 'Em andamento', stale: 'Acompanhamento desatualizado', blocked: 'Há uma pendência', completed: 'Esta parte foi concluída', abandoned: 'Encerrado sem concluir' };
const phases = {
  '0-route': ['Preparação', 'Entendendo como começar.'],
  '1-discovery': ['Descoberta', 'Entendendo o problema, as pessoas e os caminhos possíveis.'],
  '2-specification': ['Planejamento do produto', 'Definindo o que precisa ser entregue e como reconhecer um bom resultado.'],
  '3-plan': ['Definição da solução', 'Organizando a solução e a ordem do trabalho.'],
  '4-build-verify': ['Construção e verificação', 'Construindo e conferindo o resultado.'],
  '5-ready-operate': ['Validação e entrega', 'Validando o uso real e preparando a entrega.'],
  '6-evolve': ['Evolução', 'Escolhendo a próxima mudança para um produto já estável.'],
};
function controls() {
  button.disabled = !project || pending;
  historyButton.disabled = !project || pending || historyPending;
}
function clearSlowNotices() {
  clearTimeout(slowNotice);
  clearTimeout(historySlowNotice);
}
function resetHistory() {
  clearTimeout(historySlowNotice);
  historyLoaded = false;
  historyPending = false;
  historyPanel.open = false;
  historyList.replaceChildren();
  historyStatus.textContent = 'Abra esta seção para consultar o histórico.';
}
function showList(sectionId, listId, values) {
  const section = document.getElementById(sectionId);
  const list = document.getElementById(listId);
  list.replaceChildren(...values.map(value => {
    const item = document.createElement('li');
    item.textContent = value;
    return item;
  }));
  section.hidden = values.length === 0;
}
function showSuggestedQuestions(values) {
  const section = document.getElementById('record-suggested');
  const list = document.getElementById('record-suggestions');
  const current = generation;
  list.replaceChildren(...values.map(suggestion => {
    const question = suggestion.question;
    const item = document.createElement('li');
    const label = document.createElement('span');
    label.textContent = question;
    const note = document.createElement('p');
    note.className = 'hint';
    note.textContent = suggestion.blocking ? 'O Forge sinaliza que esta escolha pode impedir o próximo avanço.' : 'Esta escolha pode ser discutida sem interromper o trabalho agora.';
    const options = document.createElement('ul');
    options.className = 'suggested-options';
    options.replaceChildren(...suggestion.alternatives.map(option => {
      const row = document.createElement('li');
      const title = document.createElement('strong');
      title.textContent = option.description;
      row.append(title);
      if (option.id === suggestion.recommended_alternative_ref) {
        const recommended = document.createElement('span');
        recommended.className = 'hint';
        recommended.textContent = ' — sugestão do Forge, não uma decisão sua';
        row.append(recommended);
      }
      if (option.consequences.length) {
        const consequences = document.createElement('ul');
        consequences.replaceChildren(...option.consequences.map(value => {
          const consequence = document.createElement('li');
          consequence.textContent = value;
          return consequence;
        }));
        row.append(consequences);
      }
      const discuss = document.createElement('button');
      discuss.type = 'button';
      discuss.className = 'question-action';
      discuss.textContent = 'Conversar sobre esta opção';
      discuss.setAttribute('aria-label', `Conversar sobre a opção: ${option.description}`);
      discuss.addEventListener('click', () => prepareQuestion(`Explique em português claro a opção "${option.description}" para a pergunta "${question}" e suas consequências antes de eu decidir. Ainda não estou escolhendo esta opção.`, current));
      row.append(discuss);
      return row;
    }));
    const ask = document.createElement('button');
    ask.type = 'button';
    ask.className = 'question-action';
    ask.textContent = 'Conversar sobre isso';
    ask.setAttribute('aria-label', `Conversar sobre: ${question}`);
    ask.addEventListener('click', () => prepareQuestion(`Explique em português claro esta pergunta do registro, inclusive se ela exige uma decisão minha: ${question}`, current));
    item.append(label, note, options, ask);
    return item;
  }));
  section.hidden = values.length === 0;
}
function prepareQuestion(request, current) {
  const composer = document.getElementById('message-text');
  if (current !== generation || !project || composer.disabled) return;
  composer.value = composer.value.trim() ? `${composer.value.trimEnd()}\n\n${request}` : request;
  showWorkspacePane('conversation');
  composer.focus();
  status.textContent = 'Pergunta preparada na conversa. Revise e envie quando quiser; nenhuma decisão foi registrada.';
}
function validTextList(values) {
  return Array.isArray(values) && values.every(value => typeof value === 'string' && value.trim());
}
function validSuggestions(values) {
  return Array.isArray(values) && values.length <= 20 && values.every(value => value
    && typeof value.question === 'string' && value.question.trim()
    && typeof value.blocking === 'boolean'
    && Array.isArray(value.alternatives) && value.alternatives.length >= 2 && value.alternatives.length <= 8
    && value.alternatives.every(option => option && typeof option.id === 'string' && option.id.trim()
      && typeof option.description === 'string' && option.description.trim()
      && validTextList(option.consequences))
    && new Set(value.alternatives.map(option => option.id)).size === value.alternatives.length
    && value.alternatives.some(option => option.id === value.recommended_alternative_ref));
}
function recordFailure(error) {
  if (error === 'O Forge está ocupado. Tente consultar o registro novamente.')
    return 'O Forge está ocupado. Tente atualizar o andamento daqui a pouco. A conversa não foi interrompida; nenhum progresso foi presumido.';
  if (error === 'O Forge demorou para responder. Você pode tentar novamente.')
    return 'O Forge demorou para responder. Tente atualizar o andamento novamente. A conversa não foi interrompida; nenhum progresso foi presumido.';
  if (error === 'O estado deste projeto não está disponível. Nada foi recriado ou alterado.' || error === 'Esta pasta ainda não usa o Forge. Escolha-a em Minha conversa e clique em “Continuar nesta pasta”.') {
    return 'O acompanhamento deste projeto não está disponível nesta pasta. Nada foi recriado. Confira a pasta em “Escolher pasta do projeto” antes de continuar.';
  }
  return 'Não foi possível atualizar o andamento. Nenhum progresso foi presumido; você pode continuar conversando e tentar novamente depois.';
}
export function setProgressProject(value) {
  project = value; generation++;
  clearSlowNotices();
  pending = false;
  workspacePhase.hidden = true;
  workspacePhase.textContent = '';
  resetHistory();
  document.querySelector('.record-more').open = false;
  recordPanel.hidden = !value;
  result.hidden = true;
  questionsShortcut.hidden = true;
  explainPending.hidden = true;
  status.textContent = value ? 'Atualizando o andamento pelo Forge…' : 'Escolha uma pasta para ver o andamento do projeto.';
  controls();
  if (value) void loadProgress();
}
export function invalidateProgress() {
  generation++;
  clearSlowNotices();
  workspacePhase.hidden = true;
  workspacePhase.textContent = '';
  resetHistory();
  if (!result.hidden || pending) {
    result.hidden = true;
    status.textContent = 'A conversa pode ter mudado o trabalho. Atualize o andamento para conferir.';
  }
  questionsShortcut.hidden = true;
  explainPending.hidden = true;
  pending = false;
  controls();
}
export function refreshProgressAfterTurn() {
  void loadProgress();
}
async function loadProgress() {
  if (!project || pending) return;
  const current = ++generation;
  clearTimeout(slowNotice);
  workspacePhase.hidden = true;
  workspacePhase.textContent = '';
  resetHistory();
  const hadFocus = document.activeElement === button;
  pending = true; controls(); result.hidden = true;
  questionsShortcut.hidden = true;
  explainPending.hidden = true;
  if (hadFocus) status.focus();
  status.textContent = 'Atualizando o andamento pelo Forge…';
  const notice = setTimeout(() => {
    if (current === generation && pending) status.textContent = 'O Forge está demorando para atualizar o andamento. Você pode continuar conversando; nenhum progresso será presumido.';
  }, 6000);
  slowNotice = notice;
  try {
    const data = await globalThis.__TAURI__.core.invoke('inspect_progress', { projectRoot: project.project_root });
    if (current !== generation) return;
    if (!data || !labels[data.status] || !Number.isSafeInteger(data.recorded_pending_count) || data.recorded_pending_count < 0 || !validSuggestions(data.suggested_questions)) throw new Error('Invalid record');
    if (data.accepted_direction) {
      const direction = data.accepted_direction;
      if (direction.origin !== 'forge_cooperative_record' || typeof direction.outcome !== 'string' || !direction.outcome.trim() || !Number.isSafeInteger(direction.revision) || direction.revision < 1 || !['initial', 'material_supersession', 'non_material_clarification'].includes(direction.revision_kind) || !validTextList(direction.constraints) || !validTextList(direction.unacceptable_outcomes) || !validTextList(direction.open_uncertainties)) throw new Error('Invalid direction');
    }
    const direction = data.accepted_direction;
    const hasWork = !!data.focus && data.status !== 'absent';
    if (hasWork) {
      if (typeof data.focus.title !== 'string' || typeof data.focus.intended_outcome !== 'string' || typeof data.focus.current_activity !== 'string' || typeof data.focus.next_step !== 'string' || !Number.isSafeInteger(data.focus.open_decision_count) || data.focus.open_decision_count < 0) throw new Error('Invalid record');
      for (const [id, field] of [['record-title', 'title'], ['record-activity', 'current_activity'], ['record-next', 'next_step']]) document.getElementById(id).textContent = data.focus[field];
      document.getElementById('record-outcome').textContent = data.focus.intended_outcome;
      document.getElementById('record-decisions').textContent = data.focus.open_decision_count === 0
        ? 'O Forge não mostra decisões em aberto neste acompanhamento.'
        : `O Forge mostra ${data.focus.open_decision_count} ${data.focus.open_decision_count === 1 ? 'decisão em aberto' : 'decisões em aberto'} neste acompanhamento. Converse com seu agente para entender o que precisa decidir.`;
    } else if (data.status !== 'absent') {
      throw new Error('Missing record details');
    }
    document.getElementById('record-work').hidden = !hasWork;
    const phase = phases[data.phase] || ['Etapa registrada', 'Converse com seu agente para entender esta etapa.'];
    const recordState = document.getElementById('record-state');
    recordState.textContent = data.status === 'absent' && direction
      ? 'Direção registrada; próximo trabalho pendente' : stateLabels[data.status];
    recordState.hidden = data.status === 'absent';
    result.dataset.state = data.status;
    document.querySelector('.record-stage').hidden = data.status === 'absent';
    const emptyHelp = document.getElementById('record-empty-help');
    emptyHelp.hidden = data.status !== 'absent';
    startConversation.hidden = data.status !== 'absent';
    emptyHelp.textContent = direction
      ? 'O objetivo está registrado. O próximo trabalho ainda não foi definido no Forge. Continue a conversa para combiná-lo com o agente.'
      : 'Comece pela conversa. Quando um próximo passo for registrado no Forge, ele aparecerá aqui. Seus arquivos continuam na pasta escolhida.';
    document.getElementById('record-phase').textContent = phase[0];
    document.getElementById('record-phase-help').textContent = data.status === 'completed'
      ? `${phase[1]} Este trabalho foi concluído; a etapa geral pode continuar aqui.` : phase[1];
    document.getElementById('record-activity-label').textContent = data.status === 'completed'
      ? 'Resultado registrado' : data.status === 'abandoned' ? 'Último registro' : 'Agora';
    workspacePhase.textContent = data.status === 'absent'
      ? direction ? 'Direção registrada; próximo trabalho pendente' : stateLabels.absent
      : data.status === 'stale' ? `Etapa do projeto (desatualizada): ${phase[0]}` : `Etapa do projeto: ${phase[0]}`;
    workspacePhase.hidden = false;
    const directionPanel = document.getElementById('record-direction');
    const directionCard = document.getElementById('record-direction-card');
    directionCard.hidden = !direction;
    directionCard.open = false;
    directionPanel.open = false;
    if (direction) {
      document.getElementById('record-revision').textContent = direction.revision_kind === 'initial'
        ? 'Primeira direção registrada.'
        : direction.revision_kind === 'material_supersession'
          ? `Direção revista no registro (revisão ${direction.revision}). A versão anterior não é a direção atual.`
          : `Detalhes acrescentados ao registro (revisão ${direction.revision}).`;
      document.getElementById('record-direction-outcome').textContent = direction.outcome;
      showList('record-constraints', 'record-constraints-list', direction.constraints);
      showList('record-unacceptable', 'record-unacceptable-list', direction.unacceptable_outcomes);
      showList('record-uncertainties', 'record-uncertainties-list', direction.open_uncertainties);
    }
    const pendingPanel = document.getElementById('record-pending');
    pendingPanel.hidden = data.recorded_pending_count === 0 && data.suggested_questions.length === 0;
    questionsShortcut.hidden = pendingPanel.hidden;
    explainPending.hidden = data.recorded_pending_count === 0;
    document.getElementById('record-pending-count').textContent = data.recorded_pending_count === 0
      ? 'Nenhuma decisão pendente foi recuperada do registro.'
      : `${data.recorded_pending_count} ${data.recorded_pending_count === 1 ? 'decisão pendente foi recuperada' : 'decisões pendentes foram recuperadas'} do registro. O texto original da escolha não está disponível aqui. Peça ao agente para consultar a origem antes de decidir.`;
    showSuggestedQuestions(data.suggested_questions);
    result.hidden = false;
    status.textContent = `${data.status === 'absent' && direction ? 'A direção foi registrada no Forge, mas o próximo trabalho ainda não.' : labels[data.status]} Consultado às ${new Date().toLocaleTimeString('pt-BR')}.`;
  } catch (error) {
    if (current === generation) status.textContent = recordFailure(error);
  } finally {
    clearTimeout(notice);
    if (slowNotice === notice) slowNotice = undefined;
    if (current === generation) { pending = false; controls(); }
  }
}
button.addEventListener('click', loadProgress);
startConversation.addEventListener('click', () => {
  if (!project || result.hidden || startConversation.hidden) return;
  showWorkspacePane('conversation');
  document.getElementById('message-text').focus();
});
questionsShortcut.addEventListener('click', () => {
  if (!project || result.hidden || questionsShortcut.hidden) return;
  const heading = document.getElementById('record-pending-heading');
  heading.focus({ preventScroll: true });
  heading.scrollIntoView({ block: 'start', behavior: 'smooth' });
});
explainPending.addEventListener('click', () => {
  if (!project || result.hidden || explainPending.hidden) return;
  const composer = document.getElementById('message-text');
  if (composer.disabled) {
    status.textContent = 'Aguarde a conversa ficar pronta para pedir uma explicação.';
    return;
  }
  const request = 'Consulte o registro do Forge deste projeto e explique quais decisões ainda estão pendentes e por quê. Mostre a pergunta e as opções somente se conseguir verificar o texto na fonte original. Se não conseguir recuperá-lo, diga isso claramente. Não trate sugestões como escolhas minhas e não registre uma decisão.';
  composer.value = composer.value.trim() ? `${composer.value.trimEnd()}\n\n${request}` : request;
  showWorkspacePane('conversation');
  composer.focus();
  status.textContent = 'Pedido preparado na conversa. Revise e envie quando quiser; nenhuma decisão foi registrada.';
});
document.getElementById('explain-record').addEventListener('click', () => {
  if (!project || result.hidden || document.getElementById('record-work').hidden) return;
  const composer = document.getElementById('message-text');
  if (composer.disabled) {
    status.textContent = 'Aguarde a conversa ficar pronta para pedir uma explicação.';
    return;
  }
  const request = 'Explique em linguagem simples a atividade atual e o próximo passo que constam no registro do Forge. Compare com a conversa mais recente antes de assumir que o registro está atualizado. Diga se precisa de alguma decisão minha.';
  composer.value = composer.value.trim() ? `${composer.value.trimEnd()}\n\n${request}` : request;
  showWorkspacePane('conversation');
  composer.focus();
  status.textContent = 'Pedido preparado na conversa. Revise e envie quando quiser; nada foi enviado.';
});
document.getElementById('explain-direction').addEventListener('click', () => {
  if (!project || result.hidden || document.getElementById('record-direction-card').hidden) return;
  const composer = document.getElementById('message-text');
  if (composer.disabled) {
    status.textContent = 'Aguarde a conversa ficar pronta para pedir uma explicação.';
    return;
  }
  const request = 'Explique em português claro a direção atual que consta no registro do Forge: o objetivo, o que foi combinado e o que devemos evitar. Compare com nossa conversa; se houver diferença, avise. Não trate perguntas sugeridas como decisões aprovadas e não altere o registro sem me consultar.';
  composer.value = composer.value.trim() ? `${composer.value.trimEnd()}\n\n${request}` : request;
  showWorkspacePane('conversation');
  composer.focus();
  status.textContent = 'Pedido preparado na conversa. Revise e envie quando quiser; nada foi enviado ou alterado no registro.';
});

function validHistory(data) {
  return data && Array.isArray(data.revisions) && data.revisions.length <= 100
    && Number.isSafeInteger(data.earlier_count) && data.earlier_count >= 0
    && data.revisions.filter(entry => entry.active).length <= 1
    && data.revisions.every(entry => entry && typeof entry.active === 'boolean'
      && ['forge_cooperative_record', 'human_intent_record'].includes(entry.origin)
      && Number.isSafeInteger(entry.revision) && entry.revision > 0
      && (entry.revision_kind === null || ['initial', 'material_supersession', 'non_material_clarification'].includes(entry.revision_kind))
      && typeof entry.outcome === 'string' && entry.outcome.trim()
      && validTextList(entry.constraints) && validTextList(entry.unacceptable_outcomes)
      && Number.isSafeInteger(entry.accepted_at_unix) && entry.accepted_at_unix >= 0);
}
function historyDetail(label, values) {
  if (!values.length) return null;
  const detail = document.createElement('div');
  detail.className = 'record-detail';
  const strong = document.createElement('strong');
  strong.textContent = label;
  const list = document.createElement('ul');
  list.replaceChildren(...values.map(value => {
    const item = document.createElement('li');
    item.textContent = value;
    return item;
  }));
  detail.append(strong, list);
  return detail;
}
function renderHistory(data) {
  const rows = data.revisions.slice().reverse().map(entry => {
    const card = document.createElement('article');
    card.className = 'direction-entry';
    const detail = document.createElement('details');
    const summary = document.createElement('summary');
    const heading = document.createElement('h4');
    heading.textContent = `Revisão ${entry.revision} · ${entry.active ? 'Direção atual' : 'Direção anterior'}`;
    summary.append(heading);
    const origin = document.createElement('p');
    origin.className = 'hint';
    origin.textContent = `${entry.origin === 'forge_cooperative_record' ? 'Registro cooperativo do Forge; não prova aprovação humana independente' : 'Registro de intenção com origem humana'} · ${new Date(entry.accepted_at_unix * 1000).toLocaleDateString('pt-BR')}`;
    const outcome = document.createElement('p');
    outcome.textContent = entry.outcome;
    detail.append(summary, origin, outcome);
    const constraints = historyDetail('Combinados registrados', entry.constraints);
    const unacceptable = historyDetail('O que evitar', entry.unacceptable_outcomes);
    if (constraints) detail.append(constraints);
    if (unacceptable) detail.append(unacceptable);
    card.append(detail);
    return card;
  });
  historyList.replaceChildren(...rows);
  historyStatus.textContent = data.revisions.length
    ? `${data.revisions.length} ${data.revisions.length === 1 ? 'direção registrada' : 'direções registradas'}${data.earlier_count ? `; ${data.earlier_count} anterior(es) não exibida(s) neste limite de leitura` : ''}. As versões anteriores não são a direção atual.`
    : 'Nenhuma direção anterior foi recuperada deste registro.';
}
async function loadHistory() {
  if (!project || pending || historyPending) return;
  const current = generation;
  const root = project.project_root;
  historyPending = true;
  controls();
  historyStatus.textContent = 'Consultando o histórico do Forge…';
  clearTimeout(historySlowNotice);
  const notice = setTimeout(() => {
    if (current === generation && historyPending) historyStatus.textContent = 'O histórico está levando mais tempo para abrir. Você pode continuar conversando enquanto esperamos.';
  }, 6000);
  historySlowNotice = notice;
  try {
    const data = await globalThis.__TAURI__.core.invoke('inspect_direction_history', { projectRoot: root });
    if (current !== generation) return;
    if (!validHistory(data)) throw new Error('Invalid history');
    renderHistory(data);
    historyLoaded = true;
  } catch {
    if (current === generation) historyStatus.textContent = 'Não foi possível consultar o histórico. Nada foi alterado; tente novamente.';
  } finally {
    clearTimeout(notice);
    if (historySlowNotice === notice) historySlowNotice = undefined;
    if (current === generation) { historyPending = false; controls(); }
  }
}
historyPanel.addEventListener('toggle', () => {
  if (historyPanel.open && !historyLoaded) void loadHistory();
});
historyButton.addEventListener('click', loadHistory);
