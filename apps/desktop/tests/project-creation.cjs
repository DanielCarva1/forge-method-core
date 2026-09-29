// Focused browser-double check. Native folder and filesystem behavior are
// separately owned by Rust; this covers the complete visible UI handoff.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');

const mime = file => file.endsWith('.html') ? 'text/html' : file.endsWith('.css') ? 'text/css'
  : file.endsWith('.png') ? 'image/png' : 'text/javascript';

(async () => {
  const server = createServer(async (request, response) => {
    const file = request.url === '/' ? 'index.html' : decodeURIComponent(request.url.slice(1));
    if (!/^(?:[a-z-]+\.(?:mjs|js|css|html)|assets\/[a-z-]+\.png)$/.test(file)) {
      response.writeHead(404).end(); return;
    }
    try {
      response.writeHead(200, { 'Content-Type': mime(file) })
        .end(await readFile(path.join(__dirname, '..', 'ui', file)));
    } catch { response.writeHead(404).end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_EXECUTABLE_PATH || undefined });
    const page = await browser.newPage();
    await page.addInitScript(() => {
      window.defaultCalls = [];
      window.startCalls = [];
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: 'test' };
        if (command === 'create_default_project') {
          window.defaultCalls.push(args.idea);
          if (window.defaultError) throw window.defaultError;
          return 'D:\\Projetos\\criar um jardim de ideias';
        }
        if (command === 'start_project') {
          window.startCalls.push(args.projectRoot);
          if (window.startError) throw window.startError;
          return { project_id: 'new-project', project_root: args.projectRoot };
        }
        if (command === 'inspect_progress') return { status: 'absent', phase: null, focus: null, recorded_pending_count: 0, suggested_questions: [], accepted_direction: null };
        throw new Error(`Unexpected native command: ${command}`);
      } } };
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    await page.locator('#workspace').waitFor({ state: 'visible' });
    if (process.env.FORGE_NEW_PROJECT_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_NEW_PROJECT_SCREENSHOT, fullPage: true });
    await page.locator('#message-text').fill('Quero criar um jardim de ideias.');
    await page.evaluate(() => { window.defaultError = 'Não foi possível criar o novo projeto em Documentos.'; });
    await page.locator('#create-default-project').click();
    await page.locator('#default-project-status').filter({ hasText: 'Não foi possível criar' }).waitFor();
    assert.equal(await page.locator('#project-root').inputValue(), '');
    assert.equal(await page.locator('#message-text').inputValue(), 'Quero criar um jardim de ideias.');
    assert.deepEqual(await page.evaluate(() => window.startCalls), []);

    await page.evaluate(() => { window.defaultError = null; window.startError = 'O Forge não pôde preparar este projeto.'; });
    await page.locator('#create-default-project').click();
    await page.locator('#default-project-status').filter({ hasText: 'A pasta foi criada, mas o projeto ainda não ficou pronto' }).waitFor();
    assert.equal(await page.locator('#project-root').inputValue(), 'D:\\Projetos\\criar um jardim de ideias');
    assert.equal(await page.locator('#message-text').inputValue(), 'Quero criar um jardim de ideias.');
    await page.evaluate(() => { window.startError = null; });
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await page.locator('#project-root').inputValue(), 'D:\\Projetos\\criar um jardim de ideias');
    assert.deepEqual(await page.evaluate(() => window.startCalls), ['D:\\Projetos\\criar um jardim de ideias', 'D:\\Projetos\\criar um jardim de ideias']);
    assert.deepEqual(await page.evaluate(() => window.defaultCalls), ['Quero criar um jardim de ideias.', 'Quero criar um jardim de ideias.']);
    assert.equal(await page.locator('#message-text').inputValue(), 'Quero criar um jardim de ideias.');
    assert.equal(await page.locator('#project-name').textContent(), 'criar um jardim de ideias');
    console.log('PASS: one-click default needs no path; creation/start failures keep the draft and allow retry in the same folder.');
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
