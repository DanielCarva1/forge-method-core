// One real, disposable first-project task through the installed Windows app.
// Send only once. Preserve the fixture on failure for diagnosis; never retry.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, mkdir, readFile, rm } = require('node:fs/promises');
const { once } = require('node:events');
const path = require('node:path');
const assert = require('node:assert/strict');

const executable = process.env.FORGE_DESKTOP_EXE;
if (!executable) throw new Error('Set FORGE_DESKTOP_EXE to the installed app');
const fixtureRoot = process.env.FORGE_TEST_WORKSPACE_ROOT || 'C:\\ForgeFast';

(async () => {
  const parent = await mkdtemp(path.join(fixtureRoot, 'forge-native-first-result-'));
  const projectRoot = path.join(parent, 'project');
  await mkdir(projectRoot);
  const profile = await mkdtemp(path.join(fixtureRoot, 'forge-native-first-profile-'));
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  let child;
  let browser;
  let project;
  let passed = false;
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
    await page.getByRole('link', { name: 'Começar com minha ideia' }).click();
    const prompt = 'Quero uma página simples para uma pessoa guardar uma primeira ideia. Crie site/index.html nesta pasta com um título visível "Minha primeira ideia" e um pequeno formulário estático. Não use rede, bibliotecas externas nem publique. Confira o arquivo criado e me dê um link relativo para abri-lo no Forge.';
    await page.locator('#message-text').fill(prompt);
    await page.locator('#custom-folder-option summary').click();
    await page.locator('#project-root').fill(projectRoot);
    assert.equal(await page.locator('#send-label').textContent(), 'Enviar e abrir projeto');
    assert.equal(await page.locator('#message-text').inputValue(), prompt);
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), 0);
    const started = Date.now();
    await page.evaluate(() => {
      window.forgeObservedStatuses = [];
      const node = document.getElementById('agent-status');
      new MutationObserver(() => window.forgeObservedStatuses.push(node.textContent)).observe(node, { childList: true, subtree: true, characterData: true });
    });
    console.log(`SEND_ONCE project=${projectRoot}`);
    await page.locator('#send-message').click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
    project = await page.locator('#confirmed-root').textContent();
    assert.equal(path.resolve(project), path.resolve(projectRoot));
    await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 480000 });
    const html = await readFile(path.join(project, 'site', 'index.html'), 'utf8');
    assert.match(html, /Minha primeira ideia/);
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), 1);
    assert.ok(await page.locator('#messages article[data-role="agent"]').count() >= 1);
    const responseSeconds = Math.round((Date.now() - started) / 1000);
    const observedStatuses = await page.evaluate(() => window.forgeObservedStatuses);
    assert.ok(observedStatuses.some(value => /conferindo o projeto|alterando arquivos|preparando a resposta/.test(value)),
      'The native app must describe at least one real Codex activity without exposing protocol details');
    const citedFile = page.locator('#messages article[data-role="agent"] .message-file-link[data-preview-path="site/index.html"]').last();
    assert.equal(await citedFile.count(), 1, 'The real reply must link the actual result');
    await citedFile.click();
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
    await page.frameLocator('#preview-site').getByRole('heading', { name: 'Minha primeira ideia' }).waitFor({ timeout: 20000 });
    await page.getByRole('button', { name: 'Pedir mudança neste arquivo' }).click();
    assert.match(await page.locator('#message-text').inputValue(), /site\\index\.html/);
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), 1, 'Preparing an adjustment must not send it');
    console.log(`PASS: first result, native preview and unsent adjustment through installed app and real Codex; response in ${responseSeconds}s.`);
    passed = true;
  } finally {
    await browser?.close().catch(() => {});
    if (child?.pid && child.exitCode === null && child.signalCode === null) {
      const exited = once(child, 'exit'); child.kill(); await exited;
    }
    if (passed) {
      for (const [target, prefix] of [[parent, 'forge-native-first-result-'], [profile, 'forge-native-first-profile-']]) {
        assert.equal(path.dirname(path.resolve(target)), path.resolve(fixtureRoot));
        assert.ok(path.basename(target).startsWith(prefix));
        await rm(target, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
      }
    } else console.error(`Preserved failed fixture for diagnosis: projectParent=${parent} profile=${profile} project=${project || '(not created)'}`);
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
