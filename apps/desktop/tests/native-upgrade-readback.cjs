// Read-only cross-version check against a preserved real Codex conversation.
// It never sends a turn, modifies a project file, or creates a new conversation.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { access, readFile } = require('node:fs/promises');
const { createHash } = require('node:crypto');
const path = require('node:path');
const assert = require('node:assert/strict');

const exe = process.env.FORGE_DESKTOP_EXE;
const profile = process.env.FORGE_ARTIFACT_PROFILE;
const project = process.env.FORGE_ARTIFACT_PROJECT;
assert.ok(exe && profile && project, 'Set FORGE_DESKTOP_EXE, FORGE_ARTIFACT_PROFILE and FORGE_ARTIFACT_PROJECT');
const resultFile = path.join(project, 'site', 'index.html');
const digest = bytes => createHash('sha256').update(bytes).digest('hex');

async function freePort() {
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}

async function launch() {
  const port = await freePort();
  const child = spawn(exe, [], { windowsHide: true, stdio: 'ignore', env: {
    ...process.env, WEBVIEW2_USER_DATA_FOLDER: profile,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
  } });
  let browser;
  try {
    const deadline = Date.now() + 30000;
    while (Date.now() < deadline) {
      if (child.exitCode !== null) throw new Error(`Application exited: ${child.exitCode}`);
      try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); break; }
      catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    if (!browser) throw new Error('Native WebView did not become available');
    const page = browser.contexts()[0].pages()[0];
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.evaluate(() => {
      const core = window.__TAURI__.core;
      window.sendCalls = 0;
      window.__TAURI__ = { ...window.__TAURI__, core: { ...core, invoke: (command, args) => {
        if (command === 'send_message') { window.sendCalls++; throw new Error('Readback test must never send'); }
        return core.invoke(command, args);
      } } };
    });
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
    const exit = new Promise(resolve => app.child.once('exit', resolve));
    app.child.kill();
    await exit;
  }
}

async function readback(page) {
  await page.locator('nav a[data-route="workspace"]').click();
  await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(project);
  await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
  await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 60000 });
  await page.locator('#conversation-picker summary').click();
  await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
  await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 110000 });
  const messages = page.locator('#messages article');
  const count = await messages.count();
  assert.ok(count >= 4, `Expected a preserved real conversation, got ${count} messages`);
  assert.ok(await page.locator('#messages article[data-role="user"]').count() > 0);
  assert.ok(await page.locator('#messages article[data-role="agent"]').count() > 0);
  const fileAction = page.locator('#messages article[data-role="agent"]').getByRole('button', { name: /Ver arquivo local:.*site[\\/]index\.html/ }).last();
  await fileAction.click();
  await page.locator('#preview-path').filter({ hasText: /site[\\/]index\.html/ }).waitFor({ timeout: 30000 });
  assert.equal(await page.locator('#preview-result').isVisible(), true);
  assert.equal(await page.locator('#preview-site').isVisible(), true, 'Real HTML should use the protected native preview');
  await page.frameLocator('#preview-site').locator('h1').waitFor({ timeout: 20000 });
  assert.ok((await page.frameLocator('#preview-site').locator('h1').textContent()).trim(),
    'The protected preview must render actual project content, not merely an empty iframe');
  await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor({ timeout: 30000 });
  if ((await page.locator('#record-state').textContent()) === 'Esta parte foi concluída') {
    assert.doesNotMatch(await page.locator('#record-phase-help').textContent(), /Este trabalho foi concluído/,
      'A completed-work status must not be repeated in the phase description');
  }
  assert.equal(await page.evaluate(() => window.sendCalls), 0);
  return count;
}

(async () => {
  await access(profile); await access(project); await access(resultFile);
  const before = digest(await readFile(resultFile));
  let app;
  try {
    app = await launch();
    const count = await readback(app.page);
    if (process.env.FORGE_UPGRADE_SCREENSHOT) await app.page.screenshot({ path: process.env.FORGE_UPGRADE_SCREENSHOT, fullPage: true });
    await stop(app); app = null;
    app = await launch();
    assert.equal(await readback(app.page), count, 'A second app process must not replay a turn');
    assert.equal(digest(await readFile(resultFile)), before, 'Readback must not modify the delivered file');
    console.log(`PASS: native Desktop resumed ${count} real messages and rendered the protected preview across two process starts without Send or artifact changes.`);
  } finally { await stop(app); }
})().catch(error => { console.error(error); process.exitCode = 1; });
