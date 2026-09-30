// Native WebView with actual project IPC; historical comparison uses controlled data.
// No model request, account change, or authored historical agreement.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const fs = require('node:fs/promises');
const path = require('node:path');
const { tmpdir } = require('node:os');
const assert = require('node:assert/strict');
(async () => {
  const profile = await fs.mkdtemp(path.join(tmpdir(), 'forge-native-directions-'));
  assert.ok(path.resolve(profile).startsWith(path.resolve(tmpdir()) + path.sep));
  const project = path.join(profile, 'project');
  await fs.mkdir(project);
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  const child = spawn(process.env.FORGE_DESKTOP_EXE, [], { windowsHide: true, stdio: 'ignore', env: {
    ...process.env, WEBVIEW2_USER_DATA_FOLDER: path.join(profile, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
  } });
  let browser;
  try {
    for (let i = 0; i < 100; i++) {
      try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); break; }
      catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    assert.ok(browser, 'Native WebView must start');
    const page = browser.contexts()[0].pages()[0];
    await page.locator('nav a[data-route="workspace"]').click();
    await page.locator('#custom-folder-option summary').click();
    await page.locator('#project-root').fill(project);
    await page.locator('#start-project').click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 60000 });
    await page.evaluate(async root => {
      const native = window.__TAURI__.core;
      const actual = await native.invoke('inspect_direction_history', { projectRoot: root });
      if (actual.revisions.length !== 0) throw new Error('Fresh project unexpectedly has history');
      window.directionCalls = []; window.directionMode = 'two';
      const revisions = [
        { active: false, origin: 'forge_cooperative_record', revision: 1, revision_kind: 'initial', outcome: 'Um jardim de ideias', constraints: ['Somente local', 'Sem cadastro'], unacceptable_outcomes: ['Perder arquivos'], accepted_at_unix: 1780000000 },
        { active: true, origin: 'forge_cooperative_record', revision: 2, revision_kind: 'material_supersession', outcome: 'Um jardim de ideias para a família <script>não executar</script>', constraints: ['Sem cadastro', 'Uso compartilhado'], unacceptable_outcomes: ['Perder arquivos', 'Publicar sem pedir'], accepted_at_unix: 1781000000 },
      ];
      const facade = Object.create(native);
      Object.defineProperty(facade, 'invoke', { value: (command, args) => {
        window.directionCalls.push(command);
        if (command !== 'inspect_direction_history') return native.invoke(command, args);
        return Promise.resolve({ earlier_count: window.directionMode === 'one' ? 4 : 0,
          revisions: window.directionMode === 'one' ? revisions.slice(1) : window.directionMode === 'empty' ? [] : revisions });
      } });
      window.__TAURI__.core = facade;
    }, project);
    await page.locator('#message-text').fill('Minha ideia ainda não enviada');
    await page.getByRole('button', { name: 'Andamento', exact: true }).click();
    await page.locator('#direction-history summary').click();
    await page.locator('#direction-history-status').filter({ hasText: '2 direções registradas' }).waitFor();
    const current = page.locator('#direction-history-list article').first();
    assert.match(await current.locator('summary').textContent(), /Um jardim de ideias para a família/);
    await current.locator('summary').click();
    assert.match(await current.textContent(), /O que mudou/);
    assert.match(await current.textContent(), /Combinados acrescentados[\s\S]*Uso compartilhado/);
    assert.match(await current.textContent(), /Combinados retirados[\s\S]*Somente local/);
    assert.match(await current.textContent(), /Cuidados acrescentados[\s\S]*Publicar sem pedir/);
    assert.equal(await page.locator('#direction-history-list script').count(), 0);
    if (process.env.FORGE_HISTORY_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_HISTORY_SCREENSHOT, fullPage: true });
    await page.evaluate(() => { window.directionMode = 'one'; });
    await page.locator('#refresh-direction-history').click();
    await page.locator('#direction-history-status').filter({ hasText: '1 direção registrada' }).waitFor();
    assert.doesNotMatch(await page.locator('#direction-history-list').textContent(), /O que mudou/);
    await page.evaluate(() => { window.directionMode = 'empty'; });
    await page.locator('#refresh-direction-history').click();
    await page.locator('#direction-history-status').filter({ hasText: 'Nenhuma direção anterior' }).waitFor();
    assert.equal(await page.locator('#direction-history-list article').count(), 0);
    assert.equal(await page.locator('#message-text').inputValue(), 'Minha ideia ainda não enviada');
    const calls = await page.evaluate(() => window.directionCalls);
    assert.ok(calls.includes('inspect_direction_history'));
    assert.ok(!calls.some(command => /send_message|connect_agent|respond|answer|start_project/.test(command)), 'Review must not send, connect or record a choice');
    console.log('PASS: installed native history compares controlled revisions literally; added/removed wording, script text, bounded/empty history, unchanged draft, no model/write calls. Actual fresh-project readback returned empty history.');
  } finally {
    if (browser) await browser.close();
    if (child.exitCode === null && child.signalCode === null) {
      const exited = new Promise(resolve => child.once('exit', resolve)); child.kill(); await exited;
    }
    await fs.rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
