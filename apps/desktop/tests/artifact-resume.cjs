// Continue an interrupted native artifact journey without replaying its work.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { access, readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');

const profile = process.env.FORGE_ARTIFACT_PROFILE;
const project = process.env.FORGE_ARTIFACT_PROJECT;
const executable = process.env.FORGE_DESKTOP_EXE;
const heading = process.env.FORGE_ARTIFACT_HEADING || 'Jardim de ideias';
if (!profile || !project || !executable) throw new Error('Set FORGE_ARTIFACT_PROFILE, FORGE_ARTIFACT_PROJECT and FORGE_DESKTOP_EXE');

(async () => {
  const artifact = path.join(project, 'site', 'index.html');
  await access(artifact);
  assert.match(await readFile(artifact, 'utf8'), /Jardim de ideias/);
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  const child = spawn(executable, [], {
    windowsHide: true,
    stdio: 'ignore',
    env: { ...process.env, WEBVIEW2_USER_DATA_FOLDER: profile,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1` },
  });
  let browser;
  try {
    const deadline = Date.now() + 25000;
    while (Date.now() < deadline) {
      if (child.exitCode !== null) throw new Error(`Application exited: ${child.exitCode}`);
      try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); break; }
      catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    if (!browser) throw new Error('Native WebView did not become available');
    const page = browser.contexts()[0].pages()[0] || await browser.contexts()[0].waitForEvent('page', { timeout: 5000 });
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.locator('nav a[data-route="workspace"]').click();
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(project);
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 30000 });
    await page.locator('#conversation-picker summary').click();
    await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 90000 });
    await page.locator('#messages article[data-role="agent"]').nth(1).waitFor({ timeout: 90000 });
    const messagesBefore = await page.locator('#messages article').count();
    assert.ok(messagesBefore >= 4, 'Expected the real creation prompt and its reply after the initial no-tools turn');
    let reply = page.locator('#messages article[data-role="agent"]').last();
    let fileAction = reply.getByRole('button', { name: /Ver arquivo local:/ }).first();
    let followUpSent = false;
    if (await fileAction.count() === 0) {
      if (process.env.FORGE_ARTIFACT_READ_ONLY === '1') throw new Error('Existing real conversation has no linked file; read-only probe will not send a repair turn');
      // The parent test was interrupted while Codex was still answering. Ask
      // only for a link to the already-created file; do not repeat creation.
      const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
      await composer.fill('Sem usar ferramentas nem alterar arquivos, responda em português apenas com um link Markdown relativo para o arquivo já criado: [Abrir a página](site/index.html).');
      await page.getByRole('button', { name: 'Enviar', exact: true }).click();
      followUpSent = true;
      await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 130000 });
      reply = page.locator('#messages article[data-role="agent"]').last();
      fileAction = reply.getByRole('button', { name: /Ver arquivo local:/ }).first();
    }
    assert.equal(await fileAction.count(), 1, 'Real Codex reply must expose a local-file action');
    await fileAction.click();
    await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 20000 });
    assert.equal(await page.locator('#preview-path').textContent(), 'site\\index.html');
    await page.frameLocator('#preview-site').getByRole('heading', { name: heading, exact: true }).waitFor({ timeout: 20000 });
    assert.match(await page.locator('.preview-origin').first().textContent(), /Publicação não verificada/);
    if (process.env.FORGE_EXPECT_EMPTY_RECORD_COPY === '1') {
      await page.locator('#progress-status').filter({ hasText: 'Ainda não há andamento registrado no Forge' }).waitFor({ timeout: 35000 });
      assert.equal(await page.locator('#workspace-phase').textContent(), 'Sem andamento registrado');
      assert.equal(await page.locator('.record-stage').isVisible(), false);
      assert.match(await page.locator('#record-empty-help').textContent(), /Você pode começar ou continuar pela conversa/);
    }
    if (process.env.FORGE_ARTIFACT_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_ARTIFACT_SCREENSHOT, fullPage: true });
    await page.getByRole('button', { name: 'Pedir mudança neste arquivo' }).click();
    const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    assert.match(await composer.inputValue(), /site\\index\.html/);
    assert.equal(await page.locator('#messages article').count(), messagesBefore + (followUpSent ? 2 : 0), 'Preparing a change must not send another turn');
    console.log(`PASS: resumed the real Codex conversation, opened its generated local HTML from ${followUpSent ? 'a bounded follow-up' : 'an existing linked'} reply, and prepared a change request without auto-sending it.`);
  } finally {
    if (browser) await browser.close().catch(() => {});
    child.kill();
    await new Promise(resolve => { if (child.exitCode !== null) resolve(); else child.once('exit', resolve); });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
