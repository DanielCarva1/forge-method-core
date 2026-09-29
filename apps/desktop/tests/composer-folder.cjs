// Focused browser check: a draft starts a default project or uses an optional folder without an automatic Send.
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
      : file.endsWith('.css') ? 'text/css'
        : file.endsWith('.png') ? 'image/png' : 'text/html';
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
      window.folderChoices = 0;
      window.defaultProjects = 0;
      window.projectStarts = 0;
      window.sends = 0;
      window.failStart = true;
      window.__TAURI__ = { core: { Channel: class {}, invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: '0.1.54' };
        if (command === 'choose_project_folder') { window.folderChoices++; return 'D:\\new-project'; }
        if (command === 'create_default_project') { window.defaultProjects++; return 'D:\\new-project'; }
        if (command === 'start_project') {
          window.projectStarts++;
          if (window.failStart) throw 'Não foi possível preparar esta pasta.';
          return { project_id: 'new-project', project_root: args.projectRoot };
        }
        if (command === 'connect_agent') return { thread_id: 'new-thread', messages: [], resumed: false };
        if (command === 'send_message') window.sends++;
      } } };
    });
    await page.setViewportSize({ width: 360, height: 700 });
    await page.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    const firstIdea = await page.evaluate(() => {
      const conversation = document.querySelector('#project-conversation').getBoundingClientRect();
      const project = document.querySelector('#project-panel').getBoundingClientRect();
      const composer = document.querySelector('#message-text').getBoundingClientRect();
      return { conversationTop: conversation.top, projectTop: project.top, composerTop: composer.top,
        workspaceTop: document.querySelector('#workspace').getBoundingClientRect().top,
        pageWidth: document.body.scrollWidth };
    });
    if (process.env.FORGE_UI_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_UI_SCREENSHOT, fullPage: true });
    assert.ok(firstIdea.conversationTop < firstIdea.projectTop, 'On a narrow first-use screen, the idea must come before folder setup');
    assert.ok(firstIdea.composerTop - firstIdea.workspaceTop < 700, `The first composer must be reachable within one short screen: ${JSON.stringify(firstIdea)}`);
    assert.equal(firstIdea.pageWidth, 360);
    await page.setViewportSize({ width: 1180, height: 820 });
    const wideOrder = await page.evaluate(() => ({
      conversationLeft: document.querySelector('#project-conversation').getBoundingClientRect().left,
      projectLeft: document.querySelector('#project-panel').getBoundingClientRect().left,
      composerTop: document.querySelector('#message-text').getBoundingClientRect().top,
      pageWidth: document.body.scrollWidth,
    }));
    assert.ok(wideOrder.conversationLeft < wideOrder.projectLeft, 'Wide first-use keeps conversation before the folder sidebar');
    assert.ok(wideOrder.composerTop < 820, `Wide first-use must show the composer without scrolling: ${JSON.stringify(wideOrder)}`);
    assert.equal(wideOrder.pageWidth, 1180);
    if (process.env.FORGE_UI_SCREENSHOT_DESKTOP) await page.screenshot({ path: process.env.FORGE_UI_SCREENSHOT_DESKTOP, fullPage: true });
    await page.setViewportSize({ width: 360, height: 700 });
    await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).fill('Quero fazer um jardim de ideias.');
    assert.equal(await page.locator('#send-label').textContent(), 'Enviar e criar projeto');
    await page.locator('#custom-folder-option summary').click();
    await page.locator('#browse-project').click();
    assert.deepEqual(await page.evaluate(() => [window.folderChoices, window.projectStarts, window.sends]), [1, 0, 0]);
    assert.equal(await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).inputValue(), 'Quero fazer um jardim de ideias.');
    assert.deepEqual(await page.evaluate(() => [...document.querySelector('.workspace').children].slice(0, 2).map(node => node.id)),
      ['project-conversation', 'project-panel'], 'Choosing a folder must not move its form ahead of the idea');
    await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).fill('');
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill('');
    await page.goto(`http://127.0.0.1:${server.address().port}/#explore`);
    await page.reload();
    await page.getByRole('link', { name: /Arte e criação/ }).click();
    assert.match(await page.locator('#idea-selection-status').innerText(), /Arte e criação.*Nada foi enviado/);
    assert.equal(await page.evaluate(() => document.body.scrollWidth), 360);
    await page.locator('#idea-choose-folder').waitFor({ state: 'visible' });
    const draft = page.locator('#message-text');
    const idea = await draft.inputValue();
    assert.match(idea, /artístico/);
    await page.getByRole('button', { name: 'Começar projeto com esta ideia' }).click();
    await page.locator('#project-status').filter({ hasText: 'Não foi possível preparar esta pasta' }).waitFor();
    assert.equal(await page.locator('#project-root').inputValue(), 'D:\\new-project');
    assert.deepEqual(await page.evaluate(() => [window.folderChoices, window.defaultProjects, window.projectStarts, window.sends]), [0, 1, 1, 0]);
    assert.equal(await draft.inputValue(), idea);
    assert.deepEqual(await page.evaluate(() => [...document.querySelector('.workspace').children].slice(0, 2).map(node => node.id)),
      ['project-conversation', 'project-panel'], 'A failed preparation must keep the idea-first order');
    assert.match(await page.locator('#idea-selection-status').innerText(), /Não foi possível preparar esta pasta.*não foi enviada/);
    assert.deepEqual(await page.evaluate(() => [window.folderChoices, window.defaultProjects, window.projectStarts, window.sends]), [0, 1, 1, 0]);
    assert.equal(await draft.inputValue(), idea);
    await page.evaluate(() => { window.failStart = false; });
    await page.getByRole('button', { name: 'Preparar projeto nesta pasta' }).click();
    await page.waitForFunction(() => !document.querySelector('#project-result').hidden);
    assert.deepEqual(await page.evaluate(() => [window.folderChoices, window.defaultProjects, window.projectStarts, window.sends]), [0, 1, 2, 0]);
    assert.equal(await draft.inputValue(), idea);
    await page.locator('#idea-choose-folder').waitFor({ state: 'hidden' });
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.waitForFunction(() => window.sends === 1);
    console.log('PASS: narrow idea-first layout, optional folder, default Explore project, retry, and explicit Send.');
  } finally {
    await browser?.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
