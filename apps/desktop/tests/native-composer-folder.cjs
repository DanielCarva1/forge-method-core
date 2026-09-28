// Focused hidden-native check: an Explore draft prepares a real folder without sending to Codex.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm } = require('node:fs/promises');
const { once } = require('node:events');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

const executable = process.env.FORGE_DESKTOP_EXE;
if (!executable) throw new Error('Set FORGE_DESKTOP_EXE');

(async () => {
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-native-composer-profile-'));
  const project = await mkdtemp(path.join(tmpdir(), 'forge-native-composer-project-'));
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  let child;
  let browser;
  try {
    child = spawn(executable, [], { windowsHide: true, stdio: 'ignore', env: {
      ...process.env,
      WEBVIEW2_USER_DATA_FOLDER: profile,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
    } });
    const deadline = Date.now() + 30000;
    while (Date.now() < deadline) {
      if (child.exitCode !== null) throw new Error(`Forge exited early: ${child.exitCode}`);
      try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); break; }
      catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    if (!browser) throw new Error('Native WebView unavailable');
    const page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.evaluate(() => {
      window.nativeCalls = { start: 0, inspect: 0, send: 0 };
      const core = window.__TAURI__.core;
      const facade = Object.create(core);
      Object.defineProperty(facade, 'invoke', { value: (command, args) => {
        if (command === 'start_project') window.nativeCalls.start++;
        if (command === 'inspect_project') window.nativeCalls.inspect++;
        if (command === 'send_message') window.nativeCalls.send++;
        return core.invoke(command, args);
      } });
      window.__TAURI__.core = facade;
    });
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.getByRole('link', { name: /Arte e criação/ }).click();
    const draft = page.locator('#message-text');
    const idea = await draft.inputValue();
    assert.match(idea, /artístico/);
    await page.locator('#project-root').fill(project);
    await page.getByRole('button', { name: 'Preparar projeto' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
    assert.equal(await page.locator('#confirmed-root').textContent(), project);
    assert.equal(await draft.inputValue(), idea);
    assert.deepEqual(await page.evaluate(() => window.nativeCalls), { start: 1, inspect: 0, send: 0 });
    assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isEnabled(), true);
    await page.evaluate(() => { location.hash = '#home'; });
    await page.locator('#home-project-action').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#home-project-action').textContent(), 'Continuar este projeto');
    assert.match(await page.locator('#home-project-copy').textContent(), new RegExp(path.basename(project)));
    assert.equal(await page.locator('#home-hero-project-action').textContent(), 'Continuar último projeto');
    await page.locator('#home-hero-project-action').click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 30000 });
    assert.deepEqual(await page.evaluate(() => window.nativeCalls), { start: 1, inspect: 1, send: 0 });
    assert.equal(await page.locator('#confirmed-root').textContent(), project);
    await page.evaluate(() => { location.hash = '#updates'; });
    await page.locator('#updates h3').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#about').evaluate(node => node.open), true);
    assert.match(await page.locator('#app-version').textContent(), /Versão instalada:/);
    console.log('PASS: hidden native Forge prepared and reopened one real project from Home with native inspection and no Codex Send; update help is reachable.');
  } finally {
    await browser?.close().catch(() => {});
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      const exited = once(child, 'exit');
      child.kill();
      await exited;
    }
    const forgeState = path.join(path.dirname(project), `forge-${path.basename(project)}`);
    for (const [target, prefix] of [[profile, 'forge-native-composer-profile-'], [project, 'forge-native-composer-project-'], [forgeState, 'forge-forge-native-composer-project-']]) {
      assert.equal(path.dirname(path.resolve(target)), path.resolve(tmpdir()));
      assert.ok(path.basename(target).startsWith(prefix));
      await rm(target, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
    }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
