// Optional focused browser check. It does not prove native WebView IPC.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');

const assets = new Map([
  ['/', ['index.html', 'text/html']],
  ['/styles.css', ['styles.css', 'text/css']],
  ['/main.mjs', ['main.mjs', 'text/javascript']],
  ['/navigation.mjs', ['navigation.mjs', 'text/javascript']],
  ['/explore.mjs', ['explore.mjs', 'text/javascript']],
  ['/recent-projects.mjs', ['recent-projects.mjs', 'text/javascript']],
  ['/project-display.mjs', ['project-display.mjs', 'text/javascript']],
  ['/connection.mjs', ['connection.mjs', 'text/javascript']],
  ['/chat.mjs', ['chat.mjs', 'text/javascript']],
  ['/message-format.mjs', ['message-format.mjs', 'text/javascript']],
  ['/conversation-reference.mjs', ['conversation-reference.mjs', 'text/javascript']],
  ['/assets/forge.png', ['assets/forge.png', 'image/png']],
  ['/assets/explore-artwork.png', ['assets/explore-artwork.png', 'image/png']],
  ['/appearance.js', ['appearance.js', 'text/javascript']],
  ['/progress.mjs', ['progress.mjs', 'text/javascript']],
  ['/preview.mjs', ['preview.mjs', 'text/javascript']],
]);
async function openProjectSetup(page) {
  if (!await page.locator('#project-setup').evaluate(node => node.open)) await page.locator('#project-setup summary').click();
}
async function openConversation(page) {
  if (!await page.locator('#conversation-picker').evaluate(node => node.open)) await page.locator('#conversation-picker summary').click();
  await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
}

(async () => {
  const server = createServer(async (req, res) => {
    const asset = assets.get(req.url);
    if (!asset) { res.writeHead(404).end(); return; }
    try {
      const data = await readFile(path.join(__dirname, '../ui', asset[0]));
      res.writeHead(200, { 'Content-Type': asset[1] }).end(data);
    } catch { res.writeHead(500).end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    browser = await chromium.launch({
      headless: true,
      executablePath: process.env.PLAYWRIGHT_EXECUTABLE_PATH || undefined,
    });
    const page = await browser.newPage();
    const url = `http://127.0.0.1:${server.address().port}`;
    const workspaceUrl = `${url}#workspace`;
    await page.goto(url);
    await page.locator('nav a[data-route="home"][aria-current="page"]').waitFor();
    if (process.env.FORGE_HOME_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_HOME_SCREENSHOT, fullPage: true });
    assert.equal(await page.locator('#home').isVisible(), true);
    assert.equal(await page.locator('#workspace').isHidden(), true);
    assert.equal(await page.locator('#agent-access-note').isHidden(), true);
    assert.equal(await page.locator('nav a[data-route="home"]').getAttribute('aria-current'), 'page');
    await page.setViewportSize({ width: 360, height: 720 });
    await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    for (const [route, heading, link] of [
      ['home', '.hero h1', 'Início'],
      ['explore', '.explore-heading h1', 'Explorar'],
      ['projects', '.projects-heading h1', 'Meus projetos'],
      ['workspace', '.workspace-screen .screen-heading h1', 'Minha conversa'],
    ]) {
      await page.getByRole('link', { name: link, exact: true }).click();
      await page.locator(`#${route}`).waitFor({ state: 'visible' });
      assert.equal(await page.locator(heading).evaluate(node => node.scrollWidth <= node.clientWidth + 1), true,
        `${route} heading must reflow without clipping at 360px and 200% text size`);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true,
        `${route} must not require horizontal scrolling at 360px and 200% text size`);
      assert.equal(await page.locator(heading).evaluate(node =>
        document.querySelector('.appearance').getBoundingClientRect().bottom <= node.getBoundingClientRect().top), true,
      `${route} heading must not sit behind appearance controls at 360px and 200% text size`);
    }
    await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.getByRole('link', { name: 'Início', exact: true }).click();
    await page.getByRole('link', { name: 'Continuar um projeto' }).click();
    await page.locator('#projects').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#recent-projects').getAttribute('role'), 'group');
    assert.equal(await page.locator('#projects-empty').isVisible(), true);
    assert.match(await page.locator('#projects-empty').textContent(), /pasta.*vazia/i);
    await page.locator('#projects-empty').getByRole('link', { name: 'Explorar ideias' }).click();
    await page.locator('#explore').waitFor({ state: 'visible' });
    await page.getByRole('link', { name: 'Meus projetos', exact: true }).click();
    await page.getByRole('link', { name: 'Escolher uma pasta' }).click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#project-root').inputValue(), '');
    assert.equal(await page.getByRole('region', { name: 'Histórico da conversa' }).isVisible(), true);
    assert.equal(await page.locator('#workspace').isVisible(), true);
    assert.equal(await page.locator('#project-record').isVisible(), false, 'Do not show a record for an unconfirmed folder');
    assert.equal(await page.locator('#home').isHidden(), true);
    assert.equal(await page.locator('nav a[data-route="workspace"]').getAttribute('aria-current'), 'page');
    console.log('PASS: home and workspace are distinct, reachable screens with current navigation.');
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.locator('#explore').waitFor({ state: 'visible' });
    assert.equal(await page.getByRole('group', { name: 'Temas para explorar' }).isVisible(), true);
    assert.equal(await page.locator('#home').isHidden(), true);
    assert.equal(await page.locator('#workspace').isHidden(), true);
    assert.equal(await page.locator('nav a[data-route="explore"]').getAttribute('aria-current'), 'page');
    assert.equal(await page.locator('.category-card').count(), 8);
    const ideaPanel = await page.locator('.open-idea').boundingBox();
    const ideaTitle = await page.locator('.open-idea h2').boundingBox();
    assert.ok(ideaTitle.x >= ideaPanel.x + 175, 'Decorative foliage must not cover the callout title');
    if (process.env.FORGE_EXPLORE_SCREENSHOT) {
      await page.screenshot({ path: process.env.FORGE_EXPLORE_SCREENSHOT, fullPage: true });
    }
    for (const width of [390, 1180]) {
      await page.setViewportSize({ width, height: 844 });
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
      assert.equal(await page.locator('.category-card:visible').count(), 8);
      if (width === 390 && process.env.FORGE_EXPLORE_MOBILE_SCREENSHOT) {
        await page.screenshot({ path: process.env.FORGE_EXPLORE_MOBILE_SCREENSHOT, fullPage: true });
      }
    }
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.getByRole('searchbox', { name: 'O que te interessa?' }).fill('musica');
    assert.equal(await page.locator('.category-card:visible').count(), 1);
    await page.getByRole('link', { name: /Música/ }).click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.match(await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).inputValue(), /música/i);
    // A browser may clear a search input while the screen is hidden, without firing input.
    await page.locator('#category-query').evaluate(node => { node.value = ''; });
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.getByRole('searchbox', { name: 'O que te interessa?' }).fill('');
    await page.waitForFunction(() => [...document.querySelectorAll('.category-card')].every(card => !card.hidden));
    assert.equal(await page.locator('.category-card:visible').count(), 8);
    await page.getByRole('link', { name: /Arte e criação/ }).click();
    const ideaDraft = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    assert.match(await ideaDraft.inputValue(), /artístico/i);
    await ideaDraft.fill('Quero desenhar um livro ilustrado para crianças.');
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.getByRole('link', { name: /Tecnologia/ }).click();
    assert.equal(await ideaDraft.inputValue(), 'Quero desenhar um livro ilustrado para crianças.');
    assert.match(await page.locator('#idea-selection-status').textContent(), /ideia escrita foi mantida/);
    await page.getByRole('link', { name: 'Como funciona' }).click();
    await page.locator('#home').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#idea-selection-status').isHidden(), true);
    assert.equal(await page.locator('#about').evaluate(node => node.open), true);
    assert.equal(await page.locator('#about .cards').isVisible(), true);
    await page.waitForFunction(() => document.activeElement === document.querySelector('#about summary'));
    await page.goto(`${url}#about`);
    assert.equal(await page.locator('#about').evaluate(node => node.open), true);
    await page.getByRole('link', { name: 'Minha conversa', exact: true }).click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await page.getByRole('button', { name: 'Continuar nesta pasta' }).isVisible(), true);
    if (process.env.FORGE_WORKSPACE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_WORKSPACE_SCREENSHOT, fullPage: true });
    console.log('PASS: Explore filters eight approachable themes and carries the chosen idea into the conversation.');
    const projectsPage = await browser.newPage();
    await projectsPage.addInitScript(() => {
      window.projectChecks = []; window.startCalls = []; window.previewReads = []; window.browserOpens = [];
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: '0.1.0' };
        if (command === 'choose_project_folder') {
          if (window.folderError) throw 'Não foi possível abrir a seleção de pastas.';
          return window.folderChoice ?? null;
        }
        if (command === 'inspect_project') {
          window.projectChecks.push(args.projectRoot);
          if (window.rejectProject) throw 'Este projeto não está disponível.';
          const root = args.projectRoot;
          return { project_id: root.endsWith('two') ? 'second-project' : 'first-project', project_root: root };
        }
        if (command === 'start_project') {
          window.startCalls.push(args.projectRoot);
          const root = args.projectRoot;
          return { project_id: root.endsWith('two') ? 'second-project' : root.endsWith('new') ? 'new-project' : 'first-project', project_root: root };
        }
        if (command === 'choose_preview_file') return window.previewChoice || null;
        if (command === 'open_site_in_browser') {
          window.browserOpens.push(args);
          if (window.rejectBrowserOpen) throw 'Navegador indisponível';
          return null;
        }
        if (command === 'inspect_preview') {
          window.previewReads.push(args);
          if (args.filePath.includes('outside')) throw 'Este arquivo não pertence ao projeto aberto.';
          return window.previewKind === 'image'
            ? { kind: 'image', relative_path: 'result.png', size_bytes: 100, content: window.previewImage }
            : window.previewKind === 'html'
              ? { kind: 'text', relative_path: 'site/index.html', size_bytes: 100, content: '<h1>Site local</h1>', render_url: 'http://forgepreview.localhost/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/index%2Ehtml' }
            : window.previewKind === 'markdown'
              ? { kind: 'text', relative_path: 'notes.md', size_bytes: 100, content: '# Resultado\n- **Item** seguro\n<script>não executar</script>\n[fora](https://outside.example/)' }
            : { kind: 'text', relative_path: 'result.txt', size_bytes: 16, content: window.previewContent || '<script>primeiro</script>' };
        }
      } } };
    });
    await projectsPage.goto(`${url}#projects`);
    if (process.env.FORGE_PROJECTS_EMPTY_SCREENSHOT) await projectsPage.screenshot({ path: process.env.FORGE_PROJECTS_EMPTY_SCREENSHOT, fullPage: true });
    assert.equal(await projectsPage.locator('#projects-empty').isVisible(), true);
    assert.equal(await projectsPage.locator('#projects-open-folder-label').textContent(), 'Escolher uma pasta');
    await projectsPage.getByRole('link', { name: 'Escolher uma pasta' }).click();
    await projectsPage.locator('#workspace').waitFor({ state: 'visible' });
    if (process.env.FORGE_EMPTY_FOLDER_SCREENSHOT) await projectsPage.screenshot({ path: process.env.FORGE_EMPTY_FOLDER_SCREENSHOT, fullPage: true });
    assert.equal(await projectsPage.locator('#project-root').getAttribute('placeholder'), 'Nenhuma pasta escolhida');
    assert.equal(await projectsPage.locator('#project-root').inputValue(), '', 'The empty-state label must not become a real project path');
    assert.equal(await projectsPage.getByRole('button', { name: 'Continuar nesta pasta' }).isVisible(), true);
    await projectsPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Escolha uma pasta' }).waitFor();
    assert.equal(await projectsPage.locator('#project-root').getAttribute('aria-invalid'), 'true');
    assert.equal(await projectsPage.evaluate(() => document.activeElement?.id), 'project-root');
    await projectsPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('   ');
    await projectsPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    assert.deepEqual(await projectsPage.evaluate(() => window.startCalls), [], 'Blank paths must not reach the native initializer');
    await projectsPage.evaluate(() => { window.folderChoice = 'D:\\one'; });
    await projectsPage.getByRole('button', { name: 'Escolher pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Pasta escolhida' }).waitFor();
    assert.equal(await projectsPage.locator('#project-root').inputValue(), 'D:\\one');
    assert.equal(await projectsPage.locator('#project-root').getAttribute('aria-invalid'), null);
    assert.deepEqual(await projectsPage.evaluate(() => window.projectChecks), []);
    await projectsPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await projectsPage.locator('#new-conversation-choice').isHidden(), true, 'A project without a known conversation must not offer another one');
    assert.equal(await projectsPage.locator('#resume-last-conversation').isHidden(), true, 'A project without a saved conversation must not offer a resume shortcut');
    assert.deepEqual(await projectsPage.evaluate(() => window.startCalls), ['D:\\one']);
    assert.equal(await projectsPage.locator('#project-setup').evaluate(node => node.open), false);
    assert.equal(await projectsPage.getByRole('heading', { name: 'Seu projeto' }).isVisible(), true);
    assert.equal(await projectsPage.locator('#project-step').textContent(), 'PROJETO EM USO');
    assert.equal(await projectsPage.locator('#workspace-title').textContent(), 'one');
    assert.equal(await projectsPage.locator('#project-name').textContent(), 'one');
    assert.equal(await projectsPage.locator('#confirmed-project-id').textContent(), 'first-project');
    assert.equal(await projectsPage.locator('#workspace-back').getAttribute('href'), '#projects');
    assert.equal(await projectsPage.locator('#workspace-back-label').textContent(), 'Voltar aos projetos');
    assert.equal(await projectsPage.locator('nav a[aria-current="page"]').textContent(), 'Minha conversa');
    assert.equal(await projectsPage.evaluate(() => document.activeElement?.id), 'message-text');
    assert.equal(await projectsPage.locator('#project-location').evaluate(node => node.open), false);
    assert.equal(await projectsPage.locator('#confirmed-root').isVisible(), false);
    await projectsPage.locator('#project-location summary').click();
    assert.ok(await projectsPage.locator('#project-location summary').evaluate(node => node.getBoundingClientRect().height >= 48), 'Confirmed-folder disclosure keeps a 48px target');
    assert.equal(await projectsPage.locator('#confirmed-root').textContent(), 'D:\\one');
    assert.equal(await projectsPage.locator('#confirmed-root').isVisible(), true);
    await projectsPage.locator('#project-location summary').click();
    assert.equal(await projectsPage.locator('#project-preview').isVisible(), true);
    assert.equal(await projectsPage.locator('.preview-empty').isVisible(), true, 'An actual empty state precedes any local result');
    assert.equal(await projectsPage.locator('#preview-status').isVisible(), false, 'The untouched empty state does not repeat the same message');
    assert.equal(await projectsPage.evaluate(() => document.querySelector('#project-record').getBoundingClientRect().top < document.querySelector('.preview').getBoundingClientRect().top), true, 'The real record precedes an empty preview');
    assert.deepEqual(await projectsPage.locator('.workspace > .panel').evaluateAll(nodes => nodes.map(node => node.id || (node.classList.contains('conversation') ? 'conversation' : 'project'))), ['conversation', 'project-record', 'project-preview', 'project']);
    assert.equal(await projectsPage.locator('#refresh-preview').isVisible(), false, 'No refresh action before choosing a file');
    assert.equal(await projectsPage.locator('#project-record').isVisible(), true);
    assert.equal(await projectsPage.locator('.project #project-record').count(), 0, 'Record is a separate panel, not folder setup');
    await projectsPage.evaluate(() => { window.previewChoice = 'D:\\one\\result.txt'; });
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await projectsPage.evaluate(() => document.activeElement?.id), 'preview-heading', 'Moving the preview above the record gives keyboard focus to its result heading');
    assert.deepEqual(await projectsPage.locator('.workspace > .panel').evaluateAll(nodes => nodes.map(node => node.id || (node.classList.contains('conversation') ? 'conversation' : 'project'))), ['conversation', 'project-preview', 'project-record', 'project']);
    assert.equal(await projectsPage.locator('#refresh-preview').isVisible(), true);
    assert.equal(await projectsPage.locator('.workspace').evaluate(node => node.classList.contains('preview-loaded')), true);
    assert.equal(await projectsPage.evaluate(() => {
      const preview = document.querySelector('#project-preview');
      const heading = preview.querySelector('.preview-heading-row');
      const result = preview.querySelector('#preview-result');
      const tools = preview.querySelector('.preview-actions');
      return heading.compareDocumentPosition(result) & Node.DOCUMENT_POSITION_FOLLOWING
        ? !!(result.compareDocumentPosition(tools) & Node.DOCUMENT_POSITION_FOLLOWING)
        : false;
    }), true, 'The result stays ahead of file tools in reading and keyboard order');
    assert.equal(await projectsPage.locator('.preview-intro').isVisible(), false, 'Loaded preview omits repeated setup guidance');
    assert.equal(await projectsPage.locator('.preview-empty').isVisible(), false);
    assert.equal(await projectsPage.evaluate(() => document.querySelector('.preview').getBoundingClientRect().top < document.querySelector('.project').getBoundingClientRect().top), true);
    assert.equal(await projectsPage.evaluate(() => document.querySelector('#project-record').getBoundingClientRect().bottom < document.querySelector('.project').getBoundingClientRect().top), true);
    assert.equal(await projectsPage.locator('#preview-text').textContent(), '<script>primeiro</script>');
    assert.equal(await projectsPage.locator('#preview-result script').count(), 0);
    assert.match(await projectsPage.locator('#preview-result').textContent(), /Publicação não verificada/);
    await projectsPage.getByRole('button', { name: 'Abrir prévia' }).click();
    assert.equal(await projectsPage.locator('#preview-dialog').isVisible(), true);
    assert.equal(await projectsPage.locator('#preview-dialog-site-note').isVisible(), false, 'A text file must not show site-only limitations');
    assert.equal(await projectsPage.locator('#preview-dialog-text').textContent(), '<script>primeiro</script>');
    assert.equal(await projectsPage.locator('#preview-dialog script').count(), 0);
    await projectsPage.keyboard.press('Escape');
    assert.equal(await projectsPage.locator('#preview-dialog').isVisible(), false);
    await projectsPage.evaluate(() => {
      window.previewRequestSubmits = 0;
      document.getElementById('message-form').addEventListener('submit', () => { window.previewRequestSubmits++; });
    });
    await projectsPage.locator('#message-text').fill('Mantenha a paleta atual.');
    await projectsPage.getByRole('button', { name: 'Abrir prévia' }).click();
    await projectsPage.getByRole('button', { name: 'Pedir mudança na conversa' }).click();
    assert.equal(await projectsPage.locator('#preview-dialog').isVisible(), false);
    assert.equal(await projectsPage.locator('#message-text').inputValue(), 'Mantenha a paleta atual.\nQuero mudar o arquivo result.txt: ');
    await projectsPage.locator('#message-text').fill('Mantenha a paleta atual.');
    await projectsPage.getByRole('button', { name: 'Pedir mudança neste arquivo' }).click();
    assert.equal(await projectsPage.locator('#message-text').inputValue(), 'Mantenha a paleta atual.\nQuero mudar o arquivo result.txt: ');
    assert.equal(await projectsPage.evaluate(() => document.activeElement?.id), 'message-text');
    assert.equal(await projectsPage.evaluate(() => window.previewRequestSubmits), 0);
    if (process.env.FORGE_PREVIEW_SCREENSHOT) await projectsPage.screenshot({ path: process.env.FORGE_PREVIEW_SCREENSHOT, fullPage: true });
    assert.equal(await projectsPage.locator('.workspace-screen .screen-heading .intro').isVisible(), false, 'Confirmed project omits repeated setup explanation');
    assert.equal(await projectsPage.evaluate(() => {
      const conversation = document.querySelector('.workspace.project-ready .conversation').getBoundingClientRect();
      const composer = document.querySelector('#message-form').getBoundingClientRect();
      return composer.bottom <= conversation.bottom && composer.left >= conversation.left && composer.right <= conversation.right;
    }), true, 'Composer must remain inside the conversation card');
    await projectsPage.evaluate(() => { window.previewContent = 'Atualizado'; });
    await projectsPage.getByRole('button', { name: 'Atualizar prévia' }).click();
    assert.equal(await projectsPage.locator('#preview-text').textContent(), 'Atualizado');
    await projectsPage.evaluate(() => { window.previewKind = 'markdown'; window.previewChoice = 'D:\\one\\notes.md'; });
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await projectsPage.locator('#preview-markdown h3').textContent(), 'Resultado');
    assert.equal(await projectsPage.locator('#preview-markdown li strong').textContent(), 'Item');
    assert.equal(await projectsPage.locator('#preview-markdown script').count(), 0);
    assert.equal(await projectsPage.locator('#preview-markdown a').count(), 0);
    assert.match(await projectsPage.locator('#preview-markdown').textContent(), /\[fora\]\(https:\/\/outside\.example\/\)/);
    assert.equal(await projectsPage.locator('#preview-text').isVisible(), false);
    await projectsPage.getByRole('button', { name: 'Ver texto original' }).click();
    assert.match(await projectsPage.locator('#preview-text').textContent(), /^# Resultado/);
    assert.equal(await projectsPage.locator('#preview-markdown').isVisible(), false);
    await projectsPage.getByRole('button', { name: 'Abrir prévia' }).click();
    assert.equal(await projectsPage.locator('#preview-dialog-text').isVisible(), true);
    await projectsPage.getByRole('button', { name: 'Ver leitura' }).last().click();
    assert.equal(await projectsPage.locator('#preview-dialog-markdown h3').textContent(), 'Resultado');
    assert.equal(await projectsPage.locator('#preview-dialog script').count(), 0);
    await projectsPage.getByRole('button', { name: 'Fechar prévia' }).click();
    await projectsPage.evaluate(() => { window.previewChoice = null; });
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    assert.match(await projectsPage.locator('#preview-text').textContent(), /^# Resultado/);
    assert.equal(await projectsPage.locator('.workspace').evaluate(node => node.classList.contains('preview-loaded')), true);
    const imageFixture = `data:image/png;base64,${(await readFile(path.join(__dirname, '..', 'ui', 'assets', 'forge.png'))).toString('base64')}`;
    await projectsPage.evaluate(image => { window.previewKind = 'image'; window.previewImage = image; window.previewChoice = 'D:\\one\\result.png'; }, imageFixture);
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    await projectsPage.waitForFunction(() => document.querySelector('#preview-image').naturalWidth > 0);
    assert.equal(await projectsPage.locator('#preview-text').isVisible(), false);
    assert.equal(await projectsPage.locator('#preview-site-note').isVisible(), false);
    assert.equal(await projectsPage.locator('#preview-browser-action').isVisible(), false);
    await projectsPage.getByRole('button', { name: 'Abrir prévia' }).click();
    await projectsPage.waitForFunction(() => document.querySelector('#preview-dialog-image').naturalWidth > 0);
    assert.equal(await projectsPage.locator('#preview-dialog-image').isVisible(), true);
    assert.equal(await projectsPage.locator('#preview-dialog-site-note').isVisible(), false, 'An image must not show site-only limitations');
    await projectsPage.getByRole('button', { name: 'Fechar prévia' }).click();
    assert.equal(await projectsPage.locator('#preview-dialog').isVisible(), false);
    let externalPreviewRequests = 0;
    await projectsPage.route('https://outside.example/**', route => { externalPreviewRequests++; return route.abort(); });
    await projectsPage.route('http://forgepreview.localhost/**', route => route.fulfill({
      contentType: 'text/html',
      headers: { 'Content-Security-Policy': "default-src 'none'; script-src 'none'; connect-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; frame-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'; sandbox" },
      body: '<!doctype html><style>h1{color:rgb(11, 80, 34)}</style><h1>Site local</h1><script>parent.previewEscaped=true</script><img src="https://outside.example/tracker">',
    }));
    await projectsPage.evaluate(() => { window.previewKind = 'html'; window.previewChoice = 'D:\\one\\site\\index.html'; });
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await projectsPage.locator('#preview-site').isVisible(), true);
    assert.equal(await projectsPage.locator('#preview-browser-action').isVisible(), true);
    assert.match(await projectsPage.locator('#preview-browser-action .hint').textContent(), /pode executar código e acessar a internet/);
    assert.deepEqual(await projectsPage.evaluate(() => window.browserOpens), [], 'A protected preview must never open the browser automatically');
    await projectsPage.getByRole('button', { name: 'Usar no navegador' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Abertura solicitada ao navegador padrão' }).waitFor();
    assert.deepEqual(await projectsPage.evaluate(() => window.browserOpens), [{ projectRoot: 'D:\\one', filePath: 'D:\\one\\site\\index.html' }]);
    await projectsPage.evaluate(() => { window.rejectBrowserOpen = true; });
    await projectsPage.getByRole('button', { name: 'Usar no navegador' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Não foi possível abrir esta página' }).waitFor();
    await projectsPage.evaluate(() => { window.rejectBrowserOpen = false; });
    assert.equal(await projectsPage.locator('#preview-site-note').isVisible(), true);
    assert.equal(await projectsPage.locator('#preview-site-note').evaluate(node => node.open), false);
    await projectsPage.locator('#preview-site-note summary').click();
    assert.equal(await projectsPage.locator('#preview-site-note').evaluate(node => node.open), true);
    assert.match(await projectsPage.locator('#preview-site-note p').textContent(), /imagens da internet/);
    await projectsPage.frameLocator('#preview-site').getByRole('heading', { name: 'Site local' }).waitFor();
    if (process.env.FORGE_HTML_SCREENSHOT) await projectsPage.screenshot({ path: process.env.FORGE_HTML_SCREENSHOT, fullPage: true });
    assert.equal(await projectsPage.frameLocator('#preview-site').locator('h1').evaluate(node => getComputedStyle(node).color), 'rgb(11, 80, 34)');
    assert.equal(await projectsPage.evaluate(() => window.previewEscaped), undefined);
    assert.equal(externalPreviewRequests, 0);
    await projectsPage.getByRole('button', { name: 'Ver código' }).click();
    assert.equal(await projectsPage.locator('#preview-site').isVisible(), false);
    assert.equal(await projectsPage.locator('#preview-text').textContent(), '<h1>Site local</h1>');
    await projectsPage.getByRole('button', { name: 'Abrir prévia' }).click();
    assert.equal(await projectsPage.locator('#preview-dialog-text').isVisible(), true);
    assert.equal(await projectsPage.locator('#preview-dialog-site-note').isVisible(), true, 'A site must explain its restricted local preview');
    await projectsPage.getByRole('button', { name: 'Ver prévia visual' }).last().click();
    assert.equal(await projectsPage.locator('#preview-dialog-site').isVisible(), true);
    assert.equal(await projectsPage.locator('#preview-dialog-more').isVisible(), true);
    assert.equal(await projectsPage.locator('#preview-dialog-site').evaluate(node => getComputedStyle(node).pointerEvents), 'none');
    await projectsPage.getByRole('button', { name: 'Mostrar mais da página' }).click();
    assert.equal(await projectsPage.locator('#preview-dialog-site').evaluate(node => node.style.height), '1360px');
    await projectsPage.getByRole('button', { name: 'Ver código' }).last().click();
    assert.equal(await projectsPage.locator('#preview-dialog-more').isVisible(), false);
    await projectsPage.getByRole('button', { name: 'Fechar prévia' }).click();
    await projectsPage.setViewportSize({ width: 390, height: 844 });
    assert.equal(await projectsPage.evaluate(() => {
      const project = document.querySelector('.project').getBoundingClientRect();
      const chat = document.querySelector('.conversation').getBoundingClientRect();
      const preview = document.querySelector('.preview').getBoundingClientRect();
      const record = document.querySelector('#project-record').getBoundingClientRect();
      return chat.bottom < preview.top && preview.bottom < record.top && record.bottom < project.top;
    }), true);
    assert.deepEqual(await projectsPage.locator('.workspace > .panel').evaluateAll(nodes => nodes.map(node => node.id || (node.classList.contains('conversation') ? 'conversation' : 'project'))), ['conversation', 'project-preview', 'project-record', 'project']);
    assert.equal(await projectsPage.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    await projectsPage.setViewportSize({ width: 1280, height: 720 });
    assert.equal(await projectsPage.evaluate(() => {
      const chat = document.querySelector('.conversation').getBoundingClientRect();
      const body = document.querySelector('.conversation-body').getBoundingClientRect();
      const invitation = document.querySelector('.empty-conversation p').getBoundingClientRect();
      const send = document.getElementById('send-message').getBoundingClientRect();
      return invitation.bottom <= body.bottom - 2 && body.height >= 140 && send.bottom <= chat.bottom - 12;
    }), true, 'A multiline draft must not clip the conversation invitation or Send at 1280x720');
    assert.equal(await projectsPage.evaluate(() => {
      const preview = document.querySelector('.preview').getBoundingClientRect();
      const record = document.querySelector('#project-record').getBoundingClientRect();
      const project = document.querySelector('.project').getBoundingClientRect();
      return preview.bottom < record.top && record.bottom < project.top;
    }), true, 'The record precedes secondary folder controls beside the conversation');
    await projectsPage.evaluate(() => { window.previewChoice = 'D:\\outside.txt'; });
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'não pertence ao projeto' }).waitFor();
    assert.equal(await projectsPage.locator('#preview-result').isVisible(), false);
    assert.equal(await projectsPage.locator('#open-preview').isHidden(), true);
    assert.equal(await projectsPage.locator('.workspace').evaluate(node => node.classList.contains('preview-loaded')), false);
    assert.deepEqual(await projectsPage.evaluate(() => window.previewReads.map(read => read.projectRoot)), ['D:\\one', 'D:\\one', 'D:\\one', 'D:\\one', 'D:\\one', 'D:\\one']);
    if (process.env.FORGE_CONFIRMED_SCREENSHOT) await projectsPage.screenshot({ path: process.env.FORGE_CONFIRMED_SCREENSHOT, fullPage: true });
    await openProjectSetup(projectsPage);
    await projectsPage.evaluate(() => { window.folderChoice = null; });
    await projectsPage.getByRole('button', { name: 'Escolher pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Seleção cancelada' }).waitFor();
    assert.equal(await projectsPage.locator('#project-root').inputValue(), 'D:\\one');
    assert.equal(await projectsPage.locator('#project-result').isVisible(), true);
    await projectsPage.evaluate(() => { window.folderError = true; });
    await projectsPage.getByRole('button', { name: 'Escolher pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Não foi possível abrir' }).waitFor();
    assert.equal(await projectsPage.locator('#project-root').inputValue(), 'D:\\one');
    await projectsPage.evaluate(() => { window.folderError = false; });
    await projectsPage.locator('#project-setup summary').click();
    await projectsPage.getByRole('link', { name: 'Meus projetos' }).click();
    assert.equal(await projectsPage.locator('.recent-project').count(), 1);
    assert.equal(await projectsPage.locator('.recent-project h2').textContent(), 'one');
    assert.equal(await projectsPage.locator('.recent-project p').textContent(), 'D:\\one');
    assert.equal(await projectsPage.locator('#projects-empty').isHidden(), true);
    await projectsPage.locator('nav a[data-route="workspace"]').click();
    assert.equal(await projectsPage.locator('#project-setup').evaluate(node => node.open), false);
    await projectsPage.getByRole('link', { name: 'Meus projetos' }).click();
    if (process.env.FORGE_PROJECTS_SCREENSHOT) {
      await projectsPage.screenshot({ path: process.env.FORGE_PROJECTS_SCREENSHOT, fullPage: true });
    }
    await projectsPage.reload();
    assert.equal(await projectsPage.locator('.recent-project').count(), 1);
    await projectsPage.getByRole('button', { name: 'Abrir one na pasta D:\\one' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.match(await projectsPage.locator('#project-status').textContent(), /começar ou continuar a conversa; nada foi enviado/);
    assert.deepEqual(await projectsPage.evaluate(() => window.projectChecks), ['D:\\one']);
    await openProjectSetup(projectsPage);
    await projectsPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\two');
    assert.equal(await projectsPage.locator('#project-preview').isVisible(), false);
    assert.equal(await projectsPage.locator('#workspace-title').textContent(), 'Vamos dar vida à sua ideia.');
    assert.equal(await projectsPage.locator('#workspace-back').getAttribute('href'), '#explore');
    await projectsPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await projectsPage.locator('#workspace-title').textContent(), 'two');
    assert.equal(await projectsPage.locator('#project-name').textContent(), 'two');
    assert.equal(await projectsPage.locator('#confirmed-project-id').textContent(), 'second-project');
    await projectsPage.getByRole('link', { name: 'Meus projetos' }).click();
    assert.equal(await projectsPage.locator('.recent-project').count(), 2);
    assert.equal(await projectsPage.locator('#projects-open-folder-label').textContent(), 'Abrir outro projeto');
    await projectsPage.locator('#project-root').evaluate(input => { input.disabled = true; });
    await projectsPage.getByRole('button', { name: 'Abrir one na pasta D:\\one' }).click();
    await projectsPage.locator('#projects-status').filter({ hasText: 'Não foi possível trocar' }).waitFor();
    assert.equal(await projectsPage.locator('#project-root').inputValue(), 'D:\\two');
    await projectsPage.locator('#project-root').evaluate(input => { input.disabled = false; });
    await projectsPage.evaluate(() => { window.rejectProject = true; });
    await projectsPage.getByRole('button', { name: 'Abrir one na pasta D:\\one' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'não está disponível' }).waitFor();
    assert.equal(await projectsPage.locator('#project-result').isHidden(), true);
    assert.equal(await projectsPage.locator('#project-setup').evaluate(node => node.open), true);
    assert.equal(await projectsPage.locator('#workspace-title').textContent(), 'Vamos dar vida à sua ideia.');
    assert.equal(await projectsPage.locator('#workspace-back').getAttribute('href'), '#explore');
    assert.equal(await projectsPage.locator('#connect-agent').isDisabled(), true);
    await projectsPage.getByRole('link', { name: 'Meus projetos' }).click();
    await projectsPage.getByRole('button', { name: 'Remover one da lista de projetos' }).click();
    await projectsPage.getByRole('button', { name: 'Remover two da lista de projetos' }).click();
    assert.equal(await projectsPage.locator('#projects-empty').isVisible(), true);
    assert.equal(await projectsPage.locator('#projects-open-folder-label').textContent(), 'Escolher uma pasta');
    await projectsPage.evaluate(() => { Storage.prototype.setItem = () => { throw new Error('Unavailable'); }; window.rejectProject = false; });
    await projectsPage.getByRole('link', { name: 'Escolher uma pasta' }).click();
    assert.equal(await projectsPage.locator('#project-setup').evaluate(node => node.open), true);
    await projectsPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\three');
    await projectsPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    await projectsPage.getByRole('link', { name: 'Meus projetos' }).click();
    assert.equal(await projectsPage.locator('.recent-project').count(), 1);
    await projectsPage.locator('#projects-status').filter({ hasText: 'Não foi possível guardar' }).waitFor();
    for (const width of [390, 1180]) {
      await projectsPage.setViewportSize({ width, height: 844 });
      assert.equal(await projectsPage.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    }
    await projectsPage.getByRole('link', { name: 'Abrir outro projeto' }).click();
    await projectsPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\new');
    await projectsPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await projectsPage.locator('#project-result-label').textContent(), 'PROJETO PRONTO');
    assert.equal(await projectsPage.locator('#conversation-step').textContent(), 'SUA CONVERSA');
    assert.deepEqual(await projectsPage.evaluate(() => window.startCalls), ['D:\\two', 'D:\\three', 'D:\\new']);
    assert.equal(await projectsPage.locator('#connect-agent').isEnabled(), true);
    assert.equal(await projectsPage.locator('#message-text').isEnabled(), true);
    assert.equal(await projectsPage.locator('#send-message').isEnabled(), true);
    assert.equal(await projectsPage.locator('#agent-access-note').isVisible(), true);
    assert.match(await projectsPage.locator('#agent-access-note').textContent(), /fora da pasta escolhida.*sem pedir confirmação|sem pedir confirmação.*fora da pasta escolhida/);
    await projectsPage.locator('#conversation-picker summary').click();
    assert.equal(await projectsPage.locator('#connect-help').isVisible(), true);
    const composerSize = await projectsPage.locator('#message-text').evaluate(node => node.getBoundingClientRect().height);
    await projectsPage.locator('#message-text').fill('Uma ideia com detalhes.\n'.repeat(10));
    assert.ok(await projectsPage.locator('#message-text').evaluate(node => node.getBoundingClientRect().height) > composerSize, 'Long drafts should expand the composer before scrolling');
    await projectsPage.locator('#message-text').fill('');
    await openProjectSetup(projectsPage);
    const longFolder = 'projeto-' + 'criacao'.repeat(32);
    await projectsPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill(`D:\\${longFolder}`);
    await projectsPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    await projectsPage.setViewportSize({ width: 390, height: 844 });
    assert.equal(await projectsPage.locator('#workspace-title').textContent(), longFolder);
    assert.equal(await projectsPage.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    const startsBeforeNewIdea = (await projectsPage.evaluate(() => window.startCalls)).length;
    await projectsPage.getByRole('link', { name: 'Explorar', exact: true }).click();
    await projectsPage.getByRole('link', { name: /Arte e criação/ }).click();
    await projectsPage.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await projectsPage.locator('#project-result').isHidden(), true, 'A new Explore idea must not silently reuse the previous project');
    assert.equal(await projectsPage.locator('#project-root').inputValue(), '');
    assert.equal(await projectsPage.locator('#project-setup').evaluate(node => node.open), true);
    assert.match(await projectsPage.locator('#message-text').inputValue(), /artístico/i);
    assert.equal(await projectsPage.locator('#send-message').isDisabled(), true);
    assert.equal((await projectsPage.evaluate(() => window.startCalls)).length, startsBeforeNewIdea, 'Exploring an idea must not create a project');
    await projectsPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\home-fixture');
    await projectsPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    await projectsPage.getByRole('link', { name: 'Início', exact: true }).click();
    await projectsPage.getByRole('link', { name: 'Conversar sobre uma ideia' }).click();
    await projectsPage.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await projectsPage.locator('#project-root').inputValue(), '', 'The Home new-idea action also needs a fresh project choice');
    assert.equal(await projectsPage.locator('#project-result').isHidden(), true);
    assert.equal(await projectsPage.locator('#send-message').isDisabled(), true);
    await projectsPage.close();
    console.log('PASS: My Projects empty state, verified shortcuts, reload, revalidation, unavailable project, removal, storage failure and narrow layout.');
    const manyProjectsPage = await browser.newPage();
    await manyProjectsPage.addInitScript(() => {
      const projects = Array.from({ length: 60 }, (_, index) => ({ project_id: `project-${index}`, project_root: `D:\\work\\Project-${index}` }));
      projects[3] = { project_id: 'cafe', project_root: 'D:\\Ideias\\Café' };
      projects[4] = { project_id: 'shared-a', project_root: 'D:\\first\\Shared' };
      projects[5] = { project_id: 'shared-b', project_root: 'D:\\second\\Shared' };
      localStorage.setItem('forge.projects.v1', JSON.stringify(projects));
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: '0.1.22' };
        if (command === 'inspect_project') return { project_id: 'revalidated', project_root: args.projectRoot };
        if (command === 'inspect_progress') return { status: 'absent', phase: '1-discovery', focus: null, accepted_direction: null, recorded_pending_count: 0, suggested_questions: [] };
      } } };
    });
    await manyProjectsPage.goto(`${url}#projects`);
    assert.equal(await manyProjectsPage.locator('.recent-project').count(), 50, 'Keep more useful recent shortcuts without claiming an unlimited registry');
    assert.equal(await manyProjectsPage.locator('#projects-filter-box').isVisible(), true);
    await manyProjectsPage.getByRole('searchbox', { name: 'Encontrar um projeto' }).fill('CAFE');
    assert.equal(await manyProjectsPage.locator('.recent-project').count(), 1);
    assert.equal(await manyProjectsPage.locator('#projects-filter-status').textContent(), '1 projeto encontrado nesta lista.');
    assert.match(await manyProjectsPage.locator('.recent-project').textContent(), /Café/);
    await manyProjectsPage.getByRole('searchbox', { name: 'Encontrar um projeto' }).fill('shared');
    assert.equal(await manyProjectsPage.locator('.recent-project').count(), 2);
    assert.match(await manyProjectsPage.locator('#recent-projects').textContent(), /D:\\first\\Shared/);
    assert.match(await manyProjectsPage.locator('#recent-projects').textContent(), /D:\\second\\Shared/);
    await manyProjectsPage.getByRole('searchbox', { name: 'Encontrar um projeto' }).fill('not-found');
    assert.equal(await manyProjectsPage.locator('.recent-project').count(), 0);
    assert.equal(await manyProjectsPage.locator('#projects-filter-status').textContent(), '0 projetos encontrados nesta lista.');
    assert.equal(await manyProjectsPage.locator('#projects-no-results').isVisible(), true);
    await manyProjectsPage.getByRole('searchbox', { name: 'Encontrar um projeto' }).fill('CAFE');
    await manyProjectsPage.setViewportSize({ width: 390, height: 844 });
    assert.equal(await manyProjectsPage.evaluate(() => document.documentElement.scrollWidth > innerWidth), false, 'Project search must not overflow a narrow window');
    assert.equal(await manyProjectsPage.getByRole('searchbox', { name: 'Encontrar um projeto' }).isVisible(), true);
    await manyProjectsPage.getByRole('button', { name: 'Abrir Café na pasta D:\\Ideias\\Café' }).click();
    await manyProjectsPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await manyProjectsPage.locator('#confirmed-root').textContent(), 'D:\\Ideias\\Café');
    assert.equal(await manyProjectsPage.locator('#project-filter').inputValue(), '', 'Opening a project clears an old shortcut filter');
    assert.equal(await manyProjectsPage.evaluate(() => JSON.parse(localStorage.getItem('forge.projects.v1')).length), 50);
    await manyProjectsPage.close();
    console.log('PASS: 50 recent shortcuts, accent-insensitive search, distinct same-name paths, empty search and revalidated opening.');
    const conversationListPage = await browser.newPage();
    await conversationListPage.addInitScript(() => {
      window.listCalls = []; window.connectCalls = []; window.sendCalls = 0; window.failSelection = true; window.failPage = false;
      window.__TAURI__ = { core: {
        Channel: class {},
        invoke: async (command, args) => {
          if (command === 'app_info') return { name: 'Forge', version: '0.1.1' };
          if (command === 'start_project') return { project_id: 'list-project', project_root: args.projectRoot };
          if (command === 'list_conversations') {
            window.listCalls.push(args);
            if (args.cursor && window.failPage) throw 'Página indisponível no teste.';
            return args.cursor
              ? { conversations: [
                { id: 'older', title: '<script>literal</script>', updated_at: 100, active: false },
                { id: 'oldest', title: 'Outra conversa', updated_at: 90, active: false },
              ], next_cursor: null }
              : { conversations: [
                { id: 'busy', title: 'Resposta em andamento', updated_at: 120, active: true },
                { id: 'older', title: '<script>literal</script>', updated_at: 100, active: false },
              ], next_cursor: 'next-page' };
          }
          if (command === 'connect_agent') {
            window.connectCalls.push(args.threadId);
            if (window.failSelection) throw 'A conversa não abriu no teste.';
            return { thread_id: args.threadId, resumed: true, messages: [{ id: 'old-user', role: 'user', text: 'Texto anterior', incomplete: false }] };
          }
          if (command === 'disconnect_agent') return;
          if (command === 'send_message') window.sendCalls++;
        },
      } };
    });
    await conversationListPage.goto(workspaceUrl);
    assert.equal(await conversationListPage.locator('#conversation-picker').isHidden(), true);
    await conversationListPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\list-project');
    await conversationListPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await conversationListPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    await conversationListPage.locator('#conversation-picker summary').click();
    await conversationListPage.getByRole('button', { name: 'Buscar conversas' }).click();
    await conversationListPage.locator('.conversation-choice').first().waitFor();
    assert.equal(await conversationListPage.locator('.conversation-choice').count(), 2);
    assert.equal(await conversationListPage.getByRole('button', { name: 'Em andamento: Resposta em andamento' }).isDisabled(), true);
    assert.equal(await conversationListPage.locator('#conversation-list script').count(), 0);
    await conversationListPage.waitForFunction(() => getComputedStyle(document.querySelector('.conversation-choice-description strong')).webkitLineClamp === '2');
    await conversationListPage.setViewportSize({ width: 390, height: 844 });
    assert.equal(await conversationListPage.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    await conversationListPage.setViewportSize({ width: 1180, height: 844 });
    await conversationListPage.getByRole('button', { name: 'Retomar: <script>literal</script>' }).click();
    await conversationListPage.locator('#agent-status').filter({ hasText: 'A conversa não abriu' }).waitFor();
    assert.deepEqual(await conversationListPage.evaluate(() => window.connectCalls), ['older']);
    assert.equal(await conversationListPage.evaluate(() => localStorage.getItem('forge.conversation.v1:["list-project","D:\\\\list-project"]')), null);
    await conversationListPage.locator('#conversation-picker summary').click();
    await conversationListPage.evaluate(() => { window.failPage = true; });
    await conversationListPage.getByRole('button', { name: 'Próxima página' }).click();
    await conversationListPage.locator('#conversation-list-status').filter({ hasText: 'Página indisponível' }).waitFor();
    assert.equal(await conversationListPage.locator(':focus').getAttribute('id'), 'conversation-list-status');
    assert.equal(await conversationListPage.locator('.conversation-choice').count(), 2);
    assert.equal(await conversationListPage.getByRole('button', { name: 'Em andamento: Resposta em andamento' }).count(), 1);
    await conversationListPage.evaluate(() => { window.failPage = false; });
    await conversationListPage.getByRole('button', { name: 'Próxima página' }).click();
    await conversationListPage.locator('#conversation-list-status').filter({ hasText: 'Página 2' }).waitFor();
    assert.equal(await conversationListPage.locator('.conversation-choice').count(), 2);
    assert.equal(await conversationListPage.getByRole('button', { name: 'Em andamento: Resposta em andamento' }).count(), 0);
    await conversationListPage.getByRole('button', { name: 'Página anterior' }).click();
    await conversationListPage.locator('#conversation-list-status').filter({ hasText: 'Página 1' }).waitFor();
    assert.equal(await conversationListPage.getByRole('button', { name: 'Em andamento: Resposta em andamento' }).count(), 1);
    await conversationListPage.getByRole('button', { name: 'Próxima página' }).click();
    await conversationListPage.locator('#conversation-list-status').filter({ hasText: 'Página 2' }).waitFor();
    assert.deepEqual(await conversationListPage.evaluate(() => window.listCalls.map(call => call.cursor)), [null, 'next-page', 'next-page', null, 'next-page']);
    await conversationListPage.evaluate(() => { window.failSelection = false; });
    await conversationListPage.getByRole('button', { name: 'Retomar: Outra conversa' }).click();
    await conversationListPage.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await conversationListPage.locator(':focus').getAttribute('id'), 'agent-status');
    assert.deepEqual(await conversationListPage.evaluate(() => window.connectCalls), ['older', 'oldest']);
    assert.equal(await conversationListPage.locator('#messages article').count(), 1);
    assert.equal(await conversationListPage.evaluate(() => window.sendCalls), 0);
    assert.equal(await conversationListPage.locator('#conversation-picker').isHidden(), true);
    await conversationListPage.close();
    console.log('PASS: Codex-owned conversation list, active exclusion, safe titles, pagination, failed choice and explicit resume without sending.');
    const pendingPage = await browser.newPage();
    await pendingPage.addInitScript(() => {
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: '0.1.1' };
        if (command === 'start_project') return new Promise(resolve => { window.finishProjectLookup = () => resolve({ project_id: 'pending-project', project_root: args.projectRoot }); });
      } } };
    });
    await pendingPage.goto(`${url}#workspace`);
    await pendingPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\pending');
    await pendingPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    assert.equal(await pendingPage.locator('#project-root').isDisabled(), true);
    assert.equal(await pendingPage.locator('#start-project').isDisabled(), true);
    assert.match(await pendingPage.locator('#project-status').textContent(), /Na primeira vez, isso pode levar alguns instantes/);
    await pendingPage.evaluate(() => window.finishProjectLookup());
    await pendingPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    await pendingPage.close();
    console.log('PASS: pending project lookup keeps folder and project action locked until readback.');
    const sendFirstPage = await browser.newPage();
    await sendFirstPage.addInitScript(() => {
      window.connectCalls = 0;
      window.sendCalls = 0;
      window.connectFailure = true;
      window.__TAURI__ = { core: {
        Channel: class {},
        invoke: async (command, args) => {
          if (command === 'app_info') return { name: 'Forge', version: '0.1.1' };
          if (command === 'start_project') return { project_id: 'send-first', project_root: args.projectRoot };
          if (command === 'connect_agent') {
            window.connectCalls++;
            if (window.connectFailure) throw 'Conexão indisponível no teste';
            return { thread_id: 'send-first-thread', messages: [], resumed: false };
          }
          if (command === 'send_message') window.sendCalls++;
        },
      } };
    });
    await sendFirstPage.goto(workspaceUrl);
    assert.equal(await sendFirstPage.locator('#send-message').isDisabled(), true);
    await sendFirstPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\send-first');
    await sendFirstPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await sendFirstPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.match(await sendFirstPage.locator('#agent-status').textContent(), /Projeto pronto/);
    const firstDraft = sendFirstPage.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    await firstDraft.fill('Minha primeira ideia');
    await sendFirstPage.getByRole('button', { name: 'Enviar', exact: true }).click();
    await sendFirstPage.locator('#agent-status').filter({ hasText: 'Conexão indisponível no teste' }).waitFor();
    assert.equal(await firstDraft.inputValue(), 'Minha primeira ideia');
    assert.equal(await sendFirstPage.evaluate(() => window.sendCalls), 0);
    assert.equal(await sendFirstPage.locator('#send-message').isEnabled(), true);
    await sendFirstPage.evaluate(() => { window.connectFailure = false; });
    await sendFirstPage.getByRole('button', { name: 'Enviar', exact: true }).click();
    await sendFirstPage.waitForFunction(() => window.sendCalls === 1);
    assert.equal(await sendFirstPage.evaluate(() => window.connectCalls), 2);
    assert.equal(await sendFirstPage.locator('#messages article[data-role="user"]').count(), 1);
    assert.equal(await firstDraft.inputValue(), '');
    await sendFirstPage.close();
    console.log('PASS: first Send opens Codex once, preserves the draft on connection failure, then sends exactly once on retry.');
    const loginPage = await browser.newPage();
    await loginPage.addInitScript(() => {
      window.authFinished = false; window.sentAfterLogin = 0;
      window.__TAURI__ = { core: {
        Channel: class { onmessage = null; },
        invoke: async (command, args) => {
          if (command === 'app_info') return { name: 'Forge', version: '0.1.14' };
          if (command === 'start_project') return { project_id: 'login-project', project_root: args.projectRoot };
          if (command === 'connect_agent') {
            if (!window.authFinished) throw 'Entre na sua conta ChatGPT pelo Forge e tente conectar novamente.';
            return { thread_id: 'login-thread', messages: [], resumed: false };
          }
          if (command === 'start_login') { window.loginEvents = args.events; return { user_code: 'ABCD-1234', verification_url: 'https://auth.openai.com/codex/device' }; }
          if (command === 'finish_login') return window.authFinished;
          if (command === 'cancel_login' || command === 'open_login_page') return;
          if (command === 'send_message') window.sentAfterLogin++;
        },
      } };
    });
    await loginPage.goto(workspaceUrl);
    await loginPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\login-project');
    await loginPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await loginPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    const loginDraft = loginPage.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    await loginDraft.fill('Minha ideia permanece');
    await loginPage.getByRole('button', { name: 'Enviar', exact: true }).click();
    await loginPage.locator('#login-panel').waitFor({ state: 'visible' });
    assert.match(await loginPage.locator('#agent-status').textContent(), /Sua mensagem não foi enviada/);
    assert.equal(await loginDraft.inputValue(), 'Minha ideia permanece');
    assert.equal(await loginPage.evaluate(() => window.sentAfterLogin), 0);
    assert.equal(await loginPage.locator('#send-message').isDisabled(), true);
    assert.equal(await loginPage.locator('#connect-agent').isDisabled(), true);
    assert.equal(await loginPage.locator('#find-conversations').isDisabled(), true);
    await loginPage.getByRole('button', { name: 'Entrar com ChatGPT' }).click();
    await loginPage.locator('#login-code').filter({ hasText: 'ABCD-1234' }).waitFor();
    assert.equal(await loginPage.locator('#start-login').isHidden(), true);
    assert.equal(await loginPage.locator('#login-url').textContent(), 'https://auth.openai.com/codex/device');
    await loginPage.getByRole('button', { name: 'Já entrei · verificar' }).click();
    await loginPage.locator('#login-status').filter({ hasText: 'Aguardando a confirmação' }).waitFor();
    await loginPage.evaluate(() => { window.authFinished = true; window.loginEvents.onmessage({ success: true }); });
    await loginPage.locator('#login-panel').waitFor({ state: 'hidden' });
    assert.equal(await loginDraft.inputValue(), 'Minha ideia permanece');
    await loginPage.getByRole('button', { name: 'Enviar', exact: true }).click();
    await loginPage.waitForFunction(() => window.sentAfterLogin === 1);
    await loginPage.close();
    console.log('PASS: simulated first-use ChatGPT access preserves the draft, shows a device code, waits for confirmation, then sends once. Native login completion NOT_RUN.');
    const keyboardPage = await browser.newPage();
    await keyboardPage.goto(url);
    for (const selector of ['.skip', '.brand', 'nav a:first-child', 'nav a:nth-child(2)', 'nav a:nth-child(3)', 'nav a:nth-child(4)', 'nav a:last-child', '.appearance summary', '#appearance-theme', '#appearance-contrast', '.home-actions a:first-child', '.home-actions a:last-child', '.starter-card:nth-child(1) a', '.starter-card:nth-child(2) a', '.starter-card:nth-child(3) a', '#about summary']) {
      await keyboardPage.keyboard.press('Tab');
      assert.equal(await keyboardPage.locator(selector).evaluate(node => node === document.activeElement), true, `Keyboard order: ${selector}`);
      assert.ok(await keyboardPage.locator(selector).evaluate(node => parseFloat(getComputedStyle(node).outlineWidth) >= 3), `Visible focus: ${selector}`);
      if (selector === '.appearance summary') await keyboardPage.keyboard.press('Enter');
    }
    await keyboardPage.locator('nav a[data-route="workspace"]').click();
    await keyboardPage.waitForFunction(() => document.activeElement?.id === 'workspace-title');
    assert.equal(await keyboardPage.locator('#workspace-title').evaluate(node => node === document.activeElement), true);
    await keyboardPage.keyboard.press('Shift+Tab');
    assert.equal(await keyboardPage.locator('.back-link').evaluate(node => node === document.activeElement), true);
    assert.ok(await keyboardPage.locator('.back-link').evaluate(node => parseFloat(getComputedStyle(node).outlineWidth) >= 3));
    assert.ok(await keyboardPage.locator('nav a').first().evaluate(node => node.getBoundingClientRect().height >= 48), 'Main navigation keeps a 48px target');
    for (const selector of ['#project-setup summary', '#project-root', '#browse-project', '#start-project', '#connection summary', '#retry', '.conversation-body']) {
      await keyboardPage.keyboard.press('Tab');
      assert.equal(await keyboardPage.locator(selector).evaluate(node => node === document.activeElement), true, `Workspace keyboard order: ${selector}`);
      assert.ok(await keyboardPage.locator(selector).evaluate(node => parseFloat(getComputedStyle(node).outlineWidth) >= 3), `Visible focus: ${selector}`);
      if (selector === '#connection summary') await keyboardPage.keyboard.press('Enter');
    }
    await keyboardPage.close();
    console.log('PASS: keyboard traversal of all initially available controls, disclosures and visible focus.');
    for (const width of [390, 1180]) {
      await page.setViewportSize({ width, height: 844 });
      await page.goto(workspaceUrl);
      if (!await page.locator('#connection').evaluate(details => details.open)) {
        await page.locator('#connection summary').click();
      }
      await page.getByRole('status').filter({ hasText: 'Não foi possível' }).waitFor();
      await page.getByRole('button', { name: 'Verificar novamente' }).click();
      await page.getByRole('status').filter({ hasText: 'Não foi possível' }).waitFor();
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
      if (width === 390) {
        const sizes = await page.locator('p, nav a, button, footer').evaluateAll(nodes => nodes.map(n => parseFloat(getComputedStyle(n).fontSize)));
        assert.ok(sizes.every(size => size >= 18));
      }
    }
    await page.goto(url);
    await page.reload();
    await page.keyboard.press('Tab');
    assert.equal(await page.locator(':focus').textContent(), 'Pular para o conteúdo');
    await page.goto(workspaceUrl);
    await page.locator('#workspace').waitFor({ state: 'visible' });
    await page.locator('#connection summary').click();
    // Explicit test double for UI presentation only, not agent/native evidence.
    await page.evaluate(() => { window.__TAURI__ = { core: { invoke: async () => ({ name: 'Forge', version: '0.1.0' }) } }; });
    await page.getByRole('button', { name: 'Verificar novamente' }).click();
    await page.getByRole('status').filter({ hasText: 'Aplicativo iniciado' }).waitFor();
    await page.getByText('Escolha uma pasta para começar.', { exact: false }).waitFor();
    await page.emulateMedia({ colorScheme: 'dark' });
    await page.waitForFunction(() => document.documentElement.dataset.theme === 'dark');
    assert.equal(await page.evaluate(() => getComputedStyle(document.documentElement).colorScheme), 'dark');
    // Controlled protocol double: exercise UI ordering, not real authentication.
    await page.evaluate(() => {
      window.sendCalls = 0; window.disconnectCalls = 0;
      window.__TAURI__.core.Channel = class {};
      window.__TAURI__.core.invoke = async (command, args) => {
        if (command === 'inspect_progress') {
          window.progressCalls = (window.progressCalls || 0) + 1;
          if (window.delayProgress) return new Promise(resolve => { window.resolveProgress = resolve; });
          if (window.progressFailure) throw window.progressFailure === true ? new Error('Unavailable') : window.progressFailure;
          return {
            status: window.progressState || 'current', phase: window.progressPhase || '1-discovery',
            focus: window.progressMissingFocus || window.progressState === 'absent' ? null : { title: 'Recorded task', intended_outcome: 'Accepted outcome', current_activity: 'Recorded activity', next_step: 'Recorded next step', open_decision_count: window.progressDecisionCount ?? 1 },
            accepted_direction: window.progressNoDirection ? null : { outcome: 'Create a helpful app', constraints: [window.progressConstraint || 'Keep it easy to use'], unacceptable_outcomes: ['Do not delete existing work'], open_uncertainties: [], revision: window.progressRevision || 1, revision_kind: window.progressRevisionKind || 'initial', origin: 'forge_cooperative_record' },
            recorded_pending_count: window.progressRecordedPending ?? 0,
            suggested_questions: window.progressSuggestedQuestions ?? [{ question: 'Which direction should we choose?', blocking: true, recommended_alternative_ref: 'simple', alternatives: [{ id: 'simple', description: 'Start simple', consequences: ['Can test sooner'] }, { id: 'rich', description: '<script>Build more</script>', consequences: ['Needs more review'] }] }],
          };
        }
        if (command === 'inspect_direction_history') {
          window.historyCalls = (window.historyCalls || 0) + 1;
          return { earlier_count: 0, revisions: [
            { active: false, origin: 'forge_cooperative_record', revision: 1, revision_kind: 'initial', outcome: 'Earlier direction', constraints: ['Keep files'], unacceptable_outcomes: [], accepted_at_unix: 1780000000 },
            { active: true, origin: 'forge_cooperative_record', revision: 2, revision_kind: 'material_supersession', outcome: '<script>Current direction</script>', constraints: [], unacceptable_outcomes: ['No deletion'], accepted_at_unix: 1781000000 },
          ] };
        }
        if (command === 'start_project' || command === 'inspect_project') return { project_id: 'test-project', project_root: args.projectRoot };
          if (command === 'connect_agent') { window.agentEvents = args.events; window.connectedThread = args.threadId; return { thread_id: 'test-thread', messages: args.threadId ? window.resumeMessages || [{ id: 'saved-user', role: 'user', text: 'Saved decision' }, { id: 'saved-agent', role: 'agent', text: '## Partial reply\n- Saved item' }, { id: 'saved-incomplete', role: 'agent', text: '## Still incomplete', incomplete: true }] : [], resumed: !!args.threadId }; }
        if (command === 'send_message') {
          window.sendCalls++;
          return new Promise((resolve, reject) => { window.resolveSend = resolve; window.rejectSend = reject; });
        }
        if (command === 'disconnect_agent') {
          window.disconnectCalls++;
          if (window.delayDisconnect) return new Promise(resolve => { window.resolveDisconnect = resolve; });
        }
      };
    });
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\test-project');
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    for (const [width, height] of [[1180, 820], [1280, 844]]) {
      await page.setViewportSize({ width, height });
      assert.equal(await page.evaluate(() => {
        window.scrollTo(0, 0);
        const sendBox = document.getElementById('send-message').getBoundingClientRect();
        const historyBox = document.querySelector('.conversation-body').getBoundingClientRect();
        const emptyHeadingBox = document.querySelector('.empty-conversation h3').getBoundingClientRect();
        const formBox = document.getElementById('message-form').getBoundingClientRect();
        return sendBox.top >= 0 && sendBox.bottom <= innerHeight && historyBox.height >= 150 &&
          emptyHeadingBox.top >= historyBox.top && emptyHeadingBox.bottom <= historyBox.bottom &&
          historyBox.bottom <= formBox.top + 1;
      }), true, `Project-ready invitation, history and Send must fit without overlap at ${width}x${height}`);
      if (width === 1180 && process.env.FORGE_COMPOSER_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_COMPOSER_SCREENSHOT });
      if (width === 1180) assert.equal(await page.evaluate(() => {
        window.scrollTo(0, 0);
        const activity = document.getElementById('record-activity').getBoundingClientRect();
        return activity.top >= 0 && activity.bottom <= innerHeight;
      }), true, 'The real recorded activity should be readable alongside an empty conversation at 1180x820');
    }
    await page.setViewportSize({ width: 1280, height: 720 });
    assert.equal(await page.locator('#project-record').isVisible(), true);
    assert.equal(await page.locator('.project #project-record').count(), 0);
    assert.equal(await page.locator('#project-setup').evaluate(node => node.open), false);
    assert.equal(await page.getByRole('heading', { name: 'Onde estamos' }).isVisible(), true);
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.equal(await page.evaluate(() => {
      window.scrollTo(0, 0);
      const record = document.getElementById('project-record').getBoundingClientRect();
      return record.top >= 0 && record.top < innerHeight;
    }), true, 'The empty preview must leave the project record visible in the initial desktop viewport');
    assert.equal(await page.evaluate(() => window.progressCalls || 0), 1);
    assert.equal(await page.locator('#workspace-phase').textContent(), 'Etapa do projeto: Descoberta');
    assert.equal(await page.locator('#workspace-phase').isVisible(), true);
    assert.equal(await page.locator('#record-activity').textContent(), 'Recorded activity');
    assert.equal(await page.getByRole('group', { name: 'Atividade e próximo passo registrados' }).isVisible(), true);
    assert.equal(await page.locator('#record-activity').isVisible(), true, 'Current recorded activity is readable without opening details');
    assert.equal(await page.locator('#record-next').isVisible(), true, 'The recorded next step is readable without opening details');
    assert.equal(await page.locator('#record-outcome').isVisible(), false, 'Supporting objective stays in optional details');
    assert.equal(await page.locator('#record-outcome').textContent(), 'Accepted outcome');
    await page.locator('#message-text').fill('Minha ideia continua aqui.');
    await page.getByRole('button', { name: 'Entender isto na conversa' }).click();
    assert.match(await page.locator('#message-text').inputValue(), /^Minha ideia continua aqui\.\n\nExplique em linguagem simples/);
    assert.equal(await page.evaluate(() => document.activeElement?.id), 'message-text');
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Explaining the record must not send a turn');
    await page.locator('#message-text').fill('');
    assert.equal(await page.locator('#record-pending').isVisible(), true);
    assert.equal(await page.locator('#record-questions-shortcut').isVisible(), true);
    assert.equal(await page.locator('#record-pending').evaluate(node => !!(node.compareDocumentPosition(document.getElementById('record-direction-card')) & Node.DOCUMENT_POSITION_FOLLOWING)), true,
      'Unresolved questions should precede the optional technical direction and history');
    await page.locator('#record-questions-shortcut').click();
    assert.equal(await page.evaluate(() => document.activeElement?.id), 'record-pending-heading');
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'The questions shortcut must not choose or send anything');
    assert.match(await page.locator('#record-pending').textContent(), /não são acordos/);
    assert.match(await page.locator('#record-suggestions').textContent(), /Can test sooner/);
    assert.match(await page.locator('#record-suggestions').textContent(), /sugestão do Forge, não uma decisão sua/);
    assert.equal(await page.locator('#record-suggestions script').count(), 0);
    assert.equal(await page.evaluate(() => window.historyCalls || 0), 0, 'History must be opt-in');
    await page.locator('#direction-history summary').click();
    await page.locator('#direction-history-status').filter({ hasText: '2 direções registradas' }).waitFor();
    assert.equal(await page.evaluate(() => window.historyCalls), 1);
    assert.match(await page.locator('#direction-history-list article').first().textContent(), /Direção atual/);
    assert.match(await page.locator('#direction-history-list article').last().textContent(), /Direção anterior/);
    await page.locator('#direction-history-list article').first().locator('summary').click();
    assert.equal(await page.locator('#direction-history-list article').first().locator('details').evaluate(node => node.open), true);
    assert.equal(await page.locator('#direction-history-list script').count(), 0, 'Recorded text must be literal');
    assert.match(await page.locator('#direction-history-list').textContent(), /não prova aprovação humana independente/);
    if (process.env.FORGE_HISTORY_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_HISTORY_SCREENSHOT, fullPage: true });
    const questionDraft = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    await questionDraft.fill('Minha ideia original.');
    await page.getByRole('button', { name: 'Conversar sobre: Which direction should we choose?' }).click();
    assert.equal(await questionDraft.inputValue(), 'Minha ideia original.\n\nExplique em português claro esta pergunta do registro, inclusive se ela exige uma decisão minha: Which direction should we choose?');
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Preparing a suggested question must not send a turn');
    assert.match(await page.locator('#progress-status').textContent(), /nenhuma decisão foi registrada/);
    await questionDraft.fill('');
    await page.getByRole('button', { name: 'Conversar sobre a opção: Start simple' }).click();
    assert.match(await questionDraft.inputValue(), /Explique em português claro/);
    assert.match(await questionDraft.inputValue(), /Ainda não estou escolhendo esta opção/);
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Discussing an option must not choose it or send a turn');
    await questionDraft.fill('');
    assert.equal(await page.locator('#record-direction-card').isVisible(), true);
    assert.equal(await page.locator('#record-direction-card').evaluate(node => node.open), false, 'The recorded objective should not dominate the first view');
    assert.equal(await page.locator('#record-direction-outcome').isVisible(), false, 'Technical source wording is optional reading');
    assert.equal(await page.locator('#record-direction-outcome').textContent(), 'Create a helpful app');
    assert.match(await page.locator('#record-direction-card > .hint').textContent(), /agente registrou/);
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false, 'The quieter record must not overflow a narrow window');
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.locator('#record-direction-card > summary').click();
    await questionDraft.fill('Minha ideia original.');
    await page.getByRole('button', { name: 'Pedir uma explicação na conversa' }).click();
    assert.match(await questionDraft.inputValue(), /^Minha ideia original\.\n\nExplique em português claro a direção atual/);
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Asking for a plain-language explanation prepares a draft but never sends it');
    await questionDraft.fill('');
    assert.equal(await page.locator('#record-direction').evaluate(node => node.open), false);
    await page.locator('#record-direction summary').click();
    assert.equal(await page.locator('#record-direction-outcome').textContent(), 'Create a helpful app');
    assert.equal(await page.locator('#record-constraints-list li').textContent(), 'Keep it easy to use');
    assert.equal(await page.locator('#record-constraints-list').getByText('Which direction should we choose?').count(), 0);
    assert.match(await page.locator('#record-direction').textContent(), /não comprova aprovação humana independente/);
    if (process.env.FORGE_DIRECTION_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_DIRECTION_SCREENSHOT, fullPage: true });
    await page.evaluate(() => { window.progressRevision = 2; window.progressRevisionKind = 'material_supersession'; window.progressConstraint = '<script>changed</script>'; window.progressRecordedPending = 1; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.equal(await page.locator('#record-direction-card').evaluate(node => node.open), false, 'Refreshing returns long objective details to their quiet state');
    await page.locator('#record-direction-card > summary').click();
    await page.locator('#record-direction summary').click();
    assert.match(await page.locator('#record-revision').textContent(), /Direção revista.*revisão 2/);
    assert.equal(await page.locator('#record-constraints-list li').textContent(), '<script>changed</script>');
    assert.equal(await page.locator('#record-direction script').count(), 0);
    assert.match(await page.locator('#record-pending-count').textContent(), /1 decisão pendente foi recuperada/);
    assert.match(await page.locator('#record-pending-count').textContent(), /texto original da escolha não está disponível aqui/);
    const stateNames = { current: 'Em andamento', stale: 'Acompanhamento desatualizado', blocked: 'Há uma pendência', completed: 'Esta parte foi concluída', abandoned: 'Encerrado sem concluir' };
    for (const state of ['current', 'stale', 'blocked', 'completed', 'abandoned', 'absent']) {
      await page.evaluate(state => { window.progressState = state; }, state);
      await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
      await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
      assert.equal(await page.locator('#progress-result').isVisible(), true);
      if (state === 'absent') {
        assert.match(await page.locator('#progress-status').textContent(), /direção foi registrada no Forge, mas o próximo trabalho ainda não/);
        assert.equal(await page.locator('#workspace-phase').textContent(), 'Direção registrada; próximo trabalho pendente');
        assert.equal(await page.locator('#record-state').textContent(), 'Direção registrada; próximo trabalho pendente');
        assert.equal(await page.locator('#workspace-phase').isVisible(), true);
        assert.equal(await page.locator('.record-stage').isVisible(), false, 'An absent record must not look like an active discovery stage');
        assert.match(await page.locator('#record-empty-help').textContent(), /O objetivo está registrado.*próximo trabalho ainda não foi definido/);
        assert.equal(await page.locator('#record-empty-help').isVisible(), true);
        assert.equal(await page.locator('#record-work').isVisible(), false);
        assert.equal(await page.locator('#record-direction-card').isVisible(), true);
        assert.equal(await page.locator('#record-direction').isVisible(), false);
        if (process.env.FORGE_ABSENT_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_ABSENT_SCREENSHOT, fullPage: true });
      }
      else {
        assert.equal(await page.locator('#workspace-phase').isVisible(), true);
        assert.equal(await page.locator('.record-stage').isVisible(), true);
        assert.equal(await page.locator('#record-empty-help').isVisible(), false);
        assert.equal(await page.locator('#record-phase-label').textContent(), 'ETAPA GERAL DO PROJETO');
        if (state === 'stale') assert.match(await page.locator('#workspace-phase').textContent(), /Etapa do projeto \(desatualizada\)/);
        assert.equal(await page.locator('#record-state').textContent(), stateNames[state]);
        assert.equal(await page.locator('#progress-result').getAttribute('data-state'), state);
        assert.equal(await page.locator('#record-phase').textContent(), 'Descoberta');
        assert.equal(await page.locator('#record-activity-label').textContent(), state === 'completed' ? 'Resultado registrado' : state === 'abandoned' ? 'Último registro' : 'Agora');
        if (state === 'completed') {
          assert.match(await page.locator('#progress-status').textContent(), /Esta parte do trabalho foi concluída. O projeto pode continuar/);
          assert.match(await page.locator('#record-phase-help').textContent(), /Este trabalho foi concluído; a etapa geral pode continuar aqui/);
        }
        assert.equal(await page.locator('#record-outcome').textContent(), 'Accepted outcome');
        assert.equal(await page.locator('#record-next').textContent(), 'Recorded next step');
        assert.match(await page.locator('#record-decisions').textContent(), /1 decisão em aberto/);
        if (state === 'current' && process.env.FORGE_PROGRESS_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_PROGRESS_SCREENSHOT, fullPage: true });
        if (state === 'current') {
          assert.equal(await page.locator('.record-more').evaluate(node => node.open), false);
          await page.locator('.record-more summary').click();
          assert.equal(await page.locator('#record-decisions').isVisible(), true);
        }
      }
    }
    await page.evaluate(() => { window.progressNoDirection = true; window.progressState = 'absent'; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.match(await page.locator('#progress-status').textContent(), /Ainda não há andamento registrado no Forge/);
    assert.equal(await page.locator('#workspace-phase').textContent(), 'Sem andamento registrado');
    assert.match(await page.locator('#record-empty-help').textContent(), /Você pode começar ou continuar pela conversa/);
    await page.evaluate(() => { window.progressState = 'current'; window.progressDecisionCount = 0; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    assert.match(await page.locator('#record-decisions').textContent(), /não mostra decisões em aberto/);
    await page.evaluate(() => { window.progressNoDirection = true; window.progressRecordedPending = 0; window.progressSuggestedQuestions = []; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.waitForFunction(() => document.getElementById('progress-status').textContent.includes('Consultado às')
      && document.getElementById('record-questions-shortcut').hidden);
    assert.equal(await page.locator('#record-direction').isVisible(), false);
    assert.equal(await page.locator('#record-pending').isVisible(), false);
    assert.equal(await page.locator('#record-questions-shortcut').isVisible(), false);
    await page.evaluate(() => {
      window.progressRecordedPending = 1;
      window.progressSuggestedQuestions = [{
        question: 'Choose a direction?', blocking: true, recommended_alternative_ref: 'a',
        alternatives: [
          { id: 'a', description: 'Option A', consequences: ['Can start sooner'] },
          { id: 'b', description: 'Option B', consequences: ['Needs more review'] },
        ],
      }];
    });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.equal(await page.locator('#record-direction').isVisible(), false, 'A suggestion must not become an accepted direction');
    assert.equal(await page.locator('#record-direction-card').isVisible(), false, 'No recorded direction means no visible agreement card');
    assert.equal(await page.locator('#record-pending').isVisible(), true);
    assert.match(await page.locator('#record-pending-count').textContent(), /1 decisão pendente foi recuperada/);
    assert.match(await page.locator('#record-suggestions').textContent(), /sugestão do Forge, não uma decisão sua/);
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Reading a suggestion must not send or decide');
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.evaluate(() => { window.progressMissingFocus = true; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.locator('#progress-status').filter({ hasText: 'Não foi possível atualizar' }).waitFor();
    assert.equal(await page.locator('#progress-result').isVisible(), false);
    assert.equal(await page.locator('#record-questions-shortcut').isVisible(), false);
    await page.evaluate(() => { window.progressMissingFocus = false; window.progressDecisionCount = 1; });
    await page.evaluate(() => { window.progressFailure = true; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.locator('#progress-status').filter({ hasText: 'Não foi possível atualizar' }).waitFor();
    assert.equal(await page.locator('#progress-result').isVisible(), false);
    await page.evaluate(() => { window.progressFailure = 'O Forge está ocupado. Tente consultar o registro novamente.'; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.locator('#progress-status').filter({ hasText: 'Forge está ocupado' }).waitFor();
    assert.match(await page.locator('#progress-status').textContent(), /conversa não foi interrompida/);
    await page.evaluate(() => { window.progressFailure = 'O estado deste projeto não está disponível. Nada foi recriado ou alterado.'; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.locator('#progress-status').filter({ hasText: 'Nada foi recriado' }).waitFor();
    assert.equal(await page.locator('#progress-result').isVisible(), false);
    await page.evaluate(() => { window.progressFailure = false; window.progressState = 'current'; });
    await page.evaluate(() => { window.delayProgress = true; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).focus();
    await page.keyboard.press('Enter');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'progress-status');
    await page.locator('#progress-status').filter({ hasText: 'demorando para atualizar' }).waitFor({ timeout: 8000 });
    assert.equal(await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).isDisabled(), true);
    if (process.env.FORGE_SLOW_RECORD_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_SLOW_RECORD_SCREENSHOT, fullPage: true });
    await openProjectSetup(page);
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\another-project');
    await page.evaluate(() => { window.obsoleteProgressResolve = window.resolveProgress; });
    assert.equal(await page.locator('#progress-result').isVisible(), false);
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.waitForFunction(() => window.resolveProgress !== window.obsoleteProgressResolve);
    await page.evaluate(() => { window.obsoleteProgressResolve({ status: 'current', focus: { title: 'Obsolete response' } }); });
    await page.locator('#progress-status').filter({ hasText: 'demorando para atualizar' }).waitFor({ timeout: 8000 });
    await page.evaluate(() => {
      window.resolveProgress({ status: 'current', phase: '1-discovery', focus: { title: 'Recorded task', intended_outcome: 'Accepted outcome', current_activity: 'Recorded activity', next_step: 'Recorded next step', open_decision_count: 0 }, accepted_direction: null, recorded_pending_count: 0, suggested_questions: [] });
      window.delayProgress = false;
    });
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.equal(await page.locator('#record-title').textContent(), 'Recorded task');
    await page.locator('#conversation-picker summary').click();
    await page.getByRole('button', { name: 'Abrir conversa', exact: true }).focus();
    await page.keyboard.press('Enter');
    await page.locator('#agent-status').filter({ hasText: 'Codex conectado' }).waitFor();
    assert.match(await page.locator('#agent-status').textContent(), /Codex conectado ao projeto another-project/);
    assert.doesNotMatch(await page.locator('#agent-status').textContent(), /test-project/);
    assert.equal(await page.locator('#project-status').isHidden(), true);
    assert.equal(await page.locator('#project-setup').isVisible(), true);
    assert.match(await page.locator('#project-result-hint').textContent(), /Pasta confirmada/);
    assert.equal(await page.locator('#project-result').isVisible(), true);
    assert.equal(await page.locator('#project-title').textContent(), 'Seu projeto');
    assert.equal(await page.locator('#conversation-step').textContent(), 'SUA CONVERSA');
    assert.equal(await page.locator('#connect-agent').isHidden(), true);
    assert.equal(await page.locator('#new-conversation-choice').isHidden(), true);
    assert.equal(await page.locator('#disconnect-agent').isVisible(), true);
    assert.match(await page.locator('#empty-conversation-description').textContent(), /Sua conversa está pronta/);
    assert.equal(await page.locator('#browse-project').isDisabled(), true);
    assert.equal(await page.locator('#interrupt-agent').isHidden(), true, 'Idle chat should not advertise an unavailable interrupt action');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Histórico da conversa');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'message-text');
    await page.keyboard.press('Shift+Tab');
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Histórico da conversa');
    await page.keyboard.press('Shift+Tab');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'disconnect-agent');
    await page.evaluate(() => { window.delayProgress = true; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.evaluate(() => { window.agentEvents.onmessage({ kind: 'running' }); window.resolveProgress({ status: 'current', focus: { title: 'Obsolete response' } }); window.delayProgress = false; });
    assert.equal(await page.locator('#progress-result').isVisible(), false);
    assert.equal(await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).isEnabled(), true);
    for (const [kind, state] of [['running', 'working'], ['completed', 'completed'], ['interrupted', 'interrupted'], ['failed', 'error']]) {
      const priorProgressReads = await page.evaluate(() => window.progressCalls || 0);
      await page.evaluate(kind => window.agentEvents.onmessage({ kind }), kind);
      if (kind === 'running') assert.equal(await page.locator('#progress-result').isVisible(), false);
      else {
        await page.waitForFunction(previous => (window.progressCalls || 0) > previous, priorProgressReads);
        await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
        assert.equal(await page.locator('#progress-result').isVisible(), true, 'A finished turn refreshes the Forge record without asking the person to click');
      }
      assert.equal(await page.locator('#agent-status').getAttribute('data-state'), state);
      assert.equal(await page.locator('#agent-status .status-icon').getAttribute('aria-hidden'), 'true');
      assert.ok((await page.locator('#agent-status span:last-child').textContent()).length > 15);
    }
    await page.evaluate(() => {
      window.linkPreviewReads = [];
      const previous = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = (command, args) => {
        if (command !== 'inspect_preview') return previous(command, args);
        window.linkPreviewReads.push(args);
        if (args.filePath.includes('outside')) return Promise.reject('Este arquivo não pertence ao projeto aberto.');
        return Promise.resolve({ kind: 'text', content: window.linkPreviewContent || 'Arquivo real do projeto', relative_path: 'result.txt', size_bytes: 23 });
      };
    });
    const formattedReply = '# Plano\n- **Criar** uma tela\n- Mostrar `resultado` em `site/index.html`; `https://example.com/outside.html` é apenas texto.\n\n```rust\n// site/index.html must remain code, not an action\nfn main() { println!("<script>"); }\n```\n> Confirme o **resultado** antes de publicar.\n\n| Etapa | Situação | Observação |\n| --- | --- | --- |\n| Tela | `pronta` | Leia antes de publicar |\n| Arquivo | [abrir](result.txt) | Local |\n\n---\n<script>alert(1)</script>\n[arquivo](result.txt) [fora](D:/outside.md) [web](https://example.com) [abrir](javascript:alert(1))';
    await page.evaluate(text => window.agentEvents.onmessage({ kind: 'delta', id: 'formatted-reply', text }), formattedReply.slice(0, 24));
    assert.equal(await page.locator('#messages article[data-role="agent"] h3').count(), 0);
    await page.evaluate(text => window.agentEvents.onmessage({ kind: 'message', id: 'formatted-reply', text }), formattedReply);
    const formattedBubble = page.locator('#messages article[data-role="agent"]').last();
    assert.equal(await formattedBubble.locator('h3').textContent(), 'Plano');
    assert.equal(await formattedBubble.locator('li').count(), 2);
    assert.equal(await formattedBubble.locator('li strong').textContent(), 'Criar');
    assert.notEqual(await formattedBubble.locator('li strong').evaluate(node => getComputedStyle(node).display), 'block');
    assert.match(await formattedBubble.locator('pre code').textContent(), /fn main/);
    assert.match(await formattedBubble.locator('blockquote').textContent(), /Confirme o resultado/);
    assert.equal(await formattedBubble.locator('table th').allTextContents().then(values => values.join('|')), 'Etapa|Situação|Observação');
    assert.equal(await formattedBubble.locator('table tbody tr').count(), 2);
    assert.equal(await formattedBubble.locator('hr').count(), 1);
    if (process.env.FORGE_FORMATTED_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_FORMATTED_SCREENSHOT, fullPage: true });
    assert.equal(await formattedBubble.locator('script, a[href^="javascript:"]').count(), 0);
    assert.equal(await formattedBubble.locator('.message-file-link').count(), 4, 'Only project-file candidates become actions');
    assert.equal(await page.locator('#preview-last-result').getAttribute('hidden'), '', 'Several distinct files must not be guessed as one result');
    assert.equal(await page.locator('#preview-cited-files').isVisible(), true, 'Several cited files should remain individually available beside the preview');
    await page.locator('#preview-cited-files summary').click();
    assert.equal(await page.locator('#preview-cited-files-list button').count(), 3, 'Repeated citations should not duplicate a choice');
    assert.equal(await formattedBubble.getByRole('button', { name: 'Ver arquivo local: site/index.html' }).count(), 1, 'An inline-code file reference should offer the same safe preview action');
    assert.equal(await formattedBubble.locator('code').filter({ hasText: 'https://example.com/outside.html' }).count(), 1, 'An inline-code URL must remain inert text');
    assert.match(await formattedBubble.textContent(), /\[web\]\(https:\/\/example.com\)/);
    assert.match(await formattedBubble.textContent(), /<script>alert\(1\)<\/script>/);
    await page.getByRole('button', { name: 'Ver texto original' }).click();
    assert.equal(await formattedBubble.locator('pre.message-raw').textContent(), formattedReply);
    await page.getByRole('button', { name: 'Ver texto formatado' }).click();
    assert.equal(await formattedBubble.locator('h3').textContent(), 'Plano');
    await formattedBubble.getByRole('button', { name: 'Ver arquivo local: arquivo' }).click();
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await page.locator('#preview-text').textContent(), 'Arquivo real do projeto');
    assert.deepEqual(await page.evaluate(() => window.linkPreviewReads[0]), { projectRoot: 'D:\\another-project', filePath: 'D:\\another-project\\result.txt' });
    await formattedBubble.getByRole('button', { name: 'Ver arquivo local: site/index.html' }).click();
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.linkPreviewReads[1]), { projectRoot: 'D:\\another-project', filePath: 'D:\\another-project\\site\\index.html' });
    await page.evaluate(() => { window.linkPreviewContent = 'Arquivo alterado pelo Codex'; window.agentEvents.onmessage({ kind: 'completed' }); });
    await page.locator('#preview-text').filter({ hasText: 'Arquivo alterado pelo Codex' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.linkPreviewReads[2]), { projectRoot: 'D:\\another-project', filePath: 'D:\\another-project\\site\\index.html' }, 'A completed turn refreshes only the selected project file');
    await page.getByRole('button', { name: 'Abrir prévia' }).click();
    const readsBeforeDialogCompletion = await page.evaluate(() => window.linkPreviewReads.length);
    await page.evaluate(() => { window.linkPreviewContent = 'Arquivo mudado durante leitura'; window.agentEvents.onmessage({ kind: 'completed' }); });
    assert.equal(await page.locator('#preview-dialog').evaluate(node => node.open), true, 'A completed turn must not close an enlarged preview being read');
    assert.equal(await page.evaluate(() => window.linkPreviewReads.length), readsBeforeDialogCompletion, 'An open dialog defers the native file read');
    assert.match(await page.locator('#preview-dialog-status').textContent(), /atualizada ao fechar/);
    await page.getByRole('button', { name: 'Fechar prévia' }).click();
    await page.locator('#preview-text').filter({ hasText: 'Arquivo mudado durante leitura' }).waitFor();
    assert.equal(await page.evaluate(() => window.linkPreviewReads.length), readsBeforeDialogCompletion + 1, 'Closing the dialog refreshes once');
    await page.getByRole('button', { name: 'Conferir arquivo citado: site/index.html' }).click();
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.linkPreviewReads.at(-1)), { projectRoot: 'D:\\another-project', filePath: 'D:\\another-project\\site\\index.html' }, 'A cited-file choice uses the same project-bound native preview');
    await formattedBubble.getByRole('button', { name: 'Ver arquivo local: fora' }).click();
    await page.locator('#preview-status').filter({ hasText: 'não pertence ao projeto' }).waitFor();
    assert.equal(await page.locator('#preview-result').isVisible(), false);
    const readsAfterInvalidFile = await page.evaluate(() => window.linkPreviewReads.length);
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'completed' }));
    assert.equal(await page.evaluate(() => window.linkPreviewReads.length), readsAfterInvalidFile, 'A failed preview is not retried implicitly after a turn');
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    assert.ok(await formattedBubble.locator('.message-table-scroll').evaluate(node => node.scrollWidth > node.clientWidth), 'A wide response table should scroll within its message');
    assert.ok(await formattedBubble.locator('pre').evaluate(node => parseFloat(getComputedStyle(node).fontSize) >= 18));
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'message', id: 'subheading-first', text: '## Primeiro título\n### Detalhe\n# Outro assunto\n### Sem nível intermediário' }));
    assert.deepEqual(await page.locator('#messages article[data-role="agent"]').last().locator('h3, h4, h5').evaluateAll(nodes => nodes.map(node => [node.tagName, node.textContent])), [
      ['H3', 'Primeiro título'], ['H4', 'Detalhe'], ['H3', 'Outro assunto'], ['H4', 'Sem nível intermediário'],
    ]);
    const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    await composer.fill('🎨'.repeat(17000));
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    assert.equal(await page.evaluate(() => window.sendCalls), 0);
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isEnabled(), true);
    await composer.fill('First message');
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'message', id: 'long-response', text: 'A long conversation line.\n'.repeat(100) }));
    await composer.focus();
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'send-message');
    await page.keyboard.press('Enter');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'agent-status');
    const pendingKey = 'forge.send-unconfirmed.v1:' + JSON.stringify(['test-project', 'D:\\another-project']);
    assert.equal(await page.evaluate(key => localStorage.getItem(key), pendingKey), 'test-thread', 'A delivery marker must exist before native Send resolves');
    assert.equal(await page.locator('#agent-status').evaluate(node => { const box = node.getBoundingClientRect(); return box.bottom > 0 && box.top < innerHeight; }), true);
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'running' }));
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Histórico da conversa');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Ver arquivo local: site/index.html');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Tabela da resposta');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Ver arquivo local: abrir');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Ver arquivo local: arquivo');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Ver arquivo local: fora');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'interrupt-agent');
    await page.keyboard.press('Enter');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'agent-status');
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'completed' }));
    await composer.fill('Keep this new draft');
    await page.evaluate(() => window.resolveSend({}));
    await page.waitForFunction(key => localStorage.getItem(key) === null, pendingKey);
    assert.equal(await composer.inputValue(), 'Keep this new draft');
    await page.evaluate(() => { window.delayDisconnect = true; });
    await page.getByRole('button', { name: 'Desconectar', exact: true }).focus();
    await page.keyboard.press('Enter');
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'running' }));
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isDisabled(), true);
    await page.evaluate(() => window.resolveDisconnect());
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
    assert.equal(await page.locator('#project-status').isVisible(), true);
    assert.equal(await page.locator('#project-setup').isVisible(), true);
    await page.locator('#conversation-picker summary').click();
    assert.equal(await page.locator('#connect-agent').isVisible(), true);
    assert.equal(await page.locator('#new-conversation-choice').isVisible(), true);
    assert.equal(await page.locator('#disconnect-agent').isHidden(), true);
    await page.evaluate(() => { window.delayDisconnect = false; });
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.match(await page.locator('#agent-status').textContent(), /última resposta foi interrompida.*Confira a conversa e os arquivos.*Nada foi reenviado/);
    assert.equal(await page.evaluate(() => window.connectedThread), 'test-thread');
    assert.match(await page.locator('#messages').textContent(), /Saved decision/);
    assert.match(await page.locator('#messages').textContent(), /Partial reply/);
    assert.equal(await page.locator('#messages article[data-role="agent"] .message-avatar img').count(), 2);
    assert.equal(await page.locator('#messages article[data-role="agent"] h3').textContent(), 'Partial reply');
    assert.equal(await page.locator('#messages article[data-role="agent"]').last().locator('h3').count(), 0);
    assert.equal(await page.locator('#messages article[data-role="user"] .message-avatar').getAttribute('aria-hidden'), 'true');
    assert.equal(await page.locator('#preview-last-result').isHidden(), true, 'An incomplete latest reply must not suggest a result');
    await page.evaluate(() => { window.resumeMessages = [
      { id: 'result-user', role: 'user', text: 'Crie uma página simples.' },
      { id: 'result-agent', role: 'agent', text: 'Pronto: [Ver página](site/index.html).' },
    ]; });
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.match(await page.locator('#agent-status').textContent(), /Você pode continuar de onde parou/);
    assert.equal(await page.locator('#preview-last-result').isVisible(), true, 'A single file in the restored completed reply should be easy to reopen');
    assert.match(await page.locator('#preview-intro').textContent(), /resposta cita um arquivo/);
    await page.getByRole('button', { name: 'Ver texto original' }).click();
    assert.equal(await page.locator('#preview-last-result').isVisible(), true, 'Reading the original answer must not lose its file shortcut');
    await page.getByRole('button', { name: 'Conferir arquivo citado' }).click();
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.linkPreviewReads.at(-1)), { projectRoot: 'D:\\another-project', filePath: 'D:\\another-project\\site\\index.html' });
    await page.evaluate(() => { window.resumeMessages = [
      { id: 'result-user', role: 'user', text: 'Crie uma página simples.' },
      { id: 'result-agent', role: 'agent', text: 'Pronto: [Ver página](site/index.html).' },
      { id: 'planning-user', role: 'user', text: 'Vamos planejar o próximo passo.' },
      { id: 'planning-agent', role: 'agent', text: 'O próximo passo é definir o público. Nenhum arquivo mudou.' },
    ]; });
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.locator('#preview-last-result').getAttribute('hidden'), null, 'A later planning reply must retain the earlier file shortcut');
    assert.match(await page.locator('#preview-intro').textContent(), /resultado anterior tem um arquivo/);
    assert.equal(await page.locator('#preview-result').isVisible(), true, 'An already open preview should stay visible during planning');
    assert.deepEqual(await page.evaluate(() => window.linkPreviewReads.at(-1)), { projectRoot: 'D:\\another-project', filePath: 'D:\\another-project\\site\\index.html' });
    await page.evaluate(() => { window.resumeMessages = [
      { id: 'result-user', role: 'user', text: 'Crie uma página simples.' },
      { id: 'result-agent', role: 'agent', text: 'Pronto: [Ver página](site/index.html).' },
      { id: 'planning-user', role: 'user', text: 'Vamos planejar o próximo passo.' },
      { id: 'planning-agent', role: 'agent', text: 'O próximo passo é definir o público.' },
      { id: 'interrupted-user', role: 'user', text: 'Agora implemente o próximo passo.' },
      { id: 'interrupted-agent', role: 'agent', text: 'Vou preparar', incomplete: true },
    ]; });
    const sendsBeforeInterruptedResume = await page.evaluate(() => window.sendCalls);
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.match(await page.locator('#agent-status').textContent(), /última resposta foi interrompida.*Nada foi reenviado/);
    assert.equal(await page.evaluate(() => window.sendCalls), sendsBeforeInterruptedResume, 'An interrupted readback must not resend the previous request');
    assert.equal(await page.locator('#preview-last-result').getAttribute('hidden'), null, 'An interrupted reply must keep an earlier completed result available');
    assert.equal(await page.locator('#preview-result').isVisible(), true, 'An already open result must remain visible after an interrupted reply');
    assert.match(await page.locator('#preview-intro').textContent(), /resultado anterior tem um arquivo/);
    assert.match(await page.locator('#messages article').last().textContent(), /Vou preparar/);
    await page.evaluate(() => {
      window.resumeMessages = [
        { id: 'long-result-user', role: 'user', text: 'Crie uma página simples.' },
        { id: 'long-result-agent', role: 'agent', text: 'Pronto: [Ver página](site/index.html).' },
        ...Array.from({ length: 100 }, (_, index) => [
          { id: `long-user-${index}`, role: 'user', text: `Pergunta ${index}` },
          { id: `long-agent-${index}`, role: 'agent', text: `Resposta ${index} sem novo arquivo.` },
        ]).flat(),
      ];
    });
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.locator('#messages article').count(), 202, 'A long restored history keeps every message in order');
    assert.match(await page.locator('#messages article').last().textContent(), /Resposta 99 sem novo arquivo/);
    assert.equal(await page.locator('#preview-last-result').getAttribute('hidden'), null, 'A long planning history keeps the earlier result shortcut available');
    assert.match(await page.locator('#preview-intro').textContent(), /resultado anterior tem um arquivo/);
    await page.evaluate(() => { window.resumeMessages = null; });
    assert.ok(await page.evaluate(() => {
      const workspace = document.querySelector('.workspace-screen');
      const decoration = getComputedStyle(workspace, '::before');
      const decorationEnd = workspace.getBoundingClientRect().bottom - Number.parseFloat(decoration.bottom);
      return decoration.display === 'none' || decorationEnd <= document.querySelector('footer').getBoundingClientRect().top - 8;
    }), 'Conversation foliage must finish above the footer text');
    if (process.env.FORGE_CONVERSATION_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_CONVERSATION_SCREENSHOT, fullPage: true });
    if (process.env.FORGE_CONVERSATION_LIGHT_SCREENSHOT) {
      const previousTheme = await page.evaluate(() => document.documentElement.dataset.theme);
      await page.evaluate(() => { document.documentElement.dataset.theme = 'light'; });
      await page.screenshot({ path: process.env.FORGE_CONVERSATION_LIGHT_SCREENSHOT, fullPage: true });
      await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, previousTheme);
    }
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
    await page.locator('#conversation-picker summary').click();
    await page.getByLabel('Começar outra conversa', { exact: true }).check();
    await page.evaluate(() => {
      const invoke = window.__TAURI__.core.invoke;
      window.beforeBookmarkInvoke = invoke;
      window.__TAURI__.core.invoke = async (command, args) => {
        if (command === 'connect_agent') {
          window.connectedThread = args.threadId;
          return { thread_id: 'new-thread', messages: [], resumed: !!args.threadId };
        }
        return invoke(command, args);
      };
      window.beforeBookmarkSet = Storage.prototype.setItem;
      Storage.prototype.setItem = () => { throw new Error('disk unavailable'); };
    });
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Não foi possível salvar o acesso' }).waitFor();
    assert.equal(await page.evaluate(() => window.connectedThread), null);
    assert.equal(await page.locator('#preview-last-result').isHidden(), true, 'A new empty conversation must clear the prior file shortcut');
    assert.equal(await page.locator('#preview-cited-files').isHidden(), true, 'A new empty conversation must clear prior cited files');
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.evaluate(() => window.connectedThread), 'new-thread');
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await page.evaluate(() => {
      Storage.prototype.setItem = window.beforeBookmarkSet;
      window.__TAURI__.core.invoke = async (command, args) => {
        if (command === 'connect_agent') throw 'Saved conversation unavailable';
        return window.beforeBookmarkInvoke(command, args);
      };
    });
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Saved conversation unavailable' }).waitFor();
    assert.equal(await composer.isEnabled(), true);
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isEnabled(), true);
    await page.evaluate(() => { window.__TAURI__.core.invoke = window.beforeBookmarkInvoke; });
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    console.log('PASS: restored transcript, explicit new conversation, failed save keeps latest session bookmark, unavailable resume never silently starts anew.');
    const sendsBeforeStorageFailure = await page.evaluate(() => window.sendCalls);
    await composer.fill('Must remain a draft');
    await page.evaluate(() => {
      window.beforeGuardSet = Storage.prototype.setItem;
      Storage.prototype.setItem = () => { throw new Error('disk unavailable'); };
    });
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Sua mensagem não foi enviada' }).waitFor();
    assert.equal(await page.evaluate(() => window.sendCalls), sendsBeforeStorageFailure);
    assert.equal(await composer.inputValue(), 'Must remain a draft');
    await page.evaluate(() => { Storage.prototype.setItem = window.beforeGuardSet; });
    const disconnectsBeforeRejection = await page.evaluate(() => window.disconnectCalls);
    await composer.fill('Rejected request');
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.evaluate(() => window.rejectSend('Test rejection'));
    await page.locator('#agent-status').filter({ hasText: 'Test rejection' }).waitFor();
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeRejection + 1);
    assert.equal(await page.locator('#connect-agent').isEnabled(), true);
    assert.equal(await page.locator('#messages article[data-role="user"]').last().getAttribute('data-delivery'), 'unconfirmed');
    assert.match(await page.locator('#messages article[data-role="user"]').last().textContent(), /envio não confirmado/);
    assert.equal(await composer.inputValue(), 'Rejected request');
    assert.match(await page.locator('#composer-help').textContent(), /último envio não foi confirmado/);
    const uncertainKey = 'forge.send-unconfirmed.v1:' + JSON.stringify(['test-project', 'D:\\another-project']);
    assert.equal(await page.evaluate(key => localStorage.getItem(key), uncertainKey), 'test-thread');
    const recoveryContext = await browser.newContext({ storageState: await page.context().storageState() });
    const recoveryPage = await recoveryContext.newPage();
    await recoveryPage.addInitScript(() => {
      window.recoverySends = 0;
      window.__TAURI__ = { core: { Channel: class {}, invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: '0.1.2' };
        if (command === 'inspect_project') return { project_id: 'test-project', project_root: args.projectRoot };
        if (command === 'connect_agent') return { thread_id: 'test-thread', messages: [], resumed: !!args.threadId };
        if (command === 'send_message') { window.recoverySends++; return {}; }
      } } };
    });
    await recoveryPage.goto(`${url}#projects`);
    await recoveryPage.getByRole('button', { name: 'Abrir another-project na pasta D:\\another-project' }).click();
    await recoveryPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await recoveryPage.getByRole('button', { name: 'Conferir envio anterior' }).isVisible(), true, 'An uncertain send should offer a visible read-only recovery action');
    assert.match(await recoveryPage.locator('#composer-help').textContent(), /último envio não foi confirmado/);
    await recoveryPage.getByRole('textbox', { name: 'Sua ideia começa aqui' }).fill('Do not resend without review');
    await recoveryPage.getByRole('button', { name: 'Enviar', exact: true }).click();
    assert.equal(await recoveryPage.evaluate(() => window.recoverySends), 0);
    await recoveryPage.getByLabel('Começar outra conversa').check();
    await openConversation(recoveryPage);
    await recoveryPage.locator('#agent-status').filter({ hasText: 'Retome a conversa anterior' }).waitFor();
    await recoveryPage.getByLabel('Começar outra conversa').uncheck();
    await recoveryPage.getByRole('button', { name: 'Conferir envio anterior' }).click();
    await recoveryPage.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await recoveryPage.evaluate(key => localStorage.getItem(key), uncertainKey), 'test-thread', 'An empty resumed history cannot confirm delivery');
    assert.equal(await recoveryPage.getByRole('button', { name: 'Já conferi o envio' }).isVisible(), true);
    assert.equal(await recoveryPage.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    assert.equal(await recoveryPage.evaluate(() => window.recoverySends), 0);
    await recoveryPage.reload();
    await recoveryPage.goto(`${url}#projects`);
    await recoveryPage.getByRole('button', { name: 'Abrir another-project na pasta D:\\another-project' }).click();
    await recoveryPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await recoveryPage.getByRole('button', { name: 'Conferir envio anterior' }).isVisible(), true, 'Unconfirmed delivery remains visible after reload');
    await recoveryPage.getByRole('button', { name: 'Conferir envio anterior' }).click();
    await recoveryPage.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    await recoveryPage.evaluate(() => { window.beforeReviewClear = Storage.prototype.removeItem; Storage.prototype.removeItem = () => { throw new Error('storage unavailable'); }; });
    await recoveryPage.getByRole('button', { name: 'Já conferi o envio' }).click();
    assert.equal(await recoveryPage.evaluate(key => localStorage.getItem(key), uncertainKey), 'test-thread');
    assert.equal(await recoveryPage.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    await recoveryPage.evaluate(() => { Storage.prototype.removeItem = window.beforeReviewClear; });
    await recoveryPage.getByRole('button', { name: 'Já conferi o envio' }).click();
    assert.equal(await recoveryPage.evaluate(key => localStorage.getItem(key), uncertainKey), null);
    assert.equal(await recoveryPage.getByRole('button', { name: 'Enviar', exact: true }).isEnabled(), true);
    assert.equal(await recoveryPage.evaluate(() => document.activeElement.id), 'agent-status', 'Focus must not disappear with the acknowledged button');
    assert.equal(await recoveryPage.evaluate(() => window.recoverySends), 0, 'Read-only resume must not send a new turn');
    await recoveryContext.close();
    const sendsBeforeReview = await page.evaluate(() => window.sendCalls);
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'confira o histórico' }).waitFor();
    assert.equal(await page.evaluate(() => window.sendCalls), sendsBeforeReview, 'An unconfirmed send must not retry automatically');
    assert.equal(await page.locator('#conversation-picker').evaluate(node => node.open), true, 'Recovery must reveal the history controls');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'connect-agent');
    if (!await page.locator('#conversation-picker').evaluate(node => node.open)) await page.locator('#conversation-picker summary').click();
    await page.getByLabel('Começar outra conversa').check();
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Retome a conversa anterior' }).waitFor();
    assert.equal(await page.evaluate(() => window.sendCalls), sendsBeforeReview);
    await page.getByLabel('Começar outra conversa').uncheck();
    await page.evaluate(() => {
      const invoke = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = async (command, args) => {
        if (command === 'connect_agent') return { thread_id: 'different-thread', messages: [], resumed: false };
        return invoke(command, args);
      };
      window.restoreResumeInvoke = invoke;
    });
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'não confirmou a retomada' }).waitFor();
    assert.equal(await page.evaluate(() => window.sendCalls), sendsBeforeReview);
    await page.evaluate(() => { window.__TAURI__.core.invoke = window.restoreResumeInvoke; });
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.evaluate(() => window.connectedThread), 'test-thread');
    assert.match(await page.locator('#composer-help').textContent(), /último envio não foi confirmado/);
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    await page.getByRole('button', { name: 'Já conferi o envio' }).click();
    assert.doesNotMatch(await page.locator('#composer-help').textContent(), /último envio não foi confirmado/);
    assert.equal(await page.evaluate(() => window.sendCalls), sendsBeforeReview, 'Reviewing history must not send a turn');
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    console.log('PASS: rejected send remains unconfirmed and cannot silently resend before explicit history review.');
    await page.evaluate(() => {
      const invoke = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = async (command, args) => {
        if (command === 'connect_agent') {
          args.events.onmessage({ kind: 'disconnected' });
          return { thread_id: 'already-ended-thread', messages: [], resumed: false };
        }
        return invoke(command, args);
      };
    });
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'A conexão foi encerrada' }).waitFor();
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isEnabled(), true);
    console.log('PASS: disconnect-before-connect-ack never enables sending.');
    // Long text and text enlargement must not hide controls or introduce sideways scrolling.
    await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    for (const width of [390, 1180]) {
      await page.setViewportSize({ width, height: 844 });
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
      // Single-line paths scroll within their field by design; button labels must not clip.
      assert.equal(await page.locator('button, input, textarea').evaluateAll(nodes => nodes.filter(n => n.getClientRects().length).every(n => (n.tagName === 'INPUT' || n.scrollWidth <= n.clientWidth + 2) && n.scrollHeight <= n.clientHeight + 2)), true);
    }
    await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
    await page.emulateMedia({ forcedColors: 'active' });
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isVisible(), true);
    await page.emulateMedia({ forcedColors: 'none', colorScheme: 'light' });
    await page.emulateMedia({ reducedMotion: 'reduce' });
    assert.equal(await page.locator('button, .status-icon').evaluateAll(nodes => nodes.every(node => getComputedStyle(node).animationName === 'none' && getComputedStyle(node).transitionDuration === '0s')), true);
    for (const colorScheme of ['light', 'dark']) {
      await page.emulateMedia({ colorScheme });
      await page.waitForFunction(theme => document.documentElement.dataset.theme === theme, colorScheme);
      const contrast = await page.evaluate(() => {
        const css = getComputedStyle(document.documentElement);
        const luminance = token => {
          const hex = css.getPropertyValue(token).trim().slice(1);
          const rgb = [0, 2, 4].map(i => parseInt(hex.slice(i, i + 2), 16) / 255).map(v => v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4);
          return rgb[0] * .2126 + rgb[1] * .7152 + rgb[2] * .0722;
        };
        return ['--ink', '--muted'].flatMap(text => ['--surface', '--canvas', '--soft'].map(bg => {
          const a = luminance(text), b = luminance(bg);
          return (Math.max(a, b) + .05) / (Math.min(a, b) + .05);
        }));
      });
      assert.ok(contrast.every(ratio => ratio >= 4.5), `${colorScheme} text contrast: ${contrast}`);
    }
    await page.emulateMedia({ colorScheme: 'light' });
    await page.setViewportSize({ width: 1440, height: 1080 });
    if (process.env.FORGE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_SCREENSHOT, fullPage: true });
    console.log('PASS: enlarged mobile text and forced-color controls.');
    await page.locator('.appearance summary').click();
    await page.getByLabel('Tema', { exact: true }).selectOption('dark');
    await page.getByLabel('Reforçar contraste').check();
    await page.reload();
    assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'dark');
    assert.equal(await page.evaluate(() => document.documentElement.hasAttribute('data-high-contrast')), true);
    await page.locator('.appearance summary').click();
    await page.getByLabel('Tema', { exact: true }).selectOption('light');
    await page.emulateMedia({ colorScheme: 'dark' });
    assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'light');
    await page.getByLabel('Tema', { exact: true }).selectOption('system');
    await page.waitForFunction(() => document.documentElement.dataset.theme === 'dark');
    await page.getByLabel('Reforçar contraste').focus();
    await page.keyboard.press('Space');
    assert.equal(await page.getByLabel('Reforçar contraste').isChecked(), false);
    await page.emulateMedia({ contrast: 'more' });
    assert.equal(await page.evaluate(() => {
      const css = getComputedStyle(document.documentElement);
      return ['--muted', '--line'].every(token => css.getPropertyValue(token).trim() === css.getPropertyValue('--ink').trim());
    }), true);
    await page.evaluate(() => { Storage.prototype.setItem = () => { throw new Error('Unavailable'); }; });
    await page.getByLabel('Tema', { exact: true }).selectOption('light');
    await page.locator('#appearance-status').filter({ hasText: 'não foi possível salvar' }).waitFor();
    assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'light');
    // A reload discards the in-page protocol double above. Establish a fresh,
    // connected workspace specifically for the project-switch contract.
    await page.goto(workspaceUrl);
    await page.evaluate(() => {
      window.disconnectCalls = 0;
      window.__TAURI__ = { core: {
        Channel: class {},
        invoke: async (command, args) => {
          if (command === 'start_project') return { project_id: 'test-project', project_root: args.projectRoot };
          if (command === 'inspect_project') return { project_id: 'test-project', project_root: args.projectRoot };
          if (command === 'connect_agent') {
            window.agentEvents = args.events;
            return { thread_id: 'switch-test-thread', messages: [], resumed: false };
          }
          if (command === 'disconnect_agent') window.disconnectCalls++;
        },
      } };
    });
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\first-project');
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conectado' }).waitFor();
    assert.equal(await page.locator('#project-root').isDisabled(), true, 'An active Codex connection locks direct folder editing');
    const disconnectsBeforeSwitch = await page.evaluate(() => window.disconnectCalls);
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'running' }));
    page.once('dialog', dialog => dialog.dismiss());
    await page.locator('#project-setup summary').click();
    assert.equal(await page.locator('#project-setup').evaluate(node => node.open), false);
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeSwitch);
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    page.once('dialog', dialog => dialog.dismiss());
    await page.getByRole('link', { name: /Arte e criação/ }).click();
    assert.equal(await page.evaluate(() => location.hash), '#explore', 'Declining interruption keeps the person on Explore');
    assert.equal(await page.locator('#confirmed-root').textContent(), 'D:\\first-project');
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeSwitch);
    assert.equal(await page.locator('#message-text').inputValue(), '', 'Declining interruption must not alter the existing draft');
    if (await page.locator('.appearance').evaluate(node => node.open)) await page.locator('.appearance summary').click();
    await page.getByRole('link', { name: 'Meus projetos' }).click();
    page.once('dialog', dialog => dialog.dismiss());
    await page.getByRole('link', { name: 'Abrir outro projeto' }).click();
    assert.equal(await page.evaluate(() => location.hash), '#projects');
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeSwitch);
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'completed' }));
    await page.getByRole('link', { name: 'Abrir outro projeto' }).click();
    await page.locator('#agent-status').filter({ hasText: 'Escolha uma pasta' }).waitFor();
    assert.equal(await page.locator('#project-setup').evaluate(node => node.open), true);
    assert.equal(await page.locator('#project-root').isEnabled(), true);
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeSwitch + 1);
    assert.equal(await page.locator('#project-root').inputValue(), '', 'Opening another project must not retain the previous folder');
    assert.equal(await page.locator('#project-result').isHidden(), true, 'The previous project must not appear confirmed during folder choice');
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true, 'Sending must wait for the new folder');
    assert.equal(await page.locator('#messages article').count(), 0, 'The previous transcript must not appear in the new-project view');
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\switched-project');
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await page.locator('#confirmed-root').textContent(), 'D:\\switched-project');
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conectado' }).waitFor();
    const disconnectsBeforeShortcut = await page.evaluate(() => window.disconnectCalls);
    await page.getByRole('link', { name: 'Meus projetos' }).click();
    await page.getByRole('button', { name: 'Abrir first-project na pasta D:\\first-project' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeShortcut + 1, 'Opening a different saved project must disconnect the old Codex conversation first');
    assert.equal(await page.locator('#confirmed-root').textContent(), 'D:\\first-project');
    assert.match(await page.locator('#agent-status').textContent(), /Projeto pronto\. Escreva sua ideia; a conversa abre quando você enviar\./);
    await page.evaluate(() => {
      const invoke = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = async (command, args) => {
        if (command === 'connect_agent') {
          window.agentEvents = args.events;
          return {
            thread_id: 'long-history-fixture', resumed: true,
            messages: Array.from({ length: 160 }, (_, index) => ({
              id: `history-${index}`, role: index % 2 ? 'agent' : 'user',
              text: `Histórico ${index}: ${'conteúdo legível '.repeat(20)}`, incomplete: false,
            })),
          };
        }
        return invoke(command, args);
      };
    });
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    const history = page.getByRole('region', { name: 'Histórico da conversa' });
    assert.equal(await page.locator('#messages article').count(), 160);
    assert.equal(await history.evaluate(node => node.scrollHeight > node.clientHeight && node.scrollHeight - node.clientHeight - node.scrollTop < 2), true);
    assert.equal(await composer.isVisible(), true);
    assert.equal(await composer.evaluate(node => {
      const box = node.getBoundingClientRect();
      return box.top < innerHeight && box.bottom > 0;
    }), true, 'The composer should be in the viewport after a long conversation opens');
    await page.waitForFunction(() => {
      const box = document.getElementById('send-message').getBoundingClientRect();
      return box.top >= 0 && box.bottom <= innerHeight + 2;
    }, null, { timeout: 2000 });
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).evaluate(node => {
      const box = node.getBoundingClientRect();
      return box.top >= 0 && box.bottom <= innerHeight + 2;
    }), true, 'Send should be in the viewport after a long conversation opens');
    await history.evaluate(node => { node.scrollTop = 0; });
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'message', id: 'new-while-reading', text: 'Resposta posterior' }));
    assert.equal(await history.evaluate(node => node.scrollTop), 0);
    await history.evaluate(node => { node.scrollTop = node.scrollHeight; });
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'message', id: 'new-at-bottom', text: 'Outra resposta\n'.repeat(40) }));
    assert.equal(await history.evaluate(node => node.scrollHeight - node.clientHeight - node.scrollTop < 2), true);
    await history.focus();
    assert.equal(await history.evaluate(node => document.activeElement === node), true);
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    const disconnectsBeforeExplore = await page.evaluate(() => window.disconnectCalls);
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.getByRole('link', { name: /Arte e criação/ }).click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeExplore + 1);
    assert.equal(await page.locator('#project-root').inputValue(), '');
    assert.equal(await page.locator('#project-result').isHidden(), true);
    assert.equal(await page.locator('#project-setup').evaluate(node => node.open), true);
    assert.equal(await page.locator('#messages article').count(), 0, 'Previous-project messages must not remain in the new-project view');
    assert.equal(await page.locator('#preview-cited-files').getAttribute('hidden'), '', 'Previous-project file citations must be cleared on project switch');
    assert.equal(await page.locator('#send-message').isDisabled(), true);
    assert.match(await page.locator('#message-text').inputValue(), /artístico/i);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    console.log('PASS: controlled long history stays scrollable in the conversation, resumes at latest, preserves earlier reading position, and follows new replies near the end.');
    console.log('PASS: project switching closes an idle connection; a running turn needs confirmation and does not switch when declined. Explore opens a fresh folder without showing the old transcript.');
    console.log('PASS: appearance survives reload, overrides OS, follows OS, supports keyboard and tolerates storage failure.');
    console.log('PASS: oversized Unicode remains recoverable, completion-before-ack preserves draft, pending disconnect locks controls, send rejection releases session.');
    console.log('PASS: desktop/mobile overflow, mobile text, retry, keyboard entry, dark theme, honest agent status. Native IPC NOT_RUN.');
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
