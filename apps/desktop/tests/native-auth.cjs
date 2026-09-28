// Hidden native first-use auth check with an isolated CODEX_HOME. Never complete a real login here.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn } = require('node:child_process');
const { createServer } = require('node:net');
const { mkdtemp, mkdir, rm, copyFile, access, readFile, realpath } = require('node:fs/promises');
const { tmpdir } = require('node:os');
const { createHash } = require('node:crypto');
const path = require('node:path');
const assert = require('node:assert/strict');

(async () => {
  const fakeCompletion = process.env.FORGE_AUTH_FAKE_COMPLETION === '1';
  if (!process.env.FORGE_DESKTOP_EXE)
    throw new Error('Set FORGE_DESKTOP_EXE; bundled Forge core and Codex are used when executable overrides are absent');
  const profile = await mkdtemp(path.join(tmpdir(), 'forge-native-auth-'));
  const codexHome = path.join(profile, 'codex-home');
  await mkdir(codexHome);
  const project = path.join(profile, 'project');
  await mkdir(project);
  const fakeMarker = path.join(profile, 'fake-completed');
  const skillMarker = path.join(profile, 'fake-skill-instructions.json');
  if (fakeCompletion) {
    const fixture = path.join(__dirname, 'fixtures', 'fake-login-server.cjs');
    await copyFile(fixture, path.join(profile, 'app-server'));
    await copyFile(fixture, path.join(project, 'app-server'));
  }
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
  const child = spawn(process.env.FORGE_DESKTOP_EXE, [], { cwd: profile, windowsHide: true, stdio: 'ignore', env: {
    ...process.env,
    CODEX_HOME: codexHome,
    ...(fakeCompletion ? { FORGE_CODEX_EXE: process.execPath, FORGE_FAKE_AUTH_MARKER: fakeMarker, FORGE_FAKE_SKILL_MARKER: skillMarker } : {}),
    WEBVIEW2_USER_DATA_FOLDER: path.join(profile, 'webview'),
    WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS: `--remote-debugging-port=${port} --remote-debugging-address=127.0.0.1`,
  } });
  let browser;
  try {
    for (let i = 0; i < 100; i++) {
      if (child.exitCode !== null) throw new Error(`Forge exited early: ${child.exitCode}`);
      try { browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 }); break; }
      catch { await new Promise(resolve => setTimeout(resolve, 200)); }
    }
    if (!browser) throw new Error('Native WebView unavailable');
    const page = browser.contexts()[0].pages()[0];
    await page.locator('#home').waitFor({ state: 'visible' });
    await page.locator('nav a[data-route="workspace"]').click();
    await page.locator('#workspace').waitFor({ state: 'visible' });
    await page.locator('#project-root').fill(project);
    await page.locator('#start-project').click();
    await page.locator('#project-status').filter({ hasText: 'Projeto pronto' }).waitFor({ timeout: 90000 });
    assert.match(await page.locator('#project-status').textContent(), /começar ou continuar a conversa; nada foi enviado/);
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor({ timeout: 35000 });
    assert.equal(await page.locator('#workspace-phase').textContent(), 'Próximo passo ainda não registrado');
    assert.match(await page.locator('#record-empty-help').textContent(), /Comece pela conversa/);
    const draft = page.locator('#message-text');
    await draft.fill('Rascunho reservado durante o acesso');
    await page.locator('#send-message').click();
    try { await page.locator('#login-panel').waitFor({ state: 'visible', timeout: 30000 }); }
    catch (error) {
      console.error('Native signed-out diagnostic:', { agent: await page.locator('#agent-status').textContent(), project: await page.locator('#project-status').textContent(), connected: await page.locator('#disconnect-agent').isVisible(), draft: await draft.inputValue() });
      throw error;
    }
    assert.equal(await draft.inputValue(), 'Rascunho reservado durante o acesso');
    assert.equal(await page.locator('#messages article').count(), 0);
    await page.locator('#start-login').click();
    await page.locator('#login-code').filter({ hasText: /[A-Z0-9-]+/ }).waitFor({ timeout: 60000 });
    assert.equal(await page.locator('#start-login').isHidden(), true);
    assert.equal(await page.locator('#login-url').textContent(), 'https://auth.openai.com/codex/device');
    assert.equal(await page.locator('#copy-login-code').isVisible(), true);
    assert.equal(await page.evaluate(() => typeof navigator.clipboard?.writeText), 'function');
    if (process.env.FORGE_AUTH_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_AUTH_SCREENSHOT });
    if (fakeCompletion) {
      await page.locator('#login-panel').waitFor({ state: 'hidden', timeout: 30000 });
      await access(fakeMarker);
      assert.equal(await draft.inputValue(), 'Rascunho reservado durante o acesso');
      assert.equal(await page.locator('#messages article').count(), 0);
      assert.equal(await page.locator('#send-message').isEnabled(), true);
      await page.locator('#send-message').click();
      for (let attempt = 0; attempt < 100; attempt++) {
        try { await access(skillMarker); break; }
        catch { await new Promise(resolve => setTimeout(resolve, 100)); }
      }
      const params = JSON.parse(await readFile(skillMarker, 'utf8'));
      const skill = params.developerInstructions.match(/Start Forge guidance at `([^`]+)`/)?.[1];
      assert.ok(skill?.endsWith(path.join('forge-core', 'start-forge', 'SKILL.md')));
      const skillHash = createHash('sha256').update(await readFile(skill)).digest('hex');
      assert.equal(skillHash, '10581e17d5dbb98bda3e0f3bc0b6a152736499451e1424e093dbecfafd8f0b06');
      assert.ok(params.developerInstructions.includes('Do not use a separately installed Start Forge skill'));
      assert.ok(params.developerInstructions.includes('write the file, invoke Forge, and clean up in separate tool calls'));
      if (process.env.FORGE_EXPECT_BUNDLED_CORE_INSTRUCTIONS === '1') {
        const runtime = params.developerInstructions.match(/exact Forge executable for this project: `([^`]+)`/)?.[1];
        assert.equal(await realpath(runtime), await realpath(path.join(path.dirname(path.dirname(skill)), 'forge-core.exe')));
        await access(runtime);
        assert.ok(params.developerInstructions.includes('Do not select another copy from PATH'));
      }
      console.log('PASS: native fixture login preserves the draft; first send supplies the bundled Start Forge path to the Codex thread. Provider login and agent skill execution NOT_RUN.');
    } else {
      await page.locator('#finish-login').click();
      await page.locator('#login-status').filter({ hasText: 'Aguardando a confirmação' }).waitFor();
      await page.locator('#cancel-login').click();
      await page.locator('#login-status').filter({ hasText: 'Acesso cancelado' }).waitFor();
      assert.equal(await page.locator('#start-login').isVisible(), true);
      assert.equal(await page.locator('#login-code').textContent(), '');
      assert.equal(await draft.inputValue(), 'Rascunho reservado durante o acesso');
      assert.equal(await page.locator('#send-message').isDisabled(), true);
      console.log('PASS: native first-use signed-out state offered Codex device login, verified no early send, canceled without losing draft. Real login completion NOT_RUN.');
    }
  } finally {
    if (browser) await browser.close();
    child.kill();
    await new Promise(resolve => child.exitCode !== null ? resolve() : child.once('exit', resolve));
    await rm(profile, { recursive: true, force: true, maxRetries: 12, retryDelay: 250 }).catch(() => {});
  }
})().catch(error => { console.error(error); process.exitCode = 1; });
