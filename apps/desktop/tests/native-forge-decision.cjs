// One-shot real Codex decision on an existing disposable project.
// Never retries Send: inspect the thread and Forge record after any uncertainty.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn, execFile } = require('node:child_process');
const { promisify } = require('node:util');
const { createServer } = require('node:net');
const { readFile } = require('node:fs/promises');
const { createHash } = require('node:crypto');
const path = require('node:path');
const assert = require('node:assert/strict');

const execute = promisify(execFile);
const profile = process.env.FORGE_ARTIFACT_PROFILE;
const project = process.env.FORGE_ARTIFACT_PROJECT;
const executable = process.env.FORGE_DESKTOP_EXE;
const core = process.env.FORGE_BUNDLED_CORE_EXE;
if (!profile || !project || !executable || !core) throw new Error('Set the existing artifact profile, project, installed desktop executable, and bundled core executable');
const html = path.join(project, 'site', 'index.html');
const css = path.join(project, 'site', 'assets', 'site.css');
const script = path.join(project, 'site', 'assets', 'site.js');
const prompt = 'Decidi que o Jardim de ideias é para uso pessoal: uma pessoa anota e organiza as próprias ideias, sem conta nem compartilhamento nesta primeira versão. Quero melhorar a página para esse público. Registre essa direção e o próximo trabalho no Forge deste projeto, usando o Forge que veio com o aplicativo. Por enquanto não altere os arquivos nem publique nada; explique em linguagem simples o que ficou combinado e o primeiro passo.';
const continuation = 'Você já registrou minha decisão de uso pessoal no Forge. Continue de onde parou, sem repetir nem substituir essa direção: registre o próximo trabalho no Forge para planejar a primeira versão pessoal, com um objetivo e próximo passo simples. Ainda não altere arquivos nem publique nada. Se uma escolha de produto for indispensável, explique-a em português claro e faça apenas uma pergunta.';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');

async function resumeCore() {
  const { stdout } = await execute(core, ['workflow', 'resume', '--root', project, '--json'], { timeout: 90000, maxBuffer: 8 * 1024 * 1024 });
  return JSON.parse(stdout).data;
}

async function launch() {
  const reservation = createServer();
  await new Promise(resolve => reservation.listen(0, '127.0.0.1', resolve));
  const port = reservation.address().port;
  await new Promise(resolve => reservation.close(resolve));
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
    const exited = new Promise(resolve => app.child.once('exit', resolve));
    app.child.kill();
    await exited;
  }
}

async function openExisting(page) {
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
  const before = await resumeCore();
  const readOnly = process.env.FORGE_DECISION_READ_ONLY === '1';
  const continueDecision = process.env.FORGE_DECISION_CONTINUE === '1';
  if (!readOnly) {
    assert.equal(before.current_work.status, 'absent', 'Refuse to duplicate an already accepted Work Focus');
    assert.equal(!!before.active_objective, continueDecision, 'The accepted product direction does not match this one-shot test mode');
  }
  const htmlHash = hash(await readFile(html));
  const cssHash = hash(await readFile(css));
  const scriptHash = process.env.FORGE_EXPECT_SEARCH_COMPLETE === '1' ? hash(await readFile(script)) : null;
  let app;
  try {
    app = await launch();
    if (readOnly) await app.page.evaluate(() => {
      window.readOnlySendCount = 0;
      const native = window.__TAURI__.core;
      const facade = Object.create(native);
      Object.defineProperty(facade, 'invoke', { value: (command, args) => {
        if (command === 'send_message') window.readOnlySendCount++;
        return native.invoke(command, args);
      } });
      window.__TAURI__.core = facade;
    });
    await openExisting(app.page);
    const page = app.page;
    if (readOnly) {
      if (process.env.FORGE_EXPECT_INTERRUPTED === '1') {
        assert.match(await page.locator('#agent-status').textContent(), /última resposta foi interrompida.*Confira a conversa e os arquivos.*Nada foi reenviado/);
      }
      await page.getByRole('button', { name: 'Atualizar andamento' }).click();
      await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor({ timeout: 90000 });
      console.log(`CORE_DIRECTION=${JSON.stringify(before.active_objective?.proposal || null)}`);
      console.log(`CORE_CURRENT_WORK=${JSON.stringify(before.current_work)}`);
      console.log(`UI_RECORD_STATE=${await page.locator('#record-state').textContent()}`);
      console.log(`UI_DIRECTION_VISIBLE=${await page.locator('#record-direction-card').isVisible()}`);
      console.log(`UI_DIRECTION_OUTCOME=${await page.locator('#record-direction-outcome').textContent()}`);
      console.log(`UI_WORK_VISIBLE=${await page.locator('#record-work').isVisible()}`);
      assert.equal(await page.locator('#record-direction-card').isVisible(), !!before.active_objective, 'Native UI direction must match Forge authority');
      assert.equal(await page.locator('#record-work').isVisible(), before.current_work.status !== 'absent', 'Native UI Work Focus must match Forge authority');
      if (before.active_objective) assert.equal(await page.locator('#record-direction-outcome').textContent(), before.active_objective.proposal.outcome);
      if (before.current_work.status === 'current' || before.current_work.status === 'completed') {
        assert.equal(await page.locator('#record-title').textContent(), before.current_work.focus.title);
        assert.equal(await page.locator('#record-outcome').textContent(), before.current_work.focus.intended_outcome);
        assert.equal(await page.locator('#record-next').textContent(), before.current_work.focus.next_step);
      }
      assert.equal(await page.evaluate(() => window.readOnlySendCount), 0, 'Restart and record readback must not send a turn');
      assert.equal(hash(await readFile(html)), htmlHash);
      assert.equal(hash(await readFile(css)), cssHash);
      if (scriptHash) assert.equal(hash(await readFile(script)), scriptHash);
      if (process.env.FORGE_EXPECT_DIRECTION_COPY === '1') {
        assert.equal(await page.locator('#record-state').textContent(), 'Direção registrada; próximo trabalho pendente');
        assert.match(await page.locator('#record-empty-help').textContent(), /O objetivo está registrado.*próximo trabalho ainda não foi definido/);
      }
      if (process.env.FORGE_EXPECT_PERSONAL_COMPLETE === '1') {
        assert.equal(before.current_work.status, 'completed', 'Forge must report the exact accepted Work Focus as completed');
        if (process.env.FORGE_EXPECT_SEARCH_COMPLETE === '1') {
          assert.equal(before.current_work.focus.focus_id, 'focus.jardim-ideias-busca-local');
          assert.equal(await page.locator('#record-title').textContent(), 'Adicionar busca local às ideias');
        }
        assert.match(await page.locator('#record-state').textContent(), /conclu[ií]do/i);
        const finalFile = page.locator('#messages article[data-role="agent"]').last().locator('.message-file-link[data-preview-path="site/index.html"]');
        assert.equal(await finalFile.isVisible(), true, 'The final answer must render the local file as an actionable button');
        await finalFile.click();
        await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 30000 });
        assert.equal(await page.locator('#preview-path').textContent(), 'site\\index.html');
        await page.frameLocator('#preview-site').getByRole('heading', { name: 'Jardim de ideias renovado', exact: true }).waitFor({ timeout: 30000 });
        if (process.env.FORGE_EXPECT_SEARCH_COMPLETE === '1') {
          await page.frameLocator('#preview-site').getByRole('searchbox', { name: 'Buscar nas ideias' }).waitFor({ timeout: 30000 });
        }
        if (process.env.FORGE_EXPECT_BROWSER_ACTION === '1') {
          assert.equal(await page.getByRole('button', { name: 'Usar no navegador' }).isVisible(), true, 'A functional HTML result must offer an explicit usable-browser action');
          await assert.rejects(page.evaluate(async ({ projectRoot, filePath }) => window.__TAURI__.core.invoke('open_site_in_browser', { projectRoot, filePath }), {
            projectRoot: project,
            filePath: path.join(project, 'site', 'missing.html'),
          }), /Esta página não está mais disponível/, 'Native command must reject an unavailable file without opening the browser');
        }
        assert.equal(await page.evaluate(() => window.readOnlySendCount), 0, 'Opening the final result must not send another turn');
        if (scriptHash) assert.equal(hash(await readFile(script)), scriptHash);
      }
      if (process.env.FORGE_EXPECT_PRIOR_RESULT === '1') {
        const resultAlreadyOpen = await page.locator('#preview-result').isVisible();
        if (!resultAlreadyOpen) {
          assert.equal(await page.locator('#preview-last-result').isVisible(), true, 'A closed earlier result must remain available after planning or interruption');
          assert.match(await page.locator('#preview-intro').textContent(), /resultado anterior tem um arquivo/);
          await page.getByRole('button', { name: 'Conferir arquivo citado' }).click();
        }
        await page.locator('#preview-status').filter({ hasText: 'Prévia local atualizada' }).waitFor({ timeout: 30000 });
        assert.equal(await page.locator('#preview-path').textContent(), 'site\\index.html');
        await page.frameLocator('#preview-site').getByRole('heading', { name: 'Jardim de ideias renovado', exact: true }).waitFor({ timeout: 30000 });
        console.log(`PREVIEW_VISIBLE=${await page.locator('#preview-result').isVisible()} WORKSPACE_CLASS=${await page.locator('.workspace').getAttribute('class')}`);
        assert.equal(await page.locator('#preview-result').isVisible(), true, 'The recovered result must be visible, not just loaded in a hidden iframe');
      }
      if (process.env.FORGE_DECISION_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_DECISION_SCREENSHOT, fullPage: true });
      console.log('PASS: native read-only restart reflects the exact Forge direction and Work Focus without sending or changing files.');
      return;
    }
    const usersBefore = await page.locator('#messages article[data-role="user"]').count();
    const agentsBefore = await page.locator('#messages article[data-role="agent"]').count();
    assert.doesNotMatch((await page.locator('#messages article[data-role="user"]').allInnerTexts()).join('\n'), continueDecision ? /Você já registrou minha decisão de uso pessoal/ : /Decidi que o Jardim de ideias é para uso pessoal/, 'Refuse to resend an existing decision or continuation');
    const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    await composer.fill(continueDecision ? continuation : prompt);
    console.log(`BEFORE_COUNTS=${usersBefore} user, ${agentsBefore} agent; BEFORE_FOCUS=${before.current_work.status}`);
    console.log(continueDecision ? 'SEND_ONCE: continue the already accepted product direction without repeating it.' : 'SEND_ONCE: approved product direction in the existing disposable conversation.');
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: continueDecision ? 600000 : 360000 });
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), usersBefore + 1);
    assert.ok((await page.locator('#messages article[data-role="agent"]').count()) > agentsBefore);
    const reply = await page.locator('#messages article[data-role="agent"]').last().innerText();
    console.log(`AGENT_REPLY=${reply.slice(0, 2500)}`);
    assert.equal(hash(await readFile(html)), htmlHash, 'HTML must remain unchanged');
    assert.equal(hash(await readFile(css)), cssHash, 'CSS must remain unchanged');
    const after = await resumeCore();
    console.log(`CORE_CURRENT_WORK=${JSON.stringify(after.current_work)}`);
    console.log(`CORE_PHASE=${after.current_phase}`);
    await page.getByRole('button', { name: 'Atualizar andamento' }).click();
    await page.locator('#progress-status').filter({ hasText: 'Consultado às' }).waitFor({ timeout: 90000 });
    console.log(`UI_RECORD_STATE=${await page.locator('#record-state').textContent()}`);
    console.log(`UI_RECORD_TITLE=${await page.locator('#record-title').textContent()}`);
    console.log(`UI_RECORD_OUTCOME=${await page.locator('#record-outcome').textContent()}`);
    console.log(`UI_RECORD_NEXT=${await page.locator('#record-next').textContent()}`);
    assert.equal(after.current_work.status, 'current', 'Real agent did not establish the accepted Forge Work Focus');
    assert.equal(await page.locator('#record-work').isVisible(), true, 'Native UI must show the accepted Forge record');
    console.log('PASS: real agent established the Forge Work Focus and native UI rendered the authoritative record.');
  } finally { await stop(app); }
})().catch(error => { console.error(error); process.exitCode = 1; });
