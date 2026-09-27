// Independent browser check of the disposable same-chat change fixture.
// Opens only the local file in an isolated browser context; never sends Codex.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { pathToFileURL } = require('node:url');
const path = require('node:path');
const assert = require('node:assert/strict');

const project = process.env.FORGE_ARTIFACT_PROJECT;
if (!project) throw new Error('Set FORGE_ARTIFACT_PROJECT');
const file = path.join(project, 'site', 'index.html');

(async () => {
  const browser = await chromium.launch({ headless: true, executablePath: process.env.PLAYWRIGHT_EXECUTABLE_PATH || undefined });
  try {
    const context = await browser.newContext();
    const page = await context.newPage();
    const errors = [];
    const remote = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('request', request => { if (/^https?:/i.test(request.url())) remote.push(request.url()); });
    await page.goto(pathToFileURL(file).href);
    const idea = page.getByRole('textbox', { name: 'Sua ideia' });
    for (const text of ['Café com livros', 'Desenhar flores', 'Viagem ao mar']) {
      await idea.fill(text);
      await page.getByRole('button', { name: 'Salvar ideia' }).click();
    }
    assert.equal(await page.locator('#idea-list li').count(), 3);
    const saved = await page.evaluate(() => localStorage.getItem('jardim-de-ideias:v1'));
    await page.getByRole('searchbox', { name: 'Buscar nas ideias' }).fill('CAFE');
    assert.equal(await page.locator('#idea-list li').count(), 1);
    assert.match(await page.locator('#idea-list').textContent(), /Café com livros/);
    assert.equal(await page.locator('#idea-count').textContent(), '3');
    assert.equal(await page.locator('#search-status').textContent(), '1 de 3 ideias');
    assert.equal(await page.evaluate(() => localStorage.getItem('jardim-de-ideias:v1')), saved, 'Search must not mutate stored ideas');
    await page.getByRole('searchbox', { name: 'Buscar nas ideias' }).fill('inexistente');
    assert.equal(await page.locator('#idea-list li').count(), 0);
    assert.equal(await page.locator('#no-results').isVisible(), true);
    await page.getByRole('button', { name: 'Limpar busca' }).click();
    assert.equal(await page.locator('#idea-list li').count(), 3);
    assert.equal(await page.locator('#search-status').textContent(), '');
    await page.reload();
    assert.equal(await page.locator('#idea-list li').count(), 3);
    assert.equal(await page.evaluate(() => localStorage.getItem('jardim-de-ideias:v1')), saved, 'Reload must preserve the same stored ideas');
    assert.deepEqual(errors, []);
    assert.deepEqual(remote, []);
    console.log('PASS: isolated Chromium local-file search filters accent/case, restores all ideas, leaves storage untouched, and preserves ideas after reload; no page errors or remote requests.');
  } finally { await browser.close(); }
})().catch(error => { console.error(error); process.exitCode = 1; });
