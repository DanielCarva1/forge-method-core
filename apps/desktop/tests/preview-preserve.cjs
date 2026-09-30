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
    await page.locator('#custom-folder-option summary').click();
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

    const draft = page.locator('#message-text');
    await draft.fill('Quero revisar este resultado.');
    const focus = page.getByRole('button', { name: 'Ampliar conversa' });
    await page.locator('#conversation-options summary').click();
    await focus.click();
    assert.equal(await page.locator('.workspace').evaluate(node => node.classList.contains('conversation-focus')), true);
    assert.equal(await page.locator('#project-preview').isHidden(), true, 'Reading mode leaves result available but out of the way');
    assert.equal(await page.locator('#project-record').isHidden(), true);
    assert.equal(await page.evaluate(() => {
      const conversation = document.getElementById('project-conversation').getBoundingClientRect();
      const workspace = document.querySelector('.workspace').getBoundingClientRect();
      return conversation.width >= workspace.width - 2 && document.documentElement.scrollWidth <= innerWidth;
    }), true, 'Reading mode gives the conversation the full desktop width');
    await page.evaluate(() => {
      const article = document.createElement('article');
      article.dataset.role = 'agent';
      article.innerHTML = '<span class="message-avatar" aria-hidden="true">F</span><div class="message-bubble"><strong>Codex</strong><div class="message-content">Aqui estão dois arquivos para conferir.</div></div>';
      document.getElementById('messages').append(article);
    });
    assert.equal(await page.locator('#messages article[data-role="agent"] .message-bubble').evaluate(node => {
      const bubble = node.getBoundingClientRect();
      return bubble.width >= 550 && bubble.width <= 850 && document.documentElement.scrollWidth <= innerWidth;
    }), true, 'An agent reply uses the reading space without becoming an overlong line');
    if (process.env.FORGE_READING_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_READING_SCREENSHOT });
    await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true,
      'Reading mode must not force sideways scrolling with enlarged text');
    await page.setViewportSize({ width: 901, height: 700 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true,
      'Reading mode must remain usable at the narrow desktop breakpoint');
    assert.equal(await page.getByRole('button', { name: 'Mostrar resultado e projeto' }).isVisible(), true);
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
    await page.locator('#messages article[data-role="agent"]').evaluate(node => node.remove());
    await page.getByRole('button', { name: 'Mostrar resultado e projeto' }).click();
    assert.equal(await page.locator('#project-preview').isVisible(), true, 'The reader can restore the side panels directly');
    assert.equal(await draft.inputValue(), 'Quero revisar este resultado.');
    await page.locator('#conversation-options summary').click();
    await focus.click();
    await page.evaluate(async () => (await import('/preview.mjs')).previewLinkedFile('other.txt'));
    assert.equal(await page.locator('.workspace').evaluate(node => node.classList.contains('conversation-focus')), false,
      'Opening a cited file returns to the result and project');
    assert.equal(await page.locator('#project-preview').isVisible(), true);
    assert.equal(await draft.inputValue(), 'Quero revisar este resultado.', 'Switching views never sends or erases the draft');
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await page.locator('#preview-path').textContent(), 'other.txt');
    assert.equal(await page.evaluate(() => JSON.parse(localStorage.getItem('forge.preview-files.v1'))[0].filePath), 'D:\\project\\other.txt');

    await page.evaluate(() => { window.nextFile = 'D:\\project\\archive.zip'; });
    await page.getByRole('button', { name: 'Escolher arquivo' }).click();
    await page.locator('#preview-status').filter({ hasText: 'Arquivo encontrado' }).waitFor();
    assert.equal(await page.getByRole('button', { name: 'Copiar caminho do arquivo' }).isVisible(), true);
    assert.equal(await page.evaluate(() => {
      const conversation = document.getElementById('project-conversation').getBoundingClientRect();
      const preview = document.getElementById('project-preview').getBoundingClientRect();
      return conversation.width >= preview.width * 1.15 && document.documentElement.scrollWidth <= innerWidth;
    }), true, 'A nonvisual file should keep reading space in the conversation');
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
