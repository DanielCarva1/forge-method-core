// Project/Codex IPC is a double, not native evidence.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { checkWorkspaceNavigation } = require('./workspace-navigation.cjs');
const root = path.resolve(__dirname, '../ui');
const types = { '.html': 'text/html', '.css': 'text/css', '.mjs': 'text/javascript', '.js': 'text/javascript', '.png': 'image/png' };
const server = createServer(async (request, response) => {
  const file = path.resolve(root, request.url === '/' ? 'index.html' : request.url.slice(1));
  if (!file.startsWith(root + path.sep)) { response.writeHead(404).end(); return; }
  try { response.writeHead(200, { 'Content-Type': types[path.extname(file)] || 'application/octet-stream' }).end(await readFile(file)); }
  catch { response.writeHead(404).end(); }
});
(async () => {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.addInitScript(() => {
      window.operations = [];
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        window.operations.push(command);
        if (command === 'app_info') return { name: 'Forge', version: 'test' };
        if (command === 'start_project') return { project_id: 'navigation-test', project_root: args.projectRoot };
        if (command === 'inspect_progress') return { status: 'absent', focus: null, recorded_pending_count: 0, suggested_questions: [] };
        return null;
      } } };
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    assert.equal(await page.locator('#mobile-workspace-nav').isHidden(), true);
    await page.locator('#custom-folder-option summary').click();
    await page.locator('#project-root').fill('D:\\navigation-test');
    await page.locator('#start-project').click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    const before = await page.evaluate(() => window.operations.slice());
    await checkWorkspaceNavigation(page);
    assert.deepEqual(await page.evaluate(() => window.operations), before, 'Navigation must not send, reconnect or query the engine');
    console.log('PASS: desktop shortcuts, keyboard focus, expanded conversation, narrow layout and 200% text; no backend operations.');
  } finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error); process.exitCode = 1; });
