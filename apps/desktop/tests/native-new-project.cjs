// Real default folder creation and Forge initialization;
// isolated desktop and profile, with no Codex message sent.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, stat, rm } = require('node:fs/promises');
const { once } = require('node:events');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

const executable = process.env.FORGE_DESKTOP_EXE;
if (!executable || !process.env.FORGE_CORE_EXE) throw new Error('Set FORGE_DESKTOP_EXE and FORGE_CORE_EXE');

(async () => {
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-native-new-profile-'));
  const installedParent = process.env.FORGE_TEST_INSTALLED_PROJECTS_DIR;
  const parent = installedParent || await mkdtemp(path.join(tmpdir(), 'forge-native-new-parent-'));
  const marker = `teste forge ${Date.now()}`;
  const project = installedParent ? path.join(parent, marker) : path.join(parent, 'criar algo artístico');
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  let child;
  let browser;
  try {
    child = spawn(executable, [], { windowsHide: true, stdio: 'ignore', env: {
      ...process.env, WEBVIEW2_USER_DATA_FOLDER: profile,
      ...(installedParent ? {} : { FORGE_DESKTOP_PROJECTS_DIR: parent }),
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
    await page.getByRole('link', { name: 'Explorar', exact: true }).click();
    await page.getByRole('link', { name: /Arte e criação/ }).click();
    if (installedParent) await page.locator('#message-text').fill(`Quero ${marker}.`);
    const draft = await page.locator('#message-text').inputValue();
    assert.match(draft, installedParent ? /teste forge/ : /artístico/);
    await page.locator('#create-default-project').click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
    assert.equal((await stat(project)).isDirectory(), true);
    assert.equal(await page.locator('#project-root').inputValue(), project);
    assert.equal(await page.locator('#confirmed-root').textContent(), project);
    assert.equal(await page.locator('#message-text').inputValue(), draft);
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), 0);
    console.log('PASS: hidden native one-click project created a real folder, initialized Forge, kept the idea draft and sent no Codex message.');
  } finally {
    await browser?.close().catch(() => {});
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      const exited = once(child, 'exit');
      child.kill();
      await exited;
    }
    assert.equal(path.dirname(path.resolve(profile)), path.resolve(tmpdir()));
    assert.ok(path.basename(profile).startsWith('forge-native-new-profile-'));
    await rm(profile, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
    if (installedParent) {
      assert.equal(path.dirname(path.resolve(project)), path.resolve(installedParent));
      assert.ok(path.basename(project).startsWith('teste forge '));
      await rm(project, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
    } else {
      assert.equal(path.dirname(path.resolve(parent)), path.resolve(tmpdir()));
      assert.ok(path.basename(parent).startsWith('forge-native-new-parent-'));
      await rm(parent, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
    }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
