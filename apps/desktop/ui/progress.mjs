const button = document.getElementById('refresh-progress');
const status = document.getElementById('progress-status');
const result = document.getElementById('progress-result');
let project;
let generation = 0;
let pending = false;
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
    const labels = { absent: 'Sem trabalho registrado.', current: 'Registro disponível. Ele pode não incluir a conversa mais recente.', stale: 'O Forge marcou este registro como desatualizado.', blocked: 'O trabalho registrado tem uma pendência.', completed: 'O trabalho registrado foi concluído. Isso não significa que o produto inteiro está pronto.', abandoned: 'O trabalho registrado foi encerrado sem conclusão.' };
    if (!labels[data.status]) throw new Error('Unknown record state');
    status.textContent = `${labels[data.status]} Consultado às ${new Date().toLocaleTimeString('pt-BR')}.`;
    if (data.focus && data.status !== 'absent') {
      for (const [id, field] of [['record-title', 'title'], ['record-activity', 'current_activity'], ['record-next', 'next_step']]) document.getElementById(id).textContent = data.focus[field];
      document.getElementById('record-decisions').textContent = `Decisões abertas no registro: ${data.focus.open_decision_count}. Converse com seu agente para entendê-las.`;
      result.hidden = false;
    }
  } catch {
    if (current === generation) status.textContent = 'Não foi possível consultar o registro. Nenhum progresso foi presumido; você pode continuar conversando e tentar novamente depois.';
  } finally { pending = false; controls(); }
});
