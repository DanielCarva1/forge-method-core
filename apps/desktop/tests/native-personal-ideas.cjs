// One-shot real continuation of the disposable personal-ideas project.
// A timeout is uncertain delivery: inspect the thread and files, never resend.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { spawn, execFile } = require('node:child_process');
const { promisify } = require('node:util');
const { createServer } = require('node:net');
const { readFile, stat } = require('node:fs/promises');
const { createHash } = require('node:crypto');
const path = require('node:path');
const assert = require('node:assert/strict');
const { waitForTurn } = require('./turn-observer.cjs');

const execute = promisify(execFile);
const profile = process.env.FORGE_ARTIFACT_PROFILE;
const project = process.env.FORGE_ARTIFACT_PROJECT;
const executable = process.env.FORGE_DESKTOP_EXE;
const core = process.env.FORGE_BUNDLED_CORE_EXE;
if (!profile || !project || !executable || !core) throw new Error('Set existing profile, project, installed app and bundled core');
const html = path.join(project, 'site', 'index.html');
const continueWork = process.env.FORGE_PERSONAL_CONTINUE === '1';
const finishWork = process.env.FORGE_PERSONAL_FINISH === '1';
const changeWork = process.env.FORGE_PERSONAL_CHANGE === '1';
if ([continueWork, finishWork, changeWork].filter(Boolean).length > 1) throw new Error('Choose only one one-shot mode');
const initialPrompt = 'Como pessoa testando este projeto: confirmo que a primeira versão será de uso pessoal, sem conta nem compartilhamento. Para guardar minhas ideias entre visitas, escolho salvar somente neste navegador e dispositivo; entendo que limpar os dados do navegador pode apagá-las. Quero escrever uma ideia, salvá-la, vê-la numa lista e poder removê-la. A etapa anterior era apenas de planejamento; agora autorizo implementar essa primeira versão funcional somente nesta pasta e atualizar o trabalho no Forge conforme necessário. Verifique o funcionamento em navegador apropriado, pois sei que a prévia protegida do Forge não executa JavaScript. Não publique nem altere outros projetos. Mostre o arquivo final na conversa. Se faltar uma decisão indispensável, faça uma pergunta antes de implementar.';
const continuationPrompt = 'O trabalho ativo no Forge já foi atualizado para implementar a versão pessoal. Sua resposta anterior foi interrompida depois desse registro e antes de editar arquivos. Continue exatamente dali: implemente nesta pasta o formulário, a lista e o armazenamento local no navegador que eu já escolhi; teste criação, retorno e remoção em navegador com JavaScript. Não repita minha decisão, não refaça o trabalho já registrado, não publique e não altere outros projetos. Mostre o arquivo final na conversa. Se houver bloqueio real, explique-o em vez de afirmar que terminou.';
const finishPrompt = 'A página funcional já foi implementada e testada nesta conversa, mas a resposta anterior foi interrompida enquanto você preparava o registro do resultado no Forge. Confira o que existe e os testes já executados, sem refazer a página nem repetir minha decisão. Conclua o registro do trabalho ativo no Forge deste projeto com a evidência que você pode sustentar; depois responda em português simples com o link do arquivo final. Não publique. Se houver um problema real, explique-o claramente em vez de afirmar que concluiu.';
const changePrompt = 'Quero fazer uma mudança na página que acabamos de concluir: quando minha lista de ideias crescer, quero poder encontrar uma ideia digitando uma palavra. Acrescente uma busca local que filtre a lista pelo texto, sem alterar nem apagar as ideias salvas e sem criar conta ou compartilhar dados. É um teste controlado somente nesta pasta descartável. Continue nesta mesma conversa, registre esta mudança como novo trabalho no Forge sem substituir a direção principal, implemente e verifique em navegador com JavaScript a busca, a volta à lista completa e a persistência das ideias após recarregar. Não publique. Ao terminar, explique em português simples e mostre o arquivo final. Se houver bloqueio real, explique em vez de afirmar que concluiu.';
const prompt = changeWork ? changePrompt : finishWork ? finishPrompt : continueWork ? continuationPrompt : initialPrompt;
const marker = 'Para guardar minhas ideias entre visitas, escolho salvar somente neste navegador e dispositivo';
const hash = bytes => createHash('sha256').update(bytes).digest('hex');

async function forgeState() {
  const { stdout } = await execute(core, ['workflow', 'resume', '--root', project, '--json'], { timeout: 90000, maxBuffer: 8 * 1024 * 1024 });
  return JSON.parse(stdout).data;
}

async function launch() {
  const server = createServer();
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  const port = server.address().port;
  await new Promise(resolve => server.close(resolve));
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
  await page.getByRole('button', { name: 'Conectar ao Codex', exact: true }).click();
  await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 110000 });
  await page.locator('#messages article[data-role="agent"]').nth(1).waitFor({ timeout: 60000 });
}

async function inspectTurn(page) {
  const status = (await page.locator('#agent-status').textContent()) || '';
  const messageCount = await page.locator('#messages article').count();
  const latestText = messageCount ? await page.locator('#messages article').last().textContent() : '';
  const files = await Promise.all([html, path.join(project, 'site', 'assets', 'site.css'), path.join(project, 'site', 'assets', 'site.js')]
    .map(async file => {
      try { const info = await stat(file); return [info.size, info.mtimeMs]; }
      catch { return null; }
    }));
  return {
    status,
    completed: status.includes('Resposta recebida'),
    failed: /Interrompido|A execução falhou|A conexão foi encerrada/.test(status),
    fingerprint: JSON.stringify([messageCount, latestText, files]),
  };
}

(async () => {
  const before = await forgeState();
  assert.equal(before.current_work.status, changeWork ? 'completed' : 'current');
  assert.equal(before.current_work.focus.focus_id, continueWork || finishWork || changeWork ? 'focus.jardim-ideias-v1-funcional' : 'focus.jardim-ideias-v1-pessoal-planejamento');
  assert.match(before.active_objective.proposal.outcome, /uso pessoal/);
  const initialHtml = await readFile(html);
  const scriptFile = path.join(project, 'site', 'assets', 'site.js');
  const initialScript = changeWork ? await readFile(scriptFile) : null;
  if (changeWork) {
    assert.equal(before.current_work.focus.record_digest, 'sha256:f43e52e3b1213f2ee3e5b4ae7f3b0927373158e240fa30727cbbfde435719509', 'The completed Forge record changed; inspect it before a new request');
    assert.equal(hash(initialHtml).toUpperCase(), '2B580FEAB7A754ECDFBD239AD5D5CFD48F83FA4F2DB30CAEEF7E5930F3E9B120', 'HTML fixture changed before the request');
    assert.equal(hash(initialScript).toUpperCase(), '358A78547E688CDE64831DAA0263483F0ABEF9D3F76C26A3454E3CF6EE566643', 'JavaScript fixture changed before the request');
  }
  if (finishWork || changeWork) assert.match(initialHtml.toString(), /<form\b/, 'Expected the existing implementation before the next message');
  else assert.doesNotMatch(initialHtml.toString(), /<form\b|localStorage/, 'Fixture is already implemented; refuse to duplicate the request');
  console.log(`BEFORE_HTML_SHA256=${hash(initialHtml)}`);
  if (initialScript) console.log(`BEFORE_JS_SHA256=${hash(initialScript)}`);
  let app;
  try {
    app = await launch();
    await resume(app.page);
    const page = app.page;
    const usersBefore = await page.locator('#messages article[data-role="user"]').count();
    const messagesBefore = await page.locator('#messages article').count();
    const userHistory = (await page.locator('#messages article[data-role="user"]').allInnerTexts()).join('\n');
    assert.equal(userHistory.includes(marker), continueWork || finishWork || changeWork, 'Previous decision-turn state does not match this one-shot mode');
    assert.equal(userHistory.includes(continuationPrompt.slice(0, 80)), finishWork || changeWork, 'Continuation history does not match this one-shot mode');
    assert.equal(userHistory.includes(finishPrompt.slice(0, 80)), changeWork, 'Finalization history does not match this one-shot mode');
    assert.equal(userHistory.includes(changePrompt.slice(0, 80)), false, 'Change request already sent; do not retry');
    // The old result may already be open, or its shortcut may be hidden by the
    // current preview state. That read-only path has a separate native test;
    // do not let it prevent this one-shot continuation from reaching Send.
    const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    await composer.fill(prompt);
    console.log(`BEFORE_MESSAGES=${messagesBefore} BEFORE_USERS=${usersBefore}`);
    console.log(`SEND_ONCE_AT=${new Date().toISOString()}`);
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await waitForTurn({
      inspect: () => inspectTurn(page),
      sleep: ms => new Promise(resolve => setTimeout(resolve, ms)),
      onProgress: state => console.log(`TURN_PROGRESS_AT=${new Date().toISOString()} STATUS=${state.status} FINGERPRINT_BYTES=${state.fingerprint.length}`),
    });
    console.log(`REPLY_AT=${new Date().toISOString()}`);
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), usersBefore + 1);
    const reply = await page.locator('#messages article[data-role="agent"]').last().innerText();
    console.log(`AGENT_REPLY=${reply.slice(0, 3000)}`);
    const updatedHtml = await readFile(html);
    console.log(`AFTER_HTML_SHA256=${hash(updatedHtml)}`);
    const updatedScript = changeWork ? await readFile(scriptFile) : null;
    if (updatedScript) console.log(`AFTER_JS_SHA256=${hash(updatedScript)}`);
    const afterWork = (await forgeState()).current_work;
    console.log(`AFTER_FORGE_WORK=${JSON.stringify(afterWork)}`);
    console.log(`PREVIEW_STATUS=${await page.locator('#preview-status').textContent()}`);
    console.log(`PREVIEW_VISIBLE=${await page.locator('#preview-result').isVisible()}`);
    console.log(`AFTER_MESSAGES=${await page.locator('#messages article').count()}`);
    if (changeWork) {
      assert.ok(hash(updatedHtml) !== hash(initialHtml) || hash(updatedScript) !== hash(initialScript), 'The real change request returned without changing the page or its behavior');
      assert.notEqual(afterWork.focus?.record_digest, before.current_work.focus.record_digest, 'The new change must update authoritative Forge work');
      assert.notEqual(afterWork.focus?.focus_id, before.current_work.focus.focus_id, 'The completed Work Focus must not be silently reused for new work');
      assert.equal(await page.locator('#messages article[data-role="agent"]').last().locator('.message-file-link[data-preview-path="site/index.html"]').isVisible(), true, 'The changed result must have an actionable file link');
      console.log('PASS: a new same-chat change updated the project and Forge Work Focus. Functional behavior still needs independent browser verification and native restart.');
    } else if (finishWork) {
      assert.notEqual(afterWork.focus?.record_digest, before.current_work.focus.record_digest, 'The agent replied without updating the authoritative Work Focus');
      assert.equal(await page.locator('#messages article[data-role="agent"]').last().locator('.message-file-link[data-preview-path="site/index.html"]').isVisible(), true, 'The final reply must render an actionable link to the existing project result');
      console.log('PASS: the real app turn finished, updated the existing Forge Work Focus and linked the result in the same conversation. Independently recheck any changed files.');
    } else {
      if (hash(updatedHtml) === hash(initialHtml)) throw new Error('No changed HTML after the real request; inspect agent reply and project before any new send');
      console.log('PASS: one real app turn changed the personal-ideas project and returned in the same conversation. Functional testing still requires an external browser check.');
    }
  } finally { await stop(app); }
})().catch(error => { console.error(error); process.exitCode = 1; });
