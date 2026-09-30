// Real Windows WebView + IPC + stdio protocol, with a simulated Codex server.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, mkdir, rm, copyFile, readFile } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');
(async () => {
  assert.ok(process.env.FORGE_DESKTOP_EXE);
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-native-questions-'));
  const project = path.join(profile, 'project'); await mkdir(project);
  await copyFile(path.join(__dirname, 'fixtures/fake-question-server.cjs'), path.join(project, 'app-server'));
  const marker = path.join(profile, 'responses.jsonl');
  const server = createServer(); await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port; await new Promise(resolve => server.close(resolve));
  const child = spawn(process.env.FORGE_DESKTOP_EXE, [], { windowsHide: true, stdio: 'ignore', env: {
    ...process.env, FORGE_CODEX_EXE: process.execPath, FORGE_QUESTION_MARKER: marker,
    WEBVIEW2_USER_DATA_FOLDER: path.join(profile, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
  } });
  let browser;
  try {
    for (let i = 0; i < 100; i++) {
      if (child.exitCode !== null) throw new Error(`Forge exited: ${child.exitCode}`);
      try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); break; }
      catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    assert.ok(browser);
    const page = browser.contexts()[0].pages()[0];
    await page.locator('nav a[data-route="workspace"]').click();
    await page.locator('.skip').focus(); await page.keyboard.press('Enter');
    assert.equal(await page.locator('#workspace').isVisible(), true);
    assert.equal(await page.evaluate(() => document.activeElement.id), 'main');
    await page.locator('#custom-folder-option summary').click();
    await page.locator('#project-root').fill(project);
    await page.locator('#message-text').fill('Vamos criar');
    await page.locator('#send-message').click();
    const panel = page.locator('#agent-questions');
    await panel.waitFor({ state: 'visible', timeout: 60000 });
    await page.setViewportSize({ width: 360, height: 720 });
    await page.locator('[data-mobile-pane-button="project"]').click();
    assert.equal(await panel.isVisible(), false);
    assert.equal(await page.locator('#workspace-question-cue').textContent(), ' · pergunta');
    await page.locator('[data-mobile-pane-button="conversation"]').click();
    assert.equal(await page.evaluate(() => document.activeElement.textContent), 'Uma escolha sua');
    await page.locator('#message-text').fill('Próximo pedido reservado');
    await panel.getByRole('radio', { name: /Claro/ }).check();
    await panel.locator('textarea').nth(1).fill('Jardim');
    if (process.env.FORGE_QUESTIONS_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_QUESTIONS_SCREENSHOT, fullPage: true });
    await panel.getByRole('button', { name: 'Enviar respostas' }).click();
    await panel.waitFor({ state: 'hidden' });
    assert.equal(await page.evaluate(() => document.activeElement.id), 'agent-status');
    assert.equal(await page.locator('#workspace-question-cue').textContent(), '');
    await page.locator('#messages').filter({ hasText: 'Recebi suas escolhas.' }).waitFor();
    const records = (await readFile(marker, 'utf8')).trim().split('\n').map(JSON.parse);
    assert.deepEqual(records.find(record => record.id === 900), { id: 900, result: { answers: { style: { answers: ['Claro'] }, name: { answers: ['Jardim'] } } } });
    assert.equal(records.filter(record => record.id === 900).length, 1);
    assert.equal(records.find(record => record.id === 'approval-test').error.code, -32601, 'Execution approval remains unsupported, not accepted');
    assert.equal(await page.locator('#message-text').inputValue(), 'Próximo pedido reservado');
    const answer = args => page.evaluate(args => window.__TAURI__.core.invoke('answer_questions', args).then(() => false, () => true), args);
    assert.equal(await answer({ requestId: '900', threadId: 'questions-thread', answers: { style: 'Claro', name: 'Jardim' } }), true, 'Answered request cannot be replayed');
    await page.locator('#send-message').click();
    await panel.waitFor({ state: 'visible' });
    assert.equal(await answer({ requestId: '"question-2"', threadId: 'another-thread', answers: { style: 'Claro', name: 'Jardim' } }), true, 'Wrong conversation rejected');
    await panel.getByRole('button', { name: 'Interromper esta execução' }).click();
    await panel.waitFor({ state: 'hidden' });
    await page.locator('#agent-status').filter({ hasText: 'Interrompido' }).waitFor();
    assert.equal(await answer({ requestId: '"question-2"', threadId: 'questions-thread', answers: { style: 'Claro', name: 'Jardim' } }), true, 'Interrupted request is stale');
    const finalRecords = (await readFile(marker, 'utf8')).trim().split('\n').map(JSON.parse);
    assert.equal(finalRecords.some(record => record.id === 'question-2'), false, 'No answer sent after interruption');
    console.log('PASS: native questions → exact stdio answer → agent reply; no replay, wrong-thread answer, stale answer or implicit execution approval. Codex server simulated.');
  } finally {
    if (browser) { const page = browser.contexts()[0].pages()[0]; if (page) await page.evaluate(() => window.__TAURI__.core.invoke('disconnect_agent')).catch(() => {}); await browser.close(); }
    child.kill(); await new Promise(resolve => child.exitCode !== null ? resolve() : child.once('exit', resolve));
    await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
