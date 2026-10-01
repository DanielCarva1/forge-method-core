// Presentation-only browser checks; not native project or Codex proof.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile, mkdir } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { checkCinematicShell } = require('./cinematic-shell.cjs');
const root = path.resolve(__dirname, '../ui');
const config = require('../src-tauri/tauri.conf.json');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.png': 'image/png' };
const server = createServer(async (req, res) => {
  try {
    const pathname = new URL(req.url, 'http://localhost').pathname;
    const file = path.resolve(root, '.' + (pathname === '/' ? '/index.html' : pathname));
    if (!file.startsWith(root + path.sep)) throw new Error('Outside UI');
    res.setHeader('Content-Security-Policy', config.app.security.csp);
    res.setHeader('Content-Type', mime[path.extname(file)] || 'application/octet-stream');
    res.end(await readFile(file));
  } catch { res.writeHead(404).end(); }
});
(async () => {
  let browser;
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 1180, height: 820 } });
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    const output = process.env.FORGE_VISUAL_OUTPUT;
    if (output) await mkdir(output, { recursive: true });
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    if (process.env.FORGE_VISUAL_READING === '1') {
      await require('./reading-surfaces.cjs').checkReadingSurfaces(page, output);
      assert.deepEqual(errors, []);
      return;
    }
    await checkCinematicShell(page, output);
    await page.locator('.primary-nav [data-route="explore"]').click();
    await page.locator('#category-query').fill('música');
    assert.equal(await page.locator('.category-card:visible').count(), 1);
    await page.locator('.category-card:visible').click();
    assert.ok((await page.locator('#message-text').inputValue()).includes('música'));
    assert.equal(await page.locator('#messages article').count(), 0, 'Choosing a theme does not send');
    await page.locator('.primary-nav [data-route="projects"]').click();
    await page.locator('#projects-empty').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#projects-empty').isVisible(), true);
    assert.notEqual(await page.locator('.projects-actions .action-link:not(.primary)').evaluate(el => getComputedStyle(el).color), 'rgb(255, 246, 235)', 'Folder action uses readable surface text, not scene text');
    // At-a-glance card/filter presentation only; these are not real projects.
    await page.evaluate(() => localStorage.setItem('forge.projects.v1', JSON.stringify([
      { project_id: 'presentation-a', project_root: 'C:\\PresentationOnly\\Garden' },
      { project_id: 'presentation-b', project_root: 'C:\\PresentationOnly\\Music' },
    ])));
    await page.reload();
    assert.equal(await page.locator('.recent-project').count(), 2);
    await page.locator('#project-filter').fill('Garden');
    assert.equal(await page.locator('.recent-project').count(), 1);
    await page.locator('#project-filter').fill('no matching project');
    assert.equal(await page.locator('#projects-no-results').isVisible(), true);
    await page.evaluate(() => { document.documentElement.style.fontSize = '32px'; });
    await page.setViewportSize({ width: 360, height: 820 });
    for (const route of ['home', 'explore', 'projects', 'workspace']) {
      await page.locator(`.primary-nav [data-route="${route}"]`).click();
      await page.locator(`#${route}`).waitFor({ state: 'visible' });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1), `${route}: enlarged text remains on screen`);
    }
    assert.deepEqual(errors, []);
    console.log('PASS: production CSP, category search/draft without Send, empty/recent/filter states (presentation fixture only).');
  } finally { await browser?.close(); await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error); process.exitCode = 1; });
