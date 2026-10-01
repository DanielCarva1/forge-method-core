// Shared Chromium/WebView visual checks. Sample messages, record, questions and
// login are PRESENTATION fixtures; never authentication/Codex/Core evidence.
const assert = require('node:assert/strict');
const path = require('node:path');
async function checkReadingSurfaces(page, output) {
  await page.locator('.primary-nav [data-route="workspace"]').click();
  await page.evaluate(async () => {
    document.querySelector('.workspace').classList.add('project-ready');
    for (const id of ['project-preview', 'project-record', 'conversation-options', 'message-view', 'agent-access-note']) document.getElementById(id).hidden = false;
    (await import('./mobile-workspace.mjs')).setMobileWorkspaceProject({ project_id: 'presentation-only' });
    const { renderAgentMessage } = await import('./message-format.mjs');
    const messages = document.getElementById('messages'); messages.replaceChildren();
    for (const [role, text] of [
      ['user', 'Quero um jardim de ideias: um lugar simples para mostrar meus projetos.'],
      ['agent', '## Um espaço com a sua identidade\n\nPodemos começar com uma página acolhedora, organizada em **três projetos**. Você escolhe o que mostrar; eu cuido de transformar isso em uma primeira versão.\n\n> Primeiro, algo que você já consiga abrir e experimentar. Depois, refinamos juntos.\n\n| Seu objetivo | Primeiro resultado |\n| --- | --- |\n| Mostrar seu trabalho | Uma página com seus projetos |\n| Fazer contato | Um convite simples para conversar |\n\n```css\n.jardim { color: #332923; }\n```\n\nNada precisa ser publicado agora. Podemos conferir a página aqui no Forge.'],
    ]) {
      const article = document.createElement('article'); article.dataset.role = role;
      const avatar = document.createElement('span'); avatar.className = 'message-avatar'; avatar.setAttribute('aria-hidden', 'true'); avatar.textContent = role === 'user' ? 'V' : 'F';
      const bubble = document.createElement('div'); bubble.className = 'message-bubble';
      const title = document.createElement('strong'); title.textContent = role === 'user' ? 'Você' : 'Codex · exemplo visual';
      const content = document.createElement('div'); content.className = 'message-content';
      if (role === 'agent') renderAgentMessage(content, text); else content.textContent = text;
      bubble.append(title, content); article.append(avatar, bubble); messages.append(article);
    }
    document.getElementById('chat-title').textContent = 'Jardim de ideias';
    const status = document.getElementById('agent-status'); status.dataset.state = 'completed'; status.textContent = 'Exemplo visual · nenhuma mensagem foi enviada';
    document.getElementById('progress-result').hidden = false;
    document.getElementById('record-work').hidden = false;
    document.getElementById('record-empty-help').hidden = true;
    document.getElementById('record-start-conversation').hidden = true;
    document.getElementById('record-state').hidden = false;
    document.getElementById('record-state').textContent = 'Exemplo visual';
    document.getElementById('record-outcome').textContent = 'Um lugar para suas ideias ganharem vida.';
    document.getElementById('record-next').textContent = 'Escolher os três projetos que vão aparecer na primeira página.';
    document.getElementById('record-activity').textContent = 'Estrutura inicial preparada para revisão.';
    document.getElementById('progress-status').textContent = 'Conteúdo de demonstração, não um registro real.';
    document.getElementById('record-pending').hidden = false;
    document.getElementById('record-pending-count').textContent = 'Exemplo de uma escolha em aberto. Nenhuma decisão foi registrada.';
    document.getElementById('record-decisions').textContent = 'Exemplo de detalhe registrado.';
    document.getElementById('record-title').textContent = 'Primeira página';
  });
  await page.locator('#workspace-organize').click();
  await page.locator('[data-mobile-pane-button="conversation"]').click();
  const composer = page.locator('#message-text');
  const draft = 'Quero uma página acolhedora, com espaço para mostrar meus projetos e contar um pouco sobre cada ideia. Vamos começar com jardim, música e fotografia.';
  await composer.fill(draft);
  const menu = page.locator('#conversation-options');
  await menu.locator('summary').click();
  const before = await page.locator('.conversation-body').boundingBox();
  await menu.locator('#message-view').evaluate(el => { el.hidden = false; });
  await page.keyboard.press('Tab'); await page.keyboard.press('Escape');
  assert.equal(await menu.evaluate(el => el.open), false);
  assert.equal(await menu.locator('summary').evaluate(el => el === document.activeElement), true);
  await menu.locator('summary').click(); await composer.click();
  assert.equal(await menu.evaluate(el => el.open), false);
  await menu.locator('summary').click(); await menu.locator('#message-view').click();
  assert.equal(await menu.evaluate(el => el.open), false);
  assert.equal(await menu.locator('summary').evaluate(el => el === document.activeElement), true, 'Menu action never leaves focus hidden');
  const after = await page.locator('.conversation-body').boundingBox();
  assert.ok(Math.abs(before.height - after.height) < 2, 'Options never steal reading height');
  await page.locator('#project-conversation [data-window-action="left"]').click();
  await page.waitForFunction(() => {
    const input = document.getElementById('message-text');
    return input.clientHeight >= Math.min(input.scrollHeight - 3, Math.min(innerHeight * .18, 140) - 3);
  }, null, { timeout: 5000 });
  assert.equal(await composer.inputValue(), draft, 'Width changes preserve and reflow the unsent draft');
  const shot = async name => { if (output) { await page.mouse.move(1150, 800); await page.screenshot({ path: path.join(output, `${name}.png`) }); } };
  await shot('reading-day-presentation');
  await page.locator('[data-mobile-pane-button="progress"]').click();
  await shot('record-day-presentation');
  // Contrast against solid nested surfaces; old dark scene canvas failed these.
  for (const theme of ['light', 'dark']) {
    await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
    const contrast = await page.evaluate(() => {
      const lum = color => { const c = color.match(/[\d.]+/g).slice(0, 3).map(Number).map(x => { x /= 255; return x <= .04045 ? x / 12.92 : ((x + .055) / 1.055) ** 2.4; }); return c[0]*.2126+c[1]*.7152+c[2]*.0722; };
      return ['.message-content pre', '.message-content blockquote', '.message-table-scroll th', '.record-pending', '.record-question'].map(selector => {
        const s = getComputedStyle(document.querySelector(selector)); const a = lum(s.color), b = lum(s.backgroundColor);
        return { selector, ratio: (Math.max(a,b)+.05)/(Math.min(a,b)+.05) };
      });
    });
    assert.ok(contrast.every(item => item.ratio >= 4.5), JSON.stringify({ theme, contrast }));
  }
  await page.locator('[data-mobile-pane-button="conversation"]').click();
  await page.locator('#project-conversation [data-window-action="focus"]').click();
  await page.evaluate(() => { const body = document.querySelector('.conversation-body'); body.scrollTop += document.querySelector('[data-role=agent]').getBoundingClientRect().top - body.getBoundingClientRect().top; });
  await shot('reading-night-presentation');
  await page.evaluate(async () => {
    const { createQuestions } = await import('./questions.mjs');
    window.visualAnswers = []; window.visualQuestions = createQuestions(document.getElementById('agent-questions'), args => { window.visualAnswers.push(args); }, () => {}, () => 'visual-thread');
    window.visualQuestions.receive({ kind: 'questions', id: 'visual-question', text: JSON.stringify({ threadId: 'visual-thread', questions: [{ id: 'style', question: 'Como você quer apresentar suas ideias?', options: [{ label: 'Um jardim tranquilo', description: 'Imagens em destaque, texto curto e espaço para respirar.' }, { label: 'Uma galeria criativa', description: 'Mais projetos juntos e cores presentes.' }] }] }) });
  });
  await page.locator('.question-option').first().click();
  assert.equal(await page.evaluate(() => window.visualAnswers.length), 0, 'Selecting an option is not sending an answer');
  await page.locator('#agent-questions').scrollIntoViewIfNeeded();
  await shot('question-presentation');
  await page.locator('.agent-questions textarea').fill('Prefiro algo simples e acolhedor.');
  assert.equal(await page.locator('.question-option input:checked').count(), 0);
  await page.evaluate(() => { window.visualQuestions.clear(); document.getElementById('login-panel').hidden = false; });
  await page.locator('#login-panel').scrollIntoViewIfNeeded();
  await shot('login-presentation');
  await page.evaluate(() => { document.getElementById('login-panel').hidden = true; });
  // Existing modal element, sample presentation only. Escape must close it;
  // never click acceptance or dispatch an external/native action.
  await page.evaluate(() => {
    document.getElementById('action-confirmation-title').textContent = 'Abrir site no navegador?';
    document.getElementById('action-confirmation-copy').textContent = 'Exemplo visual: confira o endereço antes de sair do Forge. Nenhum site será aberto neste teste.';
    const address = document.getElementById('action-confirmation-address'); address.textContent = 'https://example.com/projeto'; address.hidden = false;
    document.getElementById('action-confirmation-accept').textContent = 'Abrir no navegador';
    document.getElementById('action-confirmation').showModal();
    document.getElementById('action-confirmation-cancel').focus();
  });
  await shot('confirmation-presentation');
  await page.keyboard.press('Escape');
  assert.equal(await page.locator('#action-confirmation').isVisible(), false);
  await page.setViewportSize({ width: 360, height: 820 });
  await page.evaluate(() => { document.documentElement.style.fontSize = '32px'; });
  for (const pane of ['conversation', 'progress']) {
    await page.locator(`[data-mobile-pane-button="${pane}"]`).click();
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1), `${pane}: readable at 360px and enlarged text`);
  }
  assert.equal(await composer.inputValue(), draft);
  console.log('PASS: scoped reading/menu/draft/contrast/questions/login layout (sample presentation, no Send/authentication).');
}
module.exports = { checkReadingSurfaces };
