// Focused browser check for update guidance; it does not prove native browser launch.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');

const root = path.resolve(__dirname, '../ui');
const server = createServer(async (request, response) => {
  const file = path.resolve(root, `.${decodeURIComponent(request.url.split('?')[0] === '/' ? '/index.html' : request.url.split('?')[0])}`);
  if (!file.startsWith(`${root}${path.sep}`)) return response.writeHead(403).end();
  try {
    const type = file.endsWith('.mjs') || file.endsWith('.js') ? 'text/javascript'
      : file.endsWith('.css') ? 'text/css'
        : file.endsWith('.png') ? 'image/png' : 'text/html';
    const data = await readFile(file);
    response.writeHead(200, { 'Content-Type': type }).end(data);
  } catch { response.writeHead(404).end(); }
});

(async () => {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_EXECUTABLE_PATH || undefined });
    const page = await browser.newPage();
    await page.addInitScript(() => {
      window.updateCalls = 0;
      window.channels = [];
      window.__TAURI__ = { core: { Channel: class {
        constructor() { window.channels.push(this); }
      }, invoke: async command => {
        if (command === 'app_info') return { name: 'Forge', version: '0.1.54' };
        if (command === 'open_updates_page') { window.updateCalls++; if (window.updateFails) throw new Error('browser unavailable'); }
        if (command === 'start_project') return { project_id: 'project-one', project_root: 'C:\\Projects\\One' };
        if (command === 'connect_agent') return { thread_id: 'thread-one', resumed: false, messages: [] };
      } } };
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/#about`);
    await page.locator('#app-version').filter({ hasText: 'Versão instalada: 0.1.54' }).waitFor();
    const guidance = await page.locator('.updates').textContent();
    assert.match(guidance, /Forge Desktop.*número maior/s);
    assert.match(guidance, /Forge_…_x64-setup\.exe/);
    assert.match(guidance, /Não escolha “Source code” nem uma versão do Forge Core/);
    await page.setViewportSize({ width: 360, height: 720 });
    await page.evaluate(() => { document.documentElement.style.fontSize = '32px'; });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true,
      'Update guidance must wrap without horizontal scrolling at narrow width and enlarged text');
    const button = page.getByRole('button', { name: 'Encontrar atualização do Forge Desktop' });
    await button.click();
    await page.locator('#updates-status').filter({ hasText: 'solicitada ao navegador' }).waitFor();
    assert.equal(await page.evaluate(() => window.updateCalls), 1);
    await page.evaluate(() => { window.updateFails = true; });
    await button.click();
    await page.locator('#updates-url').waitFor({ state: 'visible' });
    assert.match(await page.locator('#updates-url').textContent(), /github\.com\/DanielCarva1\/forge-method-core\/releases\?q=desktop/);
    await page.setViewportSize({ width: 1280, height: 800 });
    await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
    await page.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    await page.locator('#project-root').fill('C:\\Projects\\One');
    await page.locator('#start-project').click();
    await page.locator('#project-result').waitFor({ state: 'visible' });
    await page.locator('#conversation-picker summary').click();
    await page.locator('#connect-agent').click();
    await page.locator('#disconnect-agent').waitFor({ state: 'visible' });
    await page.evaluate(() => { window.channels.at(-1).onmessage({ kind: 'update_required' }); });
    const updateAction = page.locator('#agent-update-action');
    await updateAction.waitFor({ state: 'visible' });
    assert.match(await page.locator('#agent-status').textContent(), /Forge Desktop/);
    await updateAction.click();
    await page.waitForURL('**/#updates');
    assert.equal(await page.locator('#home').isVisible(), true);
    assert.equal(await page.locator('#about').evaluate(node => node.open), true);
    assert.equal(await page.locator('#updates h3').evaluate(node => document.activeElement === node), true);
    console.log('PASS: Desktop update instructions, browser fallback and direct recovery from model-version error.');
  } finally {
    await browser?.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
