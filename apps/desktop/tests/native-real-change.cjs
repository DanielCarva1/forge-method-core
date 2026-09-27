// One-shot real Codex edit on the preserved disposable artifact fixture.
// Never retries Send: a timeout or uncertain delivery must be investigated.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { access, readFile } = require('node:fs/promises');
const { createHash } = require('node:crypto');
const path = require('node:path');
const assert = require('node:assert/strict');

const profile = process.env.FORGE_ARTIFACT_PROFILE;
const project = process.env.FORGE_ARTIFACT_PROJECT;
const executable = process.env.FORGE_DESKTOP_EXE;
if (!profile || !project || !executable) throw new Error('Set FORGE_ARTIFACT_PROFILE, FORGE_ARTIFACT_PROJECT and FORGE_DESKTOP_EXE');
const html = path.join(project, 'site', 'index.html');
const css = path.join(project, 'site', 'assets', 'site.css');
const oldTitle = 'Jardim de ideias';
const newTitle = 'Jardim de ideias renovado';
const originalHash = 'e118b0c85ec32edb763cbe9607919407db15aa4577fbbffd4759b7ac8420c907';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');

async function freePort() {
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  return port;
}

async function launch() {
  const port = await freePort();
  const child = spawn(executable, [], { windowsHide: true, stdio: 'ignore', env: {
    ...process.env, WEBVIEW2_USER_DATA_FOLDER: profile,
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
  } });
  let browser;
  const deadline = Date.now() + 30000;
  while (Date.now() < deadline) {
    if (child.exitCode !== null) throw new Error(`Application exited: ${child.exitCode}`);
    try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); break; }
    catch { await new Promise(resolve => setTimeout(resolve, 200)); }
  }
  if (!browser) { child.kill(); throw new Error('Native WebView did not become available'); }
  const page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
  await page.locator('#home').waitFor({ state: 'visible' });
  return { child, browser, page };
}

async function stop(app) {
  if (!app) return;
  await app.browser.close().catch(() => {});
  if (app.child.exitCode === null && app.child.signalCode === null) {
    const exit = new Promise(resolve => app.child.once('exit', resolve));
    app.child.kill();
    await exit;
  }
}

async function resume(page) {
  await page.locator('nav a[data-route="workspace"]').click();
  await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(project);
  await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
  await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 60000 });
  await page.locator('#conversation-picker summary').click();
  await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
  await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 110000 });
  await page.locator('#messages article[data-role="agent"]').nth(1).waitFor({ timeout: 60000 });
}

(async () => {
  await access(html); await access(css);
  const before = await readFile(html);
  assert.equal(hash(before), originalHash, 'Refuse to resend after any previous edit or fixture drift');
  const cssHash = hash(await readFile(css));
  let app;
  try {
    app = await launch();
    await resume(app.page);
    const page = app.page;
    const messagesBefore = await page.locator('#messages article').count();
    console.log(`RESUMED_MESSAGE_COUNT=${messagesBefore}`);
    console.log(`RESUMED_ROLES=${JSON.stringify(await page.locator('#messages article').evaluateAll(nodes => nodes.map(node => node.dataset.role)))}`);
    assert.ok(messagesBefore >= 4, 'Expected the untouched real artifact conversation; no automatic retry');
    assert.doesNotMatch(await page.locator('#messages article[data-role="user"]').allInnerTexts().then(texts => texts.join('\n')), /Jardim de ideias renovado/, 'The change request must not already exist');
    const usersBefore = await page.locator('#messages article[data-role="user"]').count();
    const agentsBefore = await page.locator('#messages article[data-role="agent"]').count();
    const previousReply = page.locator('#messages article[data-role="agent"]').last();
    await previousReply.getByRole('button', { name: /Ver arquivo local:/ }).first().click();
    await page.frameLocator('#preview-site').getByRole('heading', { name: oldTitle, exact: true }).waitFor({ timeout: 20000 });
    await page.getByRole('button', { name: 'Pedir mudança neste arquivo' }).click();
    const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    assert.match(await composer.inputValue(), /site\\index\.html/);
    await composer.fill(`Altere apenas site/index.html neste projeto: mude o título visível principal para "${newTitle}". Preserve o restante da página e o CSS. Não use JavaScript, rede nem publique nada. Ao terminar, inclua um link Markdown relativo para eu abrir a página aqui.`);
    assert.equal(await page.locator('#messages article').count(), messagesBefore);
    console.log(`BEFORE_HTML_SHA256=${hash(before)}`);
    console.log('SEND_ONCE: real Codex change request in the existing disposable conversation.');
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 360000 });
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), usersBefore + 1);
    assert.ok((await page.locator('#messages article[data-role="agent"]').count()) > agentsBefore);
    const messagesAfter = await page.locator('#messages article').count();
    const finalReply = page.locator('#messages article[data-role="agent"]').last();
    assert.equal(await finalReply.getByRole('button', { name: /Ver arquivo local:/ }).count(), 1, 'Reply should link the changed local file');
    assert.match(await readFile(html, 'utf8'), new RegExp(`<h1[^>]*>\\s*${newTitle}\\s*<\\/h1>`));
    assert.equal(hash(await readFile(css)), cssHash, 'CSS must not change');
    await page.frameLocator('#preview-site').getByRole('heading', { name: newTitle, exact: true }).waitFor({ timeout: 30000 });
    const changedHash = hash(await readFile(html));
    console.log(`AFTER_HTML_SHA256=${changedHash}`);
    await stop(app); app = null;

    app = await launch();
    await resume(app.page);
    assert.equal(await app.page.locator('#messages article').count(), messagesAfter, 'Restart must not replay the turn');
    assert.equal(await app.page.locator('#messages article[data-role="user"]').count(), usersBefore + 1);
    assert.ok((await app.page.locator('#messages article[data-role="agent"]').count()) > agentsBefore);
    assert.equal(hash(await readFile(html)), changedHash, 'Restart must preserve the changed file');
    const restoredAction = app.page.locator('#messages article[data-role="agent"]').last().getByRole('button', { name: /Ver arquivo local:/ }).first();
    await restoredAction.click();
    await app.page.frameLocator('#preview-site').getByRole('heading', { name: newTitle, exact: true }).waitFor({ timeout: 20000 });
    console.log('PASS: real same-chat change updated the local result, refreshed its protected preview, and survived a full app restart without replay.');
  } finally { await stop(app); }
})().catch(error => { console.error(error); process.exitCode = 1; });
