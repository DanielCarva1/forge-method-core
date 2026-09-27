// Controlled installed-binary upgrade probe. The caller installs each version
// between stages; this script keeps one disposable WebView profile and project.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdir } = require('node:fs/promises');
const assert = require('node:assert/strict');

const stage = process.env.FORGE_UPGRADE_STAGE;
const profile = process.env.FORGE_UPGRADE_PROFILE;
const project = process.env.FORGE_UPGRADE_PROJECT;
const executable = process.env.FORGE_DESKTOP_EXE;
const marker = 'Marcador alpha 2719';
if (!['create', 'resume'].includes(stage) || !profile || !project || !executable) {
  throw new Error('Set FORGE_UPGRADE_STAGE, FORGE_UPGRADE_PROFILE, FORGE_UPGRADE_PROJECT and FORGE_DESKTOP_EXE');
}

async function availablePort() {
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  return port;
}

(async () => {
  await mkdir(profile, { recursive: true });
  if (stage === 'create') await mkdir(project, { recursive: true });
  const port = await availablePort();
  const child = spawn(executable, [], {
    windowsHide: true,
    stdio: 'ignore',
    env: { ...process.env, WEBVIEW2_USER_DATA_FOLDER: profile,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1` },
  });
  let launchError;
  child.on('error', error => { launchError = error; });
  let browser;
  try {
    const deadline = Date.now() + 25000;
    while (Date.now() < deadline) {
      if (launchError) throw launchError;
      if (child.exitCode !== null) throw new Error(`Application exited: ${child.exitCode}`);
      try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); break; }
      catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    if (!browser) throw new Error('Native WebView did not become available');
    const context = browser.contexts()[0];
    const page = context.pages()[0] || await context.waitForEvent('page', { timeout: 5000 });
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.locator('nav a[data-route="workspace"]').click();
    await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(project);
    await page.getByRole('button', { name: 'Continuar nesta pasta' }).click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 30000 });

    if (stage === 'create') {
      await page.getByRole('textbox', { name: 'Sua ideia começa aqui' }).fill(`Sem usar ferramentas nem alterar arquivos, responda em português apenas: ${marker}.`);
      await page.getByRole('button', { name: 'Enviar', exact: true }).click();
      await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 130000 });
      assert.equal(await page.locator('#messages article[data-role="user"]').count(), 1);
      assert.equal(await page.locator('#messages article[data-role="agent"]').count(), 1);
      assert.match(await page.locator('#messages article[data-role="agent"]').innerText(), /Marcador alpha 2719/i);
      console.log('PASS: old installed binary created one real Codex turn in the disposable project/profile.');
    } else {
      await page.locator('#conversation-picker summary').click();
      await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
      await page.locator('#messages article[data-role="agent"]').waitFor({ timeout: 30000 });
      assert.equal(await page.locator('#messages article[data-role="user"]').count(), 1);
      assert.equal(await page.locator('#messages article[data-role="agent"]').count(), 1);
      assert.match(await page.locator('#messages article[data-role="user"]').innerText(), /Marcador alpha 2719/i);
      assert.match(await page.locator('#messages article[data-role="agent"]').innerText(), /Marcador alpha 2719/i);
      await page.waitForTimeout(1000);
      assert.equal(await page.locator('#messages article').count(), 2, 'Upgrade resume must not replay or create a turn');
      if (process.env.FORGE_TEST_ZOOM_EQUIVALENT === '1') {
        const zoomSession = await context.newCDPSession(page);
        try {
          await zoomSession.send('Emulation.setDeviceMetricsOverride', { width: 590, height: 410, deviceScaleFactor: 1, mobile: false });
          assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), true, 'Zoom-equivalent viewport should not scroll sideways');
          const draft = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
          const longDraft = 'Uma ideia longa que ainda não será enviada. '.repeat(30);
          await draft.fill(longDraft);
          assert.equal(await draft.inputValue(), longDraft);
          await page.locator('#send-message').evaluate(node => node.scrollIntoView({ block: 'end' }));
          assert.equal(await page.locator('#send-message').evaluate(node => {
            const box = node.getBoundingClientRect();
            return box.top >= 0 && box.bottom <= innerHeight + 2;
          }), true, 'Send should remain reachable with a long draft in a zoom-equivalent viewport');
          assert.equal(await page.locator('#messages article').count(), 2);
        } finally {
          await zoomSession.send('Emulation.clearDeviceMetricsOverride');
          await zoomSession.detach();
        }
        console.log('PASS: local release WebView keeps long draft and Send reachable in a zoom-equivalent viewport without sideways overflow.');
      }
      console.log('PASS: new binary restored the pre-upgrade user/reply pair without sending a new turn.');
    }
  } finally {
    if (browser) await browser.close().catch(() => {});
    child.kill();
    await new Promise(resolve => { if (child.exitCode !== null) resolve(); else child.once('exit', resolve); });
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
