// One-shot real Codex change using the text prepared by the native preview.
// This test must never retry an uncertain Send.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm, readdir, readFile } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const { createHash } = require('node:crypto');
const path = require('node:path');
const assert = require('node:assert/strict');

const exe = process.env.FORGE_DESKTOP_EXE;
const project = process.env.FORGE_TEST_PROJECT;
const threadId = process.env.FORGE_TEST_RESUME_THREAD_ID;
assert.ok(exe && project && threadId, 'Set native executable, disposable project and existing Codex thread');
const marker = 'Comece com uma ideia de cada vez.';
const html = path.join(project, 'site', 'index.html');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');

async function projectFiles(directory = project, prefix = '') {
  const files = {};
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    if (entry.name === '.git') continue;
    const relative = path.join(prefix, entry.name);
    if (entry.isDirectory()) Object.assign(files, await projectFiles(path.join(directory, entry.name), relative));
    else if (entry.isFile()) files[relative] = sha(await readFile(path.join(directory, entry.name)));
  }
  return files;
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
async function resume(page) {
  await page.locator('nav a[data-route="workspace"]').click();
  await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(project);
  await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
  await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
  await page.evaluate(async id => {
    const root = document.getElementById('project-root').value;
    const current = await window.__TAURI__.core.invoke('inspect_project', { projectRoot: root });
    localStorage.setItem(`forge.conversation.v1:${JSON.stringify([current.project_id, current.project_root])}`, id);
  }, threadId);
  await page.locator('#conversation-picker summary').click();
  await page.getByRole('button', { name: 'Conectar ao Codex', exact: true }).click();
  await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 110000 });
  return page.locator('#messages article').allTextContents();
}

(async () => {
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-prefilled-change-'));
  const beforeFiles = await projectFiles();
  assert.equal(beforeFiles['site\\index.html'], '9b283a9a5e6de29b3301dff12e8cc8b1cded96ed4abb391d63eb89fb4f60e08d',
    'Refuse a duplicate send or fixture drift');
  assert.doesNotMatch(await readFile(html, 'utf8'), /Comece com uma ideia de cada vez\./);
  let app;
  try {
    app = await launch(profile);
    const before = await resume(app.page);
    assert.ok(before.length >= 36, 'Expected the real resumed conversation');
    assert.equal(before.some(message => message.includes(marker)), false, 'Request must not already exist');

    const fileLink = app.page.locator('#messages article[data-role="agent"] .message-file-link[data-preview-path="site/index.html"]').last();
    assert.equal(await fileLink.count(), 1);
    const fileChoices = fileLink.locator('xpath=ancestor::details[contains(concat(" ", normalize-space(@class), " "), " message-file-choices ")]');
    if (await fileChoices.count() && !await fileChoices.evaluate(node => node.open)) await fileChoices.locator('summary').click();
    await fileLink.click();
    await app.page.frameLocator('#preview-site').getByRole('heading', { name: 'Jardim de ideias renovado' }).waitFor({ timeout: 20000 });
    await app.page.getByRole('button', { name: 'Pedir mudança neste arquivo' }).click();
    const draft = app.page.locator('#message-text');
    assert.match(await draft.inputValue(), /^Quero mudar o arquivo site\\index\.html: /);
    await draft.fill(`${await draft.inputValue()}Acrescente, logo abaixo da frase de abertura do cabeçalho da página, um parágrafo visível com o texto exato "${marker}". Altere somente site/index.html. Preserve o restante da página, CSS e JavaScript. Não publique nem use rede. Ao terminar, inclua um link Markdown relativo para a página.`);
    assert.equal(await app.page.locator('#messages article').count(), before.length, 'Preview action must not send');
    console.log(`PRE_SEND: ${threadId}, ${before.length} messages, HTML ${beforeFiles['site\\index.html']}`);

    await app.page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await app.page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 360000 });
    const after = await app.page.locator('#messages article').allTextContents();
    assert.ok(after.length >= before.length + 2, 'Expected user request and Codex reply');
    assert.deepEqual(after.slice(0, before.length), before, 'Previous history must retain its order');
    assert.match(after[before.length], /Quero mudar o arquivo site\\index\.html:/);
    assert.match(await readFile(html, 'utf8'), /<p[^>]*>Comece com uma ideia de cada vez\.<\/p>/);
    const afterFiles = await projectFiles();
    assert.notEqual(afterFiles['site\\index.html'], beforeFiles['site\\index.html'], 'HTML must have changed');
    for (const [file, checksum] of Object.entries(beforeFiles)) {
      if (file !== 'site\\index.html') assert.equal(afterFiles[file], checksum, `${file} must not change`);
    }
    assert.deepEqual(Object.keys(afterFiles).sort(), Object.keys(beforeFiles).sort(), 'No unrelated project file should appear');
    await app.page.frameLocator('#preview-site').getByText(marker, { exact: true }).waitFor({ timeout: 30000 });
    if (process.env.FORGE_PREFILLED_CHANGE_SCREENSHOT) await app.page.screenshot({ path: process.env.FORGE_PREFILLED_CHANGE_SCREENSHOT });
    await stop(app); app = null;

    app = await launch(profile);
    const reopened = await resume(app.page);
    assert.deepEqual(reopened, after, 'Full app restart must restore the same ordered conversation without replay');
    assert.equal(sha(await readFile(html)), afterFiles['site\\index.html'], 'Restart must preserve exactly the changed file');
    console.log(`PASS: native prefilled change sent once, updated and previewed the real HTML, and restored ${reopened.length} ordered messages after restart; other project files unchanged.`);
  } finally {
    await stop(app);
    const resolved = path.resolve(profile);
    if (!resolved.startsWith(`${path.resolve(tmpdir())}${path.sep}`)) throw new Error('Refusing cleanup outside the temporary profile');
    await rm(resolved, { recursive: true, force: true, maxRetries: 10, retryDelay: 300 });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
