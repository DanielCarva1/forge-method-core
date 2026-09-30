// Shared UI assertions for a browser double and the native WebView. No Send.
const assert = require('node:assert/strict');
async function checkWorkspaceNavigation(page) {
  const nav = page.locator('#mobile-workspace-nav');
  const panes = [['conversation', 'chat-title'], ['preview', 'preview-heading'], ['progress', 'record-heading'], ['project', 'project-title']];
  const composer = page.locator('#message-text');
  const previousDraft = await composer.inputValue();
  await composer.fill('Rascunho de navegação; não enviar.');
  await page.setViewportSize({ width: 1180, height: 820 });
  await nav.waitFor({ state: 'visible' });
  for (const [pane, heading] of panes) {
    const button = nav.locator(`[data-mobile-pane-button="${pane}"]`);
    await button.focus();
    await page.keyboard.press('Enter');
    assert.equal(await page.evaluate(() => document.activeElement.id), heading);
    assert.equal(await button.getAttribute('aria-pressed'), 'true');
    const bounds = await page.locator(`#${heading}`).boundingBox();
    const navBounds = await nav.boundingBox();
    assert.ok(bounds.y >= navBounds.y + navBounds.height, `${pane} heading should not be covered by navigation`);
  }
  await page.evaluate(() => import('./conversation-focus.mjs').then(module => module.setConversationFocus(true)));
  await nav.locator('[data-mobile-pane-button="progress"]').click();
  assert.equal(await page.locator('.workspace').evaluate(node => node.classList.contains('conversation-focus')), false);
  await page.setViewportSize({ width: 390, height: 844 });
  for (const [pane, heading] of panes) {
    await nav.locator(`[data-mobile-pane-button="${pane}"]`).click();
    assert.equal(await page.locator(`#${heading}`).isVisible(), true);
    assert.equal(await page.locator('.workspace > .panel:visible').count(), 1);
  }
  await page.evaluate(() => { document.documentElement.style.fontSize = '200%'; });
  await page.setViewportSize({ width: 360, height: 780 });
  for (const [pane, heading] of panes) {
    await nav.locator(`[data-mobile-pane-button="${pane}"]`).click();
    assert.equal(await page.locator(`#${heading}`).isVisible(), true);
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth + 1), true, `Large text must not force horizontal page scrolling in ${pane}`);
  }
  await page.evaluate(() => { document.documentElement.style.fontSize = ''; });
  await page.setViewportSize({ width: 1180, height: 820 });
  await nav.locator('[data-mobile-pane-button="conversation"]').click();
  assert.equal(await page.locator('.workspace > .panel:visible').count(), 4);
  assert.equal(await composer.inputValue(), 'Rascunho de navegação; não enviar.', 'Moving between areas preserves the unsent draft');
  await composer.fill(previousDraft);
}
module.exports = { checkWorkspaceNavigation };
