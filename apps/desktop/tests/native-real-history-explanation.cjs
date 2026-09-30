// One-shot native journey against an existing disposable project and Codex chat.
// Sends exactly once; never retries an uncertain send.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn, execFileSync } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm, readdir, readFile } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const { createHash } = require('node:crypto');
const path = require('node:path');
const assert = require('node:assert/strict');

const exe = process.env.FORGE_DESKTOP_EXE;
const core = process.env.FORGE_CORE_EXE;
const project = process.env.FORGE_TEST_PROJECT;
const threadId = process.env.FORGE_TEST_RESUME_THREAD_ID;
assert.ok(exe && core && project && threadId, 'Set Desktop, bundled core, disposable project and existing Codex thread');
const digest = value => createHash('sha256').update(value).digest('hex');

async function projectFiles(directory = project, prefix = '') {
  const files = {};
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.name === '.git') continue;
    const relative = path.join(prefix, entry.name);
    if (entry.isDirectory()) Object.assign(files, await projectFiles(path.join(directory, entry.name), relative));
    else if (entry.isFile()) files[relative] = digest(await readFile(path.join(directory, entry.name)));
  }
  return files;
}
function forgeHistory() {
  const report = JSON.parse(execFileSync(core, ['workflow', 'report', '--root', project, '--json'], { encoding: 'utf8', timeout: 30000 }));
  assert.equal(report.ok, true, 'Forge report must succeed');
  const continuity = report.data?.replacement_continuity;
  assert.ok(continuity?.objective_history, 'Forge must return objective history');
  return JSON.stringify({ objective_history: continuity.objective_history, decision_history: continuity.decision_history });
}
async function freePort() {
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}
async function launch(profile) {
  const port = await freePort();
  const child = spawn(exe, [], { windowsHide: true, stdio: 'ignore', env: {
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
    const page = browser.contexts()[0].pages()[0];
    await page.locator('#home').waitFor({ state: 'visible' });
    return { child, browser, page };
  } catch (error) {
    if (browser) await browser.close().catch(() => {});
    child.kill();
    throw error;
  }
}
async function stop(app) {
  if (!app) return;
  await app.browser.close().catch(() => {});
  if (app.child.exitCode === null && app.child.signalCode === null) {
    const exited = new Promise(resolve => app.child.once('exit', resolve));
    app.child.kill();
    await exited;
  }
}
async function openProject(page) {
  await page.locator('nav a[data-route="workspace"]').click();
  await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(project);
  await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
  await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
}
async function resume(page) {
  await page.locator('#conversation-picker summary').click();
  await page.getByRole('button', { name: 'Conectar ao Codex', exact: true }).click();
  await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 110000 });
  return page.locator('#messages article').allTextContents();
}

(async () => {
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-real-history-'));
  const filesBefore = await projectFiles();
  const historyBefore = forgeHistory();
  let app;
  try {
    app = await launch(profile);
    await openProject(app.page);
    await app.page.evaluate(async id => {
      const root = document.getElementById('project-root').value;
      const current = await window.__TAURI__.core.invoke('inspect_project', { projectRoot: root });
      localStorage.setItem(`forge.conversation.v1:${JSON.stringify([current.project_id, current.project_root])}`, id);
    }, threadId);
    const before = await resume(app.page);
    assert.ok(before.length >= 30, 'Expected a substantial existing Codex conversation');
    assert.equal(before.some(text => text.includes('se não houver, diga isso sem inventar mudanças')), false,
      'This one-shot prompt must not already be present');

    await app.page.locator('#direction-history summary').click();
    await app.page.locator('#direction-history-status').filter({ hasText: '1 direção registrada' }).waitFor({ timeout: 35000 });
    assert.equal(await app.page.locator('#direction-history-list article').count(), 1);
    await app.page.getByRole('button', { name: 'Entender esta direção na conversa' }).click();
    const draft = app.page.locator('#message-text');
    assert.match(await draft.inputValue(), /se não houver, diga isso sem inventar mudanças/);
    assert.equal(await app.page.locator('#messages article').count(), before.length, 'Preparing the question must not send');
    console.log(`PRE_SEND: ${threadId}, ${before.length} messages, one objective revision`);

    await app.page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await app.page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 360000 });
    const after = await app.page.locator('#messages article').allTextContents();
    assert.ok(after.length >= before.length + 2, 'Expected one user question and at least one Codex reply');
    assert.deepEqual(after.slice(0, before.length), before, 'Earlier messages must keep their order');
    assert.match(after[before.length], /se não houver, diga isso sem inventar mudanças/);
    assert.ok(after.slice(before.length + 1).join(' ').trim().length > 30, 'Codex reply must have substantive text');
    console.log(`REPLY: ${after.slice(before.length + 1).join(' ').slice(0, 900)}`);
    assert.deepEqual(await projectFiles(), filesBefore, 'Read-only explanation must not edit project files');
    assert.equal(forgeHistory(), historyBefore, 'Read-only explanation must not change objective history');

    await stop(app); app = null;
    app = await launch(profile);
    await openProject(app.page);
    const reopened = await resume(app.page);
    assert.deepEqual(reopened, after, 'Native process restart must restore the same ordered conversation without resending');
    console.log(`PASS: real Codex explanation sent once and restored ${reopened.length} ordered messages; project files and Forge objective history unchanged.`);
  } finally {
    await stop(app);
    const resolved = path.resolve(profile);
    if (!resolved.startsWith(`${path.resolve(tmpdir())}${path.sep}`)) throw new Error('Refusing cleanup outside the temporary profile');
    await rm(resolved, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
