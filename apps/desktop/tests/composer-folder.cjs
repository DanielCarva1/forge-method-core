// Focused browser check: a draft advances from folder choice to project preparation without a Send.
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
      window.projectStarts = 0;
      window.sends = 0;
      window.failStart = true;
      window.__TAURI__ = { core: { Channel: class {}, invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: '0.1.54' };
        if (command === 'choose_project_folder') { window.folderChoices++; return 'D:\\new-project'; }
        if (command === 'start_project') {
          window.projectStarts++;
          if (window.failStart) throw 'Não foi possível preparar esta pasta.';
          return { project_id: 'new-project', project_root: args.projectRoot };
        }
        if (command === 'connect_agent') return { thread_id: 'new-thread', messages: [], resumed: false };
        if (command === 'send_message') window.sends++;
      } } };
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/#explore`);
    await page.getByRole('link', { name: /Arte e criação/ }).click();
    await page.setViewportSize({ width: 360, height: 700 });
    assert.match(await page.locator('#idea-selection-status').innerText(), /Arte e criação.*Nada foi enviado/);
    assert.equal(await page.evaluate(() => document.body.scrollWidth), 360);
    assert.equal(await page.locator('#idea-choose-folder').isVisible(), true);
    const draft = page.locator('#message-text');
    const idea = await draft.inputValue();
    assert.match(idea, /artístico/);
    await page.getByRole('button', { name: 'Escolher pasta para esta ideia' }).click();
    assert.equal(await page.locator('#project-root').inputValue(), 'D:\\new-project');
    assert.deepEqual(await page.evaluate(() => [window.folderChoices, window.projectStarts, window.sends]), [1, 0, 0]);
    assert.equal(await draft.inputValue(), idea);
    await page.getByRole('button', { name: 'Preparar projeto nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Não foi possível preparar esta pasta' }).waitFor();
    assert.match(await page.locator('#idea-selection-status').innerText(), /Não foi possível preparar esta pasta.*não foi enviada/);
    assert.deepEqual(await page.evaluate(() => [window.folderChoices, window.projectStarts, window.sends]), [1, 1, 0]);
    assert.equal(await draft.inputValue(), idea);
    await page.evaluate(() => { window.failStart = false; });
    await page.getByRole('button', { name: 'Preparar projeto nesta pasta' }).click();
    await page.waitForFunction(() => !document.querySelector('#project-result').hidden);
    assert.deepEqual(await page.evaluate(() => [window.folderChoices, window.projectStarts, window.sends]), [1, 2, 0]);
    assert.equal(await draft.inputValue(), idea);
    await page.locator('#idea-choose-folder').waitFor({ state: 'hidden' });
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.waitForFunction(() => window.sends === 1);
    console.log('PASS: chosen folder is prepared without reopening picker or sending; failed preparation preserves draft; explicit Send sends once.');
  } finally {
    await browser?.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
