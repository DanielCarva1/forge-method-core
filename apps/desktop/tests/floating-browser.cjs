// Presentation-only fixture using the real UI modules, not native/Core proof.
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { createServer } = require('node:http');
const { readFile } = require('node:fs/promises');
const path = require('node:path');
const assert = require('node:assert/strict');
const { checkFloatingWorkspace } = require('./floating-workspace.cjs');
const root = path.resolve(__dirname, '../ui');
const config = require('../src-tauri/tauri.conf.json');
const mime = { '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript', '.css': 'text/css', '.png': 'image/png' };
const server = createServer(async (req, res) => {
  try {
    const file = path.resolve(root, '.' + (new URL(req.url, 'http://localhost').pathname === '/' ? '/index.html' : new URL(req.url, 'http://localhost').pathname));
    if (!file.startsWith(root + path.sep)) throw new Error('Outside UI');
    res.setHeader('Content-Security-Policy', config.app.security.csp);
    res.setHeader('Content-Type', mime[path.extname(file)] || 'application/octet-stream');
    res.end(await readFile(file));
  } catch { res.writeHead(404).end(); }
});
async function presentationProject(page) {
  await page.evaluate(async () => {
    document.querySelector('.workspace').classList.add('project-ready');
    for (const id of ['project-preview', 'project-record']) document.getElementById(id).hidden = false;
    // Realistic closed history/permission disclosures, not fabricated messages.
    for (const id of ['conversation-picker', 'conversation-options', 'agent-access-note']) document.getElementById(id).hidden = false;
    const module = await import('./mobile-workspace.mjs');
    module.setMobileWorkspaceProject({ project_id: 'presentation-only' });
  });
}
(async () => {
  let browser;
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  try {
    browser = await chromium.launch({ headless: true });
    const page = await browser.newPage({ viewport: { width: 1180, height: 820 } });
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    await page.goto(`http://127.0.0.1:${server.address().port}/#workspace`);
    await presentationProject(page);
    await checkFloatingWorkspace(page, process.env.FORGE_FLOAT_SCREENSHOT);
    // Reload actual modules: geometry survives while a fresh project confirms itself.
    await page.locator('#project-conversation [data-window-action="right"]').click();
    const saved = await page.evaluate(() => localStorage.getItem('forge.workspace-layout.v1'));
    await page.reload(); await presentationProject(page);
    assert.equal(await page.evaluate(() => localStorage.getItem('forge.workspace-layout.v1')), saved);
    const panel = await page.locator('#project-conversation').boundingBox();
    const stage = await page.locator('.workspace').boundingBox();
    assert.ok(panel.x > stage.x + stage.width / 2);
    // Warm text stays readable even over the brightest/darkest scene pixel.
    for (const theme of ['light', 'dark']) {
      const worst = await page.evaluate(theme => {
        document.documentElement.dataset.theme = theme;
        const surface = getComputedStyle(document.getElementById('project-conversation'));
        const rgb = value => value.match(/[\d.]+/g).map(Number);
        const background = rgb(surface.backgroundColor), alpha = background[3] ?? 1;
        const luminance = channels => channels.slice(0, 3).map(v => v / 255).map(v => v <= .04045 ? v / 12.92 : ((v + .055) / 1.055) ** 2.4).reduce((sum, v, i) => sum + v * [.2126, .7152, .0722][i], 0);
        const colors = [surface.color, getComputedStyle(document.querySelector('.window-caption')).color];
        return Math.min(...colors.flatMap(color => [0, 255].map(scene => {
          const foreground = luminance(rgb(color));
          const backdrop = luminance(background.slice(0, 3).map(v => alpha * v + (1 - alpha) * scene));
          return (Math.max(foreground, backdrop) + .05) / (Math.min(foreground, backdrop) + .05);
        })));
      }, theme);
      assert.ok(worst >= 4.5, `${theme} primary/supporting panel text must retain >=4.5:1 (actual ${worst})`);
      console.log(`PASS: ${theme} primary/supporting panel text worst-case scene contrast ${worst.toFixed(2)}:1; not full-screen compliance.`);
    }
    if (process.env.FORGE_FLOAT_SCREENSHOT) await page.screenshot({ path: process.env.FORGE_FLOAT_SCREENSHOT.replace('.png', '-night.png') });
    await page.evaluate(() => { document.documentElement.style.fontSize = '32px'; });
    await page.waitForFunction(() => !document.querySelector('.workspace').classList.contains('floating-workspace'));
    assert.equal(await page.locator('#project-conversation .window-chrome').isVisible(), false);
    assert.deepEqual(errors, []);
    console.log('PASS: CSP, persisted geometry after reload, large-text fallback; presentation fixture only.');
  } finally { await browser?.close(); await new Promise(resolve => server.close(resolve)); }
})().catch(error => { console.error(error); process.exitCode = 1; });
