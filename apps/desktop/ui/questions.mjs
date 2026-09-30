// Explicit answers to a pending Codex question. No draft/history persistence.
export function createQuestions(panel, answer, interrupt, currentThread) {
  const pending = new Map();
  let active = null;
  let epoch = 0;
  function paint() {
    panel.replaceChildren();
    active = pending.keys().next().value ?? null;
    panel.hidden = active === null;
    if (active === null) return;
    const requestId = active;
    const request = pending.get(requestId);
    const heading = document.createElement('h3');
    heading.textContent = 'Uma escolha sua';
    heading.tabIndex = -1;
    const hint = document.createElement('p');
    hint.textContent = 'Escolha uma opção ou escreva sua resposta. Nada é enviado sem seu clique.';
    const form = document.createElement('form');
    const entries = [];
    for (const [index, question] of request.questions.entries()) {
      const fieldset = document.createElement('fieldset');
      const legend = document.createElement('legend');
      legend.textContent = question.question;
      fieldset.append(legend);
      for (const option of question.options ?? []) {
        const label = document.createElement('label');
        label.className = 'question-option';
        const radio = document.createElement('input');
        radio.type = 'radio'; radio.name = `question-${index}`; radio.value = option.label;
        const description = document.createElement('span');
        const title = document.createElement('strong'); title.textContent = option.label;
        const detail = document.createElement('small'); detail.textContent = option.description;
        description.append(title, detail); label.append(radio, description); fieldset.append(label);
      }
      const label = document.createElement('label');
      label.textContent = question.options?.length ? 'Ou responda com suas palavras' : 'Sua resposta';
      const text = document.createElement('textarea');
      text.rows = 2; text.maxLength = 8000;
      label.append(text); fieldset.append(label);
      text.addEventListener('input', () => { if (text.value) fieldset.querySelectorAll('input').forEach(input => { input.checked = false; }); });
      fieldset.addEventListener('change', event => { if (event.target.type === 'radio') text.value = ''; });
      entries.push({ question, fieldset, text }); form.append(fieldset);
    }
    const actions = document.createElement('div'); actions.className = 'question-actions';
    const send = document.createElement('button'); send.type = 'submit'; send.className = 'primary'; send.textContent = 'Enviar respostas';
    const cancel = document.createElement('button'); cancel.type = 'button'; cancel.textContent = 'Interromper esta execução';
    const status = document.createElement('p'); status.setAttribute('role', 'status'); status.setAttribute('aria-live', 'polite');
    actions.append(send, cancel); form.append(actions, status); panel.append(heading, hint, form);
    let sending = false;
    const version = epoch;
    const valid = () => version === epoch && pending.get(requestId) === request && currentThread() === request.threadId;
    form.addEventListener('submit', async event => {
      event.preventDefault();
      if (sending || !valid()) return;
      const answers = Object.create(null);
      for (const { question, fieldset, text } of entries) {
        const value = text.value.trim() || fieldset.querySelector('input:checked')?.value;
        if (!value) { status.textContent = 'Responda a todas as perguntas antes de enviar.'; text.focus(); return; }
        answers[question.id] = value;
      }
      sending = true; send.disabled = true; cancel.disabled = true;
      form.querySelectorAll('input,textarea').forEach(input => { input.disabled = true; });
      status.textContent = 'Enviando suas respostas…';
      try {
        await answer({ requestId, threadId: request.threadId, answers });
        if (valid()) { pending.delete(requestId); paint(); }
      } catch (error) {
        if (valid()) {
          status.textContent = typeof error === 'string' ? error : 'Não foi possível confirmar o envio. Confira a conexão.';
          sending = false; send.disabled = false; cancel.disabled = false;
          form.querySelectorAll('input,textarea').forEach(input => { input.disabled = false; });
        }
      }
    });
    cancel.addEventListener('click', async () => {
      if (sending || !valid()) return;
      cancel.disabled = true;
      try { await interrupt(); if (valid()) { pending.clear(); paint(); } }
      catch { if (valid()) { status.textContent = 'Não foi possível interromper. Confira a conexão.'; cancel.disabled = false; } }
    });
  }
  return {
    receive(event) {
      if (event.kind === 'questions') {
        const request = JSON.parse(event.text);
        if (request.threadId !== currentThread() || pending.has(event.id)) return;
        pending.set(event.id, request);
        if (active === null) paint();
      } else if (event.kind === 'questions_resolved') {
        pending.delete(event.id); if (active === event.id) paint();
      }
    },
    clear() { epoch++; pending.clear(); paint(); },
  };
}
