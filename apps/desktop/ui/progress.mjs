const button = document.getElementById('refresh-progress');
const status = document.getElementById('progress-status');
const result = document.getElementById('progress-result');
let project;
let generation = 0;
let pending = false;
const labels = { absent: 'Sem trabalho registrado.', current: 'Registro disponível. Ele pode não incluir a conversa mais recente.', stale: 'O Forge marcou este registro como desatualizado.', blocked: 'O trabalho registrado tem uma pendência.', completed: 'O trabalho registrado foi concluído. Isso não significa que o produto inteiro está pronto.', abandoned: 'O trabalho registrado foi encerrado sem conclusão.' };
const stateLabels = { current: 'Em andamento no registro', stale: 'Registro desatualizado', blocked: 'Pendência registrada', completed: 'Concluído no registro', abandoned: 'Encerrado sem concluir' };
const phases = {
  '0-route': ['Preparação', 'Entendendo como começar.'],
  '1-discovery': ['Descoberta', 'Entendendo o problema, as pessoas e os caminhos possíveis.'],
  '2-specification': ['Planejamento do produto', 'Definindo o que precisa ser entregue e como reconhecer um bom resultado.'],
  '3-plan': ['Definição da solução', 'Organizando a solução e a ordem do trabalho.'],
  '4-build-verify': ['Construção e verificação', 'Construindo e conferindo o resultado.'],
  '5-ready-operate': ['Validação e entrega', 'Validando o uso real e preparando a entrega.'],
  '6-evolve': ['Evolução', 'Escolhendo a próxima mudança para um produto já estável.'],
};
function controls() { button.disabled = !project || pending; }
export function setProgressProject(value) {
  project = value; generation++;
  result.hidden = true;
  status.textContent = value ? 'Consulte o último registro quando precisar. A consulta pode levar alguns segundos.' : 'Confira um projeto antes de consultar seus registros.';
  controls();
}
export function invalidateProgress() {
  generation++;
  if (!result.hidden || pending) {
    result.hidden = true;
    status.textContent = 'O registro pode estar desatualizado. Consulte novamente para conferir.';
  }
}
button.addEventListener('click', async () => {
  if (!project || pending) return;
  const current = ++generation;
  const hadFocus = document.activeElement === button;
  pending = true; controls(); result.hidden = true;
  if (hadFocus) status.focus();
  status.textContent = 'Consultando os registros do Forge…';
  try {
    const data = await globalThis.__TAURI__.core.invoke('inspect_progress', { projectRoot: project.project_root });
    if (current !== generation) return;
    if (!labels[data.status]) throw new Error('Unknown record state');
    if (data.focus && data.status !== 'absent') {
      if (typeof data.focus.title !== 'string' || typeof data.focus.intended_outcome !== 'string' || typeof data.focus.current_activity !== 'string' || typeof data.focus.next_step !== 'string' || !Number.isSafeInteger(data.focus.open_decision_count) || data.focus.open_decision_count < 0) throw new Error('Invalid record');
      const phase = phases[data.phase] || ['Etapa registrada', 'Converse com seu agente para entender esta etapa.'];
      document.getElementById('record-state').textContent = stateLabels[data.status];
      result.dataset.state = data.status;
      document.getElementById('record-phase').textContent = phase[0];
      document.getElementById('record-phase-help').textContent = phase[1];
      for (const [id, field] of [['record-title', 'title'], ['record-activity', 'current_activity'], ['record-next', 'next_step']]) document.getElementById(id).textContent = data.focus[field];
      document.getElementById('record-outcome').textContent = data.focus.intended_outcome;
      document.getElementById('record-decisions').textContent = data.focus.open_decision_count === 0
        ? 'Nenhuma decisão aberta consta neste registro.'
        : `${data.focus.open_decision_count} ${data.focus.open_decision_count === 1 ? 'decisão aberta consta' : 'decisões abertas constam'} neste registro. Converse com seu agente para entender o que precisa decidir.`;
      result.hidden = false;
    } else if (data.status !== 'absent') {
      throw new Error('Missing record details');
    }
    status.textContent = `${labels[data.status]} Consultado às ${new Date().toLocaleTimeString('pt-BR')}.`;
  } catch {
    if (current === generation) status.textContent = 'Não foi possível consultar o registro. Nenhum progresso foi presumido; você pode continuar conversando e tentar novamente depois.';
  } finally { pending = false; controls(); }
});
