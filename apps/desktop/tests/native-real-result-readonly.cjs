// Read-only native-app journey: a real Codex result, change draft and
// conversation survive a full native process restart without another Send.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm, readFile } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const { createHash } = require('node:crypto');
const path = require('node:path');
const assert = require('node:assert/strict');
const { clickConversationAction } = require('./conversation-options.cjs');

const exe = process.env.FORGE_DESKTOP_EXE;
const project = process.env.FORGE_TEST_PROJECT;
const threadId = process.env.FORGE_TEST_RESUME_THREAD_ID;
assert.ok(exe && project && threadId, 'Set native app executable, existing project and existing Codex thread');
const site = path.join(project, 'site', 'index.html');
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

async function freePort() {
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}

async function launch(profile) {
  const port = await freePort();
  const child = spawn(exe, [], { windowsHide: true, stdio: 'ignore', env: {
    ...process.env,
    WEBVIEW2_USER_DATA_FOLDER: profile,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
  } });
  let browser;
  try {
    const deadline = Date.now() + 30000;
    while (Date.now() < deadline) {
      if (child.exitCode !== null) throw new Error(`Forge exited early: ${child.exitCode}`);
      try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); break; }
      catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    if (!browser) throw new Error('Native WebView unavailable');
    const page = browser.contexts()[0].pages()[0];
    await page.locator('#home').waitFor({ state: 'visible' });
    return { child, browser, page };
  } catch (error) {
    if (browser) await browser.close().catch(() => {});
    child.kill();
    throw error;
  }
}

async function stop(app) {
  if (!app) return;
  await app.browser.close().catch(() => {});
  if (app.child.exitCode === null && app.child.signalCode === null) {
    const exited = new Promise(resolve => app.child.once('exit', resolve));
    app.child.kill();
    await exited;
  }
}

async function openProject(page) {
  await page.locator('nav a[data-route="workspace"]').click();
  await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(project);
  await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
  await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
}

async function resume(page) {
  await page.locator('#conversation-picker summary').click();
  await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
  await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 110000 });
  const messages = page.locator('#messages article');
  assert.ok(await messages.count() >= 30, 'Expected the existing substantial real conversation');
  const history = page.getByRole('region', { name: 'Histórico da conversa' });
  assert.ok(await history.evaluate(node => node.scrollHeight > node.clientHeight), 'Real history must overflow its reading region');
  await history.evaluate(node => { node.scrollTop = 0; });
  const jump = page.getByRole('button', { name: 'Ir para a mensagem mais recente' });
  await jump.waitFor({ state: 'visible' });
  if (process.env.FORGE_REAL_HISTORY_JUMP_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_REAL_HISTORY_JUMP_SCREENSHOT });
  const countBeforeJump = await messages.count();
  await jump.click();
  assert.equal(await history.evaluate(node => node.scrollHeight - node.clientHeight - node.scrollTop < 2), true);
  assert.equal(await jump.isHidden(), true);
  assert.equal(await messages.count(), countBeforeJump, 'Navigation within history must not send or duplicate a message');
  return {
    count: await messages.count(),
    first: await messages.first().textContent(),
    last: await messages.last().textContent(),
  };
}

async function inspectResult(page) {
  const messagesBefore = await page.locator('#messages article').count();
  const result = page.locator('#messages article[data-role="agent"] .message-file-link[data-preview-path="site/index.html"]').last();
  assert.equal(await result.count(), 1, 'Real reply must expose its local HTML file');
  const reply = result.locator('xpath=ancestor::div[contains(concat(" ", normalize-space(@class), " "), " message-bubble ")]');
  const choices = reply.locator('.message-file-choices');
  if (await choices.count()) {
    await choices.locator('summary').click();
    await choices.getByRole('button', { name: 'Conferir arquivo da resposta: site/index.html' }).click();
  } else {
    await result.click();
  }
  await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
  assert.equal(await page.locator('#preview-path').textContent(), 'site\\index.html');
  await page.frameLocator('#preview-site').getByRole('heading', { name: 'Jardim de ideias renovado' }).waitFor();
  const changeAction = page.getByRole('button', { name: 'Pedir mudança neste arquivo' });
  const changeBounds = await changeAction.boundingBox();
  assert.ok(changeBounds && changeBounds.y >= 0 && changeBounds.y < await page.evaluate(() => innerHeight), 'Change action should be visible with the result before scrolling');
  assert.equal(await page.evaluate(() => !!(document.getElementById('request-preview-change').compareDocumentPosition(document.getElementById('preview-site-note')) & Node.DOCUMENT_POSITION_FOLLOWING)), true, 'Change action should precede optional safety details');
  if (process.env.FORGE_REAL_RESULT_PREVIEW_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_REAL_RESULT_PREVIEW_SCREENSHOT });
  await changeAction.click();
  const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
  assert.match(await composer.inputValue(), /site\\index\.html/);
  assert.equal(await composer.evaluate(node => document.activeElement === node), true, 'Change request should focus the composer');
  const composerBounds = await composer.boundingBox();
  assert.ok(composerBounds.y >= 0 && composerBounds.y < await page.evaluate(() => innerHeight), 'Change request should expose the composer');
  if (process.env.FORGE_REAL_RESULT_CHANGE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_REAL_RESULT_CHANGE_SCREENSHOT });
  assert.equal(await page.locator('#messages article').count(), messagesBefore, 'Preparing a change must not send a message');
}

(async () => {
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-real-result-readonly-'));
  const initialHash = sha256(await readFile(site));
  let app;
  try {
    app = await launch(profile);
    await openProject(app.page);
    await app.page.evaluate(async id => {
      const project = await window.__TAURI__.core.invoke('inspect_project', { projectRoot: document.getElementById('project-root').value });
      localStorage.setItem(`forge.conversation.v1:${JSON.stringify([project.project_id, project.project_root])}`, id);
    }, threadId);
    const before = await resume(app.page);
    await inspectResult(app.page);
    await clickConversationAction(app.page, 'Desconectar');
    await app.page.locator('#agent-status').filter({ hasText: 'Desconectado' }).waitFor();
    await stop(app); app = null;

    app = await launch(profile);
    await openProject(app.page);
    await app.page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
    assert.equal(await app.page.locator('#preview-path').textContent(), 'site\\index.html', 'Previously selected real result should reopen without another click');
    await app.page.frameLocator('#preview-site').getByRole('heading', { name: 'Jardim de ideias renovado' }).waitFor();
    const after = await resume(app.page);
    assert.deepEqual(after, before, 'Full process restart must preserve message count and order');
    await inspectResult(app.page);
    assert.equal(sha256(await readFile(site)), initialHash, 'Read-only journey must not edit the generated page');
    console.log(`PASS: native app reopened ${after.count} real messages, previewed the changed HTML and prepared a new request across a full process restart; no Send or file edit.`);
  } finally {
    await stop(app);
    const resolved = path.resolve(profile);
    if (!resolved.startsWith(`${path.resolve(tmpdir())}${path.sep}`)) throw new Error('Refusing cleanup outside the temporary profile');
    await rm(resolved, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
