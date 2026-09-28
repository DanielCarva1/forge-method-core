// Windows-only development smoke test against a real Tauri WebView.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, mkdir, rm, access, readFile, writeFile } = require('node:fs/promises');
const { once } = require('node:events');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

async function openConversation(page) {
  if (!await page.locator('#conversation-picker').evaluate(node => node.open)) await page.locator('#conversation-picker summary').click();
  await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
}

async function operateFolderDialog(page, mode, folder, trigger = '#browse-project') {
  const helper = spawn('py', ['-3.12', path.join(__dirname, 'folder-dialog.py'), mode, ...(folder ? [folder] : [])], { windowsHide: true });
  let output = '';
  helper.stdout.on('data', chunk => { output += chunk; });
  helper.stderr.on('data', chunk => { output += chunk; });
  await page.locator(trigger).click();
  const [code] = await once(helper, 'exit');
  assert.equal(code, 0, `Native folder-dialog helper failed: ${output}`);
}

async function operatePreviewDialog(page, file) {
  const helper = spawn('py', ['-3.12', path.join(__dirname, 'folder-dialog.py'), 'file-select', file], { windowsHide: true });
  let output = '';
  helper.stdout.on('data', chunk => { output += chunk; });
  helper.stderr.on('data', chunk => { output += chunk; });
  await page.getByRole('button', { name: 'Escolher arquivo' }).click();
  const [code] = await once(helper, 'exit');
  assert.equal(code, 0, `Native file-dialog helper failed: ${output}`);
}

(async () => {
  if (!process.env.FORGE_DESKTOP_EXE) throw new Error('Set FORGE_DESKTOP_EXE to the built development executable');
  if (process.env.FORGE_TEST_NEW_IDEA_REAL_SEND === '1' && process.env.FORGE_TEST_NEW_IDEA_PROJECT !== '1') throw new Error('FORGE_TEST_NEW_IDEA_REAL_SEND requires FORGE_TEST_NEW_IDEA_PROJECT');
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-desktop-webview-'));
  let child = spawn(process.env.FORGE_DESKTOP_EXE, [], {
    windowsHide: true,
    stdio: 'ignore',
    env: {
      ...process.env,
      WEBVIEW2_USER_DATA_FOLDER: profile,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
    },
  });
  let launchError;
  child.on('error', error => { launchError = error; });
  let browser;
  let restartProject;
  let restartHistory;
  try {
    const deadline = Date.now() + 20000;
    while (Date.now() < deadline) {
      if (launchError) throw launchError;
      if (child.exitCode !== null) throw new Error(`Application exited: ${child.exitCode}`);
      try {
        browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 });
        break;
      } catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    if (!browser) throw new Error('Native WebView did not become available within 20 seconds');
    const context = browser.contexts()[0];
    let page = context.pages()[0] || await context.waitForEvent('page', { timeout: 5000 });
    await page.locator('#home').waitFor({ state: 'visible' });
    const initialViewport = await page.evaluate(() => ({ width: innerWidth, height: innerHeight }));
    await page.setViewportSize({ width: 360, height: 720 });
    await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    for (const [route, heading] of [
      ['home', '.hero h1'],
      ['explore', '.explore-heading h1'],
      ['projects', '.projects-heading h1'],
      ['workspace', '.workspace-screen .screen-heading h1'],
    ]) {
      await page.locator(`nav a[data-route="${route}"]`).click();
      await page.locator(`#${route}`).waitFor({ state: 'visible' });
      assert.equal(await page.locator(heading).evaluate(node => node.scrollWidth <= node.clientWidth + 1), true,
        `Native ${route} heading must reflow at 360px and 200% text size`);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true,
        `Native ${route} must not require horizontal scrolling at 360px and 200% text size`);
      assert.equal(await page.locator(heading).evaluate(node =>
        document.querySelector('.appearance').getBoundingClientRect().bottom <= node.getBoundingClientRect().top), true,
      `Native ${route} heading must not sit behind appearance controls`);
    }
    await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
    await page.setViewportSize({ width: 390, height: 844 });
    assert.equal(await page.evaluate(() => {
      const brand = document.querySelector('header .brand').getBoundingClientRect();
      const alpha = document.querySelector('header .development').getBoundingClientRect();
      const nav = document.querySelector('header > nav').getBoundingClientRect();
      return alpha.top < brand.bottom && alpha.left >= brand.right && nav.top >= brand.bottom - 1;
    }), true, 'Native narrow header should keep Alpha with the brand');
    await page.setViewportSize(initialViewport);
    await page.locator('nav a[data-route="home"]').click();
    console.log('PASS: native Home, Explore, Projects and Workspace headings reflow at 360px / 200% text without clipping or appearance overlap.');
    if (process.env.FORGE_HOME_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_HOME_SCREENSHOT, fullPage: true });
    const formatted = await page.evaluate(async () => {
      const { renderAgentMessage } = await import('./message-format.mjs');
      const content = document.createElement('div');
      renderAgentMessage(content, '# Resultado\n- **Item** seguro\n```txt\n<script>não executar</script>\n```\n> Revise o **resultado**.\n| Arquivo | Estado |\n| --- | --- |\n| tela.html | pronta |');
      const result = { heading: content.querySelector('h3')?.textContent, item: content.querySelector('li strong')?.textContent, code: content.querySelector('pre code')?.textContent, quote: content.querySelector('blockquote')?.textContent, rows: content.querySelectorAll('table tbody tr').length, scripts: content.querySelectorAll('script').length };
      renderAgentMessage(content, '## Primeiro título\n### Detalhe\n# Outro assunto\n### Sem nível intermediário');
      result.levels = [...content.querySelectorAll('h3, h4, h5')].map(node => node.tagName);
      return result;
    });
    assert.deepEqual(formatted, { heading: 'Resultado', item: 'Item', code: '<script>não executar</script>', quote: 'Revise o resultado.', rows: 1, scripts: 0, levels: ['H3', 'H4', 'H3', 'H4'] });
    console.log('PASS: native WebView formats completed-response structure without executing agent HTML (controlled fixture).');
    await page.locator('nav a[data-route="explore"]').click();
    await page.locator('#explore').waitFor({ state: 'visible' });
    assert.equal(await page.locator('.category-card:visible').count(), 8);
    const nativeIdeaPanel = await page.locator('.open-idea').boundingBox();
    const nativeIdeaTitle = await page.locator('.open-idea h2').boundingBox();
    assert.ok(nativeIdeaTitle.x >= nativeIdeaPanel.x + 175, 'Native callout title must clear decorative foliage');
    await page.getByRole('searchbox', { name: 'O que te interessa?' }).fill('música');
    assert.equal(await page.locator('.category-card:visible').count(), 1);
    await page.getByRole('searchbox', { name: 'O que te interessa?' }).fill('');
    if (process.env.FORGE_EXPLORE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_EXPLORE_SCREENSHOT, fullPage: true });
    console.log('PASS: native Explore displays approved categories, foreground search, and unobscured callout.');
    await page.locator('nav a[data-route="workspace"]').click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    const longHistoryLayout = await page.evaluate(() => {
      const body = document.querySelector('.conversation-body');
      const messages = document.getElementById('messages');
      const jump = document.getElementById('jump-latest');
      for (let index = 0; index < 100; index++) {
        const article = document.createElement('article');
        article.textContent = `Controlled conversation line ${index}: ${'readable text '.repeat(20)}`;
        messages.append(article);
      }
      body.scrollTop = 0;
      body.dispatchEvent(new Event('scroll'));
      const result = { bounded: body.scrollHeight > body.clientHeight, keyboard: body.tabIndex === 0,
        composerVisible: document.getElementById('message-form').getBoundingClientRect().height > 0,
        jumpShown: !jump.hidden };
      jump.click();
      result.jumpScrolled = body.scrollHeight - body.clientHeight - body.scrollTop < 2 && jump.hidden;
      messages.replaceChildren();
      body.dispatchEvent(new Event('scroll'));
      return result;
    });
    assert.deepEqual(longHistoryLayout, { bounded: true, keyboard: true, composerVisible: true, jumpShown: true, jumpScrolled: true });
    console.log('PASS: native WebView keeps a controlled long-history fixture above the composer and returns to the latest message on request.');
    await page.locator('nav a[data-route="home"]').click();
    await page.getByRole('link', { name: 'Como funciona' }).click();
    await page.locator('#about .cards').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#about').evaluate(node => node.open), true);
    const nativeInfo = await page.evaluate(() => window.__TAURI__.core.invoke('app_info'));
    await page.locator('#app-version').filter({ hasText: `Versão instalada: ${nativeInfo.version}` }).waitFor();
    assert.equal(await page.getByRole('button', { name: 'Ver versões disponíveis' }).isVisible(), true);
    assert.equal(await page.locator('#updates-url').isHidden(), true);
    if (process.env.FORGE_UPDATES_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_UPDATES_SCREENSHOT, fullPage: true });
    await page.setViewportSize({ width: 360, height: 720 });
    await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
    await page.getByRole('button', { name: 'Ver versões disponíveis' }).scrollIntoViewIfNeeded();
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'Enlarged updates help must not scroll sideways');
    await page.evaluate(() => { document.documentElement.style.removeProperty('font-size'); });
    await page.setViewportSize(initialViewport);
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.getByRole('link', { name: /Arte e criação/ }).click();
    const draft = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    assert.match(await draft.inputValue(), /artístico/i);
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.getByRole('link', { name: /Jogos/ }).click();
    assert.match(await draft.inputValue(), /jogo/i);
    await draft.fill('Minha própria ideia não pode sumir.');
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.getByRole('link', { name: /Tecnologia/ }).click();
    assert.equal(await draft.inputValue(), 'Minha própria ideia não pode sumir.');
    assert.match(await page.locator('#idea-selection-status').textContent(), /ideia escrita foi mantida/);
    if (process.env.FORGE_IDEA_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_IDEA_SCREENSHOT, fullPage: true });
    console.log('PASS: native first-use help and idea selection preserve a handwritten draft.');
    await page.locator('nav a[data-route="workspace"]').click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    if (process.env.FORGE_FIRST_USE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_FIRST_USE_SCREENSHOT, fullPage: true });
    assert.equal(await page.locator('#browse-project').isEnabled(), true);
    if (process.env.FORGE_TEST_FOLDER_DIALOG === 'select') {
      await operateFolderDialog(page, 'cancel', undefined, '#send-message');
      await page.locator('#project-status').filter({ hasText: 'Seleção cancelada' }).waitFor();
      assert.equal(await page.locator('#project-root').inputValue(), '');
      assert.equal(await draft.inputValue(), 'Minha própria ideia não pode sumir.');
    }
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Escolha uma pasta' }).waitFor();
    assert.equal(await page.locator('#project-root').getAttribute('aria-invalid'), 'true');
    assert.equal(await page.evaluate(() => document.activeElement?.id), 'project-root');
    console.log('PASS: native first-use form explains a missing folder and focuses the path without starting a project.');
    await page.locator('#connection summary').click();
    await page.getByRole('status').filter({ hasText: 'Aplicativo iniciado' }).waitFor({ timeout: 5000 });
    await page.getByRole('button', { name: 'Verificar novamente' }).click();
    await page.getByRole('status').filter({ hasText: 'Aplicativo iniciado' }).waitFor({ timeout: 5000 });
    await page.getByText('Escolha uma pasta para começar.', { exact: false }).waitFor();
    if (process.env.FORGE_TEST_PROJECT) {
      const field = page.getByRole('textbox', { name: 'Pasta do projeto' });
      const submit = page.getByRole('button', { name: 'Continuar nesta pasta' });
      if (process.env.FORGE_TEST_FOLDER_DIALOG === 'select') {
        await operateFolderDialog(page, 'select', process.env.FORGE_TEST_PROJECT);
        await page.locator('#project-status').filter({ hasText: 'Pasta escolhida' }).waitFor();
        assert.equal(await field.inputValue(), process.env.FORGE_TEST_PROJECT);
        assert.equal(await page.locator('#project-result').isVisible(), false);
        console.log('PASS: native Windows folder dialog canceled safely, then selected the real project folder without accepting it prematurely.');
      } else await field.fill(process.env.FORGE_TEST_PROJECT);
      await page.evaluate(() => {
        const originalCore = window.__TAURI__.core;
        const original = originalCore.invoke;
        window.progressReadErrors = [];
        const facade = Object.create(originalCore);
        Object.defineProperty(facade, 'invoke', { value: async (command, args) => {
          try { return await original(command, args); }
          catch (error) {
            if (command === 'inspect_progress') window.progressReadErrors.push(String(error));
            throw error;
          }
        } });
        window.__TAURI__.core = facade;
        window.restoreProgressInvoke = () => { window.__TAURI__.core = originalCore; };
      });
      await submit.click();
      const setupStartedAt = Date.now();
      try {
        await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
      } catch (error) {
        console.error('Project setup diagnostics:', {
          status: await page.locator('#project-status').textContent().catch(() => 'unavailable'),
          elapsedMs: Date.now() - setupStartedAt,
          root: process.env.FORGE_TEST_PROJECT,
        });
        throw error;
      }
      assert.equal(await page.locator('#project-setup').evaluate(node => node.open), false);
      assert.equal(await page.locator('#project-title').textContent(), 'Seu projeto');
      assert.equal(await page.locator('#confirmed-root').textContent(), process.env.FORGE_TEST_PROJECT);
      assert.equal(await page.locator('#project-name').textContent(), path.basename(process.env.FORGE_TEST_PROJECT));
      assert.ok((await page.locator('#confirmed-project-id').textContent()).length > 0);
      assert.equal(await page.locator('#confirmed-project-id').isVisible(), false);
      assert.equal(await page.locator('#workspace-title').textContent(), path.basename(process.env.FORGE_TEST_PROJECT));
      assert.equal(await page.locator('#recent-projects .recent-project h2').first().textContent(), path.basename(process.env.FORGE_TEST_PROJECT));
      assert.equal(await page.locator('#recent-projects .project-icon img').first().getAttribute('src'), 'assets/forge.png');
      assert.equal(await page.locator('#workspace-back').getAttribute('href'), '#projects');
      for (const selector of ['nav a:first-child', '#workspace-back', '#project-location summary']) {
        assert.ok(await page.locator(selector).evaluate(node => node.getBoundingClientRect().height >= 48), `${selector} should keep a 48px native hit target`);
      }
      assert.equal(await page.getByRole('heading', { name: 'Onde estamos' }).isVisible(), true);
      assert.equal(await page.locator('#project-record').isVisible(), true);
      assert.equal(await page.locator('.preview-empty').isVisible(), true);
      assert.match(await page.locator('#preview-intro').textContent(), /arquivos citados na conversa.*escolha um da pasta/);
      assert.equal(await page.locator('.project #project-record').count(), 0);
      if (await page.evaluate(() => innerWidth > 900)) {
        assert.equal(await page.evaluate(() => document.querySelector('#project-preview').getBoundingClientRect().top < document.querySelector('.project').getBoundingClientRect().top), true);
        assert.equal(await page.evaluate(() => document.querySelector('#project-preview').getBoundingClientRect().top < document.querySelector('#project-record').getBoundingClientRect().top), true);
        const nativeViewport = await page.evaluate(() => {
          window.scrollTo(0, 0);
          const history = document.querySelector('.conversation-body').getBoundingClientRect();
          const invitation = document.querySelector('.empty-conversation h3').getBoundingClientRect();
          const form = document.getElementById('message-form').getBoundingClientRect();
          const send = document.getElementById('send-message').getBoundingClientRect();
          return { width: innerWidth, height: innerHeight, history: { top: history.top, bottom: history.bottom, height: history.height }, invitation: { top: invitation.top, bottom: invitation.bottom }, form: { top: form.top }, send: { top: send.top, bottom: send.bottom } };
        });
        assert.equal(nativeViewport.history.height >= 150 && nativeViewport.invitation.top >= nativeViewport.history.top &&
          nativeViewport.invitation.bottom <= nativeViewport.history.bottom && nativeViewport.history.bottom <= nativeViewport.form.top + 1 &&
          nativeViewport.send.top >= 0 && nativeViewport.send.bottom <= nativeViewport.height + 2,
        true, `Native project-ready invitation, history and Send should fit without overlap: ${JSON.stringify(nativeViewport)}`);
      }
      if (process.env.FORGE_TEST_CONVERSATION_LIST === '1') {
        await page.evaluate(() => {
          window.forgePickerCalls = { listed: [], connected: [], sent: 0 };
          const originalCore = window.__TAURI__.core;
          const invoke = originalCore.invoke;
          const observedInvoke = async (command, args) => {
            if (command === 'connect_agent') window.forgePickerCalls.connected.push(args.threadId);
            if (command === 'send_message') window.forgePickerCalls.sent++;
            const result = await invoke(command, args);
            if (command === 'list_conversations') window.forgePickerCalls.listed.push(...result.conversations.map(choice => choice.id));
            return result;
          };
          const facade = Object.create(originalCore);
          Object.defineProperty(facade, 'invoke', { value: observedInvoke });
          window.__TAURI__.core = facade;
        });
        await page.locator('#conversation-picker summary').click();
        await page.getByRole('button', { name: 'Buscar conversas' }).click();
        await page.locator('#conversation-list-status').filter({ hasText: 'Página 1' }).waitFor({ timeout: 30000 });
        const choices = await page.locator('#conversation-list .conversation-choice').count();
        assert.ok(choices > 0 && choices <= 6, 'Expected a bounded real Codex conversation page for the confirmed project');
        assert.equal(await page.locator('#messages article').count(), 0, 'Listing must not open or replay a conversation');
        if (process.env.FORGE_PICKER_SCREENSHOT) await page.locator('#conversation-picker').screenshot({ path: process.env.FORGE_PICKER_SCREENSHOT });
        const firstPageIds = await page.evaluate(() => window.forgePickerCalls.listed.slice());
        if (await page.getByRole('button', { name: 'Próxima página' }).isVisible()) {
          await page.getByRole('button', { name: 'Próxima página' }).click();
          await page.locator('#conversation-list-status').filter({ hasText: 'Página 2' }).waitFor({ timeout: 30000 });
          assert.ok(await page.locator('#conversation-list .conversation-choice').count() <= 6);
          await page.getByRole('button', { name: 'Página anterior' }).click();
          await page.locator('#conversation-list-status').filter({ hasText: 'Página 1' }).waitFor({ timeout: 30000 });
        }
        console.log(`PASS: native read-only Codex index listed ${choices} project conversations per page without opening or sending a turn.`);
        if (process.env.FORGE_TEST_CONVERSATION_PICKER_ID) {
          const expected = process.env.FORGE_TEST_CONVERSATION_PICKER_ID;
          const index = firstPageIds.indexOf(expected);
          assert.ok(index >= 0, `Expected selected real conversation in the indexed first page (captured ${firstPageIds.length} IDs, ${choices} rows)`);
          await page.locator('.conversation-choice').nth(index).getByRole('button', { name: /^Retomar:/ }).click();
          await page.locator('#agent-status').filter({ hasText: /Conversa retomada|Não foi possível|não abriu|incompatível/ }).waitFor({ timeout: 110000 });
          assert.match(await page.locator('#agent-status').textContent(), /Conversa retomada/, 'Selected real conversation must resume');
          const calls = await page.evaluate(() => window.forgePickerCalls);
          assert.deepEqual(calls.connected, [expected], 'Picker must resume the exact selected Codex thread');
          assert.equal(calls.sent, 0, 'Opening history must not send a turn');
          const project = await page.evaluate(root => window.__TAURI__.core.invoke('inspect_project', { projectRoot: root }), process.env.FORGE_TEST_PROJECT);
          const bookmark = await page.evaluate(key => localStorage.getItem(key), `forge.conversation.v1:${JSON.stringify([project.project_id, project.project_root])}`);
          assert.equal(bookmark, expected, 'Successful explicit resume updates the last-conversation bookmark');
          await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
          await page.locator('#agent-status').filter({ hasText: 'Desconectado' }).waitFor({ timeout: 10000 });
          console.log(`PASS: native picker resumed the exact selected real Codex thread with ${await page.locator('#messages article').count()} messages and no send.`);
        } else await page.locator('#conversation-picker summary').click();
      }
      // The desktop record query has a 90-second bound for large projects.
      await page.locator('#progress-status').filter({ hasText: /Consultado às|Não foi possível atualizar/ }).waitFor({ timeout: 105000 });
      let progressStatus = await page.locator('#progress-status').textContent();
      if (/Não foi possível atualizar/.test(progressStatus)) {
        console.log('PARTIAL: automatic Forge record lookup failed on first attempt; native error:', await page.evaluate(() => window.progressReadErrors));
        await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
        await page.locator('#progress-status').filter({ hasText: /Consultado às|Não foi possível atualizar/ }).waitFor({ timeout: 105000 });
        progressStatus = await page.locator('#progress-status').textContent();
      }
      assert.match(progressStatus, /Consultado às/, 'Native Forge record lookup must succeed, not merely finish');
      if (process.env.FORGE_NATIVE_VIEWPORT_SCREENSHOT) {
        await page.evaluate(() => window.scrollTo(0, 0));
        await page.screenshot({ path: process.env.FORGE_NATIVE_VIEWPORT_SCREENSHOT });
      }
      await page.setViewportSize({ width: 390, height: 844 });
      assert.equal(await page.locator('#mobile-workspace-nav').isVisible(), true, 'Native narrow workspace exposes direct panel choices');
      assert.equal(await page.locator('#project-conversation').isVisible(), true);
      assert.equal(await page.locator('#project-record').isVisible(), false);
      await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Andamento' }).click();
      assert.equal(await page.locator('#project-record').isVisible(), true, 'Native narrow workspace can inspect the real Forge record');
      assert.equal(await page.locator('#project-panel').isVisible(), false, 'Record view does not repeat the folder and connection card');
      if (process.env.FORGE_MOBILE_PROGRESS_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_MOBILE_PROGRESS_SCREENSHOT, fullPage: true });
      await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Projeto' }).click();
      assert.equal(await page.locator('#project-panel').isVisible(), true, 'Native narrow project controls remain one choice away');
      assert.equal(await page.locator('#project-name').isVisible(), false, 'The narrow project name is not duplicated in the card');
      assert.equal(await page.locator('#project-setup summary').textContent(), 'Trocar de projeto');
      if (process.env.FORGE_MOBILE_PROJECT_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_MOBILE_PROJECT_SCREENSHOT, fullPage: true });
      await page.locator('#project-setup summary').click();
      assert.equal(await page.locator('#browse-project').isVisible(), true, 'The Windows folder picker is still reachable from the project pane');
      await page.locator('#project-setup summary').click();
      await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Prévia' }).click();
      assert.equal(await page.locator('#project-preview').isVisible(), true);
      if (process.env.FORGE_MOBILE_PREVIEW_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_MOBILE_PREVIEW_SCREENSHOT, fullPage: true });
      await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Conversa' }).click();
      assert.equal(await page.locator('#project-conversation').isVisible(), true);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
      if (process.env.FORGE_MOBILE_WORKSPACE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_MOBILE_WORKSPACE_SCREENSHOT, fullPage: true });
      await page.setViewportSize(initialViewport);
      console.log('PASS: native narrow workspace switches conversation, real record, project controls and preview without changing the project.');
      await page.evaluate(() => window.restoreProgressInvoke());
      assert.ok((await page.locator('#record-state').textContent()).length > 0);
      assert.ok((await page.locator('#record-phase').textContent()).length > 0);
      const recordedPhase = await page.locator('#workspace-phase').textContent();
      assert.equal(await page.locator('#workspace-phase').isVisible(), true);
      if (await page.locator('#record-pending').isVisible()) {
        assert.equal(await page.locator('#record-pending').evaluate(node => !!(node.compareDocumentPosition(document.getElementById('record-direction-card')) & Node.DOCUMENT_POSITION_FOLLOWING)), true,
          'Native unresolved questions should precede optional technical direction');
      }
      if (recordedPhase === 'Próximo passo ainda não registrado') {
        assert.equal(await page.locator('#record-empty-help').isVisible(), true);
        assert.match(await page.locator('#record-empty-help').textContent(), /Comece pela conversa\..*arquivos continuam na pasta escolhida/);
        assert.equal(await page.locator('.record-stage').isVisible(), false);
        assert.equal(await page.locator('#record-work').isVisible(), false);
        assert.equal(await page.locator('#record-direction').isVisible(), false);
      } else {
        assert.match(recordedPhase, /Etapa do projeto:/);
        assert.ok((await page.locator('#record-outcome').textContent()).length > 0);
        assert.ok((await page.locator('#record-title').textContent()).length > 0);
        assert.ok((await page.locator('#record-next').textContent()).length > 0);
        assert.equal(await page.locator('#record-activity').isVisible(), false, 'Technical activity should start collapsed');
        if (await page.evaluate(() => innerWidth === 1180 && innerHeight === 820)) assert.equal(await page.evaluate(() => {
          window.scrollTo(0, 0);
          const next = document.getElementById('record-next').getBoundingClientRect();
          return next.top >= 0 && next.top < innerHeight;
        }), true, 'Native real recorded next step should begin in the initial workspace viewport');
        assert.equal(await page.locator('#record-next').isVisible(), true, 'Recorded next step must be visible without opening details');
        await page.locator('#record-activity-details summary').click();
        assert.equal(await page.locator('#record-activity').isVisible(), true, 'Original native Forge activity remains available on request');
        await page.locator('#record-activity-details summary').click();
        assert.equal(await page.locator('#record-outcome').isVisible(), false, 'Supporting details begin collapsed');
        assert.match(await page.locator('#record-decisions').textContent(), /neste acompanhamento/);
        assert.equal(await page.locator('#record-direction-card').isVisible(), true);
        assert.equal(await page.locator('#record-direction-card').evaluate(node => node.open), false);
        assert.equal(await page.locator('#record-direction-outcome').isVisible(), false, 'Long native record wording must be optional reading');
        if (process.env.FORGE_RECORD_COLLAPSED_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_RECORD_COLLAPSED_SCREENSHOT });
        await page.locator('#record-direction-card > summary').click();
        assert.equal(await page.getByRole('button', { name: 'Pedir uma explicação na conversa' }).isVisible(), true);
        await page.locator('#record-direction summary').click();
        assert.ok((await page.locator('#record-direction-outcome').textContent()).length > 0);
        assert.ok((await page.locator('#record-revision').textContent()).length > 0);
        assert.match(await page.locator('#record-direction').textContent(), /não comprova aprovação humana independente/);
        assert.equal(await page.locator('#record-direction script').count(), 0);
      }
      if (await page.locator('#record-pending').isVisible()) {
        assert.equal(await page.locator('#record-questions-shortcut').isVisible(), true);
        await page.locator('#record-questions-shortcut').click();
        assert.equal(await page.evaluate(() => document.activeElement?.id), 'record-pending-heading', 'Native questions shortcut should focus the actual Forge questions');
        if (await page.locator('#record-suggested').isVisible()) {
          assert.equal(await page.locator('#record-suggested').evaluate(node => node.open), false,
            'Native current Forge suggestions should start collapsed');
          assert.equal(await page.getByRole('button', { name: 'Entender sugestões na conversa' }).isVisible(), true);
          const draft = page.locator('#message-text');
          const originalDraft = await draft.inputValue();
          await page.getByRole('button', { name: 'Entender sugestões na conversa' }).click();
          assert.match(await draft.inputValue(), /Consulte as perguntas que o Forge sugere agora/);
          assert.match(await draft.inputValue(), /Não trate sugestões como decisões minhas/);
          assert.equal(await page.locator('#messages article').count(), 0, 'The real Forge suggestion action must not send a turn');
          await draft.fill(originalDraft);
          await page.locator('#record-suggested summary').click();
          await page.locator('#record-suggestions .question-action').first().click();
          assert.match(await draft.inputValue(), /Explique em português claro/);
          assert.match(await draft.inputValue(), /Ainda não estou escolhendo esta opção/);
          assert.equal(await page.locator('#messages article').count(), 0, 'Discussing a real suggested question only prepares a draft');
          await draft.fill(originalDraft);
        }
      } else assert.equal(await page.locator('#record-questions-shortcut').isVisible(), false);
      if (process.env.FORGE_TEST_PENDING_ACTION === '1') {
        await page.evaluate(() => {
          const core = window.__TAURI__.core;
          const facade = Object.create(core);
          window.pendingActionCore = core;
          Object.defineProperty(facade, 'invoke', { value: async (command, args) => {
            const data = await core.invoke(command, args);
            return command === 'inspect_progress'
              ? { ...data, recorded_pending_count: 1, suggested_questions: [] }
              : data;
          } });
          window.__TAURI__.core = facade;
        });
        await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
        await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
        assert.equal(await page.getByRole('button', { name: 'Entender escolhas em aberto' }).isVisible(), true);
        if (process.env.FORGE_PENDING_SCREENSHOT) await page.locator('#project-record').screenshot({ path: process.env.FORGE_PENDING_SCREENSHOT });
        const draft = page.locator('#message-text');
        const originalDraft = await draft.inputValue();
        const messageCount = await page.locator('#messages article').count();
        await page.getByRole('button', { name: 'Entender escolhas em aberto' }).click();
        assert.match(await draft.inputValue(), /Se não conseguir recuperá-lo, diga isso claramente/);
        assert.equal(await page.locator('#messages article').count(), messageCount, 'A controlled pending decision must prepare a draft, not send a turn');
        await draft.fill(originalDraft);
        await page.evaluate(() => { window.__TAURI__.core = window.pendingActionCore; delete window.pendingActionCore; });
        await page.getByRole('button', { name: 'Atualizar andamento', exact: true }).click();
        await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor();
        console.log('PASS: native WebView prepares a pending-decision explanation from controlled readback without sending or recording a choice.');
      }
      if (process.env.FORGE_RECORD_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_RECORD_SCREENSHOT, fullPage: true });
      console.log(`PASS: actual bounded Forge workflow readback displayed ${recordedPhase === 'Próximo passo ainda não registrado' ? 'its honest empty state' : 'separately from agent activity'}.`);
      if (process.env.FORGE_TEST_DIRECTION_HISTORY === '1') {
        await page.locator('#direction-history summary').click();
        await page.locator('#direction-history-status').filter({ hasText: /direç(ão|ões) registrada/ }).waitFor({ timeout: 35000 });
        const count = await page.locator('#direction-history-list article').count();
        assert.ok(count >= 1, 'Expected at least one real accepted objective revision');
        assert.match(await page.locator('#direction-history-list article').first().textContent(), /Direção atual/);
        assert.equal(await page.locator('#direction-history-list script').count(), 0);
        const draft = page.locator('#message-text');
        const originalDraft = await draft.inputValue();
        await page.getByRole('button', { name: count === 1 ? 'Entender esta direção na conversa' : 'Entender mudanças na conversa' }).click();
        assert.match(await draft.inputValue(), /histórico do objetivo registrado deste projeto/);
        assert.match(await draft.inputValue(), /Não presuma minha aprovação/);
        assert.equal(await page.locator('#messages article').count(), 0, 'Objective-history explanation only prepares a draft');
        await draft.fill(originalDraft);
        console.log(`PASS: native read-only Forge report projected ${count} real direction revision(s) on demand.`);
      }
      if (process.env.FORGE_TEST_RESUME_THREAD_ID) {
        await page.evaluate(async ({ root, id }) => {
          const project = await window.__TAURI__.core.invoke('inspect_project', { projectRoot: root });
          localStorage.setItem(`forge.conversation.v1:${JSON.stringify([project.project_id, project.project_root])}`, id);
        }, { root: process.env.FORGE_TEST_PROJECT, id: process.env.FORGE_TEST_RESUME_THREAD_ID });
        const started = Date.now();
        await openConversation(page);
        await page.waitForFunction(() => !document.getElementById('agent-status')?.textContent?.includes('Conectando ao Codex'), null, { timeout: 110000 });
        const status = await page.locator('#agent-status').textContent();
        assert.match(status, /Conversa retomada/, `Real-history resume status: ${status}`);
        const count = await page.locator('#messages article').count();
        const initialElapsed = Date.now() - started;
        const minimumHistoryMessages = Number(process.env.FORGE_TEST_MIN_HISTORY_MESSAGES || 50);
        assert.ok(Number.isSafeInteger(minimumHistoryMessages) && minimumHistoryMessages > 0, 'Real-history minimum must be a positive integer');
        assert.ok(count >= minimumHistoryMessages, `Expected at least ${minimumHistoryMessages} real conversation messages, got ${count}`);
        const scroll = await page.locator('.conversation-body').evaluate(node => ({ bounded: node.scrollHeight > node.clientHeight, nearEnd: node.scrollHeight - node.clientHeight - node.scrollTop < 2 }));
        assert.deepEqual(scroll, { bounded: true, nearEnd: true });
        assert.equal(await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).isEnabled(), true);
        assert.equal(await page.locator('#message-form').evaluate(node => {
          const box = node.getBoundingClientRect();
          return box.top < innerHeight && box.bottom > 0;
        }), true, 'The composer should be visible after opening a long conversation');
        await page.waitForFunction(() => {
          const box = document.getElementById('send-message').getBoundingClientRect();
          return box.top >= 0 && box.bottom <= innerHeight + 2;
        }, null, { timeout: 2000 });
        assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).evaluate(node => {
          const box = node.getBoundingClientRect();
          return box.top >= 0 && box.bottom <= innerHeight + 2;
        }), true, 'Send should be visible after opening a long conversation');
        const firstMessage = await page.locator('#messages article').first().textContent();
        const lastMessage = await page.locator('#messages article').last().textContent();
        if (process.env.FORGE_REAL_HISTORY_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_REAL_HISTORY_SCREENSHOT });
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado' }).waitFor({ timeout: 10000 });
        await page.reload();
        await page.locator('nav a[data-route="workspace"]').click();
        await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(process.env.FORGE_TEST_PROJECT);
        await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
        await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 15000 });
        await openConversation(page);
        await page.waitForFunction(() => !document.getElementById('agent-status')?.textContent?.includes('Conectando ao Codex'), null, { timeout: 110000 });
        assert.match(await page.locator('#agent-status').textContent(), /Conversa retomada/, 'Real-history resume after reload');
        assert.equal(await page.locator('#messages article').count(), count);
        assert.equal(await page.locator('#messages article').first().textContent(), firstMessage);
        assert.equal(await page.locator('#messages article').last().textContent(), lastMessage);
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado' }).waitFor({ timeout: 10000 });
        console.log(`PASS: resumed ${count} real Codex conversation messages without sending a turn; initially visible after ${initialElapsed} ms, then reloaded the WebView and preserved first/last message order.`);
      }
      // A failed lookup must hide the preceding project's identity.
      await page.locator('#project-setup summary').click();
      assert.equal(await page.locator('#project-setup').evaluate(node => node.open), true);
      const legacyProject = path.join(profile, 'legacy-project');
      const existingFile = path.join(legacyProject, 'existing-work.txt');
      await mkdir(legacyProject);
      await writeFile(existingFile, 'Keep this existing work unchanged.\n');
      if (process.env.FORGE_TEST_FOLDER_DIALOG === 'select') {
        await operateFolderDialog(page, 'select', legacyProject);
        await page.locator('#project-status').filter({ hasText: 'Pasta escolhida' }).waitFor();
        assert.equal(await field.inputValue(), legacyProject);
        assert.equal(await page.locator('#project-result').isVisible(), false);
      } else await field.fill(legacyProject);
      await assert.rejects(access(path.join(legacyProject, '.forge-method.yaml')), { code: 'ENOENT' });
      await submit.click();
      try { await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 }); }
      catch (error) {
        console.error('Existing-folder onboarding status:', await page.locator('#project-status').textContent());
        throw error;
      }
      assert.equal(await page.locator('#confirmed-root').textContent(), legacyProject);
      assert.equal(await readFile(existingFile, 'utf8'), 'Keep this existing work unchanged.\n');
      await access(path.join(legacyProject, '.forge-method.yaml'));
      await access(path.join(profile, 'forge-legacy-project', '.forge-method'));
      console.log('PASS: a folder with pre-existing work received Forge onboarding without changing its file; linked project opened through the same action.');
      await page.locator('#project-setup summary').click();
      await field.fill(path.join(profile, 'missing-folder'));
      assert.equal(await page.locator('#project-result').isVisible(), false);
      assert.equal(await page.locator('#project-status').textContent(), '');
      await submit.click();
      await page.locator('#project-status').filter({ hasText: 'pasta que existe' }).waitFor();
      assert.equal(await page.locator('#project-result').isVisible(), false);
      assert.equal(await page.locator('#workspace-title').textContent(), 'Vamos dar vida à sua ideia.');
      assert.equal(await page.locator('#workspace-back').getAttribute('href'), '#explore');
      const newProject = path.join(profile, 'new-project');
      restartProject = newProject;
      await mkdir(newProject);
      await field.fill(newProject);
      await submit.click();
      await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
      assert.equal(await page.locator('#project-setup').evaluate(node => node.open), false);
      assert.equal(await page.locator('#confirmed-root').textContent(), newProject);
      assert.equal(await page.locator('#workspace-title').textContent(), 'new-project');
      await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor({ timeout: 35000 });
      assert.equal(await page.locator('#workspace-phase').textContent(), 'Próximo passo ainda não registrado');
      assert.equal(await page.locator('#record-empty-help').isVisible(), true);
      assert.equal(await page.locator('#record-state').isVisible(), false, 'A new project must not repeat the empty record status as a badge');
      await page.setViewportSize({ width: 390, height: 844 });
      await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Andamento' }).click();
      const messagesBeforeRecordAction = await page.locator('#messages article[data-role="user"]').count();
      await page.getByRole('button', { name: 'Conversar sobre meu projeto' }).click();
      assert.equal(await page.locator('#project-conversation').isVisible(), true);
      assert.equal(await page.evaluate(() => document.activeElement?.id), 'message-text');
      assert.equal(await page.locator('#messages article[data-role="user"]').count(), messagesBeforeRecordAction, 'The record shortcut must not Send');
      await page.setViewportSize(initialViewport);
      console.log('PASS: one folder action initialized a new project and its Forge record without a second setup command.');
      assert.equal(await page.locator('#connect-agent').isEnabled(), true);
      assert.equal(await page.locator('#send-message').isDisabled(), true, 'No-op Send is unavailable with an empty new-project composer');
      if (process.env.FORGE_TEST_NARROW_ZOOM === '1') {
        await page.setViewportSize({ width: 390, height: 844 });
        await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Conversa' }).click();
        await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
        const zoomLayout = await page.evaluate(() => ({
          documentWidth: document.documentElement.scrollWidth,
          viewportWidth: innerWidth,
          pane: document.querySelector('.workspace')?.dataset.mobilePane,
        }));
        assert.equal(zoomLayout.pane, 'conversation');
        assert.ok(zoomLayout.documentWidth <= zoomLayout.viewportWidth, `Narrow enlarged conversation overflows horizontally: ${JSON.stringify(zoomLayout)}`);
        const enlargedComposer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
        await enlargedComposer.fill('Minha ideia tem detalhes. '.repeat(25));
        await enlargedComposer.focus();
        await page.keyboard.press('Tab');
        assert.equal(await page.evaluate(() => document.activeElement?.id), 'send-message');
        const sendBounds = await page.locator('#send-message').evaluate(node => {
          const bounds = node.getBoundingClientRect();
          return { top: bounds.top, bottom: bounds.bottom, height: bounds.height, width: bounds.width, viewportHeight: innerHeight };
        });
        assert.ok(sendBounds.top >= 0 && sendBounds.bottom <= sendBounds.viewportHeight && sendBounds.height >= 44 && sendBounds.width >= 44,
          `Keyboard focus must reveal the enlarged Send target: ${JSON.stringify(sendBounds)}`);
        if (process.env.FORGE_NARROW_ZOOM_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_NARROW_ZOOM_SCREENSHOT, fullPage: false });
        await enlargedComposer.fill('');
        await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
        await page.setViewportSize(initialViewport);
        console.log('PASS: native 390px enlarged conversation keeps the composer and Send reachable by keyboard without horizontal overflow.');
      }
      if (process.env.FORGE_TEST_TERMINAL_RECORD === '1') {
        await page.evaluate(() => {
          const originalCore = window.__TAURI__.core;
          const invoke = originalCore.invoke;
          const facade = Object.create(originalCore);
          window.terminalRecordReads = 0;
          Object.defineProperty(facade, 'invoke', { value: async (command, args) => {
            if (command === 'inspect_progress') window.terminalRecordReads++;
            if (command === 'connect_agent') {
              window.terminalRecordEvents = args.events;
              return { thread_id: 'controlled-native-record-thread', resumed: false, messages: [] };
            }
            if (command === 'disconnect_agent') return;
            return invoke(command, args);
          } });
          window.__TAURI__.core = facade;
          window.restoreTerminalRecordInvoke = () => { window.__TAURI__.core = originalCore; };
        });
        await openConversation(page);
        await page.locator('#agent-status').filter({ hasText: 'Codex conectado' }).waitFor();
        await page.evaluate(() => window.terminalRecordEvents.onmessage({ kind: 'running' }));
        assert.equal(await page.locator('#progress-result').isVisible(), false);
        const readsBefore = await page.evaluate(() => window.terminalRecordReads);
        await page.evaluate(() => window.terminalRecordEvents.onmessage({ kind: 'completed' }));
        await page.waitForFunction(before => window.terminalRecordReads > before, readsBefore);
        await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor({ timeout: 35000 });
        assert.equal(await page.locator('#progress-result').isVisible(), true);
        assert.equal(await page.locator('#workspace-phase').textContent(), 'Próximo passo ainda não registrado');
        for (const kind of ['disconnected', 'update_required']) {
          await page.evaluate(() => window.terminalRecordEvents.onmessage({ kind: 'running' }));
          assert.equal(await page.locator('#progress-result').isVisible(), false);
          const readsBeforeTerminal = await page.evaluate(() => window.terminalRecordReads);
          await page.evaluate(kind => window.terminalRecordEvents.onmessage({ kind }), kind);
          await page.waitForFunction(before => window.terminalRecordReads > before, readsBeforeTerminal);
          await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor({ timeout: 35000 });
          assert.equal(await page.locator('#progress-result').isVisible(), true, `${kind} must restore the native Forge record`);
        }
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.evaluate(() => window.restoreTerminalRecordInvoke());
        console.log('PASS: native WebView completed, disconnected and update-required events re-read the unchanged, authoritative Forge project record (controlled agent events; no real Codex turn).');
      }
      assert.equal(await page.locator('#agent-access-note').isVisible(), true);
      assert.match(await page.locator('#agent-access-note').textContent(), /sem pedir confirmação/);
      const composerSize = await page.locator('#message-text').evaluate(node => node.getBoundingClientRect().height);
      await page.locator('#message-text').fill('Minha ideia tem detalhes.\n'.repeat(10));
      assert.ok(await page.locator('#message-text').evaluate(node => node.getBoundingClientRect().height) > composerSize, 'Native composer must expand for long drafts');
      await page.locator('#message-text').fill('');
      await access(path.join(newProject, '.forge-method.yaml'));
      await access(path.join(profile, 'forge-new-project', '.forge-method'));
      const previewText = path.join(newProject, 'result.txt');
      const previewImage = path.join(newProject, 'result.png');
      const outsidePreview = path.join(profile, 'outside-result.txt');
      await writeFile(previewText, '<script>fixture, not executable</script>');
      await writeFile(previewImage, await readFile(path.join(__dirname, '..', 'ui', 'assets', 'forge.png')));
      await writeFile(outsidePreview, 'outside');
      await page.evaluate(() => { window.nativePreviewCore = window.__TAURI__.core; window.nativePreviewInvoke = window.__TAURI__.core.invoke; });
      if (process.env.FORGE_TEST_PREVIEW_DIALOG === 'select') {
        await operatePreviewDialog(page, previewText);
      } else {
        await page.evaluate(file => { window.__TAURI__ = { ...window.__TAURI__, core: { ...window.nativePreviewCore, invoke: (command, args) => command === 'choose_preview_file' ? Promise.resolve(file) : window.nativePreviewInvoke(command, args) } }; }, previewText);
        await page.getByRole('button', { name: 'Escolher arquivo' }).click();
      }
      try {
        await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
      } catch (error) {
        console.error('Native preview status:', await page.locator('#preview-status').textContent());
        throw error;
      }
      assert.equal(await page.locator('#preview-text').textContent(), '<script>fixture, not executable</script>');
      assert.equal(await page.locator('#preview-result script').count(), 0);
      await page.getByRole('button', { name: 'Abrir prévia' }).click();
      assert.equal(await page.locator('#preview-dialog-text').textContent(), '<script>fixture, not executable</script>');
      assert.equal(await page.locator('#preview-dialog script').count(), 0);
      assert.equal(await page.locator('#preview-dialog-site-note').isVisible(), false, 'Text preview should not show site-only instructions');
      await page.getByRole('button', { name: 'Fechar prévia' }).click();
      assert.equal(await page.locator('#preview-dialog').isVisible(), false);
      assert.equal(await page.locator('.workspace').evaluate(node => node.classList.contains('preview-loaded')), true);
      assert.equal(await page.locator('.preview-empty').isVisible(), false);
      if (await page.evaluate(() => innerWidth > 900)) {
        assert.equal(await page.evaluate(() => document.querySelector('.preview').getBoundingClientRect().top < document.querySelector('.project').getBoundingClientRect().top), true);
        assert.equal(await page.evaluate(() => document.querySelector('#project-record').getBoundingClientRect().bottom < document.querySelector('.project').getBoundingClientRect().top), true);
      }
      await writeFile(previewText, 'updated fixture');
      await page.getByRole('button', { name: 'Atualizar prévia' }).click();
      await page.locator('#preview-text').filter({ hasText: 'updated fixture' }).waitFor({ timeout: 20000 });
      if (process.env.FORGE_TEST_TERMINAL_RECORD === '1') {
        await page.evaluate(() => {
          const originalCore = window.__TAURI__.core;
          const invoke = originalCore.invoke;
          const facade = Object.create(originalCore);
          Object.defineProperty(facade, 'invoke', { value: (command, args) => {
            if (command === 'connect_agent') {
              window.previewTerminalEvents = args.events;
              return Promise.resolve({ thread_id: 'controlled-native-preview-thread', resumed: false, messages: [] });
            }
            if (command === 'disconnect_agent') return Promise.resolve();
            return invoke(command, args);
          } });
          window.__TAURI__.core = facade;
          window.restorePreviewTerminalInvoke = () => { window.__TAURI__.core = originalCore; };
        });
        await openConversation(page);
        await page.locator('#agent-status').filter({ hasText: 'Codex conectado' }).waitFor();
        for (const kind of ['interrupted', 'failed']) {
          const changed = `file changed before ${kind}`;
          await writeFile(previewText, changed);
          await page.evaluate(kind => window.previewTerminalEvents.onmessage({ kind }), kind);
          await page.locator('#preview-text').filter({ hasText: changed }).waitFor({ timeout: 20000 });
          assert.equal(await page.locator('#preview-result').isVisible(), true);
        }
        await page.evaluate(() => {
          const previousCore = window.__TAURI__.core;
          const previousInvoke = previousCore.invoke;
          window.nativePreviewRaceReads = 0;
          const facade = Object.create(previousCore);
          Object.defineProperty(facade, 'invoke', { value: async (command, args) => {
            if (command !== 'inspect_preview') return previousInvoke(command, args);
            window.nativePreviewRaceReads++;
            const preview = await previousInvoke(command, args);
            if (window.nativePreviewRaceReads !== 1) return preview;
            return new Promise(resolve => { window.releaseNativePreviewRead = () => resolve(preview); });
          } });
          window.__TAURI__.core = facade;
          window.restoreNativePreviewRace = () => { window.__TAURI__.core = previousCore; };
        });
        await page.getByRole('button', { name: 'Atualizar prévia' }).click();
        await page.waitForFunction(() => typeof window.releaseNativePreviewRead === 'function');
        await writeFile(previewText, 'changed while preview was loading');
        await page.evaluate(() => {
          window.previewTerminalEvents.onmessage({ kind: 'failed' });
          window.releaseNativePreviewRead();
        });
        await page.locator('#preview-text').filter({ hasText: 'changed while preview was loading' }).waitFor({ timeout: 20000 });
        assert.equal(await page.evaluate(() => window.nativePreviewRaceReads), 2, 'Native preview must re-read after a terminal event interrupts an in-flight read');
        await page.evaluate(() => window.restoreNativePreviewRace());
        await page.evaluate(() => {
          const previousCore = window.__TAURI__.core;
          const previousInvoke = previousCore.invoke;
          window.nativePickerRefreshReads = 0;
          const facade = Object.create(previousCore);
          Object.defineProperty(facade, 'invoke', { value: (command, args) => {
            if (command === 'choose_preview_file') return new Promise(resolve => { window.cancelNativePreviewPicker = () => resolve(null); });
            if (command === 'inspect_preview') window.nativePickerRefreshReads++;
            return previousInvoke(command, args);
          } });
          window.__TAURI__.core = facade;
          window.restoreNativePreviewPicker = () => { window.__TAURI__.core = previousCore; };
        });
        await page.getByRole('button', { name: 'Escolher arquivo' }).click();
        await page.waitForFunction(() => typeof window.cancelNativePreviewPicker === 'function');
        await writeFile(previewText, 'changed while choosing another file');
        await page.evaluate(() => {
          window.previewTerminalEvents.onmessage({ kind: 'completed' });
          window.cancelNativePreviewPicker();
        });
        await page.locator('#preview-text').filter({ hasText: 'changed while choosing another file' }).waitFor({ timeout: 20000 });
        assert.equal(await page.evaluate(() => window.nativePickerRefreshReads), 1, 'Canceling the picker must re-read only the previously selected real file');
        await page.evaluate(() => window.restoreNativePreviewPicker());
        await page.getByRole('button', { name: 'Abrir prévia' }).click();
        await writeFile(previewText, 'changed before leaving expanded preview');
        await page.evaluate(() => {
          window.previewTerminalEvents.onmessage({ kind: 'completed' });
          location.hash = '#explore';
        });
        await page.locator('#preview-dialog').waitFor({ state: 'hidden' });
        await page.evaluate(() => { location.hash = '#workspace'; });
        await page.locator('#preview-text').filter({ hasText: 'changed before leaving expanded preview' }).waitFor({ timeout: 20000 });
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.evaluate(() => window.restorePreviewTerminalInvoke());
        console.log('PASS: terminal Codex events re-read the real project file after an in-flight read, canceled picker or expanded-preview navigation (controlled agent events; no real Send).');
      }
      await page.getByRole('button', { name: 'Abrir prévia' }).click();
      await writeFile(previewText, 'updated while enlarged');
      await page.evaluate(async () => { const { refreshPreviewAfterTurn } = await import('./preview.mjs'); await refreshPreviewAfterTurn(); });
      assert.equal(await page.locator('#preview-dialog').evaluate(node => node.open), true);
      assert.match(await page.locator('#preview-dialog-status').textContent(), /atualizada ao fechar/);
      await page.getByRole('button', { name: 'Fechar prévia' }).click();
      await page.locator('#preview-text').filter({ hasText: 'updated while enlarged' }).waitFor({ timeout: 20000 });
      const markdownFile = path.join(newProject, 'notes.md');
      await writeFile(markdownFile, '# Resultado\n- **Item** local\n<script>não executar</script>\n[fora](https://outside.example/)');
      await page.evaluate(file => { window.__TAURI__ = { ...window.__TAURI__, core: { ...window.nativePreviewCore, invoke: (command, args) => command === 'choose_preview_file' ? Promise.resolve(file) : window.nativePreviewInvoke(command, args) } }; }, markdownFile);
      await page.getByRole('button', { name: 'Escolher arquivo' }).click();
      await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
      assert.equal(await page.locator('#preview-markdown h3').textContent(), 'Resultado');
      assert.equal(await page.locator('#preview-markdown li strong').textContent(), 'Item');
      assert.equal(await page.locator('#preview-markdown script, #preview-markdown a').count(), 0);
      await page.getByRole('button', { name: 'Ver texto original' }).click();
      assert.match(await page.locator('#preview-text').textContent(), /^# Resultado/);
      await page.getByRole('button', { name: 'Abrir prévia' }).click();
      await page.getByRole('button', { name: 'Ver leitura' }).last().click();
      assert.equal(await page.locator('#preview-dialog-markdown h3').textContent(), 'Resultado');
      await page.getByRole('button', { name: 'Fechar prévia' }).click();
      console.log('PASS: native Markdown file reads as safe formatted text with original content one click away.');
      const nonvisualFile = path.join(newProject, 'report.pdf');
      await writeFile(nonvisualFile, '%PDF-1.4\nlocal fixture; not a rendered document');
      if (process.env.FORGE_TEST_NONVISUAL_DIALOG === 'select') {
        await page.evaluate(() => { window.__TAURI__ = { ...window.__TAURI__, core: window.nativePreviewCore }; });
        await operatePreviewDialog(page, nonvisualFile);
      } else {
        await page.evaluate(file => { window.__TAURI__ = { ...window.__TAURI__, core: { ...window.nativePreviewCore, invoke: (command, args) => command === 'choose_preview_file' ? Promise.resolve(file) : window.nativePreviewInvoke(command, args) } }; }, nonvisualFile);
        await page.getByRole('button', { name: 'Escolher arquivo' }).click();
      }
      await page.locator('#preview-status').filter({ hasText: 'Arquivo encontrado na pasta do projeto' }).waitFor({ timeout: 20000 });
      assert.equal(await page.locator('#preview-file-note').isVisible(), true);
      assert.equal(await page.locator('#preview-heading').textContent(), 'PDF do projeto');
      assert.equal(await page.locator('#preview-path').textContent(), 'report.pdf');
      assert.equal(await page.locator('#open-preview').isHidden(), true);
      assert.equal(await page.getByRole('button', { name: 'Abrir PDF no navegador' }).isVisible(), true);
      assert.equal(await page.locator('#open-site-browser').evaluate(node => node.classList.contains('primary')), true);
      assert.equal(await page.locator('#preview-result').evaluate(node => {
        const ids = [...node.children].map(child => child.id);
        return ids.indexOf('preview-browser-action') < ids.indexOf('request-preview-change');
      }), true);
      await page.setViewportSize({ width: 390, height: 844 });
      await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Prévia' }).click();
      assert.equal(await page.locator('#mobile-workspace-nav button').evaluateAll(nodes => nodes[0].getBoundingClientRect().top === nodes[1].getBoundingClientRect().top), true, 'Normal narrow native text keeps two workspace choices per row');
      await page.evaluate(() => { document.documentElement.style.fontSize = '36px'; });
      assert.equal(await page.locator('#mobile-workspace-nav button').evaluateAll(nodes => nodes[0].getBoundingClientRect().top !== nodes[1].getBoundingClientRect().top), true, 'Enlarged native text gives workspace choices a full row');
      assert.equal(await page.locator('#mobile-workspace-nav button').evaluateAll(nodes => nodes.every(node => { const range = document.createRange(); range.selectNodeContents(node); return range.getClientRects().length === 1; })), true, 'Native workspace choices must not split words across lines at 200% text');
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
      assert.equal(await page.getByRole('button', { name: 'Abrir PDF no navegador' }).evaluate(node => node.getBoundingClientRect().height >= 48), true);
      if (process.env.FORGE_PDF_NARROW_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_PDF_NARROW_SCREENSHOT, fullPage: true });
      await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Conversa' }).click();
      await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Prévia' }).click();
      assert.equal(await page.locator('#preview-path').textContent(), 'report.pdf');
      await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
      await page.setViewportSize(initialViewport);
      assert.match(await page.locator('#open-browser-hint').textContent(), /fora do Forge/);
      if (process.env.FORGE_NONVISUAL_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_NONVISUAL_SCREENSHOT });
      await page.getByRole('button', { name: 'Conversar sobre este arquivo' }).click();
      assert.match(await page.locator('#message-text').inputValue(), /Sobre o arquivo report\.pdf:/);
      assert.equal(await page.evaluate(() => {
        const key = `forge.draft.v1:${JSON.stringify([
          document.getElementById('confirmed-project-id').textContent,
          document.getElementById('confirmed-root').textContent,
        ])}`;
        return localStorage.getItem(key);
      }), await page.locator('#message-text').inputValue(), 'The native file shortcut must persist its complete unsent draft');
      assert.equal(await page.locator('#draft-note').isVisible(), true);
      await page.locator('#message-text').fill('');
      console.log('PASS: native unsupported project file shows a safe local card and prepares a conversation draft without opening the file; PDF picker ' + (process.env.FORGE_TEST_NONVISUAL_DIALOG === 'select' ? 'used the actual Windows dialog.' : 'response was simulated.'));
      const siteRoot = path.join(newProject, 'site');
      await mkdir(path.join(siteRoot, 'assets'), { recursive: true });
      const siteFile = path.join(siteRoot, 'index.html');
      await writeFile(siteFile, '<!doctype html><link rel="stylesheet" href="assets/site.css"><meta http-equiv="refresh" content="1;url=http://127.0.0.1:9/redirect"><main><h1>Prévia visual local</h1><img src="assets/forge.png" alt="Arte local"><p style="margin-top:1200px" id="site-tail">Fim da página</p><a href="http://127.0.0.1:9/click">Link externo</a></main><script>parent.previewEscaped=true</script>');
      await writeFile(path.join(siteRoot, 'assets', 'site.css'), 'h1 { color: rgb(11, 80, 34); }');
      await writeFile(path.join(siteRoot, 'assets', 'forge.png'), await readFile(path.join(__dirname, '..', 'ui', 'assets', 'forge.png')));
      const previewResponses = [];
      const outsidePreviewRequests = [];
      page.on('request', request => { if (request.url().startsWith('http://127.0.0.1:9/')) outsidePreviewRequests.push(request.url()); });
      page.on('response', response => { if (response.url().includes('forgepreview')) previewResponses.push({ url: response.url(), status: response.status(), headers: response.headers() }); });
      await page.evaluate(() => { window.previewEscaped = false; });
      await page.evaluate(file => { window.__TAURI__ = { ...window.__TAURI__, core: { ...window.nativePreviewCore, invoke: (command, args) => command === 'choose_preview_file' ? Promise.resolve(file) : window.nativePreviewInvoke(command, args) } }; }, siteFile);
      await page.getByRole('button', { name: 'Escolher arquivo' }).click();
      await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
      try {
        await page.frameLocator('#preview-site').getByRole('heading', { name: 'Prévia visual local' }).waitFor({ timeout: 20000 });
      } catch (error) {
        console.error('Site preview diagnosis:', previewResponses, await page.evaluate(() => ({ pageUrl: location.href, src: document.querySelector('#preview-site').src, text: document.querySelector('#preview-text').textContent, status: document.querySelector('#preview-status').textContent })), await Promise.all(page.frames().map(async frame => ({ url: frame.url(), body: (await frame.locator('body').textContent().catch(() => 'unavailable'))?.slice(0, 300) }))));
        throw error;
      }
      if (process.env.FORGE_NATIVE_DIAGNOSTICS === '1') console.log('DIAG: site heading visible');
      assert.equal(await page.locator('#preview-site-note').isVisible(), true);
      assert.equal(await page.locator('#preview-site-note').evaluate(node => node.open), false);
      await page.locator('#preview-site-note summary').click();
      assert.equal(await page.locator('#preview-site-note').evaluate(node => node.open), true);
      assert.equal(await page.frameLocator('#preview-site').locator('h1').evaluate(async node => {
        const deadline = Date.now() + 5000;
        while (Date.now() < deadline) {
          const color = getComputedStyle(node).color;
          if (color === 'rgb(11, 80, 34)') return color;
          await new Promise(resolve => setTimeout(resolve, 50));
        }
        return getComputedStyle(node).color;
      }), 'rgb(11, 80, 34)', 'Local CSS should finish loading before its color is asserted');
      if (process.env.FORGE_NATIVE_DIAGNOSTICS === '1') console.log('DIAG: local CSS applied');
      assert.ok(await page.frameLocator('#preview-site').getByRole('img', { name: 'Arte local' }).evaluate(node => node.naturalWidth > 0));
      if (process.env.FORGE_NATIVE_DIAGNOSTICS === '1') console.log('DIAG: local image loaded');
      assert.equal(await page.evaluate(() => window.previewEscaped), false);
      await page.waitForTimeout(1400);
      assert.deepEqual(outsidePreviewRequests, [], 'Preview must not request an external resource or follow automatic navigation');
      assert.equal(await page.locator('#preview-site').getAttribute('sandbox'), '');
      assert.equal(await page.evaluate(() => document.querySelector('#preview-site').contentDocument), null);
      if (process.env.FORGE_SITE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_SITE_SCREENSHOT, fullPage: true });
      await page.setViewportSize({ width: 390, height: 844 });
      await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Prévia' }).click();
      assert.equal(await page.locator('#preview-site').isVisible(), true, 'The loaded local site remains visible in the narrow preview');
      assert.equal(await page.getByRole('button', { name: 'Pedir mudança neste arquivo' }).isVisible(), true);
      assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true, 'A loaded site must not overflow the narrow workspace');
      if (process.env.FORGE_MOBILE_LOADED_PREVIEW_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_MOBILE_LOADED_PREVIEW_SCREENSHOT, fullPage: true });
      await page.setViewportSize(initialViewport);
      await page.getByRole('button', { name: 'Ver código' }).click();
      assert.match(await page.locator('#preview-text').textContent(), /<script>parent.previewEscaped=true<\/script>/);
      await page.getByRole('button', { name: 'Ver prévia visual' }).click();
      await page.getByRole('button', { name: 'Abrir prévia' }).click();
      await page.frameLocator('#preview-dialog-site').getByRole('heading', { name: 'Prévia visual local' }).waitFor({ timeout: 20000 });
      if (process.env.FORGE_NATIVE_DIAGNOSTICS === '1') console.log('DIAG: expanded site visible');
      assert.equal(await page.locator('#preview-dialog-site-note').isVisible(), true, 'Site preview should disclose its restrictions');
      const dialogFrame = page.frameLocator('#preview-dialog-site');
      assert.equal(await dialogFrame.locator('body').evaluate(() => document.documentElement.scrollHeight > innerHeight), true, 'Long local site should overflow the enlarged preview');
      assert.equal(await page.locator('#preview-dialog-site').evaluate(node => getComputedStyle(node).pointerEvents), 'none', 'Enlarged site must not accept link clicks');
      await page.getByRole('button', { name: 'Mostrar mais da página' }).click();
      assert.ok(await page.locator('#preview-dialog-site').evaluate(node => node.getBoundingClientRect().height) >= 1300, 'User must be able to reveal more of a long site');
      assert.ok(await page.locator('#preview-dialog').evaluate(node => node.scrollHeight > node.clientHeight), 'Enlarged site must be scrollable in the safe parent dialog');
      assert.deepEqual(outsidePreviewRequests, [], 'Expanding the preview must not request an external resource');
      assert.match(await dialogFrame.locator('body').evaluate(() => location.href), /forgepreview/);
      await page.getByRole('button', { name: 'Fechar prévia' }).click();
      console.log('PASS: native isolated HTML preview renders local CSS without executing scripts or sharing the app origin.');
      const imagePreview = await page.evaluate(({ root, file }) => window.nativePreviewInvoke('inspect_preview', { projectRoot: root, filePath: file }), { root: newProject, file: previewImage });
      assert.equal(imagePreview.kind, 'image');
      assert.match(imagePreview.content, /^data:image\/png;base64,/);
      await assert.rejects(page.evaluate(({ root, file }) => window.nativePreviewInvoke('inspect_preview', { projectRoot: root, filePath: file }), { root: newProject, file: outsidePreview }), /não pertence ao projeto/);
      await page.evaluate(() => { window.__TAURI__ = { ...window.__TAURI__, core: window.nativePreviewCore }; });
      await page.evaluate(async () => {
        const [{ renderAgentMessage }, { previewLinkedFile }] = await Promise.all([
          import('./message-format.mjs'), import('./preview.mjs'),
        ]);
        const fixture = document.createElement('div');
        renderAgentMessage(fixture, '[resultado](result.txt) [fora](../outside-result.txt)', previewLinkedFile);
        document.body.append(fixture);
        fixture.querySelectorAll('button')[0].click();
        fixture.remove();
      });
      await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
      assert.equal(await page.locator('#preview-text').textContent(), 'updated while enlarged');
      await page.evaluate(async () => {
        const [{ renderAgentMessage }, { previewLinkedFile }] = await Promise.all([
          import('./message-format.mjs'), import('./preview.mjs'),
        ]);
        const fixture = document.createElement('div');
        renderAgentMessage(fixture, 'Arquivo gerado: `result.txt`. URL: `https://example.com/outside.html`.', previewLinkedFile);
        if (fixture.querySelectorAll('button').length !== 1) throw new Error('Only the project-file code span should become an action');
        const action = fixture.querySelector('button');
        if (action.getAttribute('aria-label') !== 'Ver arquivo local: result.txt') throw new Error('Missing inline-code file action');
        document.body.append(fixture);
        action.click();
        fixture.remove();
      });
      await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
      assert.equal(await page.locator('#preview-text').textContent(), 'updated while enlarged');
      await page.evaluate(async () => {
        const [{ renderAgentMessage }, { previewLinkedFile }] = await Promise.all([
          import('./message-format.mjs'), import('./preview.mjs'),
        ]);
        const fixture = document.createElement('div');
        renderAgentMessage(fixture, '[fora](../outside-result.txt)', previewLinkedFile);
        document.body.append(fixture);
        fixture.querySelector('button').click();
        fixture.remove();
      });
      await page.locator('#preview-status').filter({ hasText: 'não pertence ao projeto' }).waitFor({ timeout: 20000 });
      console.log('PASS: native completed-message file action opens a project file and rejects an outside path.');
      console.log(`PASS: native read-only text/image preview, update and outside-path rejection; file picker ${process.env.FORGE_TEST_PREVIEW_DIALOG === 'select' ? 'used the actual Windows dialog' : 'response was simulated'}.`);
      await page.reload();
      await page.locator('nav a[data-route="projects"]').click();
      await page.locator('#recent-projects .recent-project').first().getByRole('button', { name: /Abrir new-project na pasta/ }).click();
      await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
      assert.equal(await page.locator('#preview-path').textContent(), 'result.txt');
      assert.equal(await page.locator('#preview-text').textContent(), 'updated while enlarged', 'Native reopening must read the selected file again rather than restore cached contents');
      assert.equal(await page.locator('#preview-result').isVisible(), true);
      console.log('PASS: native project reopening restores its last selected result through a fresh project-bound file read.');
      await page.locator('nav a[data-route="projects"]').click();
      assert.equal(await page.locator('#recent-projects .recent-project h2').first().textContent(), 'new-project');
      await page.locator('nav a[data-route="workspace"]').click();
      assert.equal(await page.locator('#project-setup').evaluate(node => node.open), false);
      console.log('PASS: empty project folder received Forge onboarding and enables Codex connection.');
      if (process.env.FORGE_TEST_AGENT_SMOKE === '1') {
        if (process.env.FORGE_TEST_AGENT_SMOKE_NARROW === '1') {
          await page.setViewportSize({ width: 390, height: 844 });
          assert.equal(await page.locator('#mobile-workspace-nav').isVisible(), true);
          await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Conversa' }).click();
          assert.equal(await page.locator('#project-conversation').isVisible(), true);
          assert.equal(await page.locator('#project-preview').isVisible(), false);
        }
        const activationProbe = process.env.FORGE_TEST_FORGE_ACTIVATION === '1';
        const prompt = activationProbe
          ? 'Quero criar um pequeno site para organizar receitas, mas ainda não decidi para quem. Antes de escrever arquivos, me ajude a definir o primeiro passo. Use o projeto aberto e o Forge que acompanha o aplicativo; não publique nem instale nada.'
          : 'Sem usar ferramentas nem alterar arquivos, responda em português apenas: Olá, vamos criar.';
        const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
        await composer.fill(prompt);
        await page.getByRole('button', { name: 'Enviar', exact: true }).click();
        try {
          await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 180000 });
        } catch (error) {
          const observed = await page.evaluate(() => ({
            agentStatus: document.querySelector('#agent-status')?.textContent,
            connectionStatus: document.querySelector('#connection-status')?.textContent,
            messages: document.querySelector('#messages')?.textContent?.slice(-1200),
            selectedPane: document.querySelector('.workspace')?.dataset.mobilePane,
            conversationVisible: !!document.querySelector('#project-conversation')?.getClientRects().length,
          }));
          throw new Error(`Real Codex reply was not received: ${JSON.stringify(observed)}`, { cause: error });
        }
        const reply = (await page.locator('#messages article[data-role="agent"]').last().innerText()).trim();
        assert.ok(reply.length > 5);
        assert.ok((await page.locator('#messages article[data-role="user"]').innerText()).includes(prompt));
        if (process.env.FORGE_TEST_AGENT_SMOKE_NARROW === '1') {
          assert.equal(await page.locator('#project-conversation').isVisible(), true);
          assert.equal(await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Conversa' }).getAttribute('aria-pressed'), 'true');
        }
        if (activationProbe) {
          const threadId = await page.evaluate(() => {
            const key = Object.keys(localStorage).find(value => value.startsWith('forge.conversation.v1:'));
            return key ? localStorage.getItem(key) : null;
          });
          assert.ok(threadId, 'The activation probe needs the actual Codex thread for tool-trace inspection');
          console.log(`ACTIVATION_PROBE_THREAD=${threadId}`);
          console.log(`ACTIVATION_PROBE_REPLY=${reply.replace(/\s+/g, ' ').slice(0, 700)}`);
        }
        if (process.env.FORGE_TEST_ARTIFACT_JOURNEY === '1') {
          await composer.fill('Crie uma página estática simples em site/index.html neste projeto, com o título "Jardim de ideias" e CSS local. Não use JavaScript, rede, publicação nem modifique arquivos fora deste projeto. Ao terminar, inclua na resposta um link Markdown relativo para eu abrir o arquivo neste aplicativo.');
          await page.getByRole('button', { name: 'Enviar', exact: true }).click();
          await page.locator('#messages article[data-role="agent"]').nth(1).waitFor({ timeout: 180000 });
          await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 180000 });
          const generatedFile = path.join(newProject, 'site', 'index.html');
          await access(generatedFile);
          const generated = await readFile(generatedFile, 'utf8');
          assert.match(generated, /Jardim de ideias/);
          const result = page.locator('#messages article[data-role="agent"]').last();
          const directPaths = await result.locator('.message-file-link').evaluateAll(nodes => nodes.map(node => node.dataset.previewPath));
          const directIndex = directPaths.findIndex(value => /(?:^|[\\/])site[\\/]index\.html$/i.test(value));
          if (directIndex >= 0) {
            await result.locator('.message-file-link').nth(directIndex).click();
          } else {
            const choices = result.locator('.message-file-choices');
            const choicePaths = await choices.locator('button').evaluateAll(nodes => nodes.map(node => node.textContent));
            const choiceIndex = choicePaths.findIndex(value => /(?:^|[\\/])site[\\/]index\.html$/i.test(value));
            assert.ok(choiceIndex >= 0, `The generated index file must be individually selectable; reply=${JSON.stringify((await result.innerText()).slice(-1400))}; paths=${JSON.stringify({ directPaths, choicePaths })}`);
            if (!await choices.evaluate(node => node.open)) await choices.locator('summary').click();
            await choices.locator('button').nth(choiceIndex).click();
          }
          await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
          assert.equal(await page.locator('#preview-path').textContent(), 'site\\index.html');
          await page.frameLocator('#preview-site').getByRole('heading', { name: 'Jardim de ideias' }).waitFor({ timeout: 20000 });
          assert.match(await page.locator('.preview-origin').first().textContent(), /Isso não confirma publicação na internet/);
          if (process.env.FORGE_ARTIFACT_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_ARTIFACT_SCREENSHOT, fullPage: true });
          await page.getByRole('button', { name: 'Pedir mudança neste arquivo' }).click();
          assert.match(await composer.inputValue(), /site\\index\.html/);
          assert.equal(await page.locator('#messages article[data-role="user"]').count(), 2, 'Preparing a change must not send another turn');
          await composer.fill('Altere apenas site/index.html neste mesmo projeto: mude o título visível da página para "Jardim de ideias renovado". Não use JavaScript, rede nem publicação.');
          await page.getByRole('button', { name: 'Enviar', exact: true }).click();
          await page.locator('#messages article[data-role="agent"]').nth(2).waitFor({ timeout: 180000 });
          await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 180000 });
          assert.match(await readFile(generatedFile, 'utf8'), /Jardim de ideias renovado/);
          if (process.env.FORGE_TEST_AGENT_SMOKE_NARROW === '1') {
            assert.equal(await page.locator('#project-conversation').isVisible(), true, 'Change request returns to the conversation');
            await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Prévia' }).click();
          }
          await page.frameLocator('#preview-site').getByRole('heading', { name: 'Jardim de ideias renovado' }).waitFor({ timeout: 20000 });
          assert.equal(await page.locator('#messages article[data-role="user"]').count(), 3);
          console.log('PASS: real Codex-created local HTML opened in the isolated preview and refreshed automatically after a follow-up in the same conversation.');
        }
        if (process.env.FORGE_TEST_AGENT_SMOKE_NARROW === '1') {
          await page.locator('#mobile-workspace-nav').getByRole('button', { name: 'Conversa' }).click();
        }
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado' }).waitFor({ timeout: 10000 });
        await page.reload();
        await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(newProject);
        await submit.click();
        await page.waitForFunction(() => document.querySelector('#project-status')?.textContent?.includes('Projeto pronto'), null, { timeout: 15000 });
        await openConversation(page);
        await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 110000 });
        assert.ok((await page.locator('#messages').innerText()).includes(prompt));
        assert.ok((await page.locator('#messages').innerText()).includes(reply));
        if (process.env.FORGE_TEST_AGENT_SMOKE_NARROW === '1') {
          assert.equal(await page.locator('#mobile-workspace-nav').isVisible(), true);
          assert.equal(await page.locator('#project-conversation').isVisible(), true);
        }
        restartHistory = {
          count: await page.locator('#messages article').count(),
          first: await page.locator('#messages article').first().textContent(),
          last: await page.locator('#messages article').last().textContent(),
        };
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado' }).waitFor({ timeout: 10000 });
        console.log('PASS: first Send opened the real Codex conversation, delivered one reply, and restored both sides after WebView reload without resending.');
      }
      if (process.env.FORGE_TEST_NEW_CONNECT === '1') {
        await openConversation(page);
        try {
          await page.waitForFunction(() => !document.querySelector('#agent-status')?.textContent?.includes('Conectando ao Codex'), null, { timeout: 110000 });
        } catch {
          throw new Error(`New-project connection did not finish: ${await page.locator('#agent-status').textContent()}`);
        }
        const connectionStatus = await page.locator('#agent-status').textContent();
        assert.match(connectionStatus, /Codex conectado/, `New-project connection status: ${connectionStatus}`);
        assert.equal(await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).isEnabled(), true);
        assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true, 'Native Send stays disabled until there is text');
        assert.equal(await page.locator('#project-status').isHidden(), true);
        assert.equal(await page.locator('#project-setup').isVisible(), true);
        assert.equal(await page.locator('#project-result').isVisible(), true);
        assert.equal(await page.locator('#project-title').textContent(), 'Seu projeto');
        assert.equal(await page.locator('#connect-agent').isHidden(), true);
        assert.equal(await page.locator('#new-conversation-choice').isHidden(), true);
        assert.equal(await page.locator('#disconnect-agent').isVisible(), true);
        assert.match(await page.locator('#empty-conversation-description').textContent(), /Sua conversa está pronta/);
        if (process.env.FORGE_CONNECTED_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_CONNECTED_SCREENSHOT, fullPage: true });
        await page.locator('#project-setup summary').click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado' }).waitFor();
        assert.equal(await page.locator('#project-status').isVisible(), true);
        assert.equal(await page.locator('#project-setup').evaluate(node => node.open), true);
        console.log('PASS: authenticated Codex connection uses the confirmed project; switching project disconnects the idle session automatically. No message was sent.');
      }
      if (process.env.FORGE_TEST_AGENT === '1') {
        if (!await page.locator('#project-setup').evaluate(node => node.open)) await page.locator('#project-setup summary').click();
        await field.fill(process.env.FORGE_TEST_PROJECT);
        await submit.click();
        await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 15000 });
        await page.evaluate(() => {
          window.forgeDeltaCount = 0;
          const NativeChannel = window.__TAURI__.core.Channel;
          const ObservedChannel = class extends NativeChannel {
            set onmessage(callback) { super.onmessage = event => { if (event.kind === 'delta') window.forgeDeltaCount++; callback(event); }; }
            get onmessage() { return super.onmessage; }
          };
          window.__TAURI__ = { ...window.__TAURI__, core: { ...window.__TAURI__.core, Channel: ObservedChannel } };
        });
        await openConversation(page);
        await page.locator('#agent-status').filter({ hasText: 'Codex conectado ao projeto' }).waitFor({ timeout: 100000 });
        assert.equal(await field.isDisabled(), true);
        const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
        await composer.fill('Esta é uma verificação somente de leitura. Não altere arquivos nem registros de estado, não publique nada, não instale nada. Consulte forge-core project resolve --root . --json para confirmar o projeto e leia apps/desktop/README.md. Em até 5 frases em português, explique quais capacidades do app estão documentadas e o que ainda falta. Não faça implementação.');
        await page.getByRole('button', { name: 'Enviar', exact: true }).click();
        try {
          await page.waitForFunction(() => /Resposta recebida|Atualize o Codex|execução falhou|conexão foi encerrada/.test(document.getElementById('agent-status').textContent), { }, { timeout: 180000 });
          assert.match(await page.locator('#agent-status').textContent(), /Resposta recebida/);
        } catch (error) {
          console.error('Native conversation status:', await page.locator('#agent-status').textContent());
          console.error('Streamed message events:', await page.evaluate(() => window.forgeDeltaCount));
          throw error;
        }
        assert.ok(await page.evaluate(() => window.forgeDeltaCount > 0));
        assert.ok((await page.locator('#messages').textContent()).includes('Codex'));
        await composer.fill('Sem usar ferramentas, escreva uma lista de 1000 exemplos de nomes de projetos, um por linha. Este pedido será interrompido para testar o botão.');
        await page.getByRole('button', { name: 'Enviar', exact: true }).click();
        const interrupt = page.getByRole('button', { name: 'Interromper', exact: true });
        await page.waitForFunction(() => !document.getElementById('interrupt-agent').disabled);
        await interrupt.click();
        await page.locator('#agent-status').filter({ hasText: 'Interrompido.' }).waitFor({ timeout: 30000 });
        await composer.fill('Sem ferramentas, responda apenas: Podemos continuar.');
        await page.getByRole('button', { name: 'Enviar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 90000 });
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor({ timeout: 10000 });
        assert.equal(await field.isDisabled(), false);
        console.log('PASS: actual ChatGPT-authenticated Codex response, streamed deltas, interruption, subsequent turn and disconnect.');
        const previousHistory = await page.locator('#messages').innerText();
        await page.reload();
        await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(process.env.FORGE_TEST_PROJECT);
        await submit.click();
        await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 15000 });
        await openConversation(page);
        await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 100000 });
        const restoredHistory = await page.locator('#messages').innerText();
        assert.ok(restoredHistory.includes('Podemos continuar'));
        assert.ok(restoredHistory.includes('Esta é uma verificação somente de leitura.'));
        assert.ok(previousHistory.includes('Podemos continuar'));
        assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true, 'A restored conversation without a draft must not offer an empty Send');
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
        console.log('PASS: history restored from Codex after transport shutdown and WebView reload, without resending a turn.');
      } else if (process.env.FORGE_TEST_AGENT_SMOKE !== '1' && process.env.FORGE_TEST_NEW_IDEA_REAL_SEND !== '1') { console.log('NOT_RUN: actual Codex conversation (real-agent smoke flags not set).'); }
    } else {
      console.log('NOT_RUN: real project resolution (FORGE_TEST_PROJECT not set).');
    }
    await page.locator('.appearance summary').click();
    await page.getByLabel('Tema', { exact: true }).selectOption('dark');
    await page.getByLabel('Reforçar contraste').check();
    await page.reload();
    assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'dark');
    assert.equal(await page.getByLabel('Reforçar contraste').isChecked(), true);
    console.log('PASS: native WebView appearance preference survives reload.');
    if (process.env.FORGE_TEST_PROCESS_RESTART === '1' || process.env.FORGE_TEST_PREVIEW_PROCESS_RESTART === '1' || ['1', 'shortcut'].includes(process.env.FORGE_TEST_DRAFT_PROCESS_RESTART)) {
      if (process.env.FORGE_TEST_PROCESS_RESTART === '1') assert.ok(restartProject && restartHistory, 'Process restart requires the real-agent smoke journey');
      if (process.env.FORGE_TEST_PREVIEW_PROCESS_RESTART === '1') assert.ok(process.env.FORGE_TEST_PROJECT, 'Preview restart requires the native project fixture');
      if (['1', 'shortcut'].includes(process.env.FORGE_TEST_DRAFT_PROCESS_RESTART)) {
        assert.ok(process.env.FORGE_TEST_PROJECT, 'Draft restart requires the native project fixture');
        const draftRoot = process.env.FORGE_TEST_DRAFT_PROCESS_RESTART === 'shortcut' ? restartProject : process.env.FORGE_TEST_PROJECT;
        assert.ok(draftRoot, 'A preview shortcut requires the generated native project');
        await page.locator('nav a[data-route="workspace"]').click();
        await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(draftRoot);
        await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
        await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 35000 });
        if (process.env.FORGE_TEST_DRAFT_PROCESS_RESTART === 'shortcut') {
          const selected = path.join(draftRoot, 'notes.md');
          await page.evaluate(file => {
            const core = window.__TAURI__.core;
            const facade = Object.create(core);
            Object.defineProperty(facade, 'invoke', { value: (command, args) => command === 'choose_preview_file' ? Promise.resolve(file) : core.invoke(command, args) });
            window.__TAURI__.core = facade;
          }, selected);
          await page.getByRole('button', { name: 'Escolher arquivo' }).click();
          await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
          await page.getByRole('button', { name: 'Pedir mudança neste arquivo' }).click();
          assert.match(await page.locator('#message-text').inputValue(), /Quero mudar o arquivo notes\.md:/);
        } else await page.locator('#message-text').fill('Rascunho preservado após reiniciar o aplicativo');
      }
      await browser.close();
      browser = null;
      const exited = once(child, 'exit');
      child.kill();
      await exited;
      child = spawn(process.env.FORGE_DESKTOP_EXE, [], {
        windowsHide: true,
        stdio: 'ignore',
        env: {
          ...process.env,
          WEBVIEW2_USER_DATA_FOLDER: profile,
          WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
        },
      });
      const restartDeadline = Date.now() + 20000;
      while (Date.now() < restartDeadline) {
        if (child.exitCode !== null) throw new Error(`Restarted application exited: ${child.exitCode}`);
        try {
          browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 });
          break;
        } catch { await new Promise(resolve => setTimeout(resolve, 200)); }
      }
      if (!browser) throw new Error('Restarted native WebView did not become available');
      const restartedContext = browser.contexts()[0];
      page = restartedContext.pages()[0] || await restartedContext.waitForEvent('page', { timeout: 5000 });
      await page.locator('#home').waitFor({ state: 'visible' });
      if (['1', 'shortcut'].includes(process.env.FORGE_TEST_DRAFT_PROCESS_RESTART)) {
        const draftRoot = process.env.FORGE_TEST_DRAFT_PROCESS_RESTART === 'shortcut' ? restartProject : process.env.FORGE_TEST_PROJECT;
        await page.evaluate(() => {
          window.draftRestartSendCount = 0;
          const core = window.__TAURI__.core;
          const facade = Object.create(core);
          Object.defineProperty(facade, 'invoke', { value: (command, args) => {
            if (command === 'send_message') window.draftRestartSendCount++;
            return core.invoke(command, args);
          } });
          window.__TAURI__.core = facade;
        });
        await page.locator('nav a[data-route="projects"]').click();
        await page.locator('#recent-projects .recent-project').filter({ hasText: draftRoot }).getByRole('button', { name: /^Abrir / }).click();
        await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 35000 });
        if (process.env.FORGE_TEST_DRAFT_PROCESS_RESTART === 'shortcut') {
          assert.match(await page.locator('#message-text').inputValue(), /Quero mudar o arquivo notes\.md:/);
          assert.equal(await page.locator('#draft-note').isVisible(), true);
        } else assert.equal(await page.locator('#message-text').inputValue(), 'Rascunho preservado após reiniciar o aplicativo');
        assert.equal(await page.evaluate(() => window.draftRestartSendCount), 0, 'Restored native draft must never be sent automatically');
        console.log(`PASS: full native process restart restored the unsent ${process.env.FORGE_TEST_DRAFT_PROCESS_RESTART === 'shortcut' ? 'preview change request' : 'project draft'} without sending a turn.`);
      }
      if (process.env.FORGE_TEST_PREVIEW_PROCESS_RESTART === '1') {
        await page.locator('nav a[data-route="projects"]').click();
        await page.locator('#recent-projects .recent-project').first().getByRole('button', { name: /Abrir new-project na pasta/ }).click();
        await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 25000 });
        assert.equal(await page.locator('#preview-path').textContent(), 'result.txt');
        assert.equal(await page.locator('#preview-text').textContent(), 'updated while enlarged');
        console.log('PASS: full native process restart reopened the project and re-read its last selected result without cached file content.');
      }
      if (process.env.FORGE_TEST_PROCESS_RESTART === '1') {
        await page.locator('nav a[data-route="workspace"]').click();
        await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(restartProject);
        await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
        await page.waitForFunction(() => document.querySelector('#project-status')?.textContent?.includes('Projeto pronto'), null, { timeout: 25000 });
        await page.evaluate(() => {
          window.restartSendCount = 0;
          const originalCore = window.__TAURI__.core;
          const invoke = originalCore.invoke;
          const facade = Object.create(originalCore);
          Object.defineProperty(facade, 'invoke', { value: (command, args) => {
            if (command === 'send_message') window.restartSendCount++;
            return invoke(command, args);
          } });
          window.__TAURI__.core = facade;
        });
        await openConversation(page);
        await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 110000 });
        assert.equal(await page.locator('#messages article').count(), restartHistory.count);
        assert.equal(await page.locator('#messages article').first().textContent(), restartHistory.first);
        assert.equal(await page.locator('#messages article').last().textContent(), restartHistory.last);
        assert.equal(await page.evaluate(() => window.restartSendCount), 0);
        console.log('PASS: full native process restart restored the real Codex conversation in order without resending a turn.');
      }
    }
    if (process.env.FORGE_TEST_NEW_IDEA_PROJECT === '1') {
      assert.ok(process.env.FORGE_TEST_PROJECT, 'The new-idea check requires FORGE_TEST_PROJECT');
      const projectForNewIdea = path.join(profile, 'new-project');
      await access(path.join(projectForNewIdea, '.forge-method.yaml'));
      await page.locator('nav a[data-route="workspace"]').click();
      if (process.env.FORGE_TEST_PROCESS_RESTART === '1') {
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado' }).waitFor({ timeout: 10000 });
      }
      if (!await page.locator('#project-setup').evaluate(node => node.open)) await page.locator('#project-setup summary').click();
      await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(projectForNewIdea);
      await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
      await page.waitForFunction(() => document.querySelector('#project-status')?.textContent?.includes('Projeto pronto'), null, { timeout: 25000 });
      await page.locator('nav a[data-route="explore"]').click();
      await page.getByRole('link', { name: /Arte e criação/ }).click();
      await page.locator('#workspace').waitFor({ state: 'visible' });
      assert.equal(await page.locator('#project-root').inputValue(), '');
      assert.equal(await page.locator('#project-result').isHidden(), true);
      assert.equal(await page.locator('#project-setup').evaluate(node => node.open), true);
      assert.equal(await page.getByRole('button', { name: 'Escolher pasta para continuar' }).isEnabled(), true);
      assert.match(await page.locator('#message-text').inputValue(), /artístico/i);
      console.log('PASS: native Explore starts a new idea with its draft but without silently reusing the previous project.');
      if (process.env.FORGE_TEST_NEW_IDEA_REAL_SEND === '1') {
        const freshIdeaProject = path.join(profile, 'fresh-idea-project');
        await mkdir(freshIdeaProject);
        await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(freshIdeaProject);
        await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
        await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 35000 });
        assert.equal(await page.locator('#confirmed-root').textContent(), freshIdeaProject);
        const safePrompt = 'Sem usar ferramentas nem alterar arquivos, responda em português apenas: A nova ideia está no projeto certo.';
        await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).fill(safePrompt);
        await page.getByRole('button', { name: 'Enviar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 130000 });
        assert.equal(await page.locator('#messages article[data-role="user"]').count(), 1);
        assert.equal(await page.locator('#messages article[data-role="agent"]').count(), 1);
        assert.match(await page.locator('#messages article[data-role="agent"]').innerText(), /nova ideia|projeto certo/i);
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado' }).waitFor({ timeout: 10000 });
        console.log('PASS: a fresh Explore idea selected a different real Forge project and delivered one Codex reply there.');
      }
    }
    if (process.env.FORGE_TEST_OPEN_ANOTHER_PROJECT === '1') {
      assert.ok(process.env.FORGE_TEST_PROJECT, 'The project-switch check requires FORGE_TEST_PROJECT');
      await page.locator('nav a[data-route="workspace"]').click();
      await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(process.env.FORGE_TEST_PROJECT);
      await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
      await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 35000 });
      await page.locator('#message-text').fill('Rascunho exclusivo deste projeto');
      await page.locator('nav a[data-route="projects"]').click();
      await page.getByRole('link', { name: 'Abrir outro projeto' }).click();
      await page.locator('#workspace').waitFor({ state: 'visible' });
      assert.equal(await page.locator('#project-root').inputValue(), '');
      assert.equal(await page.locator('#project-result').isHidden(), true);
      assert.equal(await page.locator('#messages article').count(), 0);
      assert.equal(await page.locator('#message-text').inputValue(), '', 'The old project draft must not appear in another project');
      assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).count(), 0);
      assert.equal(await page.getByRole('button', { name: 'Escolher pasta para continuar' }).count(), 1);
      await page.locator('nav a[data-route="projects"]').click();
      await page.locator('#recent-projects .recent-project').filter({ hasText: process.env.FORGE_TEST_PROJECT }).getByRole('button', { name: /^Abrir / }).click();
      await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 35000 });
      assert.equal(await page.locator('#message-text').inputValue(), 'Rascunho exclusivo deste projeto', 'Returning to the original project restores its unsent draft');
      console.log('PASS: native project switching isolates the previous folder, conversation and unsent draft; reopening restores that draft without sending.');
    }
    if (process.env.FORGE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_SCREENSHOT, fullPage: true });
    console.log('PASS: real native window, frontend-to-Rust identity and retry.');
  } finally {
    try {
      if (browser) {
        try {
          const page = browser.contexts()[0]?.pages()[0];
          if (page) await page.evaluate(() => Promise.race([
            window.__TAURI__?.core?.invoke('disconnect_agent').catch(() => {}),
            new Promise(resolve => setTimeout(resolve, 5000)),
          ])).catch(() => {}); // A closing or navigated WebView must not mask the test result.
        } finally { await browser.close(); }
      }
    } finally {
      try {
        if (child.pid && child.exitCode === null && child.signalCode === null) {
          const exited = once(child, 'exit');
          child.kill();
          await exited;
        }
      } finally {
        if (path.dirname(path.resolve(profile)) !== path.resolve(tmpdir()) || !path.basename(profile).startsWith('forge-desktop-webview-')) {
          throw new Error('Refusing cleanup outside the test profile');
        }
        await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
      }
    }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
