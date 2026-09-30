// Capture a chosen native build's first-use screens without sending or changing a project.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm, mkdir, writeFile } = require('node:fs/promises');
const { once } = require('node:events');
const path = require('node:path');
const assert = require('node:assert/strict');
const { checkWorkspaceNavigation } = require('./workspace-navigation.cjs');

const executable = process.env.FORGE_DESKTOP_EXE;
const output = process.env.FORGE_VISUAL_OUTPUT;
assert.ok(executable && output, 'Set FORGE_DESKTOP_EXE and FORGE_VISUAL_OUTPUT');

(async () => {
  const profile = await mkdtemp(path.join(process.env.TEMP, 'forge-native-visual-'));
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  let child;
  let browser;
  let fixture;
  try {
    await mkdir(output, { recursive: true });
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
    for (const [name, route] of [['home', 'home'], ['explore', 'explore'], ['projects', 'projects'], ['workspace', 'workspace']]) {
      await page.locator(`nav a[data-route="${route}"]`).click();
      await page.locator(`#${route}`).waitFor({ state: 'visible' });
      assert.equal(await page.evaluate(() => scrollY), 0, `${route} should open at the top with navigation visible`);
      await page.screenshot({ path: path.join(output, `${name}.png`), fullPage: false });
    }
    if (process.env.FORGE_VISUAL_LOADED === '1') {
      const fixtureRoot = process.env.FORGE_TEST_WORKSPACE_ROOT || 'C:\\ForgeFast';
      fixture = await mkdtemp(path.join(fixtureRoot, 'forge-native-visual-project-'));
      await mkdir(path.join(fixture, 'site'));
      await writeFile(path.join(fixture, 'site', 'index.html'), '<!doctype html><html lang="pt-BR"><meta charset="utf-8"><title>Jardim de ideias</title><h1>Jardim de ideias</h1><p>Uma página de teste local para conferir a prévia.</p></html>');
      if (process.env.FORGE_VISUAL_REFERENCE === '1') {
        const composer = page.locator('#message-text');
        await composer.fill('Quero criar algo inspirado nesta referência.');
        for (const mode of ['reference-select', 'reference-cancel']) {
          const before = await composer.inputValue();
          const helper = spawn('py', ['-3.12', path.join(__dirname, 'folder-dialog.py'), mode, path.join(fixture, 'site', 'index.html')], { windowsHide: true });
          let output = '';
          helper.stdout.on('data', chunk => { output += chunk; });
          helper.stderr.on('data', chunk => { output += chunk; });
          const exited = once(helper, 'exit');
          await page.locator('#add-reference').click();
          const [code] = await exited;
          assert.equal(code, 0, `Reference dialog failed: ${output}`);
          await page.waitForFunction(() => !document.querySelector('#add-reference').disabled);
          if (mode === 'reference-select') assert.ok((await composer.inputValue()).includes(JSON.stringify(path.join(fixture, 'site', 'index.html'))));
          else assert.equal(await composer.inputValue(), before);
        }
        assert.equal(await page.locator('#messages article').count(), 0, 'Selecting a reference is not a Send');
        console.log('PASS: actual Windows reference selection and cancellation preserve the visible draft without Send.');
      }
      await page.locator('#custom-folder-option summary').click();
      await page.locator('#project-root').fill(fixture);
      await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
      await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
      await page.evaluate(() => import('./preview.mjs').then(module => module.previewLinkedFile('site/index.html')));
      await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
      await page.frameLocator('#preview-site').getByRole('heading', { name: 'Jardim de ideias' }).waitFor({ timeout: 20000 });
      if (process.env.FORGE_VISUAL_REFERENCE === '1') {
        const fit = await page.evaluate(() => ({
          form: document.querySelector('#message-form').getBoundingClientRect().bottom,
          panel: document.querySelector('#project-conversation').getBoundingClientRect().bottom,
        }));
        assert.ok(fit.form <= fit.panel, 'Native long draft and file actions remain inside their panel');
      }
      await page.screenshot({ path: path.join(output, 'loaded-project.png'), fullPage: false });
      if (process.env.FORGE_VISUAL_NAVIGATION === '1') {
        await checkWorkspaceNavigation(page);
        await page.locator('[data-mobile-pane-button="progress"]').click();
        await page.screenshot({ path: path.join(output, 'project-navigation.png'), fullPage: false });
        console.log('PASS: native workspace navigation, keyboard focus, narrow layout and larger text without Send.');
      }
    }
    console.log(`PASS: captured native-app screens${fixture ? ' including a disposable loaded project' : ' without project mutation'} and no Send: ${output}`);
  } finally {
    await browser?.close().catch(() => {});
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      const exited = once(child, 'exit'); child.kill(); await exited;
    }
    await rm(profile, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
    if (fixture) {
      const root = process.env.FORGE_TEST_WORKSPACE_ROOT || 'C:\\ForgeFast';
      assert.equal(path.dirname(path.resolve(fixture)), path.resolve(root));
      assert.ok(path.basename(fixture).startsWith('forge-native-visual-project-'));
      await rm(fixture, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
    }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
