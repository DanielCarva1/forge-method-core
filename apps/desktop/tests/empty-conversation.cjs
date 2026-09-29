// Focused browser-double check for Codex bookmarks. Not native IPC proof.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { clickConversationAction } = require('./conversation-options.cjs');

const root = path.resolve(__dirname, '../ui');
const types = { '.html': 'text/html', '.css': 'text/css', '.mjs': 'text/javascript', '.js': 'text/javascript', '.png': 'image/png' };
const server = createServer(async (request, response) => {
  const name = request.url === '/' ? 'index.html' : request.url.slice(1);
  const file = path.resolve(root, name);
  if (!file.startsWith(root + path.sep)) { response.writeHead(404).end(); return; }
  try {
    const data = await readFile(file);
    response.writeHead(200, { 'Content-Type': types[path.extname(file)] || 'application/octet-stream' }).end(data);
  }
  catch { response.writeHead(404).end(); }
});

(async () => {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_EXECUTABLE_PATH || undefined });
  try {
    const page = await browser.newPage();
    await page.addInitScript(() => {
      window.connectCalls = [];
      window.sendCalls = 0;
      window.__TAURI__ = { core: {
        Channel: class { constructor() { window.testEvents = this; } },
        invoke: async (command, args) => {
          if (command === 'app_info') return { name: 'Forge', version: 'test' };
          if (command === 'start_project') return { project_id: 'empty-project', project_root: args.projectRoot };
          if (command === 'connect_agent') {
            window.connectCalls.push(args.threadId ?? null);
            return args.threadId
              ? { thread_id: args.threadId, resumed: true, messages: [
                { id: 'user-1', role: 'user', text: 'Uma ideia.' },
                { id: 'agent-1', role: 'agent', text: 'Vamos criar.' },
              ] }
              : { thread_id: `thread-${window.connectCalls.length}`, resumed: false, messages: [] };
          }
          if (command === 'send_message') { window.sendCalls++; return; }
          if (command === 'disconnect_agent') return;
        },
      } };
    });
    const url = `http://127.0.0.1:${server.address().port}/#workspace`;
    const bookmark = 'forge.conversation.v1:["empty-project","D:\\\\empty-project"]';
    await page.goto(url);
    const open = async () => {
      await page.reload();
      await page.locator('#custom-folder-option summary').click();
      await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\empty-project');
      await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
      await page.waitForFunction(() => document.getElementById('project-status').textContent.includes('Projeto pronto'));
    };

    await open();
    const capability = page.locator('#agent-access-note');
    assert.equal(await capability.locator('summary').isVisible(), true, 'The agent capability warning stays visible');
    assert.match(await capability.locator('summary').textContent(), /alterar arquivos fora deste projeto sem perguntar/);
    assert.equal(await capability.locator('p').isHidden(), true, 'The full explanation need not crowd the composer');
    await capability.locator('summary').click();
    assert.match(await capability.locator('p').textContent(), /fora da pasta escolhida sem pedir confirmação/);
    await capability.locator('summary').click();
    await page.locator('#conversation-picker summary').click();
    await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Codex conectado' }).waitFor();
    assert.equal(await page.evaluate(key => localStorage.getItem(key), bookmark), null, 'Opening an empty thread must not save a bookmark');
    assert.deepEqual(await page.evaluate(() => window.connectCalls), [null]);
    assert.equal(await page.evaluate(() => window.sendCalls), 0);

    await open();
    assert.deepEqual(await page.evaluate(() => window.connectCalls), [], 'Reopening an unsent project must not resume a nonexistent thread');
    await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).fill('Uma ideia.');
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.waitForFunction(() => window.sendCalls === 1);
    assert.deepEqual(await page.evaluate(() => window.connectCalls), [null]);
    assert.equal(await page.evaluate(key => localStorage.getItem(key), bookmark), 'thread-1', 'An acknowledged send creates the durable bookmark');
    assert.equal(await page.evaluate(() => localStorage.getItem('forge.send-unconfirmed.v1:["empty-project","D:\\\\empty-project"]')), null);

    await open();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.connectCalls), ['thread-1']);
    assert.equal(await page.locator('#messages article').count(), 2);
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Resume must not send another turn');
    await page.evaluate(() => window.testEvents.onmessage({ kind: 'running' }));
    const nextDraft = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    assert.equal(await nextDraft.isEnabled(), true, 'A running Codex turn permits drafting the next message');
    assert.match(await page.locator('#composer-help').textContent(), /preparar a próxima mensagem/);
    await nextDraft.fill('Próxima ideia enquanto o agente responde.');
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true,
      'A running turn must still block Send');
    await page.evaluate(() => window.testEvents.onmessage({ kind: 'completed' }));
    assert.equal(await nextDraft.inputValue(), 'Próxima ideia enquanto o agente responde.',
      'Finishing a turn must not erase the next draft');
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isEnabled(), true);
    await nextDraft.fill('');
    await page.evaluate(() => window.testEvents.onmessage({ kind: 'disconnected' }));
    const offlineDraft = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    assert.equal(await offlineDraft.isEnabled(), true, 'A disconnected conversation still permits local drafting');
    await offlineDraft.fill('Rascunho escrito enquanto a conexão caiu.');
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true,
      'A disconnected conversation must not permit Send');
    await page.getByRole('button', { name: 'Reabrir conversa' }).click();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Reopening after disconnection must not replay a turn');
    assert.equal(await offlineDraft.inputValue(), 'Rascunho escrito enquanto a conexão caiu.',
      'Reopening must preserve the draft written while disconnected');
    const options = page.locator('#conversation-options');
    assert.equal(await options.locator('summary').isVisible(), true, 'Secondary actions stay reachable');
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isHidden(), true,
      'The conversation should not start with technical controls expanded');
    await options.locator('summary').click();
    assert.equal(await page.getByRole('button', { name: 'Ver texto original' }).isVisible(), true);
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isVisible(), true);
    await options.locator('summary').click();
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isHidden(), true);
    await options.locator('summary').focus();
    await page.keyboard.press('Enter');
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isVisible(), true,
      'Keyboard users can open the secondary actions');
    await page.keyboard.press('Enter');
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isHidden(), true);
    await page.setViewportSize({ width: 360, height: 700 });
    await page.evaluate(() => { document.documentElement.style.fontSize = '32px'; });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true,
      'Collapsed options and conversation must not force sideways scrolling at enlarged text');
    assert.equal(await options.locator('summary').isVisible(), true);
    await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
    await page.setViewportSize({ width: 1280, height: 720 });
    await clickConversationAction(page, 'Desconectar');
    await page.getByRole('button', { name: 'Continuar conversa anterior' }).click();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Manual reconnect must not replay a turn');
    await clickConversationAction(page, 'Desconectar');
    await page.locator('#conversation-picker summary').click();
    await page.getByRole('checkbox', { name: 'Começar outra conversa' }).check();
    await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Codex conectado' }).waitFor();
    assert.equal(await page.evaluate(key => localStorage.getItem(key), bookmark), 'thread-1', 'An empty alternative must not replace the previous real conversation');
    await open();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.connectCalls), ['thread-1']);
    assert.equal(await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).inputValue(),
      'Rascunho escrito enquanto a conexão caiu.', 'The disconnected draft survives a full page reload');
    await page.evaluate(() => window.testEvents.onmessage({ kind: 'update_required' }));
    assert.equal(await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).isEnabled(), true,
      'An incompatible Codex connection must not block editing a local draft');
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true,
      'An incompatible Codex connection must still block Send');
    console.log('PASS: bookmarks and resume never replay; disconnected/update-required states preserve local drafting while Send stays blocked.');
  } finally {
    await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
