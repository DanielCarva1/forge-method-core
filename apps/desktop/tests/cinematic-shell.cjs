const assert = require('node:assert/strict');
const path = require('node:path');

// The same checks run against browser presentation and the installed WebView.
async function checkCinematicShell(page, output) {
  await page.locator('.primary-nav').waitFor({ state: 'visible' });
  // Hidden routes may not fetch their CSS artwork until first shown. Require
  // the real packaged images to decode before taking visual evidence.
  await page.evaluate(async () => {
    await Promise.all(['forge.png', 'explore-artwork.png', 'cinematic-atelier.png'].map(async file => {
      const image = new Image(); image.src = `assets/${file}`; await image.decode();
    }));
  });
  const nav = page.locator('.primary-nav');
  await page.locator('.appearance summary').click();
  await page.locator('#appearance-theme').selectOption('light');
  await page.locator('#appearance-theme').press('Escape');
  assert.equal(await page.locator('.appearance').getAttribute('open'), null);
  for (const route of ['home', 'explore', 'projects', 'workspace']) {
    await nav.locator(`[data-route="${route}"]`).click();
    await page.locator(`#${route}`).waitFor({ state: 'visible' });
    await page.waitForFunction(route => document.activeElement === document.querySelector(`#${route} h1`), route);
    assert.equal(await page.locator('[data-screen]:visible').count(), 1);
    assert.equal(await nav.locator('[aria-current="page"]').getAttribute('data-route'), route);
    assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1), `${route}: no horizontal page overflow`);
    await page.mouse.move(1150, 800);
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    if (output) await page.screenshot({ path: path.join(output, `${route}.png`) });
  }
  const explore = nav.locator('[data-route="explore"]');
  await page.keyboard.press("Tab");
  await explore.focus();
  assert.ok((await explore.evaluate(el => getComputedStyle(el, '::after').content)).includes('Explorar'));
  await explore.press('Escape');
  assert.equal(await explore.evaluate(el => getComputedStyle(el, '::after').display), 'none');
  await page.locator('.appearance summary').click();
  await page.locator('#appearance-theme').selectOption('dark');
  await nav.locator('[data-route="home"]').click();
  await page.locator('#home').waitFor({ state: 'visible' });
  await page.mouse.move(1150, 800);
  assert.equal(await page.locator('.appearance').getAttribute('open'), null, 'Outside click closes comfort menu');
  if (output) await page.screenshot({ path: path.join(output, 'home-night.png') });
  for (const width of [700, 360]) {
    await page.setViewportSize({ width, height: 820 });
    for (const route of ['home', 'explore', 'projects', 'workspace']) {
      await nav.locator(`[data-route="${route}"]`).click();
      await page.locator(`#${route}`).waitFor({ state: "visible" });
      assert.ok(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1), `${width}/${route}: no horizontal page overflow`);
      assert.equal(await nav.locator(`[data-route="${route}"]`).isVisible(), true);
    }
    await page.locator('.appearance summary').click();
    const menu = await page.locator('.appearance-popover').boundingBox();
    assert.ok(menu.x >= 0 && menu.x + menu.width <= width, 'Comfort menu stays on screen');
    await page.locator('#appearance-contrast').check();
    assert.ok(await page.locator('html').getAttribute('data-high-contrast') !== null);
    await page.locator('#appearance-contrast').uncheck();
    await page.locator('#appearance-theme').press('Escape');
  }
  await page.setViewportSize({ width: 1180, height: 820 });
  await page.locator('.appearance summary').click();
  await page.locator('#appearance-theme').selectOption('light');
  await page.locator('#appearance-theme').press('Escape');
  await nav.locator('[data-route="workspace"]').click();
  console.log('PASS: four cinematic routes, focus/active route, hover+keyboard names/Escape, day/night, comfort dismissal, contrast and narrow screens.');
}
module.exports = { checkCinematicShell };
