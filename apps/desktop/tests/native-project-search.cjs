// Read-only native project-shortcut search. Only the disposable WebView profile changes.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

const executable = process.env.FORGE_DESKTOP_EXE;
const projectRoot = process.env.FORGE_ARTIFACT_PROJECT;
if (!executable || !projectRoot) throw new Error('Set FORGE_DESKTOP_EXE and existing FORGE_ARTIFACT_PROJECT');

(async () => {
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-native-project-search-'));
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  const child = spawn(executable, [], { windowsHide: true, stdio: 'ignore', env: {
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
    const page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
    await page.locator('#home').waitFor({ state: 'visible' });
    const project = await page.evaluate(root => window.__TAURI__.core.invoke('inspect_project', { projectRoot: root }), projectRoot);
    await page.evaluate(project => {
      const shortcuts = [project, ...Array.from({ length: 55 }, (_, index) => ({ project_id: `other-${index}`, project_root: `D:\\other\\Project-${index}` }))];
      localStorage.setItem('forge.projects.v1', JSON.stringify(shortcuts));
    }, project);
    await page.reload();
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.evaluate(() => {
      window.nativeSendCount = 0;
      const core = window.__TAURI__.core;
      const facade = Object.create(core);
      Object.defineProperty(facade, 'invoke', { value: (command, args) => {
        if (command === 'send_message') window.nativeSendCount++;
        return core.invoke(command, args);
      } });
      window.__TAURI__.core = facade;
    });
    await page.getByRole('link', { name: 'Meus projetos' }).click();
    assert.equal(await page.locator('.recent-project').count(), 50);
    const search = page.getByRole('searchbox', { name: 'Encontrar um projeto' });
    await search.fill('none-match');
    assert.equal(await page.locator('#projects-no-results').isVisible(), true);
    await search.fill(project.project_root);
    assert.equal(await page.locator('.recent-project').count(), 1);
    assert.equal(await page.locator('.recent-project p').textContent(), project.project_root);
    await search.fill(path.basename(project.project_root));
    assert.equal(await page.locator('.recent-project').count(), 1);
    if (process.env.FORGE_PROJECT_SEARCH_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_PROJECT_SEARCH_SCREENSHOT, fullPage: true });
    await page.locator('.recent-project').getByRole('button', { name: /^Abrir / }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 60000 });
    assert.equal(await page.locator('#confirmed-root').textContent(), project.project_root);
    assert.equal(await page.evaluate(() => window.nativeSendCount), 0);
    console.log('PASS: hidden native UI searches 50 local shortcuts and revalidates the exact Forge project without sending a message.');
  } finally {
    if (browser) await browser.close().catch(() => {});
    if (child.exitCode === null && child.signalCode === null) {
      const exit = new Promise(resolve => child.once('exit', resolve));
      child.kill();
      await exit;
    }
    assert.equal(path.dirname(path.resolve(profile)), path.resolve(tmpdir()));
    assert.ok(path.basename(profile).startsWith('forge-native-project-search-'));
    await rm(profile, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
