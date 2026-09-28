// Focused browser check: Home reuses a recent shortcut but native inspection remains authoritative.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');

const root = path.resolve(__dirname, '../ui');
const server = createServer(async (request, response) => {
  const url = request.url.split('?')[0];
  const file = path.resolve(root, `.${decodeURIComponent(url === '/' ? '/index.html' : url)}`);
  if (!file.startsWith(`${root}${path.sep}`)) return response.writeHead(403).end();
  try {
    const data = await readFile(file);
    const type = file.endsWith('.mjs') || file.endsWith('.js') ? 'text/javascript'
      : file.endsWith('.css') ? 'text/css' : file.endsWith('.png') ? 'image/png' : 'text/html';
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
      window.inspections = [];
      window.sends = 0;
      window.rejectInspection = false;
      window.__TAURI__ = { core: { Channel: class {}, invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: '0.1.56' };
        if (command === 'inspect_project') {
          window.inspections.push(args.projectRoot);
          if (window.rejectInspection) throw 'Esta pasta não contém o projeto esperado.';
          return { project_id: 'saved-id', project_root: args.projectRoot };
        }
        if (command === 'send_message') window.sends++;
      } } };
    });
    const url = `http://127.0.0.1:${server.address().port}/`;
    await page.goto(url);
    assert.equal(await page.locator('#home-project-action').innerText(), 'Abrir meu projeto');
    await page.evaluate(() => localStorage.setItem('forge.projects.v1', JSON.stringify([
      { project_id: 'saved-id', project_root: 'D:\\work\\Meu Projeto' },
    ])));
    await page.reload();
    assert.equal(await page.locator('#home-project-title').innerText(), 'Seu último projeto');
    assert.match(await page.locator('#home-project-copy').innerText(), /Meu Projeto.*D:\\work\\Meu Projeto/);
    assert.equal(await page.locator('#home-hero-project-action').innerText(), 'Continuar último projeto');
    await page.evaluate(() => { window.rejectInspection = true; });
    await page.locator('#home-hero-project-action').click();
    await page.locator('#project-status').filter({ hasText: 'Esta pasta não contém o projeto esperado.' }).waitFor();
    assert.equal(await page.locator('#project-result').isVisible(), false);
    assert.deepEqual(await page.evaluate(() => [window.inspections.length, window.sends]), [1, 0]);
    await page.goto(`${url}#projects`);
    await page.getByRole('button', { name: /Remover Meu Projeto da lista/ }).click();
    await page.goto(`${url}#home`);
    assert.equal(await page.locator('#home-project-action').innerText(), 'Abrir meu projeto');
    assert.equal(await page.locator('#home-hero-project-action').innerText(), 'Continuar um projeto');
    assert.equal(await page.locator('#home-project-list').isVisible(), false);
    console.log('PASS: Home offers latest shortcut, native inspection rejects stale shortcut safely, removal restores folder-opening fallback; no Send.');
  } finally {
    await browser?.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
