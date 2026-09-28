// Focused browser-double check for Codex bookmarks. Not native IPC proof.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');

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
        Channel: class {},
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
      await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\empty-project');
      await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
      await page.waitForFunction(() => document.getElementById('project-status').textContent.includes('Projeto pronto'));
    };

    await open();
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
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await page.locator('#conversation-picker summary').click();
    await page.getByRole('checkbox', { name: 'Começar outra conversa' }).check();
    await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Codex conectado' }).waitFor();
    assert.equal(await page.evaluate(key => localStorage.getItem(key), bookmark), 'thread-1', 'An empty alternative must not replace the previous real conversation');
    await open();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.connectCalls), ['thread-1']);
    console.log('PASS: empty thread is not bookmarked; acknowledged Send is; alternative empty thread preserves prior history; reopen never re-sends.');
  } finally {
    await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
