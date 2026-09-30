// Windows-only: real Tauri/WebView process restart with a controlled Codex bridge.
// Never forwards connect_agent or send_message to the real Codex CLI.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm } = require('node:fs/promises');
const { once } = require('node:events');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

if (!process.env.FORGE_DESKTOP_EXE || !process.env.FORGE_TEST_PROJECT) {
  throw new Error('Set FORGE_DESKTOP_EXE and a real linked FORGE_TEST_PROJECT');
}

(async () => {
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-desktop-guard-'));
  let child;
  let browser;
  const launch = () => spawn(process.env.FORGE_DESKTOP_EXE, [], {
    windowsHide: true,
    stdio: 'ignore',
    env: {
      ...process.env,
      WEBVIEW2_USER_DATA_FOLDER: profile,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
    },
  });
  const attach = async () => {
    const deadline = Date.now() + 20000;
    while (Date.now() < deadline) {
      if (child.exitCode !== null) throw new Error(`Application exited: ${child.exitCode}`);
      try { return await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); }
      catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    throw new Error('Native WebView did not become available');
  };
  const stop = async () => {
    if (browser) { await browser.close(); browser = null; }
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      const exited = once(child, 'exit');
      child.kill();
      await exited;
    }
  };
  const openProject = async page => {
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.locator('nav a[data-route="workspace"]').click();
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(process.env.FORGE_TEST_PROJECT);
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 35000 });
  };
  const interceptCodex = async (page, pending) => page.evaluate(pending => {
    window.fakeCodexSends = 0;
    window.fakeCodexConnects = 0;
    const nativeCore = window.__TAURI__.core;
    const invoke = nativeCore.invoke;
    const controlledInvoke = (command, args) => {
      if (command === 'connect_agent') { window.fakeCodexConnects++; window.fakeCodexEvents = args.events; return Promise.resolve({
        thread_id: 'controlled-native-thread',
        resumed: !!args.threadId,
        messages: args.threadId ? [{ id: 'earlier', role: 'user', text: 'Controlled history for review' }] : [],
      }); }
      if (command === 'send_message') {
        window.fakeCodexSends++;
        return pending ? new Promise(() => {}) : Promise.resolve();
      }
      return invoke(command, args);
    };
    // Tauri freezes core.invoke itself; replace only the writable core facade.
    const facade = Object.create(nativeCore);
    Object.defineProperty(facade, 'invoke', { value: controlledInvoke });
    window.__TAURI__.core = facade;
  }, pending);
  try {
    child = launch();
    browser = await attach();
    let page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
    await openProject(page);
    await interceptCodex(page, true);
    assert.equal(await page.evaluate(() => String(window.__TAURI__.core.invoke).includes('fakeCodexConnects')), true, 'Controlled bridge must intercept native commands');
    await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).fill('Controlled pending native send');
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    const markerKey = await page.evaluate(() => {
      const project = JSON.parse(localStorage.getItem('forge.projects.v1'))[0];
      return `forge.send-unconfirmed.v1:${JSON.stringify([project.project_id, project.project_root])}`;
    });
    await page.waitForFunction(key => localStorage.getItem(key) === 'controlled-native-thread' && window.fakeCodexSends === 1, markerKey);
    assert.equal(await page.locator('#messages article[data-delivery="pending"]').count(), 1);
    await stop(); // Hard process exit before the mocked native Send can acknowledge.

    child = launch();
    browser = await attach();
    page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
    await openProject(page);
    assert.equal(await page.evaluate(key => localStorage.getItem(key), markerKey), 'controlled-native-thread');
    assert.equal(await page.getByRole('button', { name: 'Conferir envio anterior' }).isVisible(), true);
    await interceptCodex(page, false);
    const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    await composer.fill('Do not replay the pending send');
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    assert.equal(await page.evaluate(() => window.fakeCodexSends), 0);
    assert.match(await page.locator('#agent-status').textContent(), /envio anterior não foi confirmado/i);
    await page.getByLabel('Começar outra conversa').check();
    await page.getByRole('button', { name: 'Conectar ao Codex', exact: true }).click();
    assert.match(await page.locator('#agent-status').textContent(), /Retome a conversa anterior/);
    await page.getByLabel('Começar outra conversa').uncheck();
    await page.getByRole('button', { name: 'Conferir envio anterior' }).click();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await page.evaluate(key => localStorage.getItem(key), markerKey), 'controlled-native-thread');
    assert.match(await page.locator('#messages').textContent(), /Controlled history for review/);
    assert.equal(await page.evaluate(() => window.fakeCodexSends), 0);
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    if (process.env.FORGE_REVIEW_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_REVIEW_SCREENSHOT });
    await page.getByRole('button', { name: 'Já conferi o envio' }).click();
    assert.equal(await page.evaluate(key => localStorage.getItem(key), markerKey), null);
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isEnabled(), true);
    await page.evaluate(() => window.fakeCodexEvents.onmessage({ kind: 'disconnected' }));
    const draft = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    assert.equal(await draft.isEnabled(), true, 'Native UI permits local drafting after disconnection');
    await draft.fill('Native draft while disconnected');
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true,
      'Native UI blocks Send after disconnection');
    await page.getByRole('button', { name: 'Reabrir conversa' }).click();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor();
    assert.equal(await draft.inputValue(), 'Native draft while disconnected');
    assert.equal(await page.evaluate(() => window.fakeCodexSends), 0, 'Reopen must not replay or send the offline draft');
    await page.evaluate(() => window.fakeCodexEvents.onmessage({ kind: 'running' }));
    assert.equal(await draft.isEnabled(), true, 'Native UI permits drafting during an active turn');
    await draft.fill('Native next message while Codex works');
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    await page.evaluate(() => window.fakeCodexEvents.onmessage({ kind: 'completed' }));
    assert.equal(await draft.inputValue(), 'Native next message while Codex works');
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isEnabled(), true);
    assert.equal(await page.evaluate(() => window.fakeCodexSends), 0);
    console.log('PASS: hidden native restart guarded uncertain Send; drafts persisted across disconnection, reconnection and an active turn without replay.');
    console.log('NOT_RUN: real Codex delivery, response and native ambiguous transport outcome (controlled bridge only).');
  } finally {
    await stop();
    if (path.dirname(path.resolve(profile)) !== path.resolve(tmpdir()) || !path.basename(profile).startsWith('forge-desktop-guard-')) {
      throw new Error('Refusing cleanup outside the test profile');
    }
    await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
