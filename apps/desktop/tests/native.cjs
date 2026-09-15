// Windows-only development smoke test against a real Tauri WebView.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm, access } = require('node:fs/promises');
const { once } = require('node:events');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

(async () => {
  if (!process.env.FORGE_DESKTOP_EXE) throw new Error('Set FORGE_DESKTOP_EXE to the built development executable');
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-desktop-webview-'));
  const child = spawn(process.env.FORGE_DESKTOP_EXE, [], {
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
    const page = context.pages()[0] || await context.waitForEvent('page', { timeout: 5000 });
    await page.getByRole('status').filter({ hasText: 'Aplicativo iniciado' }).waitFor({ timeout: 5000 });
    await page.getByRole('button', { name: 'Verificar novamente' }).click();
    await page.getByRole('status').filter({ hasText: 'Aplicativo iniciado' }).waitFor({ timeout: 5000 });
    await page.getByText('Nenhum agente conectado.', { exact: false }).waitFor();
    if (process.env.FORGE_TEST_PROJECT) {
      const field = page.getByRole('textbox', { name: 'Pasta do projeto' });
      const submit = page.getByRole('button', { name: 'Conferir projeto' });
      await field.fill(process.env.FORGE_TEST_PROJECT);
      await submit.click();
      await page.locator('#project-status').filter({ hasText: 'Projeto encontrado' }).waitFor({ timeout: 15000 });
      assert.equal(await page.locator('#confirmed-root').textContent(), process.env.FORGE_TEST_PROJECT);
      assert.ok((await page.locator('#project-name').textContent()).length > 0);
      // A failed lookup must hide the preceding project's identity.
      await field.fill(path.join(profile, 'missing-folder'));
      assert.equal(await page.locator('#project-result').isVisible(), false);
      assert.equal(await page.locator('#project-status').textContent(), '');
      await submit.click();
      await page.locator('#project-status').filter({ hasText: 'pasta que existe' }).waitFor();
      assert.equal(await page.locator('#project-result').isVisible(), false);
      // Existing but unlinked folder: do not initialize or repair it silently.
      await field.fill(profile);
      await submit.click();
      await page.locator('#project-status').filter({ hasText: 'Não foi possível identificar' }).waitFor({ timeout: 15000 });
      assert.equal(await page.locator('#project-result').isVisible(), false);
      await assert.rejects(access(path.join(profile, '.forge-method.yaml')), { code: 'ENOENT' });
      await assert.rejects(access(path.join(profile, '.forge-method')), { code: 'ENOENT' });
      console.log('PASS: real Forge project resolution, invalid folder, unlinked folder, stale identity hidden.');
    } else {
      console.log('NOT_RUN: real project resolution (FORGE_TEST_PROJECT not set).');
    }
    if (process.env.FORGE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_SCREENSHOT, fullPage: true });
    console.log('PASS: real native window, frontend-to-Rust identity and retry; agent connection remains absent.');
  } finally {
    try {
      if (browser) await browser.close();
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
