// Optional focused browser check. It does not prove native WebView IPC.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');

const assets = new Map([
  ['/', ['index.html', 'text/html']],
  ['/styles.css', ['styles.css', 'text/css']],
  ['/main.mjs', ['main.mjs', 'text/javascript']],
  ['/connection.mjs', ['connection.mjs', 'text/javascript']],
  ['/chat.mjs', ['chat.mjs', 'text/javascript']],
]);

(async () => {
  const server = createServer(async (req, res) => {
    const asset = assets.get(req.url);
    if (!asset) { res.writeHead(404).end(); return; }
    try {
      const data = await readFile(path.join(__dirname, '../ui', asset[0]));
      res.writeHead(200, { 'Content-Type': asset[1] }).end(data);
    } catch { res.writeHead(500).end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage();
    const url = `http://127.0.0.1:${server.address().port}`;
    for (const width of [390, 1180]) {
      await page.setViewportSize({ width, height: 844 });
      await page.goto(url);
      await page.getByRole('status').filter({ hasText: 'Não foi possível' }).waitFor();
      await page.getByRole('button', { name: 'Verificar novamente' }).click();
      await page.getByRole('status').filter({ hasText: 'Não foi possível' }).waitFor();
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
      if (width === 390) {
        const sizes = await page.locator('p, nav a, button, footer').evaluateAll(nodes => nodes.map(n => parseFloat(getComputedStyle(n).fontSize)));
        assert.ok(sizes.every(size => size >= 18));
      }
    }
    await page.goto(url);
    await page.keyboard.press('Tab');
    assert.equal(await page.locator(':focus').textContent(), 'Pular para o conteúdo');
    // Explicit test double for UI presentation only, not agent/native evidence.
    await page.evaluate(() => { window.__TAURI__ = { core: { invoke: async () => ({ name: 'Forge', version: '0.1.0' }) } }; });
    await page.getByRole('button', { name: 'Verificar novamente' }).click();
    await page.getByRole('status').filter({ hasText: 'Aplicativo iniciado' }).waitFor();
    await page.getByText('Nenhum agente conectado.', { exact: false }).waitFor();
    await page.emulateMedia({ colorScheme: 'dark' });
    assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).colorScheme), 'dark');
    // Controlled protocol double: exercise UI ordering, not real authentication.
    await page.evaluate(() => {
      window.sendCalls = 0; window.disconnectCalls = 0;
      window.__TAURI__.core.Channel = class {};
      window.__TAURI__.core.invoke = async (command, args) => {
        if (command === 'inspect_project') return { project_id: 'test-project', project_root: 'D:\\test-project' };
        if (command === 'connect_agent') { window.agentEvents = args.events; return 'test-thread'; }
        if (command === 'send_message') {
          window.sendCalls++;
          return new Promise((resolve, reject) => { window.resolveSend = resolve; window.rejectSend = reject; });
        }
        if (command === 'disconnect_agent') {
          window.disconnectCalls++;
          if (window.delayDisconnect) return new Promise(resolve => { window.resolveDisconnect = resolve; });
        }
      };
    });
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\test-project');
    await page.getByRole('button', { name: 'Conferir projeto' }).click();
    await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Codex conectado' }).waitFor();
    const composer = page.getByRole('textbox', { name: 'Conte sua ideia' });
    await composer.fill('🎨'.repeat(17000));
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    assert.equal(await page.evaluate(() => window.sendCalls), 0);
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isEnabled(), true);
    await composer.fill('First message');
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'completed' }));
    await composer.fill('Keep this new draft');
    await page.evaluate(() => window.resolveSend({}));
    assert.equal(await composer.inputValue(), 'Keep this new draft');
    await page.evaluate(() => { window.delayDisconnect = true; });
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'running' }));
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isDisabled(), true);
    await page.evaluate(() => window.resolveDisconnect());
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
    await page.evaluate(() => { window.delayDisconnect = false; });
    await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
    await composer.fill('Rejected request');
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.evaluate(() => window.rejectSend('Test rejection'));
    await page.locator('#agent-status').filter({ hasText: 'Test rejection' }).waitFor();
    assert.equal(await page.evaluate(() => window.disconnectCalls), 2);
    assert.equal(await page.getByRole('button', { name: 'Conectar Codex', exact: true }).isEnabled(), true);
    await page.evaluate(() => {
      const invoke = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = async (command, args) => {
        if (command === 'connect_agent') {
          args.events.onmessage({ kind: 'disconnected' });
          return 'already-ended-thread';
        }
        return invoke(command, args);
      };
    });
    await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'A conexão foi encerrada' }).waitFor();
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isEnabled(), true);
    console.log('PASS: disconnect-before-connect-ack never enables sending.');
    console.log('PASS: oversized Unicode remains recoverable, completion-before-ack preserves draft, pending disconnect locks controls, send rejection releases session.');
    console.log('PASS: desktop/mobile overflow, mobile text, retry, keyboard entry, dark theme, honest agent status. Native IPC NOT_RUN.');
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
