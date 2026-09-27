// One-shot real continuation of the disposable personal-ideas project.
// A timeout is uncertain delivery: inspect the thread and files, never resend.
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
if (!profile || !project || !executable || !core) throw new Error('Set existing profile, project, installed app and bundled core');
const html = path.join(project, 'site', 'index.html');
const continueWork = process.env.FORGE_PERSONAL_CONTINUE === '1';
const initialPrompt = 'Como pessoa testando este projeto: confirmo que a primeira versão será de uso pessoal, sem conta nem compartilhamento. Para guardar minhas ideias entre visitas, escolho salvar somente neste navegador e dispositivo; entendo que limpar os dados do navegador pode apagá-las. Quero escrever uma ideia, salvá-la, vê-la numa lista e poder removê-la. A etapa anterior era apenas de planejamento; agora autorizo implementar essa primeira versão funcional somente nesta pasta e atualizar o trabalho no Forge conforme necessário. Verifique o funcionamento em navegador apropriado, pois sei que a prévia protegida do Forge não executa JavaScript. Não publique nem altere outros projetos. Mostre o arquivo final na conversa. Se faltar uma decisão indispensável, faça uma pergunta antes de implementar.';
const continuationPrompt = 'O trabalho ativo no Forge já foi atualizado para implementar a versão pessoal. Sua resposta anterior foi interrompida depois desse registro e antes de editar arquivos. Continue exatamente dali: implemente nesta pasta o formulário, a lista e o armazenamento local no navegador que eu já escolhi; teste criação, retorno e remoção em navegador com JavaScript. Não repita minha decisão, não refaça o trabalho já registrado, não publique e não altere outros projetos. Mostre o arquivo final na conversa. Se houver bloqueio real, explique-o em vez de afirmar que terminou.';
const prompt = continueWork ? continuationPrompt : initialPrompt;
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
  await page.getByRole('button', { name: 'Abrir conversa', exact: true }).click();
  await page.locator('#agent-status').filter({ hasText: 'Conversa retomada' }).waitFor({ timeout: 110000 });
  await page.locator('#messages article[data-role="agent"]').nth(1).waitFor({ timeout: 60000 });
}

(async () => {
  const before = await forgeState();
  assert.equal(before.current_work.status, 'current');
  assert.equal(before.current_work.focus.focus_id, continueWork ? 'focus.jardim-ideias-v1-funcional' : 'focus.jardim-ideias-v1-pessoal-planejamento');
  assert.match(before.active_objective.proposal.outcome, /uso pessoal/);
  const initialHtml = await readFile(html);
  assert.doesNotMatch(initialHtml.toString(), /<form\b|localStorage/, 'Fixture is already implemented; refuse to duplicate the request');
  console.log(`BEFORE_HTML_SHA256=${hash(initialHtml)}`);
  let app;
  try {
    app = await launch();
    await resume(app.page);
    const page = app.page;
    const usersBefore = await page.locator('#messages article[data-role="user"]').count();
    const messagesBefore = await page.locator('#messages article').count();
    const userHistory = (await page.locator('#messages article[data-role="user"]').allInnerTexts()).join('\n');
    assert.equal(userHistory.includes(marker), continueWork, 'Previous decision-turn state does not match this one-shot mode');
    assert.equal(userHistory.includes(continuationPrompt.slice(0, 80)), false, 'Continuation already sent; do not retry');
    // The old result may already be open, or its shortcut may be hidden by the
    // current preview state. That read-only path has a separate native test;
    // do not let it prevent this one-shot continuation from reaching Send.
    const composer = page.getByRole('textbox', { name: 'Sua ideia começa aqui' });
    await composer.fill(prompt);
    console.log(`BEFORE_MESSAGES=${messagesBefore} BEFORE_USERS=${usersBefore}`);
    console.log(`SEND_ONCE_AT=${new Date().toISOString()}`);
    await page.getByRole('button', { name: 'Enviar', exact: true }).click();
    await page.locator('#agent-status').filter({ hasText: 'Resposta recebida' }).waitFor({ timeout: 600000 });
    console.log(`REPLY_AT=${new Date().toISOString()}`);
    assert.equal(await page.locator('#messages article[data-role="user"]').count(), usersBefore + 1);
    const reply = await page.locator('#messages article[data-role="agent"]').last().innerText();
    console.log(`AGENT_REPLY=${reply.slice(0, 3000)}`);
    const updatedHtml = await readFile(html);
    console.log(`AFTER_HTML_SHA256=${hash(updatedHtml)}`);
    console.log(`AFTER_FORGE_WORK=${JSON.stringify((await forgeState()).current_work)}`);
    console.log(`PREVIEW_STATUS=${await page.locator('#preview-status').textContent()}`);
    console.log(`PREVIEW_VISIBLE=${await page.locator('#preview-result').isVisible()}`);
    console.log(`AFTER_MESSAGES=${await page.locator('#messages article').count()}`);
    if (hash(updatedHtml) === hash(initialHtml)) throw new Error('No changed HTML after the real request; inspect agent reply and project before any new send');
    console.log('PASS: one real app turn changed the personal-ideas project and returned in the same conversation. Functional testing still requires an external browser check.');
  } finally { await stop(app); }
})().catch(error => { console.error(error); process.exitCode = 1; });
