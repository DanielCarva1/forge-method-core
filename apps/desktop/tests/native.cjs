// Windows-only development smoke test against a real Tauri WebView.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, rm, access } = require('node:fs/promises');
const { once } = require('node:events');
const { tmpdir } = require('node:os');
const path = require('node:path');
const assert = require('node:assert/strict');

(async () => {
  if (!process.env.FORGE_DESKTOP_EXE) throw new Error('Set FORGE_DESKTOP_EXE to the built development executable');
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-desktop-webview-'));
  const child = spawn(process.env.FORGE_DESKTOP_EXE, [], {
    windowsHide: true,
    stdio: 'ignore',
    env: {
      ...process.env,
      WEBVIEW2_USER_DATA_FOLDER: profile,
      WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
    },
  });
  let launchError;
  child.on('error', error => { launchError = error; });
  let browser;
  try {
    const deadline = Date.now() + 20000;
    while (Date.now() < deadline) {
      if (launchError) throw launchError;
      if (child.exitCode !== null) throw new Error(`Application exited: ${child.exitCode}`);
      try {
        browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 });
        break;
      } catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    if (!browser) throw new Error('Native WebView did not become available within 20 seconds');
    const context = browser.contexts()[0];
    const page = context.pages()[0] || await context.waitForEvent('page', { timeout: 5000 });
    await page.locator('nav a[data-route="workspace"]').click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    assert.equal(await page.getByRole('button', { name: 'Escolher pasta' }).isEnabled(), true);
    await page.locator('#connection summary').click();
    await page.getByRole('status').filter({ hasText: 'Aplicativo iniciado' }).waitFor({ timeout: 5000 });
    await page.getByRole('button', { name: 'Verificar novamente' }).click();
    await page.getByRole('status').filter({ hasText: 'Aplicativo iniciado' }).waitFor({ timeout: 5000 });
    await page.getByText('Nenhum agente conectado.', { exact: false }).waitFor();
    if (process.env.FORGE_TEST_PROJECT) {
      const field = page.getByRole('textbox', { name: 'Pasta do projeto' });
      const submit = page.getByRole('button', { name: 'Conferir projeto' });
      await field.fill(process.env.FORGE_TEST_PROJECT);
      await submit.click();
      await page.locator('#project-status').filter({ hasText: 'Projeto encontrado' }).waitFor({ timeout: 15000 });
      assert.equal(await page.locator('#confirmed-root').textContent(), process.env.FORGE_TEST_PROJECT);
      assert.ok((await page.locator('#project-name').textContent()).length > 0);
      await page.getByRole('button', { name: 'Consultar registro', exact: true }).click();
      await page.locator('#progress-status').filter({ hasText: /Consultado às|Não foi possível consultar/ }).waitFor({ timeout: 35000 });
      const progressStatus = await page.locator('#progress-status').textContent();
      assert.match(progressStatus, /Consultado às/, 'Native Forge record lookup must succeed, not merely finish');
      assert.ok((await page.locator('#record-state').textContent()).length > 0);
      assert.ok((await page.locator('#record-title').textContent()).length > 0);
      assert.ok((await page.locator('#record-next').textContent()).length > 0);
      assert.match(await page.locator('#record-decisions').textContent(), /neste registro/);
      console.log('PASS: actual bounded Forge workflow readback displayed separately from agent activity.');
      // A failed lookup must hide the preceding project's identity.
      await field.fill(path.join(profile, 'missing-folder'));
      assert.equal(await page.locator('#project-result').isVisible(), false);
      assert.equal(await page.locator('#project-status').textContent(), '');
      await submit.click();
      await page.locator('#project-status').filter({ hasText: 'pasta que existe' }).waitFor();
      assert.equal(await page.locator('#project-result').isVisible(), false);
      // Existing but unlinked folder: do not initialize or repair it silently.
      await field.fill(profile);
      await submit.click();
      await page.locator('#project-status').filter({ hasText: 'Não foi possível consultar' }).waitFor({ timeout: 20000 });
      assert.equal(await page.locator('#project-result').isVisible(), false);
      await assert.rejects(access(path.join(profile, '.forge-method.yaml')), { code: 'ENOENT' });
      await assert.rejects(access(path.join(profile, '.forge-method')), { code: 'ENOENT' });
      console.log('PASS: real Forge project resolution, invalid folder, unlinked folder, stale identity hidden.');
      if (process.env.FORGE_TEST_AGENT === '1') {
        await field.fill(process.env.FORGE_TEST_PROJECT);
        await submit.click();
        await page.locator('#project-status').filter({ hasText: 'Projeto encontrado' }).waitFor({ timeout: 15000 });
        await page.evaluate(() => {
          window.forgeDeltaCount = 0;
          const NativeChannel = window.__TAURI__.core.Channel;
          const ObservedChannel = class extends NativeChannel {
            set onmessage(callback) { super.onmessage = event => { if (event.kind === 'delta') window.forgeDeltaCount++; callback(event); }; }
            get onmessage() { return super.onmessage; }
          };
          window.__TAURI__ = { ...window.__TAURI__, core: { ...window.__TAURI__.core, Channel: ObservedChannel } };
        });
        await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Codex conectado ao projeto' }).waitFor({ timeout: 100000 });
        assert.equal(await field.isDisabled(), true);
        const composer = page.getByRole('textbox', { name: 'Conte sua ideia' });
        await composer.fill('Esta é uma verificação somente de leitura. Não altere arquivos nem registros de estado, não publique nada, não instale nada. Consulte forge-core project resolve --root . --json para confirmar o projeto e leia apps/desktop/README.md. Em até 5 frases em português, explique quais capacidades do app estão documentadas e o que ainda falta. Não faça implementação.');
        await page.getByRole('button', { name: 'Enviar', exact: true }).click();
        try {
          await page.waitForFunction(() => /Resposta recebida|Atualize o Codex|execução falhou|conexão foi encerrada/.test(document.getElementById('agent-status').textContent), { }, { timeout: 180000 });
          assert.match(await page.locator('#agent-status').textContent(), /Resposta recebida/);
        } catch (error) {
          console.error('Native conversation status:', await page.locator('#agent-status').textContent());
          console.error('Streamed message events:', await page.evaluate(() => window.forgeDeltaCount));
          throw error;
        }
        assert.ok(await page.evaluate(() => window.forgeDeltaCount > 0));
        assert.ok((await page.locator('#messages').textContent()).includes('Codex'));
        await composer.fill('Sem usar ferramentas, escreva uma lista de 1000 exemplos de nomes de projetos, um por linha. Este pedido será interrompido para testar o botão.');
        await page.getByRole('button', { name: 'Enviar', exact: true }).click();
        const interrupt = page.getByRole('button', { name: 'Interromper', exact: true });
        await page.waitForFunction(() => !document.getElementById('interrupt-agent').disabled);
        await interrupt.click();
        await page.locator('#agent-status').filter({ hasText: 'Interrompido.' }).waitFor({ timeout: 30000 });
        await composer.fill('Sem ferramentas, responda apenas: Podemos continuar.');
        await page.getByRole('button', { name: 'Enviar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 90000 });
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor({ timeout: 10000 });
        assert.equal(await field.isDisabled(), false);
        console.log('PASS: actual ChatGPT-authenticated Codex response, streamed deltas, interruption, subsequent turn and disconnect.');
        const previousHistory = await page.locator('#messages').innerText();
        await page.reload();
        await page.getByRole('textbox', { name: 'Pasta do projeto' }).fill(process.env.FORGE_TEST_PROJECT);
        await page.getByRole('button', { name: 'Conferir projeto' }).click();
        await page.locator('#project-status').filter({ hasText: 'Projeto encontrado' }).waitFor({ timeout: 15000 });
        await page.getByRole('button', { name: 'Conectar Codex', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 100000 });
        const restoredHistory = await page.locator('#messages').innerText();
        assert.ok(restoredHistory.includes('Podemos continuar'));
        assert.ok(restoredHistory.includes('Esta é uma verificação somente de leitura.'));
        assert.ok(previousHistory.includes('Podemos continuar'));
        assert.equal(await page.getByRole('button', { name: 'Enviar', exact: true }).isEnabled(), true);
        await page.getByRole('button', { name: 'Desconectar', exact: true }).click();
        await page.locator('#agent-status').filter({ hasText: 'Desconectado.' }).waitFor();
        console.log('PASS: history restored from Codex after transport shutdown and WebView reload, without resending a turn.');
      } else { console.log('NOT_RUN: actual Codex conversation (FORGE_TEST_AGENT not set).'); }
    } else {
      console.log('NOT_RUN: real project resolution (FORGE_TEST_PROJECT not set).');
    }
    await page.locator('.appearance summary').click();
    await page.getByLabel('Tema', { exact: true }).selectOption('dark');
    await page.getByLabel('Reforçar contraste').check();
    await page.reload();
    assert.equal(await page.evaluate(() => document.documentElement.dataset.theme), 'dark');
    assert.equal(await page.getByLabel('Reforçar contraste').isChecked(), true);
    console.log('PASS: native WebView appearance preference survives reload.');
    if (process.env.FORGE_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_SCREENSHOT, fullPage: true });
    console.log('PASS: real native window, frontend-to-Rust identity and retry.');
  } finally {
    try {
      if (browser) {
        try {
          const page = browser.contexts()[0]?.pages()[0];
          if (page) await page.evaluate(() => Promise.race([
            window.__TAURI__?.core?.invoke('disconnect_agent').catch(() => {}),
            new Promise(resolve => setTimeout(resolve, 5000)),
          ]));
        } finally { await browser.close(); }
      }
    } finally {
      try {
        if (child.pid && child.exitCode === null && child.signalCode === null) {
          const exited = once(child, 'exit');
          child.kill();
          await exited;
        }
      } finally {
        if (path.dirname(path.resolve(profile)) !== path.resolve(tmpdir()) || !path.basename(profile).startsWith('forge-desktop-webview-')) {
          throw new Error('Refusing cleanup outside the test profile');
        }
        await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
      }
    }
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
