// Focused browser double: references reuse the ordinary text draft and Send.
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
      window.sent = []; window.starts = 0; window.referenceMode = 'choose';
      window.__TAURI__ = { core: { Channel: class {}, invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: 'test' };
        if (command === 'choose_reference_file') {
          if (window.referenceMode === 'cancel') return null;
          if (window.referenceMode === 'fail') throw 'Picker unavailable';
          if (window.referenceMode === 'pending') return new Promise(resolve => { window.resolveReference = resolve; });
          return 'D:\\Referências\\ideia bonita.png';
        }
        if (command === 'create_default_project') return 'D:\\reference-test';
        if (command === 'start_project') { window.starts++; return { project_id: 'reference-test', project_root: args.projectRoot }; }
        if (command === 'connect_agent') return { thread_id: 'reference-thread', messages: [], resumed: false };
        if (command === 'send_message') window.sent.push(args.text);
      } } };
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    const input = page.locator('#message-text');
    const choose = page.locator('#add-reference');
    const copy = 'Minha ideia\nPreciso de uma imagem bonita\nCom várias cores\nSem perder meu texto';
    await input.fill(copy);
    const height = (await input.boundingBox()).height;
    assert.ok(height > 90, 'Multiline text grows without dragging the textarea');
    await choose.click();
    await page.locator('#reference-status').filter({ hasText: 'Arquivo indicado' }).waitFor();
    const draft = await input.inputValue();
    assert.ok(draft.startsWith(copy));
    assert.ok(draft.includes(JSON.stringify('D:\\Referências\\ideia bonita.png')));
    assert.deepEqual(await page.evaluate(() => window.sent), []);
    assert.equal(await page.evaluate(() => window.starts), 0);
    await choose.click();
    assert.equal(await input.inputValue(), draft, 'Same file is not duplicated');
    for (const mode of ['cancel', 'fail']) {
      await page.evaluate(mode => { window.referenceMode = mode; }, mode);
      await choose.click();
      await page.waitForFunction(() => !document.querySelector('#add-reference').disabled);
      assert.equal(await input.inputValue(), draft);
    }
    await page.evaluate(() => { window.referenceMode = 'pending'; });
    await choose.click();
    await page.waitForFunction(() => typeof window.resolveReference === 'function');
    assert.equal(await page.locator('#send-message').isDisabled(), true);
    await page.evaluate(() => document.querySelector('#message-form').requestSubmit());
    assert.equal(await page.evaluate(() => window.starts), 0, 'Even direct form submission waits for selection');
    await page.evaluate(() => { location.hash = '#home'; });
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.evaluate(() => window.resolveReference('D:\\stale.png'));
    await page.waitForFunction(() => !document.querySelector('#add-reference').disabled);
    assert.equal(await input.inputValue(), draft, 'Leaving the conversation discards a late selection');
    await page.evaluate(() => { location.hash = '#workspace'; });
    await input.focus();
    await input.press('Enter');
    assert.deepEqual(await page.evaluate(() => window.sent), [], 'Enter is a newline, not an accidental Send');
    const intended = await input.inputValue();
    await input.press('Control+Enter');
    await page.waitForFunction(() => window.sent.length === 1);
    assert.deepEqual(await page.evaluate(() => window.sent), [intended], 'First Send forwards the exact visible draft once');
    await input.fill(copy.repeat(6));
    const fit = await page.evaluate(() => ({
      formBottom: document.querySelector('#message-form').getBoundingClientRect().bottom,
      panelBottom: document.querySelector('#project-conversation').getBoundingClientRect().bottom,
    }));
    assert.ok(fit.formBottom <= fit.panelBottom, 'A long draft and file controls must stay inside the conversation panel');
    console.log('PASS: file selection, cancellation/failure/late-result safety, multiline writing, Enter and one explicit Ctrl+Enter Send.');
  } finally { await browser.close(); await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error); process.exitCode = 1; });
