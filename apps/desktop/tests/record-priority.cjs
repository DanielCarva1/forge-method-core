// Focused browser check for the human-facing Forge record. Native IPC is mocked.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');

const uiRoot = path.resolve(__dirname, '../ui');
const mime = extension => ({ '.html': 'text/html', '.css': 'text/css', '.js': 'text/javascript', '.mjs': 'text/javascript', '.png': 'image/png' })[extension] || 'application/octet-stream';

(async () => {
  const server = createServer(async (request, response) => {
    const pathname = decodeURIComponent(new URL(request.url, 'http://localhost').pathname);
    const file = path.resolve(uiRoot, `.${pathname === '/' ? '/index.html' : pathname}`);
    if (!file.startsWith(`${uiRoot}${path.sep}`)) return response.writeHead(403).end();
    try {
      response.writeHead(200, { 'Content-Type': mime(path.extname(file)) }).end(await readFile(file));
    } catch { response.writeHead(404).end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_EXECUTABLE_PATH || undefined });
    const page = await browser.newPage({ viewport: { width: 1280, height: 800 } });
    await page.addInitScript(() => {
      window.recordStatus = 'current';
      window.directionRecorded = false;
      window.sendCalls = 0;
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: 'test' };
        if (command === 'start_project') return { project_id: 'record-test', project_root: args.projectRoot };
        if (command === 'inspect_progress') return {
          status: window.recordStatus, phase: '1-discovery',
          focus: window.recordStatus === 'absent' ? null : {
            title: 'Test focus', intended_outcome: 'Criar um resultado útil para a pessoa',
            current_activity: 'Conferindo uma implementação', next_step: 'Testar o resultado no aplicativo', open_decision_count: 0,
          },
          accepted_direction: window.directionRecorded ? {
            origin: 'forge_cooperative_record', outcome: 'Um jardim de ideias utilizável', revision: 1,
            revision_kind: 'initial', constraints: [], unacceptable_outcomes: [], open_uncertainties: [],
          } : null, recorded_pending_count: 0, suggested_questions: [],
        };
        if (command === 'connect_agent') return { thread_id: 'record-thread', messages: [], resumed: false };
        if (command === 'list_conversations') return { conversations: [], next_cursor: null };
        if (command === 'send_message') { window.sendCalls++; throw new Error('Unexpected Send'); }
        return null;
      } } };
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    await page.locator('#custom-folder-option summary').click();
    await page.locator('#project-root').fill(String.raw`D:\record-test`);
    await page.locator('#start-project').click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.equal(await page.locator('#record-outcome').textContent(), 'Criar um resultado útil para a pessoa');
    assert.equal(await page.locator('#record-outcome').isVisible(), true);
    assert.equal(await page.locator('#record-next').textContent(), 'Testar o resultado no aplicativo');
    assert.equal(await page.locator('#record-next').isVisible(), true);
    assert.equal(await page.locator('#workspace-phase').textContent(), 'Em andamento');
    assert.equal(await page.locator('#record-phase').isVisible(), false);
    if (process.env.FORGE_RECORD_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_RECORD_SCREENSHOT, fullPage: true });
    await page.locator('.record-stage summary').click();
    assert.equal(await page.locator('#record-phase').textContent(), 'Descoberta');
    assert.equal(await page.locator('#record-phase').isVisible(), true);
    await page.evaluate(() => { window.recordStatus = 'stale'; });
    await page.locator('#refresh-progress').click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.equal(await page.locator('#workspace-phase').textContent(), 'Acompanhamento desatualizado');
    assert.equal(await page.locator('#record-phase').isVisible(), false, 'Refresh should close optional process details');
    await page.evaluate(() => { window.recordStatus = 'absent'; });
    await page.locator('#refresh-progress').click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.equal(await page.locator('#record-work').isVisible(), false);
    assert.equal(await page.locator('.record-stage').isVisible(), false);
    assert.equal(await page.locator('#record-state').isVisible(), false, 'No direction or work means no state chip');
    await page.evaluate(() => { window.directionRecorded = true; });
    await page.locator('#refresh-progress').click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.equal(await page.locator('#record-state').textContent(), 'Direção registrada; próximo trabalho pendente');
    assert.equal(await page.locator('#record-state').isVisible(), true, 'A recorded objective without a next step must remain visible');
    assert.equal(await page.locator('#record-work').isVisible(), false, 'Do not invent a work focus');
    assert.equal(await page.evaluate(() => window.sendCalls), 0);
    console.log('PASS: objective and next step lead; stage is optional; stale and absent states stay honest.');
  } finally {
    await browser?.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
