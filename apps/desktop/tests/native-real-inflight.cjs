// Windows-only: the real Codex accepts a harmless turn, but the UI loses its
// acknowledgement before a hard app restart. Never resends the original text.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, mkdir, rm } = require('node:fs/promises');
const { once } = require('node:events');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

if (!process.env.FORGE_DESKTOP_EXE) throw new Error('Set FORGE_DESKTOP_EXE');

(async () => {
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-real-inflight-'));
  const project = path.join(profile, 'project');
  await mkdir(project);
  let child;
  let browser;
  const launch = () => spawn(process.env.FORGE_DESKTOP_EXE, [], {
    windowsHide: true,
    stdio: 'ignore',
    env: { ...process.env, WEBVIEW2_USER_DATA_FOLDER: profile,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1` },
  });
  const attach = async () => {
    const deadline = Date.now() + 25000;
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
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(project);
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
  };
  try {
    child = launch();
    browser = await attach();
    let page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
    await openProject(page);
    await page.evaluate(() => {
      window.realSends = 0;
      window.nativeAck = false;
      const core = window.__TAURI__.core;
      const invoke = core.invoke;
      const facade = Object.create(core);
      Object.defineProperty(facade, 'invoke', { value: (command, args) => {
        if (command !== 'send_message') return invoke(command, args);
        window.realSends++;
        return invoke(command, args).then(() => {
          window.nativeAck = true;
          return new Promise(() => {}); // Only the UI acknowledgement is lost.
        });
      } });
      window.__TAURI__.core = facade;
    });
    const prompt = 'Sem usar ferramentas nem alterar arquivos, responda em português apenas: Recuperação segura confirmada.';
    await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).fill(prompt);
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    const markerKey = await page.evaluate(() => {
      const entry = JSON.parse(localStorage.getItem('forge.projects.v1'))[0];
      return `forge.send-unconfirmed.v1:${JSON.stringify([entry.project_id, entry.project_root])}`;
    });
    await page.waitForFunction(key => window.realSends === 1 && window.nativeAck && !!localStorage.getItem(key), markerKey, { timeout: 180000 });
    const thread = await page.evaluate(key => localStorage.getItem(key), markerKey);
    assert.equal(await page.locator('#messages article[data-delivery="pending"]').count(), 1);
    await stop();

    child = launch();
    browser = await attach();
    page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
    await openProject(page);
    assert.equal(await page.evaluate(key => localStorage.getItem(key), markerKey), thread);
    assert.equal(await page.getByRole('button', { name: 'Conferir envio anterior' }).isVisible(), true);
    await page.evaluate(() => {
      window.recoverySends = 0;
      const core = window.__TAURI__.core;
      const invoke = core.invoke;
      const facade = Object.create(core);
      Object.defineProperty(facade, 'invoke', { value: (command, args) => {
        if (command === 'send_message') window.recoverySends++;
        return invoke(command, args);
      } });
      window.__TAURI__.core = facade;
    });
    let resumed = false;
    const deadline = Date.now() + 120000;
    while (Date.now() < deadline) {
      await page.getByRole('button', { name: 'Conferir envio anterior' }).click();
      await page.waitForFunction(() => !document.querySelector('#agent-status')?.textContent?.includes('Conectando ao Codex'), null, { timeout: 110000 });
      const status = await page.locator('#agent-status').textContent();
      if (status.includes('Conversa retomada')) { resumed = true; break; }
      if (!status.includes('ainda está em execução')) throw new Error(`Could not resume real Codex thread: ${status}`);
      await new Promise(resolve => setTimeout(resolve, 2500));
    }
    assert.equal(resumed, true, 'Real Codex thread should become resumable');
    assert.equal(await page.evaluate(key => localStorage.getItem(key), markerKey), thread, 'Resume alone must not clear the uncertain send');
    assert.equal(await page.evaluate(() => window.recoverySends), 0);
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isDisabled(), true);
    assert.equal(await page.getByRole('button', { name: 'Já conferi o envio' }).isVisible(), true);
    const historyContainsPrompt = (await page.locator('#messages').textContent()).includes(prompt);
    console.log(`PASS: real Codex accepted a harmless turn before lost UI acknowledgement; process restart resumed its thread without replay, kept Send blocked until explicit review, and reported the accepted prompt in history: ${historyContainsPrompt}.`);
  } finally {
    await stop();
    if (path.dirname(path.resolve(profile)) !== path.resolve(tmpdir()) || !path.basename(profile).startsWith('forge-real-inflight-')) {
      throw new Error('Refusing cleanup outside the test profile');
    }
    await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
