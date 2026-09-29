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
    await page.goto(`http://127.0.0.1:${server.address().port}/#home`);
    if (process.env.FORGE_HOME_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_HOME_SCREENSHOT, fullPage: true });
    await page.getByRole('link', { name: 'Começar com minha ideia' }).click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#workspace-back-label').textContent(), 'Voltar às ideias');
    await page.getByRole('link', { name: 'Meus projetos', exact: true }).click();
    if (process.env.FORGE_PROJECTS_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_PROJECTS_SCREENSHOT, fullPage: true });
    await page.getByRole('link', { name: 'Criar um projeto' }).click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#workspace-back-label').textContent(), 'Voltar aos projetos');
    assert.equal(await page.locator('#project-root').inputValue(), '');
    assert.equal(await page.locator('#custom-folder-option').evaluate(node => node.open), false, 'Folder selection stays optional on the direct creation path');
    await page.getByRole('link', { name: 'Meus projetos', exact: true }).click();
    await page.getByRole('link', { name: 'Escolher uma pasta' }).click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#custom-folder-option').evaluate(node => node.open), true, 'Explicit folder path opens its controls');
    await page.getByRole('link', { name: 'Meus projetos', exact: true }).click();
    await page.getByRole('link', { name: 'Criar um projeto' }).click();
    assert.equal(await page.locator('#custom-folder-option').evaluate(node => node.open), false);
    if (process.env.FORGE_NEW_PROJECT_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_NEW_PROJECT_SCREENSHOT, fullPage: true });
    await page.locator('#message-text').fill('Quero criar um jardim de ideias.');
    assert.equal(await page.locator('#send-label').textContent(), 'Enviar e criar projeto');
    await page.evaluate(() => { window.defaultError = 'Não foi possível criar o novo projeto em Documentos.'; });
    await page.locator('#send-message').click();
    await page.locator('#default-project-status').filter({ hasText: 'Não foi possível criar' }).waitFor();
    assert.equal(await page.locator('#project-root').inputValue(), '');
    assert.equal(await page.locator('#message-text').inputValue(), 'Quero criar um jardim de ideias.');
    assert.deepEqual(await page.evaluate(() => window.startCalls), []);

    await page.evaluate(() => { window.defaultError = null; window.startError = 'O Forge não pôde preparar este projeto.'; });
    await page.locator('#send-message').click();
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
    const flowPage = await browser.newPage();
    await flowPage.addInitScript(() => {
      window.sentTexts = [];
      window.defaultCalls = 0;
      window.__TAURI__ = { core: {
        Channel: class {},
        invoke: async (command, args) => {
          if (command === 'app_info') return { name: 'Forge', version: 'test' };
          if (command === 'create_default_project') { window.defaultCalls++; return 'D:\\Projetos\\primeira ideia'; }
          if (command === 'start_project') return { project_id: 'one-click-project', project_root: args.projectRoot };
          if (command === 'connect_agent') return { thread_id: 'one-click-thread', messages: [], resumed: false };
          if (command === 'send_message') { window.sentTexts.push(args.text); return null; }
          if (command === 'inspect_progress') return { status: 'absent', phase: null, focus: null, recorded_pending_count: 0, suggested_questions: [], accepted_direction: null };
          throw new Error(`Unexpected native command: ${command}`);
        },
      } };
    });
    await flowPage.goto(`http://127.0.0.1:${server.address().port}/#home`);
    await flowPage.getByRole('link', { name: 'Começar com minha ideia' }).click();
    const firstIdea = 'Quero um site simples para guardar minhas ideias.';
    await flowPage.locator('#message-text').fill(firstIdea);
    await flowPage.locator('#send-message').click();
    await flowPage.waitForFunction(() => window.sentTexts.length === 1);
    assert.deepEqual(await flowPage.evaluate(() => window.sentTexts), [firstIdea], 'One explicit Send must not require a second click or duplicate the message');
    assert.equal(await flowPage.evaluate(() => window.defaultCalls), 1);
    assert.equal(await flowPage.locator('#messages article[data-role="user"]').count(), 1);
    assert.equal(await flowPage.locator('#message-text').inputValue(), '');
    await flowPage.close();
    console.log('PASS: one explicit Send creates the default project and sends the preserved idea exactly once in the browser double.');
    const changedDraftPage = await browser.newPage();
    await changedDraftPage.addInitScript(() => {
      window.sentTexts = [];
      window.__TAURI__ = { core: {
        Channel: class {},
        invoke: async (command, args) => {
          if (command === 'app_info') return { name: 'Forge', version: 'test' };
          if (command === 'create_default_project') return new Promise(resolve => { window.resolveDefault = () => resolve('D:\\Projetos\\ideia alterada'); });
          if (command === 'start_project') return { project_id: 'changed-draft-project', project_root: args.projectRoot };
          if (command === 'connect_agent') return { thread_id: 'changed-draft-thread', messages: [], resumed: false };
          if (command === 'send_message') { window.sentTexts.push(args.text); return null; }
          if (command === 'inspect_progress') return { status: 'absent', phase: null, focus: null, recorded_pending_count: 0, suggested_questions: [], accepted_direction: null };
          throw new Error(`Unexpected native command: ${command}`);
        },
      } };
    });
    await changedDraftPage.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    await changedDraftPage.locator('#message-text').fill('Minha primeira intenção.');
    await changedDraftPage.locator('#send-message').click();
    await changedDraftPage.waitForFunction(() => typeof window.resolveDefault === 'function');
    await changedDraftPage.locator('#message-text').fill('Mudei de ideia enquanto o projeto era preparado.');
    await changedDraftPage.evaluate(() => window.resolveDefault());
    await changedDraftPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.deepEqual(await changedDraftPage.evaluate(() => window.sentTexts), [], 'Changing the draft must cancel the pending first Send');
    assert.equal(await changedDraftPage.locator('#message-text').inputValue(), 'Mudei de ideia enquanto o projeto era preparado.');
    await changedDraftPage.close();
    console.log('PASS: changing the draft while the project is prepared cancels the pending Send.');
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
