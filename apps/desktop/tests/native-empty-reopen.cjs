// Focused hidden-native proof: an unsent Codex thread is not resumed after restart.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm } = require('node:fs/promises');
const { once } = require('node:events');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

const executable = process.env.FORGE_DESKTOP_EXE;
const projectRoot = process.env.FORGE_TEST_PROJECT;
if (!executable || !projectRoot) throw new Error('Set FORGE_DESKTOP_EXE and FORGE_TEST_PROJECT');

(async () => {
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-native-empty-reopen-'));
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  let child;
  let browser;
  const start = () => spawn(executable, [], { windowsHide: true, stdio: 'ignore', env: {
    ...process.env,
    WEBVIEW2_USER_DATA_FOLDER: profile,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
  } });
  const attach = async () => {
    const deadline = Date.now() + 30000;
    while (Date.now() < deadline) {
      if (child.exitCode !== null) throw new Error(`Forge exited early: ${child.exitCode}`);
      try { return await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); }
      catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    throw new Error('Native WebView unavailable');
  };
  const stop = async () => {
    if (browser) { await browser.close(); browser = null; }
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      const exited = once(child, 'exit');
      child.kill();
      await exited;
    }
  };
  try {
    child = start();
    browser = await attach();
    let page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.getByRole('link', { name: 'Minha conversa' }).click();
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(projectRoot);
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 35000 });
    await page.locator('#conversation-picker summary').click();
    await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Codex conectado' }).waitFor({ timeout: 60000 });
    assert.equal(await page.evaluate(() => {
      const project = JSON.parse(localStorage.getItem('forge.projects.v1'))[0];
      return localStorage.getItem(`forge.conversation.v1:${JSON.stringify([project.project_id, project.project_root])}`);
    }), null, 'An empty native Codex thread must not be bookmarked');
    await stop();

    child = start();
    browser = await attach();
    page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.evaluate(() => {
      window.reopenCalls = { connect: 0, send: 0 };
      const core = window.__TAURI__.core;
      const facade = Object.create(core);
      Object.defineProperty(facade, 'invoke', { value: (command, args) => {
        if (command === 'connect_agent') window.reopenCalls.connect++;
        if (command === 'send_message') window.reopenCalls.send++;
        return core.invoke(command, args);
      } });
      window.__TAURI__.core = facade;
    });
    await page.getByRole('link', { name: 'Meus projetos' }).click();
    await page.locator('.recent-project').getByRole('button', { name: /^Abrir / }).click();
    await page.waitForFunction(() => document.getElementById('project-status').textContent.includes('Projeto pronto'), null, { timeout: 35000 });
    assert.equal(await page.locator('#confirmed-root').textContent(), projectRoot);
    assert.deepEqual(await page.evaluate(() => window.reopenCalls), { connect: 0, send: 0 });
    assert.equal(await page.locator('#messages article').count(), 0);
    console.log('PASS: hidden native app opened an empty real Codex thread, restarted and reopened its project without trying to resume or send it.');
  } finally {
    await stop();
    assert.equal(path.dirname(path.resolve(profile)), path.resolve(tmpdir()));
    assert.ok(path.basename(profile).startsWith('forge-native-empty-reopen-'));
    await rm(profile, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
