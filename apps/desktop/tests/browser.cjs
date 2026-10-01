// Optional focused browser check. It does not prove native WebView IPC.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { openConversationOptions, clickConversationAction } = require('./conversation-options.cjs');

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
  ['/conversation-focus.mjs', ['conversation-focus.mjs', 'text/javascript']],
  ['/message-format.mjs', ['message-format.mjs', 'text/javascript']],
  ['/conversation-reference.mjs', ['conversation-reference.mjs', 'text/javascript']],
  ['/assets/forge.png', ['assets/forge.png', 'image/png']],
  ['/assets/explore-artwork.png', ['assets/explore-artwork.png', 'image/png']],
  ['/appearance.js', ['appearance.js', 'text/javascript']],
  ['/progress.mjs', ['progress.mjs', 'text/javascript']],
  ['/direction-changes.mjs', ['direction-changes.mjs', 'text/javascript']],
  ['/preview.mjs', ['preview.mjs', 'text/javascript']],
  ['/mobile-workspace.mjs', ['mobile-workspace.mjs', 'text/javascript']],
  ['/workspace-layout.mjs', ['workspace-layout.mjs', 'text/javascript']],
  ['/assets/cinematic-atelier.png', ['assets/cinematic-atelier.png', 'image/png']],
]);
async function openProjectSetup(page) {
  if (!await page.locator('#project-setup').evaluate(node => node.open)) await page.locator('#project-setup > summary').click();
  if (!await page.locator('#custom-folder-option').evaluate(node => node.open)) await page.locator('#custom-folder-option > summary').click();
}
async function openConversation(page) {
  if (!await page.locator('#conversation-picker').evaluate(node => node.open)) await page.locator('#conversation-picker summary').click();
  await page.getByRole('button', { name: 'Conectar ao Codex', exact: true }).click();
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
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => {
      const brand = document.querySelector('header .brand').getBoundingClientRect();
      const alpha = document.querySelector('header .development').getBoundingClientRect();
      const nav = document.querySelector('header > nav').getBoundingClientRect();
      return alpha.top < brand.bottom && alpha.left >= brand.right && nav.top >= brand.bottom - 1;
    }), true, 'Narrow header should keep Alpha beside the brand and navigation below without overlap');
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
    assert.equal(await page.getByRole('region', { name: 'Histórico da conversa' }).isVisible(), false, 'Conversation remains hidden until a project is confirmed');
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
    for (const width of [1536, 1280, 1180, 1100]) {
      await page.setViewportSize({ width, height: 844 });
      const heading = await page.locator('.explore-heading > div').boundingBox();
      const search = await page.locator('.category-search').boundingBox();
      const cards = await page.locator('.category-grid').boundingBox();
      if (width >= 1160) {
        assert.ok(search.x >= heading.x + heading.width - 1, 'Wide Explore search must sit beside the invitation');
      } else {
        assert.ok(search.y >= heading.y + heading.height - 1, 'Narrow Explore search must stack below the invitation');
      }
      assert.ok(cards.y >= Math.max(heading.y + heading.height, search.y + search.height) - 1,
        'Explore cards must remain below the invitation and search');
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true,
        `Explore must not require horizontal scrolling at ${width}px`);
    }
    await page.setViewportSize({ width: 1280, height: 720 });
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
    await page.getByRole('button', { name: 'Encontrar atualização do Forge Desktop' }).click();
    await page.locator('#updates-status').filter({ hasText: 'Não foi possível abrir' }).waitFor();
    assert.match(await page.locator('#updates-url').textContent(), /github\.com\/DanielCarva1\/forge-method-core\/releases/);
    const updatesPage = await browser.newPage();
    await updatesPage.addInitScript(() => {
      window.updatesCalls = 0;
      window.__TAURI__ = { core: { invoke: async command => {
        if (command === 'app_info') return { name: 'Forge', version: '0.1.45' };
        if (command === 'open_updates_page') { window.updatesCalls++; if (window.updatesFail) throw new Error('browser unavailable'); }
      } } };
    });
    await updatesPage.goto(`${url}#about`);
    await updatesPage.locator('#app-version').filter({ hasText: 'Versão instalada: 0.1.45' }).waitFor();
    await updatesPage.getByRole('button', { name: 'Encontrar atualização do Forge Desktop' }).click();
    await updatesPage.locator('#updates-status').filter({ hasText: 'solicitada ao navegador' }).waitFor();
    assert.equal(await updatesPage.evaluate(() => window.updatesCalls), 1);
    assert.equal(await updatesPage.locator('#updates-url').isHidden(), true);
    await updatesPage.evaluate(() => { window.updatesFail = true; });
    await updatesPage.getByRole('button', { name: 'Encontrar atualização do Forge Desktop' }).click();
    await updatesPage.locator('#updates-status').filter({ hasText: 'Não foi possível abrir' }).waitFor();
    assert.equal(await updatesPage.locator('#updates-url').isVisible(), true);
    await updatesPage.close();
    await page.getByRole('link', { name: 'Minha conversa', exact: true }).click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#custom-folder-option summary').isVisible(), true);
    await page.locator('#custom-folder-option summary').click();
    assert.equal(await page.getByRole('button', { name: 'Continuar nesta pasta' }).isVisible(), true);
    if (process.env.FORGE_WORKSPACE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_WORKSPACE_SCREENSHOT, fullPage: true });
    console.log('PASS: Explore filters eight approachable themes and carries the chosen idea into the conversation.');
    const projectsPage = await browser.newPage();
    await projectsPage.addInitScript(() => {
      window.projectChecks = []; window.startCalls = []; window.previewReads = []; window.browserOpens = []; window.pdfBrowserOpens = [];
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
        if (command === 'open_pdf_in_browser') {
          window.pdfBrowserOpens.push(args);
          if (window.rejectBrowserOpen) throw 'Navegador indisponível';
          return null;
        }
        if (command === 'inspect_preview') {
          window.previewReads.push(args);
          if (args.filePath.includes('outside')) throw 'Este arquivo não pertence ao projeto aberto.';
          return window.previewKind === 'image'
            ? { kind: 'image', relative_path: 'result.png', size_bytes: 100, content: window.previewImage }
            : window.previewKind === 'file'
              ? { kind: 'file', relative_path: 'output/report.pdf', size_bytes: 2084, content: '' }
            : window.previewKind === 'otherFile'
              ? { kind: 'file', relative_path: 'output/archive.zip', size_bytes: 2084, content: '' }
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
    await projectsPage.locator('#browse-project').click();
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
    assert.match(await projectsPage.locator('#preview-intro').textContent(), /arquivos citados na conversa.*escolha um da pasta/);
    assert.equal(await projectsPage.locator('#preview-status').isVisible(), false, 'The untouched empty state does not repeat the same message');
    assert.equal(await projectsPage.evaluate(() => document.querySelector('#project-preview').getBoundingClientRect().top < document.querySelector('#project-record').getBoundingClientRect().top), true, 'The empty preview entry point is visible before the record');
    assert.deepEqual(await projectsPage.locator('.workspace > .panel').evaluateAll(nodes => nodes.map(node => node.id || (node.classList.contains('conversation') ? 'conversation' : 'project'))), ['project-conversation', 'project-preview', 'project-record', 'project-panel']);
    assert.equal(await projectsPage.locator('#refresh-preview').isVisible(), false, 'No refresh action before choosing a file');
    assert.equal(await projectsPage.locator('#project-record').isVisible(), true);
    assert.equal(await projectsPage.locator('.project #project-record').count(), 0, 'Record is a separate panel, not folder setup');
    await projectsPage.evaluate(() => { window.previewChoice = 'D:\\one\\result.txt'; });
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await projectsPage.evaluate(() => document.activeElement?.id), 'preview-heading', 'Choosing a result gives keyboard focus to its heading');
    assert.deepEqual(await projectsPage.locator('.workspace > .panel').evaluateAll(nodes => nodes.map(node => node.id || (node.classList.contains('conversation') ? 'conversation' : 'project'))), ['project-conversation', 'project-preview', 'project-record', 'project-panel']);
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
    assert.deepEqual(await projectsPage.evaluate(() => JSON.parse(localStorage.getItem('forge.preview-files.v1'))),
      [{ projectRoot: 'D:\\one', filePath: 'D:\\one\\result.txt' }], 'Only the selected file path, not result content, is remembered');
    const returnContext = await browser.newContext({ storageState: await projectsPage.context().storageState() });
    const returnPage = await returnContext.newPage();
    await returnPage.addInitScript(() => {
      window.returnPreviewReads = [];
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        if (command === 'app_info') return { name: 'Forge', version: '0.1.0' };
        if (command === 'inspect_project') return { project_id: 'first-project', project_root: args.projectRoot };
        if (command === 'inspect_preview') {
          window.returnPreviewReads.push(args);
          if (args.filePath.includes('outside')) throw 'Este arquivo não pertence ao projeto aberto.';
          return { kind: 'text', relative_path: 'result.txt', size_bytes: 16, content: 'Result read again after reopening' };
        }
      } } };
    });
    await returnPage.goto(`${url}#projects`);
    await returnPage.locator('#recent-projects .recent-project').getByRole('button', { name: /Abrir one na pasta/ }).click();
    await returnPage.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await returnPage.locator('#preview-text').textContent(), 'Result read again after reopening');
    assert.deepEqual(await returnPage.evaluate(() => window.returnPreviewReads),
      [{ projectRoot: 'D:\\one', filePath: 'D:\\one\\result.txt' }], 'Returning to a project must revalidate its last file through native IPC');
    await returnPage.evaluate(async () => {
      const { setPreviewProject } = await import('./preview.mjs');
      setPreviewProject({ project_root: 'D:\\two' });
    });
    assert.equal(await returnPage.locator('#preview-result').isHidden(), true, 'Another project must not show the prior result');
    assert.equal(await returnPage.evaluate(() => window.returnPreviewReads.length), 1, 'Another project must not read the prior file');
    await returnPage.evaluate(() => localStorage.setItem('forge.preview-files.v1', JSON.stringify([
      { projectRoot: 'D:\\one', filePath: 'D:\\outside.txt' },
    ])));
    await returnPage.reload();
    await returnPage.locator('nav a[data-route="projects"]').click();
    await returnPage.locator('#recent-projects .recent-project').getByRole('button', { name: /Abrir one na pasta/ }).click();
    await returnPage.locator('#preview-status').filter({ hasText: 'última prévia não está mais disponível' }).waitFor();
    assert.equal(await returnPage.locator('#preview-result').isHidden(), true, 'A rejected remembered path must never show a stale result');
    assert.equal(await returnPage.evaluate(() => JSON.parse(localStorage.getItem('forge.preview-files.v1')).length), 0,
      'A failed remembered shortcut should be removed rather than retried forever');
    await returnContext.close();
    assert.equal(await projectsPage.evaluate(() => !!(document.getElementById('request-preview-change').compareDocumentPosition(document.querySelector('.preview-origin')) & Node.DOCUMENT_POSITION_FOLLOWING)), true, 'Change request appears before result metadata and optional browser controls');
    assert.equal(await projectsPage.locator('#preview-result script').count(), 0);
    assert.match(await projectsPage.locator('#preview-result').textContent(), /Isso não confirma publicação na internet/);
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
    await projectsPage.setViewportSize({ width: 1280, height: 844 });
    assert.equal(await projectsPage.evaluate(() => {
      const conversation = document.getElementById('project-conversation').getBoundingClientRect();
      const preview = document.getElementById('project-preview').getBoundingClientRect();
      return document.querySelector('.workspace').classList.contains('preview-loaded')
        && preview.width >= conversation.width * 1.15 && document.documentElement.scrollWidth <= innerWidth;
    }), true, 'An actual result should receive a wider desktop preview without horizontal overflow');
    if (process.env.FORGE_PREVIEW_SCREENSHOT) await projectsPage.screenshot({ path: process.env.FORGE_PREVIEW_SCREENSHOT, fullPage: true });
    const mobileDraft = await projectsPage.locator('#message-text').inputValue();
    await projectsPage.setViewportSize({ width: 390, height: 844 });
    assert.equal(await projectsPage.locator('#mobile-workspace-nav').isVisible(), true);
    await projectsPage.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    assert.equal(await projectsPage.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'Large mobile text must not make the document scroll sideways');
    assert.equal(await projectsPage.locator('#mobile-workspace-nav button').evaluateAll(nodes => nodes.every(node => node.getBoundingClientRect().height >= 48)), true);
    await projectsPage.evaluate(() => { document.documentElement.style.fontSize = ''; });
    assert.equal(await projectsPage.locator('#project-conversation').isVisible(), true);
    assert.equal(await projectsPage.locator('#project-preview').isVisible(), false);
    assert.equal(await projectsPage.locator('#project-record').isVisible(), false);
    await projectsPage.locator('#mobile-workspace-nav').getByRole('button', { name: 'Resultado' }).click();
    assert.equal(await projectsPage.locator('#project-preview').isVisible(), true);
    assert.equal(await projectsPage.evaluate(() => document.activeElement?.dataset.mobilePaneButton), 'preview', 'Panel buttons retain keyboard focus');
    assert.equal(await projectsPage.locator('#preview-text').textContent(), '<script>primeiro</script>', 'Switching views must retain the actual loaded result');
    assert.equal(await projectsPage.locator('#project-conversation').isVisible(), false);
    assert.equal(await projectsPage.locator('#mobile-workspace-nav').getByRole('button', { name: 'Resultado' }).getAttribute('aria-pressed'), 'true');
    await projectsPage.locator('#mobile-workspace-nav').getByRole('button', { name: 'Andamento' }).click();
    assert.equal(await projectsPage.locator('#project-record').isVisible(), true);
    assert.equal(await projectsPage.evaluate(() => document.activeElement?.dataset.mobilePaneButton), 'progress');
    assert.equal(await projectsPage.locator('#project-panel').isVisible(), false, 'The record should not repeat the folder and connection card');
    await projectsPage.locator('#mobile-workspace-nav').getByRole('button', { name: 'Projeto' }).click();
    assert.equal(await projectsPage.locator('#project-panel').isVisible(), true, 'The project folder remains directly reachable');
    assert.equal(await projectsPage.locator('#project-record').isVisible(), false);
    assert.equal(await projectsPage.locator('#project-name').isVisible(), false, 'The narrow project card does not repeat the title above it');
    assert.equal(await projectsPage.locator('#workspace-title').isVisible(), true);
    assert.equal(await projectsPage.locator('#project-setup > summary').textContent(), 'Trocar de projeto');
    await projectsPage.locator('#project-setup > summary').click();
    assert.equal(await projectsPage.locator('#browse-project').isVisible(), true, 'The same folder picker remains reachable');
    await projectsPage.locator('#project-setup > summary').click();
    await projectsPage.locator('#mobile-workspace-nav').getByRole('button', { name: 'Projeto' }).focus();
    await projectsPage.keyboard.press('Shift+Tab');
    assert.equal(await projectsPage.evaluate(() => document.activeElement?.dataset.mobilePaneButton), 'progress');
    await projectsPage.keyboard.press('Enter');
    assert.equal(await projectsPage.locator('#project-record').isVisible(), true, 'Keyboard activation switches panels');
    await projectsPage.locator('#mobile-workspace-nav').getByRole('button', { name: 'Resultado' }).click();
    await projectsPage.getByRole('button', { name: 'Pedir mudança neste arquivo' }).click();
    assert.equal(await projectsPage.locator('#project-conversation').isVisible(), true, 'A change request must return to the same conversation');
    assert.equal(await projectsPage.evaluate(() => document.activeElement?.id), 'message-text');
    assert.match(await projectsPage.locator('#message-text').inputValue(), /Quero mudar o arquivo result\.txt: $/);
    assert.equal(await projectsPage.evaluate(() => window.previewRequestSubmits), 0, 'Changing panes must not send a message');
    await projectsPage.locator('#message-text').fill(mobileDraft);
    await projectsPage.setViewportSize({ width: 1280, height: 720 });
    assert.equal(await projectsPage.locator('#mobile-workspace-nav').isVisible(), false);
    assert.equal(await projectsPage.locator('#project-preview').isVisible(), true);
    assert.equal(await projectsPage.locator('#project-record').isVisible(), true);
    assert.equal(await projectsPage.locator('#project-panel').isVisible(), true, 'Wide layout still shows the project card');
    assert.equal(await projectsPage.locator('#project-name').isVisible(), true, 'Wide layout keeps the project name in its sidebar');
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
    await clickConversationAction(projectsPage, 'Ver texto original');
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
    assert.equal(await projectsPage.locator('#open-site-browser').evaluate(node => node.classList.contains('primary')), true, 'Trying a created site is the primary next action');
    assert.equal(await projectsPage.locator('#request-preview-change').evaluate(node => node.classList.contains('primary')), false);
    assert.equal(await projectsPage.locator('#preview-browser-action').evaluate(node => node.previousElementSibling?.id), 'preview-site', 'The browser action belongs beside the protected site preview');
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
    await projectsPage.evaluate(() => { window.previewKind = 'file'; window.previewChoice = 'D:\\one\\output\\report.pdf'; });
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Arquivo encontrado na pasta do projeto' }).waitFor();
    assert.equal(await projectsPage.evaluate(async () => {
      const { renderAgentMessage } = await import('./message-format.mjs');
      const target = document.createElement('div');
      renderAgentMessage(target, '[relatório](output/report.pdf)', () => {});
      return target.querySelector('button')?.dataset.previewPath;
    }), 'output/report.pdf', 'A cited PDF should be selectable from the conversation');
    assert.equal(await projectsPage.locator('#preview-file-note').isVisible(), true);
    assert.equal(await projectsPage.locator('#preview-heading').textContent(), 'PDF do projeto');
    assert.equal(await projectsPage.getByRole('button', { name: 'Atualizar informações' }).isVisible(), true);
    assert.equal(await projectsPage.locator('#preview-path').textContent(), 'output/report.pdf');
    assert.equal(await projectsPage.locator('#open-preview').isHidden(), true);
    assert.equal(await projectsPage.locator('#preview-text').isVisible(), false);
    assert.equal(await projectsPage.locator('#preview-browser-action').isVisible(), true);
    assert.match(await projectsPage.locator('#open-browser-hint').textContent(), /fora do Forge/);
    assert.equal(await projectsPage.locator('#preview-result').evaluate(node => {
      const ids = [...node.children].map(child => child.id);
      return ids.indexOf('preview-browser-action') < ids.indexOf('request-preview-change');
    }), true, 'Opening a PDF must precede the secondary conversation action in DOM and keyboard order');
    assert.equal(await projectsPage.locator('#open-site-browser').evaluate(node => node.classList.contains('primary')), true);
    assert.equal(await projectsPage.locator('#request-preview-change').evaluate(node => node.classList.contains('primary')), false);
    assert.deepEqual(await projectsPage.evaluate(() => window.pdfBrowserOpens), [], 'Selecting a PDF must not open the browser automatically');
    await projectsPage.getByRole('button', { name: 'Abrir PDF no navegador' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Abertura do PDF solicitada' }).waitFor();
    assert.deepEqual(await projectsPage.evaluate(() => window.pdfBrowserOpens), [{ projectRoot: 'D:\\one', filePath: 'D:\\one\\output\\report.pdf' }]);
    await projectsPage.evaluate(() => { window.rejectBrowserOpen = true; });
    await projectsPage.getByRole('button', { name: 'Abrir PDF no navegador' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Não foi possível abrir este PDF' }).waitFor();
    await projectsPage.setViewportSize({ width: 390, height: 844 });
    await projectsPage.locator('#mobile-workspace-nav').getByRole('button', { name: 'Resultado' }).click();
    assert.equal(await projectsPage.locator('#mobile-workspace-nav button').evaluateAll(nodes => nodes[0].getBoundingClientRect().top === nodes[1].getBoundingClientRect().top), true, 'Normal narrow text keeps two workspace choices per row');
    await projectsPage.setViewportSize({ width: 360, height: 844 });
    assert.equal(await projectsPage.locator('#mobile-workspace-nav button').evaluateAll(nodes => nodes[0].getBoundingClientRect().top === nodes[1].getBoundingClientRect().top), true, 'Minimum-width window keeps the compact two-column navigation at normal text size');
    await projectsPage.setViewportSize({ width: 390, height: 844 });
    await projectsPage.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    assert.equal(await projectsPage.locator('#mobile-workspace-nav button').evaluateAll(nodes => nodes[0].getBoundingClientRect().top !== nodes[1].getBoundingClientRect().top), true, 'Enlarged text gives workspace choices a full row');
    assert.equal(await projectsPage.locator('#mobile-workspace-nav button').evaluateAll(nodes => nodes.every(node => { const range = document.createRange(); range.selectNodeContents(node); return range.getClientRects().length === 1; })), true, 'Workspace choices must not split words across lines at 200% text');
    assert.equal(await projectsPage.locator('#project-preview').isVisible(), true);
    assert.equal(await projectsPage.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'PDF actions must not widen a 200% text narrow viewport');
    assert.equal(await projectsPage.getByRole('button', { name: 'Abrir PDF no navegador' }).evaluate(node => node.getBoundingClientRect().height >= 48), true);
    await projectsPage.locator('#mobile-workspace-nav').getByRole('button', { name: 'Conversa' }).click();
    await projectsPage.locator('#mobile-workspace-nav').getByRole('button', { name: 'Resultado' }).click();
    assert.equal(await projectsPage.locator('#preview-path').textContent(), 'output/report.pdf', 'Switching panes must retain the selected PDF');
    await projectsPage.evaluate(() => { document.documentElement.style.fontSize = ''; });
    await projectsPage.setViewportSize({ width: 1280, height: 720 });
    await projectsPage.evaluate(() => { window.rejectBrowserOpen = false; window.previewKind = 'otherFile'; window.previewChoice = 'D:\\one\\output\\archive.zip'; });
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Arquivo encontrado na pasta do projeto' }).waitFor();
    assert.equal(await projectsPage.locator('#preview-browser-action').isVisible(), false, 'Other unsupported files must not expose browser opening');
    assert.equal(await projectsPage.locator('#request-preview-change').evaluate(node => node.classList.contains('primary')), true);
    await projectsPage.evaluate(() => { window.previewKind = 'file'; window.previewChoice = 'D:\\one\\output\\report.pdf'; });
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Arquivo encontrado na pasta do projeto' }).waitFor();
    await projectsPage.getByRole('button', { name: 'Conversar sobre este arquivo' }).click();
    assert.match(await projectsPage.locator('#message-text').inputValue(), /Sobre o arquivo output\/report\.pdf:/);
    const shortcutDraftKey = await projectsPage.evaluate(() => `forge.draft.v1:${JSON.stringify([
      document.getElementById('confirmed-project-id').textContent,
      document.getElementById('confirmed-root').textContent,
    ])}`);
    assert.equal(await projectsPage.evaluate(key => localStorage.getItem(key), shortcutDraftKey), await projectsPage.locator('#message-text').inputValue(),
      'A preview change shortcut must save the same unsent composer text as typing does');
    assert.equal(await projectsPage.locator('#draft-note').isVisible(), true, 'The preview shortcut must show the local-draft notice');
    await projectsPage.evaluate(() => { window.previewKind = 'html'; window.previewChoice = 'D:\\one\\site\\index.html'; });
    await projectsPage.getByRole('button', { name: 'Escolher arquivo' }).click();
    await projectsPage.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await projectsPage.locator('#preview-file-note').isVisible(), false);
    assert.equal(await projectsPage.locator('#preview-result').evaluate(node => {
      const ids = [...node.children].map(child => child.id);
      return ids.indexOf('preview-browser-action') < ids.indexOf('request-preview-change');
    }), true, 'HTML lets the user try the site before requesting another change');
    assert.equal(await projectsPage.locator('#preview-heading').textContent(), 'Prévia do resultado');
    assert.equal(await projectsPage.getByRole('button', { name: 'Pedir mudança neste arquivo' }).isVisible(), true);
    await projectsPage.setViewportSize({ width: 390, height: 844 });
    assert.equal(await projectsPage.locator('#project-conversation').isVisible(), true);
    assert.equal(await projectsPage.locator('#project-preview').isVisible(), false);
    assert.equal(await projectsPage.locator('#project-record').isVisible(), false);
    assert.deepEqual(await projectsPage.locator('.workspace > .panel').evaluateAll(nodes => nodes.map(node => node.id || (node.classList.contains('conversation') ? 'conversation' : 'project'))), ['project-conversation', 'project-preview', 'project-record', 'project-panel']);
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
    assert.equal(await projectsPage.locator('#preview-result').isVisible(), true, 'A rejected replacement must preserve the last valid preview');
    assert.equal(await projectsPage.locator('#preview-path').textContent(), 'site/index.html');
    assert.equal(await projectsPage.locator('.workspace').evaluate(node => node.classList.contains('preview-loaded')), true);
    assert.deepEqual(await projectsPage.evaluate(() => window.previewReads.map(read => read.projectRoot)), Array(10).fill('D:\\one'));
    if (process.env.FORGE_CONFIRMED_SCREENSHOT) await projectsPage.screenshot({ path: process.env.FORGE_CONFIRMED_SCREENSHOT, fullPage: true });
    await openProjectSetup(projectsPage);
    await projectsPage.evaluate(() => { window.folderChoice = null; });
    await projectsPage.locator('#browse-project').click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Seleção cancelada' }).waitFor();
    assert.equal(await projectsPage.locator('#project-root').inputValue(), 'D:\\one');
    assert.equal(await projectsPage.locator('#project-result').isVisible(), true);
    await projectsPage.evaluate(() => { window.folderError = true; });
    await projectsPage.locator('#browse-project').click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Não foi possível abrir' }).waitFor();
    assert.equal(await projectsPage.locator('#project-root').inputValue(), 'D:\\one');
    await projectsPage.evaluate(() => { window.folderError = false; });
    await projectsPage.locator('#project-setup > summary').click();
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
    assert.equal(await projectsPage.evaluate(() => JSON.parse(localStorage.getItem('forge.preview-files.v1')).some(entry => entry.projectRoot === 'D:\\one')), false,
      'Removing a local project shortcut must also forget its last preview path');
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
    await projectsPage.getByRole('button', { name: 'Remover three da lista de projetos' }).click();
    assert.equal(await projectsPage.locator('.recent-project').count(), 1, 'A failed shortcut removal must remain visible rather than contradict stored state');
    assert.equal(await projectsPage.evaluate(() => JSON.parse(localStorage.getItem('forge.projects.v1')).length), 0, 'A failed shortcut write cannot change stored shortcuts');
    await projectsPage.locator('#projects-status').filter({ hasText: 'Não foi possível remover' }).waitFor();
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
    assert.equal(await projectsPage.locator('#send-message').isDisabled(), true, 'An empty confirmed-project composer must not offer a no-op Send');
    assert.equal(await projectsPage.locator('#agent-access-note').isVisible(), true);
    assert.match(await projectsPage.locator('#agent-access-note').textContent(), /fora da pasta escolhida.*sem pedir confirmação|sem pedir confirmação.*fora da pasta escolhida/);
    await projectsPage.locator('#conversation-picker summary').click();
    assert.equal(await projectsPage.locator('#connect-help').isVisible(), true);
    const composerSize = await projectsPage.locator('#message-text').evaluate(node => node.getBoundingClientRect().height);
    await projectsPage.locator('#message-text').fill('Uma ideia com detalhes.\n'.repeat(10));
    assert.equal(await projectsPage.locator('#send-message').isEnabled(), true, 'Typing a message enables Send');
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
    assert.equal(await projectsPage.getByRole('button', { name: 'Escolher pasta para continuar' }).isEnabled(), true, 'A suggested idea should lead directly to folder choice');
    assert.equal((await projectsPage.evaluate(() => window.startCalls)).length, startsBeforeNewIdea, 'Exploring an idea must not create a project');
    await projectsPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\home-fixture');
    await projectsPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await projectsPage.waitForFunction(() => document.getElementById('project-status').textContent.includes('Projeto pronto'));
    await projectsPage.getByRole('link', { name: 'Início', exact: true }).click();
    await projectsPage.getByRole('link', { name: 'Conversar sobre uma ideia' }).click();
    await projectsPage.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await projectsPage.locator('#project-root').inputValue(), '', 'The Home new-idea action also needs a fresh project choice');
    assert.equal(await projectsPage.locator('#project-result').isHidden(), true);
    assert.equal(await projectsPage.locator('#message-text').inputValue(), '', 'A fresh idea without a starter must not inherit the previous project draft');
    assert.equal(await projectsPage.getByRole('button', { name: 'Escolher pasta para continuar' }).isDisabled(), true, 'A fresh idea without text cannot be sent');
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
    await manyProjectsPage.waitForFunction(() => document.getElementById('project-status').textContent.includes('Projeto pronto'));
    assert.equal(await manyProjectsPage.locator('#confirmed-root').textContent(), 'D:\\Ideias\\Café');
    assert.equal(await manyProjectsPage.locator('#project-filter').inputValue(), '', 'Opening a project clears an old shortcut filter');
    assert.equal(await manyProjectsPage.evaluate(() => JSON.parse(localStorage.getItem('forge.projects.v1')).length), 50);
    await manyProjectsPage.close();
    console.log('PASS: 50 recent shortcuts, accent-insensitive search, distinct same-name paths, empty search and revalidated opening.');
    const reopenedPage = await browser.newPage();
    await reopenedPage.addInitScript(() => {
      window.reopenConnects = []; window.reopenSends = 0;
      window.__TAURI__ = { core: {
        Channel: class {},
        invoke: async (command, args) => {
          if (command === 'app_info') return { name: 'Forge', version: '0.1.1' };
          if (command === 'inspect_project') return { project_id: 'saved-project', project_root: args.projectRoot };
          if (command === 'connect_agent') {
            window.reopenConnects.push(args.threadId);
            return { thread_id: args.threadId, resumed: true, messages: [
              { id: 'prior-user', role: 'user', text: 'Minha ideia anterior.' },
              { id: 'prior-agent', role: 'agent', text: 'Podemos continuar daqui.' },
            ] };
          }
          if (command === 'send_message') window.reopenSends++;
        },
      } };
    });
    await reopenedPage.goto(`${url}#projects`);
    await reopenedPage.evaluate(() => {
      localStorage.setItem('forge.projects.v1', JSON.stringify([{ project_id: 'saved-project', project_root: 'D:\\saved' }]));
      localStorage.setItem('forge.conversation.v1:["saved-project","D:\\\\saved"]', 'saved-thread');
    });
    await reopenedPage.reload();
    await reopenedPage.getByRole('button', { name: 'Abrir saved na pasta D:\\saved' }).click();
    await reopenedPage.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.deepEqual(await reopenedPage.evaluate(() => window.reopenConnects), ['saved-thread']);
    assert.equal(await reopenedPage.locator('#messages article').count(), 2);
    assert.match(await reopenedPage.locator('#messages').textContent(), /Minha ideia anterior.*Podemos continuar daqui/s);
    assert.equal(await reopenedPage.evaluate(() => window.reopenSends), 0, 'Opening a saved project restores history without sending a turn');
    await reopenedPage.close();
    console.log('PASS: reopening a saved project displays its Codex conversation without another click or Send.');
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
      window.folderChoices = 0;
      window.projectStarts = 0;
      window.connectFailure = true;
      window.__TAURI__ = { core: {
        Channel: class {},
        invoke: async (command, args) => {
          if (command === 'app_info') return { name: 'Forge', version: '0.1.1' };
          if (command === 'choose_project_folder') { window.folderChoices++; return 'D:\\send-first'; }
          if (command === 'start_project') { window.projectStarts++; return { project_id: 'send-first', project_root: args.projectRoot }; }
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
    const firstDraft = sendFirstPage.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    await firstDraft.fill('Minha primeira ideia');
    await sendFirstPage.getByRole('button', { name: 'Escolher pasta para continuar' }).click();
    assert.equal(await sendFirstPage.locator('#project-root').inputValue(), 'D:\\send-first');
    assert.equal(await firstDraft.inputValue(), 'Minha primeira ideia');
    assert.equal(await sendFirstPage.evaluate(() => window.folderChoices), 1);
    assert.equal(await sendFirstPage.evaluate(() => window.projectStarts), 0, 'Selecting a folder must not start or modify the project');
    assert.equal(await sendFirstPage.evaluate(() => window.sendCalls), 0, 'Selecting a folder must not send the draft');
    await sendFirstPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await sendFirstPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.match(await sendFirstPage.locator('#agent-status').textContent(), /Projeto pronto/);
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
      window.copiedLoginCode = null;
      Object.defineProperty(navigator, 'clipboard', { value: { writeText: async text => {
        if (window.copyLoginFails) throw new Error('clipboard unavailable');
        window.copiedLoginCode = text;
      } } });
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
    await loginPage.locator('#login-code').filter({ hasText: 'ABCD-1234' }).waitFor();
    assert.equal(await loginPage.locator('#start-login').isHidden(), true);
    assert.equal(await loginPage.locator('#login-url').textContent(), 'https://auth.openai.com/codex/device');
    await loginPage.getByRole('button', { name: 'Copiar código' }).click();
    await loginPage.locator('#copy-login-status').filter({ hasText: 'Código copiado' }).waitFor();
    assert.equal(await loginPage.evaluate(() => window.copiedLoginCode), 'ABCD-1234');
    await loginPage.evaluate(() => { window.copyLoginFails = true; });
    await loginPage.getByRole('button', { name: 'Copiar código' }).click();
    await loginPage.locator('#copy-login-status').filter({ hasText: 'Selecione o código' }).waitFor();
    assert.equal(await loginDraft.inputValue(), 'Minha ideia permanece');
    await loginPage.setViewportSize({ width: 360, height: 720 });
    await loginPage.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    await loginPage.getByRole('button', { name: 'Copiar código' }).click();
    assert.equal(await loginPage.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'Enlarged first-use login must not scroll sideways');
    assert.equal(await loginPage.locator('#copy-login-code').isVisible(), true);
    const copyButtonBox = await loginPage.locator('#copy-login-code').boundingBox();
    const copyStatusBox = await loginPage.locator('#copy-login-status').boundingBox();
    assert.ok(copyButtonBox && copyStatusBox && copyStatusBox.y - (copyButtonBox.y + copyButtonBox.height) < 120,
      'Copy feedback should remain near the code at enlarged text size');
    await loginPage.setViewportSize({ width: 1180, height: 820 });
    await loginPage.evaluate(() => { document.documentElement.style.removeProperty('font-size'); });
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
    for (const selector of ['#project-setup > summary', '#project-root', '#browse-project', '#start-project', '#connection summary', '#retry', '.conversation-body']) {
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
    await page.getByText('Comece um projeto para conversar.', { exact: false }).waitFor();
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
          const revisions = [
            { active: false, origin: 'forge_cooperative_record', revision: 1, revision_kind: 'initial', outcome: 'Earlier direction', constraints: ['Keep files'], unacceptable_outcomes: [], accepted_at_unix: 1780000000 },
            { active: true, origin: 'forge_cooperative_record', revision: 2, revision_kind: 'material_supersession', outcome: '<script>Current direction</script>', constraints: [], unacceptable_outcomes: ['No deletion'], accepted_at_unix: 1781000000 },
          ];
          return { earlier_count: 0, revisions: window.historyOneRevision ? revisions.slice(1) : revisions };
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
    await page.setViewportSize({ width: 390, height: 420 });
    await page.locator('#message-text').click();
    const compactComposer = await page.evaluate(() => {
      const input = document.getElementById('message-text').getBoundingClientRect();
      const send = document.getElementById('send-message').getBoundingClientRect();
      return { inputTop: input.top, inputBottom: input.bottom, sendTop: send.top,
        sendBottom: send.bottom, viewportHeight: innerHeight,
        horizontalOverflow: document.documentElement.scrollWidth > innerWidth };
    });
    assert.equal(compactComposer.horizontalOverflow, false, 'The compact conversation must not scroll horizontally');
    assert.ok(compactComposer.inputTop >= 0 && compactComposer.inputBottom <= compactComposer.viewportHeight &&
      compactComposer.sendTop >= 0 && compactComposer.sendBottom <= compactComposer.viewportHeight,
    `Focused writing and Send should stay reachable together in a short viewport: ${JSON.stringify(compactComposer)}`);
    await page.setViewportSize({ width: 360, height: 720 });
    await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    await page.locator('#message-text').click();
    const enlargedComposer = await page.evaluate(() => {
      const input = document.getElementById('message-text').getBoundingClientRect();
      const send = document.getElementById('send-message').getBoundingClientRect();
      return { inputTop: input.top, inputBottom: input.bottom, sendTop: send.top,
        sendBottom: send.bottom, viewportHeight: innerHeight, documentWidth: document.documentElement.scrollWidth,
        viewportWidth: innerWidth };
    });
    assert.equal(enlargedComposer.documentWidth <= enlargedComposer.viewportWidth && enlargedComposer.inputTop >= 0 &&
      enlargedComposer.inputBottom <= enlargedComposer.viewportHeight && enlargedComposer.sendTop >= 0 &&
      enlargedComposer.sendBottom <= enlargedComposer.viewportHeight, true,
    `At 360px and 200% text, empty writing and Send must remain reachable: ${JSON.stringify(enlargedComposer)}`);
    const enlargedDraft = 'Uma ideia com detalhes.\n'.repeat(12);
    await page.locator('#message-text').fill(enlargedDraft);
    await page.locator('#send-message').focus();
    assert.equal(await page.evaluate(() => {
      const send = document.getElementById('send-message').getBoundingClientRect();
      return document.documentElement.scrollWidth <= innerWidth && send.top >= 0 && send.bottom <= innerHeight;
    }), true, 'Send must remain keyboard-reachable after a long enlarged draft');
    assert.equal(await page.locator('#message-text').inputValue(), enlargedDraft, 'Navigating to Send must not lose the long draft');
    await page.locator('#message-text').fill('');
    await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
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
      assert.equal(await page.evaluate(() => {
        const input = document.getElementById('message-text').getBoundingClientRect();
        const send = document.getElementById('send-message').getBoundingClientRect();
        const notice = document.getElementById('agent-access-note').getBoundingClientRect();
        return input.width >= 180 && send.left >= input.right - 2 &&
          send.top < input.bottom && send.bottom > input.top && notice.bottom <= input.top;
      }), true, `Writing and Send should share one composer row while the capability notice stays visible at ${width}x${height}`);
      assert.equal(await page.evaluate(() => {
        const conversation = document.getElementById('project-conversation').getBoundingClientRect();
        const preview = document.getElementById('project-preview').getBoundingClientRect();
        return !document.querySelector('.workspace').classList.contains('preview-loaded')
          && conversation.width >= preview.width * 1.15 && document.documentElement.scrollWidth <= innerWidth;
      }), true, 'Before a result exists, desktop writing should have more space than the empty preview');
      if (width === 1180 && process.env.FORGE_COMPOSER_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_COMPOSER_SCREENSHOT });
      if (width === 1180) assert.equal(await page.evaluate(() => {
        window.scrollTo(0, 0);
        const preview = document.getElementById('project-preview').getBoundingClientRect();
        const next = document.getElementById('record-next').getBoundingClientRect();
        return preview.top >= 0 && preview.bottom < next.top && next.top < innerHeight;
      }), true, 'Preview choice and the recorded next step should be discoverable alongside an empty conversation at 1180x820');
    }
    await page.setViewportSize({ width: 901, height: 844 });
    await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    assert.equal(await page.evaluate(() => {
      const conversation = document.getElementById('project-conversation');
      const preview = document.getElementById('project-preview');
      return document.documentElement.scrollWidth <= innerWidth
        && conversation.scrollWidth <= conversation.clientWidth + 1
        && preview.scrollWidth <= preview.clientWidth + 1;
    }), true, 'Intermediate desktop width and 200% text must not clip the conversation or preview');
    await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
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
    }), true, 'The compact empty preview must leave the project record visible in the initial desktop viewport');
    assert.equal(await page.evaluate(() => window.progressCalls || 0), 1);
    assert.equal(await page.locator('#workspace-phase').textContent(), 'Em andamento');
    assert.equal(await page.locator('#workspace-phase').isVisible(), true);
    assert.equal(await page.locator('#record-activity').textContent(), 'Recorded activity');
    assert.equal(await page.getByRole('group', { name: 'Atividade e próximo passo registrados' }).isVisible(), true);
    assert.equal(await page.locator('#record-activity').isVisible(), false, 'Technical activity should not dominate the first read');
    assert.equal(await page.locator('#record-next').isVisible(), true, 'The recorded next step is readable without opening details');
    await page.locator('#record-activity-details summary').click();
    assert.equal(await page.locator('#record-activity').isVisible(), true, 'Original recorded activity remains available on request');
    await page.locator('#record-activity-details summary').click();
    assert.equal(await page.locator('#record-outcome').isVisible(), true, 'The recorded objective should be visible before optional process details');
    assert.equal(await page.locator('#record-outcome').textContent(), 'Accepted outcome');
    await page.locator('#message-text').fill('Minha ideia continua aqui.');
    await page.getByRole('button', { name: 'Entender isto na conversa' }).click();
    assert.match(await page.locator('#message-text').inputValue(), /^Minha ideia continua aqui\.\n\nExplique em linguagem simples/);
    const recordDraftKey = await page.evaluate(() => `forge.draft.v1:${JSON.stringify([
      document.getElementById('confirmed-project-id').textContent,
      document.getElementById('confirmed-root').textContent,
    ])}`);
    assert.equal(await page.evaluate(key => localStorage.getItem(key), recordDraftKey), await page.locator('#message-text').inputValue(),
      'A Forge record shortcut must save its complete unsent prompt, not only the earlier typed text');
    assert.equal(await page.locator('#draft-note').isVisible(), true, 'The Forge record shortcut must show the local-draft notice');
    assert.equal(await page.evaluate(() => document.activeElement?.id), 'message-text');
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Explaining the record must not send a turn');
    await page.setViewportSize({ width: 390, height: 844 });
    await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Andamento' }).click();
    await page.getByRole('button', { name: 'Entender isto na conversa' }).click();
    assert.equal(await page.locator('#project-conversation').isVisible(), true, 'A record explanation returns to the same mobile conversation');
    assert.equal(await page.evaluate(() => document.activeElement?.id), 'message-text');
    assert.equal(await page.evaluate(() => window.sendCalls), 0);
    await page.setViewportSize({ width: 1280, height: 720 });
    await page.locator('#message-text').fill('');
    assert.equal(await page.locator('#record-pending').isVisible(), true);
    assert.equal(await page.locator('#record-questions-shortcut').isVisible(), true);
    assert.equal(await page.locator('#record-questions-shortcut').textContent(), 'Ver perguntas');
    assert.equal(await page.locator('#explain-pending').isVisible(), false, 'A suggestion alone is not a recovered pending decision');
    assert.equal(await page.locator('#record-pending').evaluate(node => !!(node.compareDocumentPosition(document.getElementById('record-direction-card')) & Node.DOCUMENT_POSITION_FOLLOWING)), true,
      'Unresolved questions should precede the optional technical direction and history');
    await page.locator('#record-questions-shortcut').click();
    assert.equal(await page.evaluate(() => document.activeElement?.id), 'record-pending-heading');
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'The questions shortcut must not choose or send anything');
    assert.equal(await page.locator('#record-suggestion-note').isVisible(), false, 'Suggestion-only copy should not repeat the same warning');
    assert.equal(await page.locator('#record-pending-heading').textContent(), 'Perguntas para explorar');
    assert.match(await page.locator('#record-pending-count').textContent(), /sugeriu perguntas para explorar/);
    assert.doesNotMatch(await page.locator('#record-pending-count').textContent(), /escolha pendente/);
    assert.equal(await page.locator('#record-suggested').evaluate(node => node.open), false,
      'Technical Forge suggestions should not fill the default project view');
    assert.equal(await page.locator('#record-suggestions').isVisible(), false);
    assert.equal(await page.getByRole('button', { name: 'Entender sugestões na conversa' }).isVisible(), true);
    await page.locator('#message-text').fill('Quero entender meu projeto.');
    await page.getByRole('button', { name: 'Entender sugestões na conversa' }).click();
    assert.match(await page.locator('#message-text').inputValue(), /^Quero entender meu projeto\.\n\nConsulte as perguntas que o Forge sugere/);
    assert.match(await page.locator('#message-text').inputValue(), /Não trate sugestões como decisões minhas/);
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Explaining suggestions must not send or register a decision');
    await page.locator('#message-text').fill('');
    await page.locator('#record-suggested summary').click();
    assert.equal(await page.locator('#record-suggestions').isVisible(), true);
    assert.match(await page.locator('#record-suggestions').textContent(), /Can test sooner/);
    assert.match(await page.locator('#record-suggestions').textContent(), /sugestão do Forge, não uma decisão sua/);
    assert.equal(await page.locator('#record-suggestions script').count(), 0);
    assert.equal(await page.evaluate(() => window.historyCalls || 0), 0, 'History must be opt-in');
    assert.equal(await page.locator('#explain-direction-history').isVisible(), false);
    await page.locator('#direction-history summary').click();
    await page.locator('#direction-history-status').filter({ hasText: '2 direções registradas' }).waitFor();
    assert.equal(await page.evaluate(() => window.historyCalls), 1);
    assert.equal(await page.locator('#explain-direction-history').isVisible(), true);
    await page.getByRole('button', { name: 'Entender mudanças na conversa' }).click();
    assert.match(await page.locator('#message-text').inputValue(), /histórico do objetivo registrado deste projeto/);
    assert.match(await page.locator('#message-text').inputValue(), /Se houver versões anteriores/);
    assert.match(await page.locator('#message-text').inputValue(), /Não presuma minha aprovação/);
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Explaining objective history must remain an unsent draft');
    await page.locator('#message-text').fill('');
    assert.match(await page.locator('#direction-history-list article').first().textContent(), /Direção atual/);
    assert.match(await page.locator('#direction-history-list article').last().textContent(), /Direção anterior/);
    await page.locator('#direction-history-list article').first().locator('summary').click();
    assert.equal(await page.locator('#direction-history-list article').first().locator('details').evaluate(node => node.open), true);
    assert.match(await page.locator('#direction-history-list article').first().textContent(), /O que mudou/);
    assert.match(await page.locator('#direction-history-list article').first().textContent(), /Combinados retirados/);
    assert.equal(await page.locator('#direction-history-list script').count(), 0, 'Recorded text must be literal');
    assert.match(await page.locator('#direction-history-list').textContent(), /não prova aprovação humana independente/);
    await page.evaluate(() => { window.historyOneRevision = true; });
    await page.getByRole('button', { name: 'Consultar histórico' }).click();
    await page.locator('#direction-history-status').filter({ hasText: '1 direção registrada' }).waitFor();
    assert.doesNotMatch(await page.locator('#direction-history-status').textContent(), /As versões anteriores/);
    assert.equal(await page.locator('#direction-history-list article').count(), 1);
    await page.locator('#message-text').fill('');
    await page.getByRole('button', { name: 'Entender esta direção na conversa' }).click();
    assert.match(await page.locator('#message-text').inputValue(), /se não houver, diga isso sem inventar mudanças/);
    assert.equal(await page.evaluate(() => window.sendCalls), 0);
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
    await page.locator('#record-activity-details summary').click();
    assert.equal(await page.locator('#record-activity').isVisible(), true);
    await page.evaluate(() => { window.progressRevision = 2; window.progressRevisionKind = 'material_supersession'; window.progressConstraint = '<script>changed</script>'; window.progressRecordedPending = 1; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.equal(await page.locator('#record-activity-details').evaluate(node => node.open), false, 'Refreshing should close old activity details');
    assert.equal(await page.locator('#record-direction-card').evaluate(node => node.open), false, 'Refreshing returns long objective details to their quiet state');
    await page.locator('#record-direction-card > summary').click();
    await page.locator('#record-direction summary').click();
    assert.match(await page.locator('#record-revision').textContent(), /Direção revista.*revisão 2/);
    assert.equal(await page.locator('#record-constraints-list li').textContent(), '<script>changed</script>');
    assert.equal(await page.locator('#record-direction script').count(), 0);
    assert.match(await page.locator('#record-pending-count').textContent(), /Há 1 escolha em aberto/);
    assert.match(await page.locator('#record-pending-count').textContent(), /pergunta original dessa escolha não está disponível/);
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
        assert.equal(await page.locator('#record-state').isVisible(), false, 'The empty record must not repeat its status as a badge');
        assert.equal(await page.locator('#record-start-conversation').isVisible(), true);
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
        assert.equal(await page.locator('#record-state').isVisible(), true);
        assert.equal(await page.locator('#record-start-conversation').isVisible(), false);
        assert.equal(await page.locator('#workspace-phase').isVisible(), true);
        assert.equal(await page.locator('.record-stage').isVisible(), true);
        assert.equal(await page.locator('#record-empty-help').isVisible(), false);
        assert.equal(await page.locator('#record-phase-label').textContent(), 'ETAPA GERAL DO PROJETO');
        if (state === 'stale') assert.equal(await page.locator('#workspace-phase').textContent(), 'Acompanhamento desatualizado');
        assert.equal(await page.locator('#record-state').textContent(), stateNames[state]);
        assert.equal(await page.locator('#progress-result').getAttribute('data-state'), state);
        assert.equal(await page.locator('#record-phase').textContent(), 'Descoberta');
        assert.equal(await page.locator('#record-phase').isVisible(), false, 'Process stage starts collapsed');
        await page.locator('.record-stage summary').click();
        assert.equal(await page.locator('#record-phase').isVisible(), true, 'Process stage remains available on request');
        await page.locator('.record-stage summary').click();
        assert.equal(await page.locator('#record-activity-label').textContent(), state === 'completed' ? 'Resultado registrado' : state === 'abandoned' ? 'Último registro' : 'Agora');
        if (state === 'completed') {
          assert.match(await page.locator('#progress-status').textContent(), /Esta parte do trabalho foi concluída. O projeto pode continuar/);
          assert.doesNotMatch(await page.locator('#record-phase-help').textContent(), /Este trabalho foi concluído/,
            'The phase description should not repeat the completed-work status already shown above');
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
    assert.match(await page.locator('#progress-status').textContent(), /Ainda não há um próximo passo registrado/);
    assert.equal(await page.locator('#workspace-phase').textContent(), 'Próximo passo ainda não registrado');
    assert.match(await page.locator('#record-empty-help').textContent(), /Comece pela conversa\..*arquivos continuam na pasta escolhida/);
    assert.equal(await page.locator('#record-state').isVisible(), false);
    const messagesBeforeEmptyAction = await page.locator('#messages article[data-role="user"]').count();
    await page.getByRole('button', { name: 'Conversar sobre meu projeto' }).click();
    assert.equal(await page.evaluate(() => document.activeElement?.id), 'message-text', 'The empty-record action opens the same unsent composer');
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), messagesBeforeEmptyAction, 'Opening the composer does not send');
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
    assert.equal(await page.locator('#explain-pending').isVisible(), false);
    await page.evaluate(() => { window.progressRecordedPending = 1; });
    await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
    assert.equal(await page.locator('#record-pending').isVisible(), true, 'A recorded pending decision remains visible without a current suggestion');
    assert.equal(await page.locator('#explain-pending').isVisible(), true);
    assert.equal(await page.locator('#explain-suggestions').isVisible(), false);
    assert.equal(await page.locator('#record-pending-heading').textContent(), 'Escolhas em aberto');
    assert.equal(await page.locator('#record-questions-shortcut').textContent(), 'Ver escolha em aberto');
    assert.equal(await page.locator('#record-suggestion-note').isVisible(), false);
    assert.match(await page.locator('#record-pending-count').textContent(), /Há 1 escolha em aberto/);
    await page.locator('#message-text').fill('Minha pergunta original.');
    await page.setViewportSize({ width: 390, height: 844 });
    await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Andamento' }).click();
    await page.getByRole('button', { name: 'Entender escolhas em aberto' }).click();
    assert.equal(await page.locator('#project-conversation').isVisible(), true, 'Pending-decision explanation opens the same narrow conversation');
    assert.match(await page.locator('#message-text').inputValue(), /^Minha pergunta original\.\n\nConsulte o registro do Forge/);
    assert.match(await page.locator('#message-text').inputValue(), /Se não conseguir recuperá-lo, diga isso claramente/);
    assert.match(await page.locator('#message-text').inputValue(), /não registre uma decisão/);
    assert.equal(await page.evaluate(() => window.sendCalls), 0, 'Explaining a pending decision must not send or record a choice');
    await page.locator('#message-text').fill('');
    await page.setViewportSize({ width: 1280, height: 720 });
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
    assert.match(await page.locator('#record-pending-count').textContent(), /Há 1 escolha em aberto/);
    assert.equal(await page.locator('#record-questions-shortcut').textContent(), 'Ver escolha em aberto');
    assert.equal(await page.locator('#explain-suggestions').isVisible(), false, 'The pending choice remains the single primary explanation action');
    assert.equal(await page.locator('#record-suggested summary').textContent(), 'Ver perguntas sugeridas (opcional)');
    assert.equal(await page.locator('#record-suggestion-note').isVisible(), false, 'A warning about optional questions stays with those hidden questions');
    assert.equal(await page.locator('#record-suggested').evaluate(node => node.open), false);
    await page.locator('#record-suggested summary').click();
    assert.equal(await page.locator('#record-suggestion-note').isVisible(), true);
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
    assert.equal(await page.locator('#explain-pending').isVisible(), false, 'A failed refresh must not leave a stale decision action available');
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
    await page.getByRole('button', { name: 'Conectar ao Codex', exact: true }).focus();
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
    assert.equal(await page.locator('#conversation-options summary').isVisible(), true);
    assert.equal(await page.locator('#disconnect-agent').isHidden(), true);
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
    assert.equal(await page.evaluate(() => document.activeElement.tagName === 'SUMMARY' && document.activeElement.parentElement.id === 'conversation-options'), true);
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
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'running' }));
    for (const [activity, label] of [
      ['checking', 'conferindo o projeto'],
      ['editing', 'alterando arquivos'],
      ['replying', 'preparando a resposta'],
      ['private protocol text', 'trabalhando'],
    ]) {
      await page.evaluate(text => window.agentEvents.onmessage({ kind: 'activity', text }), activity);
      assert.match(await page.locator('#agent-status').textContent(), new RegExp(label));
      assert.equal((await page.locator('#agent-status').textContent()).includes('private protocol text'), false);
    }
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'completed' }));
    await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor();
    for (const kind of ['disconnected', 'update_required']) {
      await page.evaluate(() => window.agentEvents.onmessage({ kind: 'running' }));
      assert.equal(await page.locator('#progress-result').isVisible(), false);
      const priorProgressReads = await page.evaluate(() => window.progressCalls || 0);
      await page.evaluate(kind => window.agentEvents.onmessage({ kind }), kind);
      await page.waitForFunction(previous => (window.progressCalls || 0) > previous, priorProgressReads);
      await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
      assert.equal(await page.locator('#progress-result').isVisible(), true, `${kind} must restore the independent Forge record without a manual click`);
      await clickConversationAction(page, 'Desconectar');
      await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
      await openConversation(page);
      await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
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
    const formattedReply = '# Plano\n- **Criar** uma tela\n- Mostrar `resultado` em `site/index.html`; `https://example.com/outside.html` é apenas texto.\n\n```rust\n// site/index.html must remain code, not an action\nfn main() { println!("<script>"); }\n```\n> Confirme o **resultado** antes de publicar.\n\n| Etapa | Situação | Observação |\n| --- | --- | --- |\n| Tela | `pronta` | Leia antes de publicar |\n| Arquivo | [abrir](result.txt) | Local |\n\n---\n<script>alert(1)</script>\n[arquivo](result.txt) [fora](D:/outside.md) [web](https://example.com) [cred](https://@example.com) [abrir](javascript:alert(1))';
    await page.evaluate(text => window.agentEvents.onmessage({ kind: 'delta', id: 'formatted-reply', text }), formattedReply.slice(0, 24));
    assert.equal(await page.locator('#messages article[data-role="agent"]').last().locator('h3').count(), 0);
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
    assert.equal(await formattedBubble.locator('.message-web-link').count(), 1, 'An HTTPS citation needs an explicit browser action');
    assert.equal(await page.locator('#preview-last-result').getAttribute('hidden'), '', 'Several distinct files must not be guessed as one result');
    assert.equal(await formattedBubble.locator('.message-result-action').count(), 0, 'Several cited files must not produce one guessed action beside the reply');
    assert.equal(await formattedBubble.locator('.message-file-choices summary').textContent(), 'Conferir 3 arquivos da resposta');
    assert.equal(await formattedBubble.locator('.message-file-choices').evaluate(node => node.open), true, 'A short new file list should be visible without another click');
    assert.equal(await formattedBubble.locator('.message-file-choices-list button').count(), 3, 'The reply must offer each cited file without guessing one');
    await formattedBubble.locator('.message-file-choices summary').click();
    assert.equal(await formattedBubble.locator('.message-file-choices').evaluate(node => node.open), false, 'People may fold the short list again');
    assert.equal(await page.locator('#preview-cited-files').isVisible(), true, 'Several cited files should remain individually available beside the preview');
    await page.locator('#preview-cited-files summary').click();
    assert.equal(await page.locator('#preview-cited-files-list button').count(), 3, 'Repeated citations should not duplicate a choice');
    assert.equal(await formattedBubble.getByRole('button', { name: 'Ver arquivo local: site/index.html' }).count(), 1, 'An inline-code file reference should offer the same safe preview action');
    assert.equal(await formattedBubble.locator('code').filter({ hasText: 'https://example.com/outside.html' }).count(), 1, 'An inline-code URL must remain inert text');
    assert.equal(await formattedBubble.locator('.message-web-link').textContent(), 'web ↗');
    assert.match(await formattedBubble.textContent(), /\[cred\]\(https:\/\/@example\.com\)/);
    assert.match(await formattedBubble.textContent(), /\[abrir\]\(javascript:alert\(1\)\)/);
    assert.match(await formattedBubble.textContent(), /<script>alert\(1\)<\/script>/);
    await page.evaluate(() => {
      window.externalLinkCalls = [];
      const invoke = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = (command, args) => {
        if (command === 'open_external_link') { window.externalLinkCalls.push(args); return Promise.resolve(); }
        return invoke(command, args);
      };
    });
    await formattedBubble.locator('.message-web-link').click();
    await page.locator('#action-confirmation').waitFor({ state: 'visible' });
    if (process.env.FORGE_CONFIRM_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_CONFIRM_SCREENSHOT });
    assert.equal(await page.locator('#action-confirmation-address').textContent(), 'https://example.com/');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'action-confirmation-cancel', 'Cancel is the safe default');
    const confirmationViewport = page.viewportSize();
    await page.setViewportSize({ width: 390, height: 844 });
    await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'The confirmation must not widen a small screen at 200% text');
    assert.equal(await page.locator('#action-confirmation-cancel').isVisible(), true);
    await page.evaluate(() => { document.documentElement.style.removeProperty('font-size'); });
    await page.setViewportSize(confirmationViewport);
    await page.keyboard.press('Escape');
    await page.locator('#action-confirmation').waitFor({ state: 'hidden' });
    assert.equal(await page.evaluate(() => window.externalLinkCalls.length), 0, 'Escape cannot launch a browser');
    await formattedBubble.locator('.message-web-link').click();
    await page.locator('#action-confirmation-cancel').click();
    assert.equal(await page.evaluate(() => window.externalLinkCalls.length), 0, 'Declining the URL cannot launch a browser');
    await formattedBubble.locator('.message-web-link').click();
    await page.locator('#action-confirmation-accept').click();
    await formattedBubble.locator('.message-web-status').filter({ hasText: 'Abertura solicitada' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.externalLinkCalls), [{ url: 'https://example.com/' }]);
    await clickConversationAction(page, 'Ver texto original');
    assert.equal(await formattedBubble.locator('pre.message-raw').textContent(), formattedReply);
    await page.getByRole('button', { name: 'Ver texto formatado' }).click();
    assert.equal(await formattedBubble.locator('h3').textContent(), 'Plano');
    assert.equal(await formattedBubble.locator('.message-file-choices').evaluate(node => node.open), false, 'Switching text views must preserve the choice to fold files');
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
    for (const kind of ['interrupted', 'failed']) {
      const expected = `Arquivo alterado antes de ${kind}`;
      const readsBefore = await page.evaluate(() => window.linkPreviewReads.length);
      await page.evaluate(({ kind, expected }) => { window.linkPreviewContent = expected; window.agentEvents.onmessage({ kind }); }, { kind, expected });
      await page.locator('#preview-text').filter({ hasText: expected }).waitFor();
      assert.equal(await page.evaluate(() => window.linkPreviewReads.length), readsBefore + 1, `${kind} must re-read only the already selected file`);
    }
    await page.evaluate(() => {
      window.previewRaceReads = 0;
      window.pauseNextPreviewRead = true;
      const previous = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = (command, args) => {
        if (command !== 'inspect_preview') return previous(command, args);
        window.previewRaceReads++;
        if (window.rejectNextPreviewRead) {
          window.rejectNextPreviewRead = false;
          return new Promise((_, reject) => {
            window.releaseRejectedPreviewRead = () => reject('Este arquivo não está mais disponível.');
          });
        }
        if (!window.pauseNextPreviewRead) return previous(command, args);
        window.pauseNextPreviewRead = false;
        return new Promise(resolve => {
          window.releaseOldPreviewRead = () => resolve({ kind: 'text', content: 'Antes do fim do turno', relative_path: 'result.txt', size_bytes: 19 });
        });
      };
    });
    await page.getByRole('button', { name: 'Atualizar prévia' }).click();
    await page.waitForFunction(() => typeof window.releaseOldPreviewRead === 'function');
    await page.evaluate(() => {
      window.linkPreviewContent = 'Depois do fim do turno';
      window.agentEvents.onmessage({ kind: 'failed' });
      window.releaseOldPreviewRead();
    });
    await page.locator('#preview-text').filter({ hasText: 'Depois do fim do turno' }).waitFor({ timeout: 5000 });
    assert.equal(await page.evaluate(() => window.previewRaceReads), 2, 'A stopped turn during an in-flight preview read must schedule one fresh read');
    await page.evaluate(() => { window.rejectNextPreviewRead = true; });
    await page.getByRole('button', { name: 'Atualizar prévia' }).click();
    await page.waitForFunction(() => typeof window.releaseRejectedPreviewRead === 'function');
    await page.evaluate(() => {
      window.agentEvents.onmessage({ kind: 'failed' });
      window.releaseRejectedPreviewRead();
    });
    await page.locator('#preview-status').filter({ hasText: 'Este arquivo não está mais disponível' }).waitFor();
    assert.equal(await page.evaluate(() => window.previewRaceReads), 3, 'A failed in-flight read must not be retried automatically');
    await page.getByRole('button', { name: 'Atualizar prévia' }).click();
    await page.locator('#preview-text').filter({ hasText: 'Depois do fim do turno' }).waitFor();
    assert.equal(await page.evaluate(() => window.previewRaceReads), 4, 'The person can still retry a failed read explicitly');
    await page.evaluate(() => {
      const previous = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = (command, args) => command === 'choose_preview_file'
        ? new Promise(resolve => { window.cancelHeldPreviewPicker = () => resolve(null); })
        : previous(command, args);
    });
    await page.getByRole('button', { name: 'Escolher arquivo' }).click();
    await page.waitForFunction(() => typeof window.cancelHeldPreviewPicker === 'function');
    await page.evaluate(() => {
      window.linkPreviewContent = 'Alterado enquanto escolhia arquivo';
      window.agentEvents.onmessage({ kind: 'completed' });
      window.cancelHeldPreviewPicker();
    });
    await page.locator('#preview-text').filter({ hasText: 'Alterado enquanto escolhia arquivo' }).waitFor({ timeout: 5000 });
    assert.equal(await page.evaluate(() => window.previewRaceReads), 5, 'Canceling the picker after a turn must refresh the previously selected file once');
    await page.getByRole('button', { name: 'Abrir prévia' }).click();
    const readsBeforeDialogFailure = await page.evaluate(() => window.linkPreviewReads.length);
    await page.evaluate(() => { window.linkPreviewContent = 'Arquivo mudado durante leitura'; window.agentEvents.onmessage({ kind: 'failed' }); });
    assert.equal(await page.locator('#preview-dialog').evaluate(node => node.open), true, 'A failed turn must not close an enlarged preview being read');
    assert.equal(await page.evaluate(() => window.linkPreviewReads.length), readsBeforeDialogFailure, 'An open dialog defers the native file read');
    assert.match(await page.locator('#preview-dialog-status').textContent(), /atualizada ao fechar/);
    await page.getByRole('button', { name: 'Fechar prévia' }).click();
    await page.locator('#preview-text').filter({ hasText: 'Arquivo mudado durante leitura' }).waitFor();
    assert.equal(await page.evaluate(() => window.linkPreviewReads.length), readsBeforeDialogFailure + 1, 'Closing the dialog refreshes once');
    await page.getByRole('button', { name: 'Abrir prévia' }).click();
    const readsBeforeNavigation = await page.evaluate(() => window.linkPreviewReads.length);
    await page.evaluate(() => {
      window.linkPreviewContent = 'Arquivo alterado antes de navegar';
      window.agentEvents.onmessage({ kind: 'completed' });
      location.hash = '#explore';
    });
    await page.locator('#preview-dialog').waitFor({ state: 'hidden' });
    await page.evaluate(() => { location.hash = '#workspace'; });
    await page.locator('#preview-text').filter({ hasText: 'Arquivo alterado antes de navegar' }).waitFor({ timeout: 5000 });
    assert.equal(await page.evaluate(() => window.linkPreviewReads.length), readsBeforeNavigation + 1, 'Navigating away from an enlarged preview must not discard the pending project-file refresh');
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
    assert.equal(await page.locator('#project-preview').isVisible(), true, 'Opening a cited result switches to the preview on narrow screens');
    await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Conversa' }).click();
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
    assert.equal(await page.evaluate(() => document.activeElement.getAttribute('aria-label')), 'Abrir site externo: web (example.com)');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.textContent), 'Conferir 3 arquivos da resposta');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'jump-latest');
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
    await openConversationOptions(page);
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
    assert.match(await page.locator('#agent-status').textContent(), /Última resposta incompleta.*confira mensagens e arquivos.*Mudanças podem permanecer; nada foi reenviado/);
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
    await clickConversationAction(page, 'Desconectar');
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.match(await page.locator('#agent-status').textContent(), /Você pode continuar de onde parou/);
    assert.equal(await page.locator('#preview-last-result').isVisible(), true, 'A single file in the restored completed reply should be easy to reopen');
    const resultAction = page.getByRole('button', { name: 'Conferir arquivo citado na resposta: site/index.html' });
    assert.equal(await resultAction.isVisible(), true, 'The completed reply should offer its project file without requiring a search in the side panel');
    assert.match(await page.locator('#preview-intro').textContent(), /resposta cita um arquivo/);
    await clickConversationAction(page, 'Ver texto original');
    assert.equal(await page.locator('#preview-last-result').isVisible(), true, 'Reading the original answer must not lose its file shortcut');
    assert.equal(await resultAction.isVisible(), true, 'The file shortcut must remain available in original-text mode');
    await resultAction.click();
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.linkPreviewReads.at(-1)), { projectRoot: 'D:\\another-project', filePath: 'D:\\another-project\\site\\index.html' });
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'message', id: 'next-result', text: 'Nova versão: [ver arquivo](site/updated.html).' }));
    assert.equal(await page.locator('.preview-empty').isVisible(), false, 'The old side-panel shortcut is hidden while a preview is loaded');
    await page.getByRole('button', { name: 'Conferir arquivo citado na resposta: site/updated.html' }).click();
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.deepEqual(await page.evaluate(() => window.linkPreviewReads.at(-1)), { projectRoot: 'D:\\another-project', filePath: 'D:\\another-project\\site\\updated.html' }, 'A later reply can open its new file even when another preview was already loaded');
    await resultAction.click();
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    await page.evaluate(() => { window.resumeMessages = [
      { id: 'result-user', role: 'user', text: 'Crie uma página simples.' },
      { id: 'result-agent', role: 'agent', text: 'Pronto: [Ver página](site/index.html).' },
      { id: 'planning-user', role: 'user', text: 'Vamos planejar o próximo passo.' },
      { id: 'planning-agent', role: 'agent', text: 'O próximo passo é definir o público. Nenhum arquivo mudou.' },
    ]; });
    await clickConversationAction(page, 'Desconectar');
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
    await clickConversationAction(page, 'Desconectar');
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.match(await page.locator('#agent-status').textContent(), /Última resposta incompleta.*nada foi reenviado/);
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
    await clickConversationAction(page, 'Desconectar');
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
    await clickConversationAction(page, 'Desconectar');
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
    await clickConversationAction(page, 'Desconectar');
    await openConversation(page);
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.evaluate(() => window.connectedThread), 'new-thread');
    await clickConversationAction(page, 'Desconectar');
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
    await clickConversationAction(page, 'Desconectar');
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
      const clipped = await page.locator('button, input, textarea').evaluateAll(nodes => nodes.filter(n => n.getClientRects().length && ((n.tagName !== 'INPUT' && n.scrollWidth > n.clientWidth + 2) || n.scrollHeight > n.clientHeight + 2)).map(n => ({ id: n.id, text: n.textContent?.trim().slice(0, 60), width: [n.scrollWidth, n.clientWidth], height: [n.scrollHeight, n.clientHeight] })));
      assert.deepEqual(clipped, [], 'Visible controls must not clip at enlarged text');
    }
    await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
    await page.emulateMedia({ forcedColors: 'active' });
    await openConversationOptions(page);
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
    await page.evaluate(async () => {
      window.boundaryPreview = await import('./preview.mjs');
      window.boundaryPreviewReads = 0;
      window.__TAURI__ = { core: { invoke: async command => {
        if (command !== 'inspect_preview') return null;
        window.boundaryPreviewReads++;
        const preview = { kind: 'text', content: 'Result from selected project', relative_path: 'result.txt', size_bytes: 28 };
        if (!window.holdBoundaryPreview) return preview;
        window.holdBoundaryPreview = false;
        return new Promise(resolve => { window.releaseBoundaryPreview = () => resolve(preview); });
      } } };
      window.boundaryPreview.setPreviewProject({ project_root: 'D:\\boundary-project' });
      await window.boundaryPreview.previewLinkedFile('result.txt');
      window.holdBoundaryPreview = true;
    });
    await page.getByRole('button', { name: 'Atualizar prévia' }).click();
    await page.waitForFunction(() => typeof window.releaseBoundaryPreview === 'function');
    await page.evaluate(() => {
      void window.boundaryPreview.refreshPreviewAfterTurn();
      window.boundaryPreview.setPreviewProject(null);
      window.releaseBoundaryPreview();
    });
    assert.equal(await page.locator('#project-preview').isHidden(), true, 'Switching away must hide the old project preview');
    await page.evaluate(async () => {
      window.boundaryPreview.setPreviewProject({ project_root: 'D:\\next-project' });
      await window.boundaryPreview.previewLinkedFile('result.txt');
    });
    assert.equal(await page.evaluate(() => window.boundaryPreviewReads), 3, 'A queued refresh from the old project must not leak into the next project');
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
            return { thread_id: 'switch-test-thread', messages: [], resumed: !!args.threadId };
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
    await page.locator('#message-text').fill('Rascunho privado do primeiro projeto');
    assert.equal(await page.locator('#project-root').isDisabled(), true, 'An active Codex connection locks direct folder editing');
    const disconnectsBeforeSwitch = await page.evaluate(() => window.disconnectCalls);
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'running' }));
    await page.locator('#project-setup > summary').click();
    await page.locator('#action-confirmation').waitFor({ state: 'visible' });
    assert.match(await page.locator('#action-confirmation-copy').textContent(), /interrompe a resposta/);
    await page.locator('#action-confirmation-cancel').click();
    assert.equal(await page.locator('#project-setup').evaluate(node => node.open), false);
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeSwitch);
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.getByRole('link', { name: /Arte e criação/ }).click();
    await page.locator('#action-confirmation').waitFor({ state: 'visible' });
    await page.locator('#action-confirmation-cancel').click();
    assert.equal(await page.evaluate(() => location.hash), '#explore', 'Declining interruption keeps the person on Explore');
    assert.equal(await page.locator('#confirmed-root').textContent(), 'D:\\first-project');
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeSwitch);
    assert.equal(await page.locator('#message-text').inputValue(), 'Rascunho privado do primeiro projeto', 'Declining interruption must not alter the existing draft');
    if (await page.locator('.appearance').evaluate(node => node.open)) await page.locator('.appearance summary').click();
    await page.getByRole('link', { name: 'Meus projetos' }).click();
    await page.getByRole('link', { name: 'Abrir outro projeto' }).click();
    await page.locator('#action-confirmation').waitFor({ state: 'visible' });
    await page.locator('#action-confirmation-cancel').click();
    assert.equal(await page.evaluate(() => location.hash), '#projects');
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeSwitch);
    await page.getByRole('link', { name: 'Abrir outro projeto' }).click();
    await page.locator('#action-confirmation').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#action-confirmation-accept').textContent(), 'Interromper e trocar');
    await page.locator('#action-confirmation-accept').click();
    await page.locator('#agent-status').filter({ hasText: 'Escolha uma pasta' }).waitFor();
    assert.equal(await page.locator('#project-setup').evaluate(node => node.open), true);
    assert.equal(await page.locator('#project-root').isEnabled(), true);
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeSwitch + 1);
    assert.equal(await page.locator('#project-root').inputValue(), '', 'Opening another project must not retain the previous folder');
    assert.equal(await page.locator('#message-text').inputValue(), '', 'Opening another project must not carry the previous project draft');
    assert.equal(await page.locator('#project-result').isHidden(), true, 'The previous project must not appear confirmed during folder choice');
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).count(), 0, 'Sending must wait for the new folder');
    assert.equal(await page.getByRole('button', { name: 'Escolher pasta para continuar' }).count(), 1);
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
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeShortcut + 1, 'Opening a different saved project must disconnect the old Codex conversation first');
    assert.equal(await page.locator('#confirmed-root').textContent(), 'D:\\first-project');
    assert.equal(await page.locator('#message-text').inputValue(), 'Rascunho privado do primeiro projeto', 'Reopening a project restores its unsent draft in this app session');
    await page.locator('#message-text').fill('');
    await clickConversationAction(page, 'Desconectar');
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
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
    await page.locator('#project-setup > summary').click();
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.locator('#message-text').inputValue(), '', 'Clearing a draft must not resurrect it when confirming the same folder again');
    const history = page.getByRole('region', { name: 'Histórico da conversa' });
    assert.equal(await page.locator('#messages article').count(), 160);
    assert.equal(await history.evaluate(node => node.scrollHeight > node.clientHeight && node.scrollHeight - node.clientHeight - node.scrollTop < 2), true);
    assert.equal(await page.locator('#jump-latest').isHidden(), true);
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
    await page.locator('#jump-latest').waitFor({ state: 'visible' });
    if (process.env.FORGE_HISTORY_JUMP_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_HISTORY_JUMP_SCREENSHOT });
    const sendsBeforeJump = await page.evaluate(() => window.sendCalls);
    await page.locator('#jump-latest').click();
    assert.equal(await history.evaluate(node => node.scrollHeight - node.clientHeight - node.scrollTop < 2), true);
    assert.equal(await page.locator('#jump-latest').isHidden(), true);
    assert.equal(await page.evaluate(() => document.activeElement?.classList.contains('conversation-body')), true);
    assert.equal(await page.evaluate(() => window.sendCalls), sendsBeforeJump, 'Jumping to the latest message must never send');
    await history.evaluate(node => { node.scrollTop = node.scrollHeight; });
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'message', id: 'new-at-bottom', text: 'Outra resposta\n'.repeat(40) }));
    assert.equal(await history.evaluate(node => node.scrollHeight - node.clientHeight - node.scrollTop < 2), true);
    await history.focus();
    assert.equal(await history.evaluate(node => document.activeElement === node), true);
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    await history.evaluate(node => { node.scrollTop = 0; });
    await page.locator('#jump-latest').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#jump-latest').evaluate(node => node.getBoundingClientRect().right <= innerWidth), true);
    const disconnectsBeforeExplore = await page.evaluate(() => window.disconnectCalls);
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.getByRole('link', { name: /Arte e criação/ }).click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeExplore + 1);
    assert.equal(await page.locator('#project-root').inputValue(), '');
    assert.equal(await page.locator('#project-result').isHidden(), true);
    assert.equal(await page.locator('#project-setup').evaluate(node => node.open), true);
    assert.equal(await page.locator('#messages article').count(), 0, 'Previous-project messages must not remain in the new-project view');
    assert.equal(await page.locator('#jump-latest').isHidden(), true, 'The previous conversation jump must not remain on a new project');
    assert.equal(await page.locator('#preview-cited-files').getAttribute('hidden'), '', 'Previous-project file citations must be cleared on project switch');
    assert.equal(await page.getByRole('button', { name: 'Escolher pasta para continuar' }).isEnabled(), true, 'An Explore draft should lead to folder choice without sending');
    assert.match(await page.locator('#message-text').inputValue(), /artístico/i);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    const draftPage = await browser.newPage();
    await draftPage.addInitScript(() => {
      window.draftSendCalls = 0;
      window.__TAURI__ = { core: {
        Channel: class {},
        invoke: async (command, args) => {
          if (command === 'start_project' || command === 'inspect_project') return { project_id: 'draft-project', project_root: args.projectRoot };
          if (command === 'connect_agent') return { thread_id: 'draft-thread', messages: [], resumed: !!args.threadId };
          if (command === 'send_message') { window.draftSendCalls++; return {}; }
          return null;
        },
      } };
    });
    await draftPage.goto(workspaceUrl);
    await draftPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\draft-project');
    await draftPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await draftPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    await draftPage.locator('#message-text').fill('Minha ideia ainda não enviada');
    await draftPage.evaluate(() => {
      window.originalDraftInvoke = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = (command, args) => command === 'start_project'
        ? Promise.reject('Validação indisponível') : window.originalDraftInvoke(command, args);
    });
    await draftPage.locator('#project-setup > summary').click();
    await draftPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await draftPage.locator('#project-status').filter({ hasText: 'Validação indisponível' }).waitFor();
    assert.equal(await draftPage.locator('#message-text').inputValue(), 'Minha ideia ainda não enviada', 'A failed same-folder check must keep the draft');
    await draftPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\other-after-failure');
    assert.equal(await draftPage.locator('#message-text').inputValue(), '', 'Changing folders after a failed check must not leak the old draft');
    await draftPage.evaluate(() => { window.__TAURI__.core.invoke = window.originalDraftInvoke; });
    await draftPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\draft-project');
    await draftPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await draftPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await draftPage.locator('#message-text').inputValue(), 'Minha ideia ainda não enviada', 'Returning after a failed check restores the original project draft');
    await draftPage.reload();
    await draftPage.getByRole('link', { name: 'Meus projetos' }).click();
    await draftPage.locator('#recent-projects .recent-project').filter({ hasText: 'D:\\draft-project' }).getByRole('button', { name: /^Abrir / }).click();
    await draftPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await draftPage.locator('#message-text').inputValue(), 'Minha ideia ainda não enviada', 'Reopening the app must restore the confirmed project draft without sending it');
    assert.match(await draftPage.locator('#draft-note').textContent(), /Rascunho salvo neste dispositivo/);
    assert.equal(await draftPage.locator('#draft-note').isVisible(), true, 'The restored draft must be explained beside the composer');
    await draftPage.setViewportSize({ width: 360, height: 720 });
    await draftPage.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    assert.equal(await draftPage.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'Saved-draft notice must wrap at 360px and 200% text');
    assert.equal(await draftPage.getByRole('button', { name: 'Enviar', exact: true }).isVisible(), true);
    await draftPage.evaluate(() => { document.documentElement.style.fontSize = ''; });
    await draftPage.setViewportSize({ width: 1280, height: 720 });
    assert.equal(await draftPage.evaluate(() => window.draftSendCalls), 0, 'Restoring a draft must never send it');
    const savedDraftKey = 'forge.draft.v1:' + JSON.stringify(['draft-project', 'D:\\draft-project']);
    assert.equal(await draftPage.evaluate(key => localStorage.getItem(key), savedDraftKey), 'Minha ideia ainda não enviada');
    await draftPage.getByRole('link', { name: 'Meus projetos' }).click();
    await draftPage.getByRole('link', { name: 'Abrir outro projeto' }).click();
    assert.equal(await draftPage.locator('#message-text').inputValue(), '', 'Another project must not inherit a saved draft');
    await draftPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\second-draft-project');
    await draftPage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await draftPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    await draftPage.locator('#message-text').fill('Outro texto não enviado');
    await draftPage.reload();
    await draftPage.getByRole('link', { name: 'Meus projetos' }).click();
    await draftPage.locator('#recent-projects .recent-project').filter({ hasText: 'D:\\draft-project' }).getByRole('button', { name: /^Abrir / }).click();
    await draftPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await draftPage.locator('#message-text').inputValue(), 'Minha ideia ainda não enviada', 'The original project restores only its own saved draft');
    await draftPage.getByRole('link', { name: 'Meus projetos' }).click();
    await draftPage.locator('#recent-projects .recent-project').filter({ hasText: 'D:\\second-draft-project' }).getByRole('button', { name: /^Abrir / }).click();
    await draftPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await draftPage.locator('#message-text').inputValue(), 'Outro texto não enviado', 'The second project restores only its own saved draft');
    await draftPage.getByRole('link', { name: 'Meus projetos' }).click();
    await draftPage.locator('#recent-projects .recent-project').filter({ hasText: 'D:\\draft-project' }).getByRole('button', { name: /^Abrir / }).click();
    await draftPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    await draftPage.getByRole('button', { name: 'Enviar', exact: true }).click();
    await draftPage.waitForFunction(() => window.draftSendCalls === 1);
    await draftPage.waitForFunction(key => localStorage.getItem(key) === null, savedDraftKey);
    await draftPage.reload();
    await draftPage.getByRole('link', { name: 'Meus projetos' }).click();
    await draftPage.locator('#recent-projects .recent-project').filter({ hasText: 'D:\\draft-project' }).getByRole('button', { name: /^Abrir / }).click();
    await draftPage.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await draftPage.locator('#message-text').inputValue(), '', 'An accepted send must not reappear as an unsent draft after restart');
    assert.equal(await draftPage.evaluate(() => window.draftSendCalls), 0, 'Reopening after accepted send must not resend it');
    await draftPage.evaluate(() => {
      const invoke = window.__TAURI__.core.invoke;
      window.__TAURI__.core.invoke = (command, args) => command === 'send_message'
        ? Promise.reject('Delivery uncertain') : invoke(command, args);
    });
    await draftPage.locator('#message-text').fill('Texto de envio incerto');
    await draftPage.getByRole('button', { name: 'Enviar', exact: true }).click();
    await draftPage.locator('#agent-status').filter({ hasText: 'Delivery uncertain' }).waitFor();
    assert.equal(await draftPage.evaluate(key => localStorage.getItem(key), savedDraftKey), 'Texto de envio incerto', 'Uncertain send retains its draft for careful review');
    await draftPage.reload();
    await draftPage.getByRole('link', { name: 'Meus projetos' }).click();
    await draftPage.locator('#recent-projects .recent-project').filter({ hasText: 'D:\\draft-project' }).getByRole('button', { name: /^Abrir / }).click();
    await draftPage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await draftPage.locator('#message-text').inputValue(), 'Texto de envio incerto');
    assert.match(await draftPage.locator('#composer-help').textContent(), /último envio não foi confirmado/);
    assert.equal(await draftPage.evaluate(() => window.draftSendCalls), 0, 'Uncertain draft restoration must not resend it');
    await draftPage.close();
    const draftFailurePage = await browser.newPage();
    await draftFailurePage.addInitScript(() => {
      window.failureSendCalls = 0;
      window.__TAURI__ = { core: {
        Channel: class {},
        invoke: async (command, args) => {
          if (command === 'start_project' || command === 'inspect_project') return { project_id: 'failure-project', project_root: args.projectRoot };
          if (command === 'connect_agent') return { thread_id: 'failure-thread', messages: [], resumed: false };
          if (command === 'send_message') { window.failureSendCalls++; return {}; }
          return null;
        },
      } };
    });
    await draftFailurePage.goto(workspaceUrl);
    await draftFailurePage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\failure-project');
    await draftFailurePage.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await draftFailurePage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    await draftFailurePage.evaluate(() => {
      window.originalSetItem = Storage.prototype.setItem;
      Storage.prototype.setItem = function (key, value) {
        if (key.startsWith('forge.draft.v1:')) throw new Error('draft storage unavailable');
        return window.originalSetItem.call(this, key, value);
      };
    });
    await draftFailurePage.locator('#message-text').fill('Texto que não foi salvo');
    assert.match(await draftFailurePage.locator('#draft-note').textContent(), /não foi possível guardar este rascunho/i);
    assert.equal(await draftFailurePage.locator('#draft-note').isVisible(), true);
    await draftFailurePage.evaluate(() => { Storage.prototype.setItem = window.originalSetItem; });
    await draftFailurePage.locator('#message-text').fill('Texto salvo com proteção');
    const failureDraftKey = 'forge.draft.v1:' + JSON.stringify(['failure-project', 'D:\\failure-project']);
    const failureMarkerKey = 'forge.send-unconfirmed.v1:' + JSON.stringify(['failure-project', 'D:\\failure-project']);
    assert.equal(await draftFailurePage.evaluate(key => localStorage.getItem(key), failureDraftKey), 'Texto salvo com proteção');
    await draftFailurePage.evaluate(() => {
      window.originalRemoveItem = Storage.prototype.removeItem;
      Storage.prototype.removeItem = function (key) {
        if (key.startsWith('forge.draft.v1:')) throw new Error('draft removal unavailable');
        return window.originalRemoveItem.call(this, key);
      };
    });
    await draftFailurePage.getByRole('button', { name: 'Enviar', exact: true }).click();
    await draftFailurePage.locator('#agent-status').filter({ hasText: 'Envio confirmado, mas não foi possível remover o rascunho' }).waitFor();
    assert.match(await draftFailurePage.locator('#draft-note').textContent(), /não foi possível guardar este rascunho/i);
    assert.equal(await draftFailurePage.evaluate(() => window.failureSendCalls), 1);
    assert.equal(await draftFailurePage.evaluate(key => localStorage.getItem(key), failureMarkerKey), 'failure-thread', 'The uncertain-send guard must remain until stale saved text can be removed');
    await draftFailurePage.evaluate(() => { Storage.prototype.removeItem = window.originalRemoveItem; });
    await draftFailurePage.reload();
    await draftFailurePage.getByRole('link', { name: 'Meus projetos' }).click();
    await draftFailurePage.locator('#recent-projects .recent-project').filter({ hasText: 'D:\\failure-project' }).getByRole('button', { name: /^Abrir / }).click();
    await draftFailurePage.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor();
    assert.equal(await draftFailurePage.locator('#message-text').inputValue(), 'Texto salvo com proteção');
    assert.match(await draftFailurePage.locator('#composer-help').textContent(), /último envio não foi confirmado/);
    assert.equal(await draftFailurePage.evaluate(() => window.failureSendCalls), 0, 'A stale saved draft must not be sent on restart');
    await draftFailurePage.close();
    console.log('PASS: controlled long history stays scrollable in the conversation, resumes at latest, preserves earlier reading position, and follows new replies near the end.');
    console.log('PASS: project switching closes idle connections; a running turn requires an in-app choice, preserves state when declined and disconnects when accepted. Explore opens a fresh folder without showing the old transcript.');
    console.log('PASS: appearance survives reload, overrides OS, follows OS, supports keyboard and tolerates storage failure.');
    console.log('PASS: oversized Unicode remains recoverable, completion-before-ack preserves draft, pending disconnect locks controls, send rejection releases session.');
    console.log('PASS: desktop/mobile overflow, mobile text, retry, keyboard entry, dark theme, honest agent status. Native IPC NOT_RUN.');
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
