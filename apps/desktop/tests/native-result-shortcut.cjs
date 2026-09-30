// Focused Windows WebView smoke: a restored Codex file citation opens through
// the real project-bound native preview after a full process restart.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, mkdir, rm, writeFile } = require('node:fs/promises');
const { once } = require('node:events');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

async function freePort() {
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}

async function launch(exe, profile, port) {
  const child = spawn(exe, [], {
    windowsHide: true, stdio: 'ignore',
    env: { ...process.env, WEBVIEW2_USER_DATA_FOLDER: profile,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1` },
  });
  let launchError;
  child.on('error', error => { launchError = error; });
  const deadline = Date.now() + 25000;
  while (Date.now() < deadline) {
    if (launchError) throw launchError;
    if (child.exitCode !== null) throw new Error(`Application exited: ${child.exitCode}`);
    try {
      const browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 });
      const context = browser.contexts()[0];
      const page = context.pages()[0] || await context.waitForEvent('page', { timeout: 5000 });
      await page.locator('#home').waitFor({ state: 'visible' });
      return { child, browser, page };
    } catch { await new Promise(resolve => setTimeout(resolve, 200)); }
  }
  child.kill();
  throw new Error('Native WebView did not become available');
}

async function stop(app) {
  if (!app) return;
  try { await app.browser.close(); } catch { /* Process may already be gone. */ }
  if (app.child.exitCode === null && app.child.signalCode === null) {
    const exited = new Promise(resolve => app.child.once('exit', resolve));
    app.child.kill();
    await exited;
  }
}

async function openProject(page, root) {
  await page.locator('nav a[data-route="workspace"]').click();
  await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(root);
  await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
  await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
}

async function chooseFileFromProject(page, name) {
  const helper = spawn('py', ['-3.12', path.join(__dirname, 'folder-dialog.py'), 'file-select', name], { windowsHide: true });
  let output = '';
  helper.stdout.on('data', chunk => { output += chunk; });
  helper.stderr.on('data', chunk => { output += chunk; });
  await page.getByRole('button', { name: 'Escolher arquivo' }).click();
  const [code] = await once(helper, 'exit');
  assert.equal(code, 0, `Native file picker failed: ${output}`);
}

async function resumeFixture(page, citation, interrupted = false) {
  await page.evaluate(({ file, interrupted }) => {
    const native = window.__TAURI__.core;
    window.__TAURI__ = { ...window.__TAURI__, core: { ...native, invoke: (command, args) => {
      if (command === 'connect_agent') return Promise.resolve({ thread_id: 'controlled-result-thread', resumed: true, messages: [
        { id: 'fixture-user', role: 'user', text: 'Crie uma página para mim.' },
        { id: 'fixture-agent', role: 'agent', text: Array.isArray(file) ? `Aqui estão: [Primeiro](${file[0]}) e [Segundo](${file[1]}).` : `Aqui está: [Ver arquivo](${file}).` },
        ...(interrupted ? [
          { id: 'fixture-next-user', role: 'user', text: 'Agora melhore a página.' },
          { id: 'fixture-next-agent', role: 'agent', text: 'Vou começar a melhoria.', incomplete: true },
        ] : []),
      ] });
      if (command === 'disconnect_agent') return Promise.resolve();
      return native.invoke(command, args);
    } } };
  }, { file: citation, interrupted });
  await page.locator('#conversation-picker summary').click();
  await page.getByRole('button', { name: 'Conectar ao Codex', exact: true }).click();
  await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
  if (citation === 'result.txt' && process.env.FORGE_RESULT_SHORTCUT_SCREENSHOT) {
    await page.locator('#messages .message-result-action').scrollIntoViewIfNeeded();
    await page.screenshot({ path: process.env.FORGE_RESULT_SHORTCUT_SCREENSHOT });
  }
  if (Array.isArray(citation)) {
    assert.equal(await page.locator('#messages .message-result-action').count(), 0, 'Two citations must not create a guessed inline choice');
    await page.locator('#messages .message-file-choices summary').click();
    await page.getByRole('button', { name: `Conferir arquivo da resposta: ${citation[1]}` }).click();
  } else await page.getByRole('button', { name: `Conferir arquivo citado na resposta: ${citation}` }).click();
}

(async () => {
  assert.ok(process.env.FORGE_DESKTOP_EXE, 'Set FORGE_DESKTOP_EXE');
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-result-shortcut-'));
  const project = path.join(profile, 'new-project');
  const outside = path.join(profile, 'outside.txt');
  const port = await freePort();
  let app;
  try {
    await mkdir(project);
    app = await launch(process.env.FORGE_DESKTOP_EXE, profile, port);
    await openProject(app.page, project);
    await writeFile(path.join(project, 'result.txt'), 'Real local file from this project.');
    await writeFile(path.join(project, 'second.txt'), 'Second real local file from this project.');
    await writeFile(path.join(project, 'archive.zip'), 'Not a visual preview');
    await writeFile(outside, 'This file is outside the project.');
    await chooseFileFromProject(app.page, 'result.txt');
    await app.page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await app.page.locator('#preview-path').textContent(), 'result.txt',
      'Relative filename must resolve from the opened project folder');
    console.log('PASS: hidden native file picker started in the real project folder and opened a validated local file.');
    // First process establishes the project and a resumable bookmark only.
    await resumeFixture(app.page, 'result.txt');
    await app.page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await app.page.locator('#preview-text').textContent(), 'Real local file from this project.');
    await app.page.evaluate(async file => (await import('./preview.mjs')).previewLinkedFile(file), outside);
    await app.page.locator('#preview-status').filter({ hasText: 'prévia anterior foi mantida' }).waitFor();
    assert.equal(await app.page.locator('#preview-text').textContent(), 'Real local file from this project.');
    assert.equal(await app.page.locator('#preview-path').textContent(), 'result.txt');
    assert.equal(await app.page.locator('.workspace').evaluate(node => node.classList.contains('preview-loaded')), true,
      'Rejected outside file must keep the validated preview layout');
    assert.equal(await app.page.locator('.preview-empty').isVisible(), false);
    console.log('PASS: native path rejection kept the previously validated preview visible.');
    if (process.env.FORGE_RESULT_OPEN_SCREENSHOT) await app.page.screenshot({ path: process.env.FORGE_RESULT_OPEN_SCREENSHOT });
    await stop(app); app = null;

    // The WebView and desktop process are new; the local preview shortcut is
    // revalidated by native code rather than assumed to remain safe.
    app = await launch(process.env.FORGE_DESKTOP_EXE, profile, port);
    await openProject(app.page, project);
    await app.page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await app.page.locator('#preview-path').textContent(), 'result.txt');
    await resumeFixture(app.page, 'result.txt', true);
    await app.page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await app.page.locator('#preview-text').textContent(), 'Real local file from this project.');
    assert.equal(await app.page.locator('#preview-path').textContent(), 'result.txt');
    assert.match(await app.page.locator('#preview-intro').textContent(), /resultado anterior tem um arquivo/);
    const messageCount = await app.page.locator('#messages article').count();
    await app.page.getByRole('button', { name: 'Pedir mudança neste arquivo' }).click();
    assert.match(await app.page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).inputValue(), /result\.txt/);
    assert.equal(await app.page.locator('#messages article').count(), messageCount, 'Preparing a change must not send a turn');
    console.log('PASS: hidden native full-process restart retained the earlier file across an interrupted reply and prepared a change without sending a turn.');

    await app.page.reload();
    await openProject(app.page, project);
    await resumeFixture(app.page, outside.replaceAll('\\', '/'));
    await app.page.locator('#preview-status').filter({ hasText: 'não pertence ao projeto' }).waitFor();
    assert.equal(await app.page.locator('#preview-path').textContent(), 'result.txt', 'The previously restored file stays visible after a rejected citation');
    assert.equal(await app.page.locator('.workspace').evaluate(node => node.classList.contains('preview-loaded')), true);
    console.log('PASS: the same shortcut refused an outside-project path while keeping the restored valid preview.');

    await app.page.reload();
    await openProject(app.page, project);
    await resumeFixture(app.page, ['result.txt', 'second.txt']);
    await app.page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor();
    assert.equal(await app.page.locator('#preview-text').textContent(), 'Second real local file from this project.');
    assert.equal(await app.page.locator('#preview-cited-files').isVisible(), true, 'The other cited file remains available after opening one');
    const draft = app.page.locator('#message-text');
    await draft.fill('Quero conferir este arquivo antes de enviar.');
    await app.page.locator('#conversation-options summary').click();
    await app.page.getByRole('button', { name: 'Ampliar conversa' }).click();
    assert.equal(await app.page.locator('#project-preview').isHidden(), true);
    assert.equal(await app.page.evaluate(() => {
      const conversation = document.getElementById('project-conversation').getBoundingClientRect();
      const workspace = document.querySelector('.workspace').getBoundingClientRect();
      return conversation.width >= workspace.width - 2 && document.documentElement.scrollWidth <= innerWidth;
    }), true, 'Native reading mode should give the chat the full width');
    assert.equal(await app.page.locator('#messages article[data-role="agent"] .message-bubble').last().evaluate(node => {
      const bubble = node.getBoundingClientRect();
      const conversation = document.getElementById('project-conversation').getBoundingClientRect();
      return bubble.width >= conversation.width * 0.5 && bubble.width <= 900;
    }), true, 'A restored native reply should use the reading width without overlong lines');
    if (process.env.FORGE_CONVERSATION_FOCUS_SCREENSHOT) await app.page.screenshot({ path: process.env.FORGE_CONVERSATION_FOCUS_SCREENSHOT, fullPage: true });
    await app.page.evaluate(async file => (await import('./preview.mjs')).previewLinkedFile(file), path.join(project, 'archive.zip'));
    await app.page.locator('#preview-status').filter({ hasText: 'Arquivo encontrado' }).waitFor();
    assert.equal(await app.page.locator('.workspace').evaluate(node => node.classList.contains('conversation-focus')), false);
    assert.equal(await draft.inputValue(), 'Quero conferir este arquivo antes de enviar.', 'Reading and result views must preserve the unsent draft');
    assert.equal(await app.page.locator('#preview-path').textContent(), 'archive.zip');
    assert.equal(await app.page.getByRole('button', { name: 'Copiar caminho do arquivo' }).isVisible(), true,
      'A nonvisual deliverable needs a direct route to its real project file');
    if (await app.page.evaluate(() => innerWidth > 900)) assert.equal(await app.page.evaluate(() => {
      const conversation = document.getElementById('project-conversation').getBoundingClientRect();
      const preview = document.getElementById('project-preview').getBoundingClientRect();
      return conversation.width >= preview.width * 1.15 && document.documentElement.scrollWidth <= innerWidth;
    }), true, 'A nonvisual native file should keep more reading space for the conversation');
    if (process.env.FORGE_RESULT_NONVISUAL_SCREENSHOT) await app.page.screenshot({ path: process.env.FORGE_RESULT_NONVISUAL_SCREENSHOT, fullPage: true });
    const options = app.page.locator('#conversation-options');
    assert.equal(await options.locator('summary').isVisible(), true);
    assert.equal(await app.page.getByRole('button', { name: 'Desconectar', exact: true }).isHidden(), true);
    await options.locator('summary').click();
    assert.equal(await app.page.getByRole('button', { name: 'Desconectar', exact: true }).isVisible(), true);
    await options.locator('summary').click();
    const capability = app.page.locator('#agent-access-note');
    assert.equal(await capability.locator('summary').isVisible(), true);
    assert.equal(await capability.locator('p').isHidden(), true);
    await capability.locator('summary').click();
    assert.match(await capability.locator('p').textContent(), /fora da pasta escolhida sem pedir confirmação/);
    await capability.locator('summary').click();
    if (process.env.FORGE_MULTI_CITATIONS_SCREENSHOT) await app.page.screenshot({ path: process.env.FORGE_MULTI_CITATIONS_SCREENSHOT, fullPage: true });
    console.log('PASS: two restored citations remain distinct and the selected file passes native project-bound preview.');
  } finally {
    await stop(app);
    const resolved = path.resolve(profile);
    if (resolved.startsWith(`${path.resolve(tmpdir())}${path.sep}`)) await rm(resolved, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
