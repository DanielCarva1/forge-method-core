// Focused browser double: question forms, not proof of a real Codex turn.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '../ui');
const server = createServer(async (request, response) => {
  const file = path.resolve(root, request.url === '/' ? 'index.html' : request.url.slice(1));
  if (!file.startsWith(root + path.sep)) return response.writeHead(404).end();
  try {
    const type = /\.m?js$/.test(file) ? 'text/javascript' : file.endsWith('.css') ? 'text/css' : file.endsWith('.png') ? 'image/png' : 'text/html';
    response.writeHead(200, { 'Content-Type': type }).end(await readFile(file));
  } catch { response.writeHead(404).end(); }
});
(async () => {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.addInitScript(() => {
      window.answers = []; window.sent = []; window.answerMode = 'ok';
      window.__TAURI__ = { core: { Channel: class {}, invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: 'test' };
        if (command === 'create_default_project') return 'D:\\question-test';
        if (command === 'start_project') return { project_id: 'questions', project_root: args.projectRoot };
        if (command === 'connect_agent') { window.events = args.events; return { thread_id: 'thread', messages: [], resumed: false }; }
        if (command === 'send_message') window.sent.push(args.text);
        if (command === 'answer_questions') {
          window.answers.push(args);
          if (window.answerMode === 'fail') throw 'Falha de conexão';
          if (window.answerMode === 'pending') return new Promise(resolve => { window.answerDone = resolve; });
        }
        if (command === 'interrupt_agent') window.events.onmessage({ kind: 'interrupted' });
      } } };
      window.ask = (id = 'request', threadId = 'thread') => window.events.onmessage({ kind: 'questions', id, text: JSON.stringify({ threadId, turnId: 'turn', questions: [
        { id: 'style', header: 'Estilo', question: 'Como você quer o visual?', options: [{ label: 'Claro', description: 'Leve e convidativo' }, { label: 'Escuro', description: 'Com mais contraste' }] },
        { id: 'name', header: 'Nome', question: '<img src=x onerror=alert(1)> Qual é o nome?', options: null },
      ] }) });
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    await page.locator('#message-text').fill('Vamos criar');
    await page.locator('#send-message').click();
    await page.waitForFunction(() => window.sent.length === 1);
    await page.locator('#message-text').fill('Meu próximo pedido continua aqui');
    await page.evaluate(() => { window.events.onmessage({ kind: 'running' }); window.ask(); });
    const panel = page.locator('#agent-questions');
    await panel.waitFor({ state: 'visible' });
    assert.equal(await panel.locator('input:checked').count(), 0, 'No answer preselected');
    assert.equal(await panel.locator('img').count(), 0, 'Question content is literal text');
    await panel.getByRole('button', { name: 'Enviar respostas' }).click();
    assert.equal(await page.evaluate(() => window.answers.length), 0, 'Incomplete choices never sent');
    await panel.getByRole('radio', { name: /Claro/ }).check();
    await panel.locator('textarea').nth(1).fill('Jardim');
    await page.evaluate(() => { window.answerMode = 'fail'; });
    await panel.getByRole('button', { name: 'Enviar respostas' }).click();
    await panel.getByRole('status').filter({ hasText: 'Falha de conexão' }).waitFor();
    assert.equal(await panel.locator('textarea').nth(1).inputValue(), 'Jardim');
    await page.evaluate(() => { window.answerMode = 'pending'; });
    await panel.locator('textarea').first().fill('Colorido, com flores');
    assert.equal(await panel.locator('input:checked').count(), 0, 'Written answer replaces option');
    await panel.getByRole('button', { name: 'Enviar respostas' }).click();
    await page.waitForFunction(() => typeof window.answerDone === 'function');
    await page.evaluate(() => document.querySelector('#agent-questions form').requestSubmit());
    assert.equal(await page.evaluate(() => window.answers.length), 2, 'Pending submit cannot duplicate');
    const answer = await page.evaluate(() => window.answers.at(-1));
    assert.deepEqual(answer, { requestId: 'request', threadId: 'thread', answers: { style: 'Colorido, com flores', name: 'Jardim' } });
    await page.evaluate(() => window.answerDone());
    await panel.waitFor({ state: 'hidden' });
    assert.equal(await page.locator('#message-text').inputValue(), 'Meu próximo pedido continua aqui');
    await page.evaluate(() => window.ask('wrong', 'other-thread'));
    assert.equal(await panel.isHidden(), true, 'Other conversation ignored');
    await page.evaluate(() => window.ask('second'));
    await panel.waitFor({ state: 'visible' });
    await page.setViewportSize({ width: 360, height: 720 });
    await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await panel.getByRole('button', { name: 'Interromper esta execução' }).click();
    await panel.waitFor({ state: 'hidden' });
    await page.evaluate(() => window.ask('third'));
    await page.evaluate(() => window.events.onmessage({ kind: 'questions_resolved', id: 'third' }));
    assert.equal(await panel.isHidden(), true);
    console.log('PASS: explicit options/free answers, literal text, failed/pending send, draft preservation, stale/resolved/cancel safety and narrow 200% form.');
  } finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error); process.exitCode = 1; });
