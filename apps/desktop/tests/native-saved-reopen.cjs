// Focused hidden-native check: a clean saved Codex thread reopens with its project.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm } = require('node:fs/promises');
const { once } = require('node:events');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

const executable = process.env.FORGE_DESKTOP_EXE;
const suppliedProjectRoot = process.env.FORGE_TEST_PROJECT;
if (!executable) throw new Error('Set FORGE_DESKTOP_EXE');

(async () => {
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-native-saved-reopen-'));
  const projectRoot = suppliedProjectRoot || await mkdtemp(path.join(tmpdir(), 'forge-native-saved-project-'));
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
    await page.locator('nav a[data-route="workspace"]').click();
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(projectRoot);
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 35000 });
    await page.locator('#conversation-picker summary').click();
    await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Codex conectado' }).waitFor({ timeout: 60000 });
    const prompt = 'Sem usar ferramentas nem alterar arquivos, responda em português apenas: Retomada verificada.';
    await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).fill(prompt);
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 180000 });
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), 1);
    assert.equal(await page.locator('#messages article[data-role="agent"]').count(), 1);
    const bookmark = await page.evaluate(() => {
      const project = JSON.parse(localStorage.getItem('forge.projects.v1'))[0];
      return localStorage.getItem(`forge.conversation.v1:${JSON.stringify([project.project_id, project.project_root])}`);
    });
    assert.ok(bookmark, 'A real Codex thread must have a saved bookmark');
    await stop();

    child = start();
    browser = await attach();
    page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.evaluate(() => {
      window.reopenCalls = { connect: [], send: 0 };
      const core = window.__TAURI__.core;
      const facade = Object.create(core);
      Object.defineProperty(facade, 'invoke', { value: (command, args) => {
        if (command === 'connect_agent') window.reopenCalls.connect.push(args.threadId);
        if (command === 'send_message') window.reopenCalls.send++;
        return core.invoke(command, args);
      } });
      window.__TAURI__.core = facade;
    });
    assert.equal(await page.locator('#home-hero-project-action').textContent(), 'Continuar último projeto');
    await page.locator('#home-hero-project-action').click();
    try {
      await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 20000 });
    } catch (error) {
      console.error('Reopen state:', await page.evaluate(() => ({
        status: document.getElementById('agent-status').textContent,
        projectStatus: document.getElementById('project-status').textContent,
        calls: window.reopenCalls,
      })));
      throw error;
    }
    assert.equal(await page.locator('#confirmed-root').textContent(), projectRoot);
    assert.deepEqual(await page.evaluate(() => window.reopenCalls), { connect: [bookmark], send: 0 });
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), 1);
    assert.equal(await page.locator('#messages article[data-role="agent"]').count(), 1);
    assert.match(await page.locator('#messages').textContent(), /Retomada verificada/);
    console.log('PASS: hidden native Home reopened a real saved Codex conversation with its reply and validated project, without a second Send.');
  } finally {
    await stop();
    assert.equal(path.dirname(path.resolve(profile)), path.resolve(tmpdir()));
    assert.ok(path.basename(profile).startsWith('forge-native-saved-reopen-'));
    await rm(profile, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
    if (!suppliedProjectRoot) {
      const forgeState = path.join(path.dirname(projectRoot), `forge-${path.basename(projectRoot)}`);
      for (const [target, prefix] of [[projectRoot, 'forge-native-saved-project-'], [forgeState, 'forge-forge-native-saved-project-']]) {
        assert.equal(path.dirname(path.resolve(target)), path.resolve(tmpdir()));
        assert.ok(path.basename(target).startsWith(prefix));
        await rm(target, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 });
      }
    }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
