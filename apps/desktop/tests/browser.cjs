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
  ['/connection.mjs', ['connection.mjs', 'text/javascript']],
  ['/chat.mjs', ['chat.mjs', 'text/javascript']],
  ['/conversation-reference.mjs', ['conversation-reference.mjs', 'text/javascript']],
  ['/assets/forge.png', ['assets/forge.png', 'image/png']],
  ['/assets/explore-artwork.png', ['assets/explore-artwork.png', 'image/png']],
  ['/appearance.js', ['appearance.js', 'text/javascript']],
  ['/progress.mjs', ['progress.mjs', 'text/javascript']],
]);

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
    assert.equal(await page.locator('#home').isVisible(), true);
    assert.equal(await page.locator('#workspace').isHidden(), true);
    assert.equal(await page.locator('nav a[data-route="home"]').getAttribute('aria-current'), 'page');
    await page.getByRole('link', { name: 'Continuar um projeto' }).click();
    await page.locator('#projects').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#projects-empty').isVisible(), true);
    await page.getByRole('link', { name: 'Abrir outro projeto' }).click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#workspace').isVisible(), true);
    assert.equal(await page.locator('#home').isHidden(), true);
    assert.equal(await page.locator('nav a[data-route="workspace"]').getAttribute('aria-current'), 'page');
    console.log('PASS: home and workspace are distinct, reachable screens with current navigation.');
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.locator('#explore').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#home').isHidden(), true);
    assert.equal(await page.locator('#workspace').isHidden(), true);
    assert.equal(await page.locator('nav a[data-route="explore"]').getAttribute('aria-current'), 'page');
    assert.equal(await page.locator('.category-card').count(), 8);
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
    assert.match(await page.getByRole('textbox', { name: 'Conte sua ideia' }).inputValue(), /música/i);
    console.log('PASS: Explore filters eight approachable themes and carries the chosen idea into the conversation.');
    const projectsPage = await browser.newPage();
    await projectsPage.addInitScript(() => {
      window.projectChecks = [];
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
      } } };
    });
    await projectsPage.goto(`${url}#projects`);
    assert.equal(await projectsPage.locator('#projects-empty').isVisible(), true);
    await projectsPage.getByRole('link', { name: 'Abrir outro projeto' }).click();
    await projectsPage.evaluate(() => { window.folderChoice = 'D:\\one'; });
    await projectsPage.getByRole('button', { name: 'Escolher pasta' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto encontrado' }).waitFor();
    assert.equal(await projectsPage.locator('#project-root').inputValue(), 'D:\\one');
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
    await projectsPage.getByRole('link', { name: 'Meus projetos' }).click();
    assert.equal(await projectsPage.locator('.recent-project').count(), 1);
    assert.equal(await projectsPage.locator('#projects-empty').isHidden(), true);
    if (process.env.FORGE_PROJECTS_SCREENSHOT) {
      await projectsPage.screenshot({ path: process.env.FORGE_PROJECTS_SCREENSHOT, fullPage: true });
    }
    await projectsPage.reload();
    assert.equal(await projectsPage.locator('.recent-project').count(), 1);
    await projectsPage.getByRole('button', { name: 'Abrir first-project' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto encontrado' }).waitFor();
    assert.deepEqual(await projectsPage.evaluate(() => window.projectChecks), ['D:\\one']);
    await projectsPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\two');
    await projectsPage.getByRole('button', { name: 'Conferir projeto' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto encontrado' }).waitFor();
    await projectsPage.getByRole('link', { name: 'Meus projetos' }).click();
    assert.equal(await projectsPage.locator('.recent-project').count(), 2);
    await projectsPage.locator('#project-root').evaluate(input => { input.disabled = true; });
    await projectsPage.getByRole('button', { name: 'Abrir first-project' }).click();
    await projectsPage.locator('#projects-status').filter({ hasText: 'Encerre a conversa atual' }).waitFor();
    assert.equal(await projectsPage.locator('#project-root').inputValue(), 'D:\\two');
    await projectsPage.locator('#project-root').evaluate(input => { input.disabled = false; });
    await projectsPage.evaluate(() => { window.rejectProject = true; });
    await projectsPage.getByRole('button', { name: 'Abrir first-project' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'não está disponível' }).waitFor();
    assert.equal(await projectsPage.locator('#project-result').isHidden(), true);
    assert.equal(await projectsPage.locator('#connect-agent').isDisabled(), true);
    await projectsPage.getByRole('link', { name: 'Meus projetos' }).click();
    await projectsPage.getByRole('button', { name: 'Remover first-project da lista' }).click();
    await projectsPage.getByRole('button', { name: 'Remover second-project da lista' }).click();
    assert.equal(await projectsPage.locator('#projects-empty').isVisible(), true);
    await projectsPage.evaluate(() => { Storage.prototype.setItem = () => { throw new Error('Unavailable'); }; window.rejectProject = false; });
    await projectsPage.getByRole('link', { name: 'Abrir outro projeto' }).click();
    await projectsPage.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\three');
    await projectsPage.getByRole('button', { name: 'Conferir projeto' }).click();
    await projectsPage.locator('#project-status').filter({ hasText: 'Projeto encontrado' }).waitFor();
    await projectsPage.getByRole('link', { name: 'Meus projetos' }).click();
    assert.equal(await projectsPage.locator('.recent-project').count(), 1);
    await projectsPage.locator('#projects-status').filter({ hasText: 'Não foi possível guardar' }).waitFor();
    for (const width of [390, 1180]) {
      await projectsPage.setViewportSize({ width, height: 844 });
      assert.equal(await projectsPage.evaluate(() => document.documentElement.scrollWidth > innerWidth), false);
    }
    await projectsPage.close();
    console.log('PASS: My Projects empty state, verified shortcuts, reload, revalidation, unavailable project, removal, storage failure and narrow layout.');
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
    for (const selector of ['#project-root', '#browse-project', '#inspect-project', '#connection summary', '#retry', '#new-conversation']) {
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
    await page.getByText('Nenhum agente conectado.', { exact: false }).waitFor();
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
          if (window.progressFailure) throw new Error('Unavailable');
          return { status: window.progressState || 'current', focus: { title: 'Recorded task', current_activity: 'Recorded activity', next_step: 'Recorded next step', open_decision_count: 1 } };
        }
        if (command === 'inspect_project') return { project_id: 'test-project', project_root: 'D:\\test-project' };
          if (command === 'connect_agent') { window.agentEvents = args.events; window.connectedThread = args.threadId; return { thread_id: 'test-thread', messages: args.threadId ? [{ id: 'saved-user', role: 'user', text: 'Saved decision' }, { id: 'saved-agent', role: 'agent', text: 'Partial reply' }] : [], resumed: !!args.threadId }; }
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
    await page.getByRole('button', { name: 'Conferir projeto' }).click();
    assert.equal(await page.evaluate(() => window.progressCalls || 0), 0);
    for (const state of ['current', 'stale', 'blocked', 'completed', 'abandoned', 'absent']) {
      await page.evaluate(state => { window.progressState = state; }, state);
      await page.getByRole('button', { name: 'Consultar registro', exact: true }).click();
      await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
      assert.equal(await page.locator('#progress-result').isVisible(), state !== 'absent');
    }
    await page.evaluate(() => { window.progressFailure = true; });
    await page.getByRole('button', { name: 'Consultar registro', exact: true }).click();
    await page.locator('#progress-status').filter({ hasText: 'Não foi possível consultar' }).waitFor();
    assert.equal(await page.locator('#progress-result').isVisible(), false);
    await page.evaluate(() => { window.progressFailure = false; window.progressState = 'current'; });
    await page.evaluate(() => { window.delayProgress = true; });
    await page.getByRole('button', { name: 'Consultar registro', exact: true }).focus();
    await page.keyboard.press('Enter');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'progress-status');
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill('D:\\another-project');
    await page.evaluate(() => { window.resolveProgress({ status: 'current', focus: { title: 'Obsolete response' } }); window.delayProgress = false; });
    assert.equal(await page.locator('#progress-result').isVisible(), false);
    await page.getByRole('button', { name: 'Conferir projeto' }).click();
    await page.getByRole('button', { name: 'Consultar registro', exact: true }).click();
    await page.getByRole('button', { name: 'Conectar Codex', exact: true }).focus();
    await page.keyboard.press('Enter');
    await page.locator('#agent-status').filter({ hasText: 'Codex conectado' }).waitFor();
    assert.equal(await page.locator('#browse-project').isDisabled(), true);
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'message-text');
    await page.keyboard.press('Shift+Tab');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'disconnect-agent');
    await page.evaluate(() => { window.delayProgress = true; });
    await page.getByRole('button', { name: 'Consultar registro', exact: true }).click();
    await page.evaluate(() => { window.agentEvents.onmessage({ kind: 'running' }); window.resolveProgress({ status: 'current', focus: { title: 'Obsolete response' } }); window.delayProgress = false; });
    assert.equal(await page.locator('#progress-result').isVisible(), false);
    for (const [kind, state] of [['running', 'working'], ['completed', 'completed'], ['interrupted', 'interrupted'], ['failed', 'error']]) {
      await page.evaluate(kind => window.agentEvents.onmessage({ kind }), kind);
      assert.equal(await page.locator('#progress-result').isVisible(), false);
      assert.equal(await page.locator('#agent-status').getAttribute('data-state'), state);
      assert.equal(await page.locator('#agent-status .status-icon').getAttribute('aria-hidden'), 'true');
      assert.ok((await page.locator('#agent-status span:last-child').textContent()).length > 15);
    }
    const composer = page.getByRole('textbox', { name: 'Conte sua ideia' });
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
    assert.equal(await page.locator('#agent-status').evaluate(node => { const box = node.getBoundingClientRect(); return box.bottom > 0 && box.top < innerHeight; }), true);
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'running' }));
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'interrupt-agent');
    await page.keyboard.press('Enter');
    assert.equal(await page.evaluate(() => document.activeElement.id), 'agent-status');
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'completed' }));
    await composer.fill('Keep this new draft');
    await page.evaluate(() => window.resolveSend({}));
    assert.equal(await composer.inputValue(), 'Keep this new draft');
    await page.evaluate(() => { window.delayDisconnect = true; });
    await page.getByRole('button', { name: 'Desconectar', exact: true }).focus();
    await page.keyboard.press('Enter');
    await page.evaluate(() => window.agentEvents.onmessage({ kind: 'running' }));
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    assert.equal(await page.getByRole('button', { name: 'Desconectar', exact: true }).isDisabled(), true);
    await page.evaluate(() => window.resolveDisconnect());
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
    await page.evaluate(() => { window.delayDisconnect = false; });
    await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.evaluate(() => window.connectedThread), 'test-thread');
    assert.match(await page.locator('#messages').textContent(), /Saved decision/);
    assert.match(await page.locator('#messages').textContent(), /Partial reply/);
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
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
    await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Não foi possível salvar o acesso' }).waitFor();
    assert.equal(await page.evaluate(() => window.connectedThread), null);
    await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
    await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
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
    await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Saved conversation unavailable' }).waitFor();
    assert.equal(await composer.isDisabled(), true);
    await page.evaluate(() => { window.__TAURI__.core.invoke = window.beforeBookmarkInvoke; });
    await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    console.log('PASS: restored transcript, explicit new conversation, failed save keeps latest session bookmark, unavailable resume never silently starts anew.');
    const disconnectsBeforeRejection = await page.evaluate(() => window.disconnectCalls);
    await composer.fill('Rejected request');
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.evaluate(() => window.rejectSend('Test rejection'));
    await page.locator('#agent-status').filter({ hasText: 'Test rejection' }).waitFor();
    assert.equal(await page.evaluate(() => window.disconnectCalls), disconnectsBeforeRejection + 1);
    assert.equal(await page.getByRole('button', { name: 'Conectar Codex', exact: true }).isEnabled(), true);
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
    await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
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
    console.log('PASS: appearance survives reload, overrides OS, follows OS, supports keyboard and tolerates storage failure.');
    console.log('PASS: oversized Unicode remains recoverable, completion-before-ack preserves draft, pending disconnect locks controls, send rejection releases session.');
    console.log('PASS: desktop/mobile overflow, mobile text, retry, keyboard entry, dark theme, honest agent status. Native IPC NOT_RUN.');
  } finally {
    if (browser) await browser.close();
    await new Promise(resolve => server.close(resolve));
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
