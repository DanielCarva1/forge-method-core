// Browser double for the connected file/result/app actions; native authority tested separately.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '../ui');
const server = createServer(async (request, response) => {
  const file = path.resolve(root, request.url === '/' ? 'index.html' : request.url.slice(1));
  if (!file.startsWith(root + path.sep)) return response.writeHead(404).end();
  try {
    const type = /\.m?js$/.test(file) ? 'text/javascript' : file.endsWith('.css') ? 'text/css' : file.endsWith('.png') ? 'image/png' : 'text/html';
    response.writeHead(200, { 'Content-Type': type }).end(await readFile(file));
  } catch { response.writeHead(404).end(); }
});
(async () => {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const browser = await chromium.launch({ headless: true });
  try {
    const page = await browser.newPage();
    await page.addInitScript(() => {
      window.calls = []; window.listMode = 'ok'; window.pick = 'D:\\results\\README.md';
      const file = (relative_path, kind) => ({ relative_path, kind, size_bytes: 1024, modified_at: 1 });
      window.files = [file('site/index.html', 'page'), file('ideias.md', 'document'), file('imagens/capa.png', 'image')];
      window.__TAURI__ = { core: { Channel: class {}, invoke: async (command, args) => {
        window.calls.push({ command, args });
        if (command === 'app_info') return { name: 'Forge', version: 'test' };
        if (command === 'start_project') return { project_id: 'results', project_root: args.projectRoot };
        if (command === 'list_project_files') {
          if (window.listMode === 'fail') throw 'Cannot read';
          if (window.listMode === 'pending') return new Promise(resolve => { window.finishList = resolve; });
          return { files: window.files, truncated: false };
        }
        if (command === 'inspect_preview') {
          if (args.filePath.includes('missing')) throw 'Este arquivo não está mais disponível.';
          return { kind: 'text', relative_path: args.filePath.endsWith('index.html') ? 'site/index.html' : 'ideias.md', content: '# Minha ideia\nTexto real do projeto', size_bytes: 40 };
        }
        if (command === 'save_project_file_copy') { if (window.saveMode === 'fail') throw 'O destino já existe; nada foi substituído.'; if (window.saveMode === 'pending') return new Promise(resolve => { window.finishSave = resolve; }); return window.saveMode === 'cancel' ? null : 'D:/Cópias/ideias.md'; }
        if (command === 'choose_preview_file') return window.pick;
        if (command === 'connect_agent') { window.events = args.events; return { thread_id: 'results-thread', messages: [], resumed: false }; }
        if (command === 'send_message') window.events.onmessage({ kind: 'completed' });
      } } };
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    await page.locator('#custom-folder-option summary').click();
    await page.locator('#project-root').fill('D:\\results');
    await page.locator('#start-project').click();
    await page.locator('.result-file-card').first().waitFor();
    assert.equal(await page.locator('.result-file-card').count(), 3);
    await page.locator('#result-files-search').fill('ideias');
    assert.equal(await page.locator('.result-file-card').count(), 1);
    await page.locator('.result-file-card').click();
    await page.locator('#preview-path').filter({ hasText: 'ideias.md' }).waitFor();
    await page.locator('#reveal-result-file').click();
    assert.ok(await page.evaluate(() => window.calls.some(c => c.command === 'reveal_project_file' && c.args.filePath.endsWith('ideias.md'))));
    await page.locator('#save-result-copy').click();
    await page.locator('#preview-status').filter({ hasText: 'Cópia salva' }).waitFor();
    await page.evaluate(() => { window.saveMode = 'cancel'; });
    await page.locator('#save-result-copy').click();
    await page.locator('#preview-status').filter({ hasText: 'Você cancelou' }).waitFor();
    await page.evaluate(() => { window.saveMode = 'fail'; });
    await page.locator('#save-result-copy').click();
    await page.locator('#preview-status').filter({ hasText: 'nada foi substituído' }).waitFor();
    await page.locator('#message-text').fill('Meu rascunho');
    await page.locator('#request-preview-change').click();
    assert.ok((await page.locator('#message-text').inputValue()).startsWith('Meu rascunho\nQuero mudar o arquivo ideias.md:'));
    assert.equal(await page.evaluate(() => window.calls.filter(c => c.command === 'send_message').length), 0);
    await page.locator('#result-files-search').fill('');
    await page.locator('#result-files-type').selectOption('image');
    assert.equal(await page.locator('.result-file-card').count(), 1);
    await page.locator('#result-files-type').selectOption('');
    await page.evaluate(() => { window.listMode = 'fail'; });
    await page.locator('#refresh-result-files').click();
    await page.locator('#result-files-status').filter({ hasText: 'lista anterior foi mantida' }).waitFor();
    assert.equal(await page.locator('.result-file-card').count(), 3);
    await page.evaluate(() => { window.listMode = 'ok'; window.files.push({ relative_path: 'missing.txt', kind: 'document', size_bytes: 1, modified_at: 2 }); });
    await page.locator('#refresh-result-files').click();
    await page.locator('[data-result-path="missing.txt"]').click();
    await page.locator('#preview-status').filter({ hasText: 'prévia anterior foi mantida' }).waitFor();
    assert.equal(await page.locator('#preview-path').textContent(), 'ideias.md');
    await page.locator('#message-text').fill('Vamos criar');
    await page.locator('#send-message').click();
    await page.waitForFunction(() => window.events);
    await page.evaluate(() => window.events.onmessage({ kind: 'message', id: 'app', text: 'Experimente [o app](http://localhost:5173/). Outro exemplo está em https://example.com. Não publiquei nada.' }));
    const apps = page.locator('#preview-local-apps');
    await apps.waitFor({ state: 'visible' });
    assert.equal(await apps.locator('.local-app-card').count(), 1);
    const openCount = () => page.evaluate(() => window.calls.filter(c => c.command === 'open_external_link').length);
    assert.equal(await openCount(), 0, 'Receiving an address does not open/start anything');
    await apps.getByRole('button', { name: /Experimentar app local/ }).click();
    await page.locator('#action-confirmation-cancel').click();
    assert.equal(await openCount(), 0);
    await apps.getByRole('button', { name: /Experimentar app local/ }).click();
    await page.locator('#action-confirmation-accept').click();
    await page.waitForFunction(() => window.calls.some(c => c.command === 'open_external_link'));
    await page.locator('#message-text').fill('Preserve meu pedido');
    await apps.getByRole('button', { name: /Pedir mudança no app local/ }).click();
    assert.equal(await page.locator('#message-text').inputValue(), 'Preserve meu pedido\nQuero mudar o resultado em http://localhost:5173/: ');
    await page.setViewportSize({ width: 360, height: 720 });
    await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    await page.locator('[data-mobile-pane-button="preview"]').click();
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await page.evaluate(() => { window.saveMode = 'pending'; });
    await page.locator('#save-result-copy').click();
    await page.waitForFunction(() => window.finishSave);
    assert.equal(await page.locator('#save-result-copy').isDisabled(), true);
    await page.evaluate(async () => { const { setPreviewProject } = await import('/preview.mjs'); setPreviewProject(null); window.finishSave('D:/Cópias/old.md'); });
    await page.waitForFunction(() => document.querySelector('#save-result-copy').disabled);
    assert.equal((await page.locator('#preview-status').textContent()).includes('old.md'), false, 'Old-project save cannot update new project feedback');
    await page.evaluate(async () => { const { setPreviewProject } = await import('/preview.mjs'); window.listMode = 'pending'; setPreviewProject({ project_root: 'D:\\old' }); });
    await page.waitForFunction(() => window.finishList);
    await page.evaluate(async () => { const { setPreviewProject } = await import('/preview.mjs'); setPreviewProject(null); window.finishList({ files: window.files, truncated: false }); });
    assert.equal(await page.locator('.result-file-card').count(), 0, 'Late old-project list is discarded');
    console.log('PASS: reveal/copy success/cancel/failure/pending/stale draft preservation; project file discovery/filter/open/change, failed refresh/missing file preserve prior result, explicit app opening/change, narrow 200% and stale project list. Browser double.');
  } finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error); process.exitCode = 1; });
