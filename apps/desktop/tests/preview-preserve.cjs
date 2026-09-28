// Focused browser-double check: an invalid replacement cannot erase a valid preview.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');

const root = path.resolve(__dirname, '../ui');
const types = { '.html': 'text/html', '.css': 'text/css', '.mjs': 'text/javascript', '.js': 'text/javascript', '.png': 'image/png' };
const server = createServer(async (request, response) => {
  const name = request.url === '/' ? 'index.html' : request.url.slice(1);
  const file = path.resolve(root, name);
  if (!file.startsWith(root + path.sep)) { response.writeHead(404).end(); return; }
  try {
    const data = await readFile(file);
    response.writeHead(200, { 'Content-Type': types[path.extname(file)] || 'application/octet-stream' }).end(data);
  } catch { response.writeHead(404).end(); }
});

(async () => {
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_EXECUTABLE_PATH || undefined });
  try {
    const page = await browser.newPage();
    await page.addInitScript(() => {
      window.nextFile = 'D:\\project\\good.txt';
      window.reads = [];
      window.previewPickRoots = [];
      Object.defineProperty(navigator, 'clipboard', { configurable: true, value: {
        writeText: async text => {
          if (window.failClipboard) throw new Error('Clipboard unavailable');
          window.copiedPreviewPath = text;
        },
      } });
      window.__TAURI__ = { core: {
        invoke: async (command, args) => {
          if (command === 'app_info') return { name: 'Forge', version: 'test' };
          if (command === 'start_project') return { project_id: 'preview-project', project_root: args.projectRoot };
          if (command === 'choose_preview_file') {
            window.previewPickRoots.push(args?.projectRoot);
            if (window.rejectPicker) throw 'A pasta do projeto não está mais disponível.';
            return window.nextFile;
          }
          if (command === 'inspect_preview') {
            window.reads.push(args.filePath);
            if (args.filePath.includes('outside')) throw 'Este arquivo não pertence ao projeto aberto.';
            if (args.filePath.endsWith('.zip')) return { kind: 'file', relative_path: 'archive.zip', content: '', size_bytes: 20 };
            return { kind: 'text', relative_path: args.filePath.split('\\').at(-1), content: `Conteúdo de ${args.filePath.split('\\').at(-1)}`, size_bytes: 20 };
          }
        },
      } };
    });
    await page.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\project');
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.waitForFunction(() => document.getElementById('project-status').textContent.includes('Projeto pronto'));
    await page.getByRole('button', { name: 'Escolher arquivo' }).click();
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.previewPickRoots), ['D:\\project']);
    assert.equal(await page.locator('#preview-path').textContent(), 'good.txt');

    await page.evaluate(() => { window.rejectPicker = true; });
    await page.getByRole('button', { name: 'Escolher arquivo' }).click();
    await page.locator('#preview-status').filter({ hasText: 'A pasta do projeto não está mais disponível.' }).waitFor();
    assert.equal(await page.locator('#preview-path').textContent(), 'good.txt');
    await page.evaluate(() => { window.rejectPicker = false; });

    await page.evaluate(() => { window.nextFile = 'D:\\outside.txt'; });
    await page.getByRole('button', { name: 'Escolher arquivo' }).click();
    await page.locator('#preview-status').filter({ hasText: 'prévia anterior foi mantida' }).waitFor();
    assert.equal(await page.locator('#preview-path').textContent(), 'good.txt');
    assert.equal(await page.locator('#preview-text').textContent(), 'Conteúdo de good.txt');
    assert.equal(await page.locator('#preview-result').isVisible(), true);
    assert.equal(await page.locator('.workspace').evaluate(node => node.classList.contains('preview-loaded')), true);
    assert.equal(await page.locator('.preview-empty').isVisible(), false);

    await page.evaluate(async () => (await import('/preview.mjs')).previewLinkedFile('D:\\outside.txt'));
    await page.locator('#preview-status').filter({ hasText: 'prévia anterior foi mantida' }).waitFor();
    assert.equal(await page.locator('#preview-path').textContent(), 'good.txt');
    assert.equal(await page.locator('.workspace').evaluate(node => node.classList.contains('preview-loaded')), true);

    await page.evaluate(async () => (await import('/preview.mjs')).previewLinkedFile('other.txt'));
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await page.locator('#preview-path').textContent(), 'other.txt');
    assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('forge.preview-files.v1'))[0].filePath), 'D:\\project\\other.txt');

    await page.evaluate(() => { window.nextFile = 'D:\\project\\archive.zip'; });
    await page.getByRole('button', { name: 'Escolher arquivo' }).click();
    await page.locator('#preview-status').filter({ hasText: 'Arquivo encontrado' }).waitFor();
    assert.equal(await page.getByRole('button', { name: 'Copiar caminho do arquivo' }).isVisible(), true);
    await page.getByRole('button', { name: 'Copiar caminho do arquivo' }).click();
    assert.equal(await page.evaluate(() => window.copiedPreviewPath), 'D:\\project\\archive.zip');
    assert.match(await page.locator('#preview-status').textContent(), /Caminho copiado/);
    await page.evaluate(() => { window.failClipboard = true; });
    await page.getByRole('button', { name: 'Copiar caminho do arquivo' }).click();
    assert.match(await page.locator('#preview-status').textContent(), /Não foi possível copiar/);
    assert.equal(await page.locator('#preview-path').textContent(), 'archive.zip');
    console.log('PASS: failed picker and cited-file replacements preserve the validated preview; valid replacement updates it.');
  } finally {
    await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
