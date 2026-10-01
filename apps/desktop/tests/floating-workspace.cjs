// Focused shared browser/native assertions. Moving a panel must never Send.
const assert = require('node:assert/strict');
async function checkFloatingWorkspace(page, output) {
  await page.setViewportSize({ width: 1180, height: 820 });
  const workspace = page.locator('.workspace');
  await page.waitForFunction(() => document.querySelector('.workspace').classList.contains('floating-workspace'));
  const panel = page.locator('#project-conversation');
  const draft = page.locator('#message-text');
  await draft.fill('Rascunho da janela flutuante; não enviar.');
  await page.evaluate(() => {
    window.__floatCalls = [];
    const core = window.__TAURI__?.core;
    if (core) {
      const facade = Object.create(core);
      Object.defineProperty(facade, 'invoke', { value: (command, args) => { window.__floatCalls.push(command); return core.invoke(command, args); } });
      window.__TAURI__.core = facade;
      if (window.__TAURI__.core !== facade) throw new Error('Native command observer not installed');
    }
  });
  await page.locator('[data-mobile-pane-button="conversation"]').click();
  const move = panel.locator('[data-window-action="move"]');
  const first = await panel.boundingBox();
  await move.focus(); await page.keyboard.press('ArrowRight');
  const moved = await panel.boundingBox(); assert.ok(moved.x > first.x);
  const grab = await move.boundingBox();
  await page.mouse.move(grab.x + 18, grab.y + 18); await page.mouse.down();
  await page.mouse.move(grab.x + 70, grab.y + 28); await page.mouse.up();
  assert.ok((await panel.boundingBox()).x > moved.x, 'Real pointer drag must move existing panel');
  // Canceling a drag restores the starting geometry, not an intermediate save.
  const beforeCancel = await panel.boundingBox();
  const cancelGrab = await move.boundingBox();
  await page.mouse.move(cancelGrab.x + 18, cancelGrab.y + 18); await page.mouse.down();
  await page.mouse.move(cancelGrab.x + 45, cancelGrab.y + 18);
  await page.keyboard.press('Escape'); await page.mouse.up();
  assert.equal((await panel.boundingBox()).x, beforeCancel.x);
  const resize = panel.locator('[data-window-action="resize"]');
  await resize.focus(); const beforeResize = await panel.boundingBox(); await page.keyboard.press('ArrowLeft');
  assert.ok((await panel.boundingBox()).width < beforeResize.width);
  await panel.locator('[data-window-action="left"]').click();
  const snapped = await panel.boundingBox(); const stage = await workspace.boundingBox();
  assert.ok(Math.abs(snapped.x - stage.x) < 2);
  assert.ok(snapped.width <= stage.width / 2);
  const sendBox = await page.locator('#send-message').boundingBox();
  assert.ok(sendBox.y >= snapped.y && sendBox.y + sendBox.height <= snapped.y + snapped.height, 'Send must fit inside the snapped conversation');
  assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= document.documentElement.clientWidth + 1), true, 'The cinematic scene must not create horizontal page overflow');
  await panel.locator('[data-window-action="focus"]').click();
  assert.equal(await workspace.getAttribute('data-window-focus'), 'conversation');
  assert.ok((await panel.boundingBox()).width > snapped.width);
  await panel.locator('[data-window-action="focus"]').click();
  await panel.locator('[data-window-action="minimize"]').click();
  assert.equal(await panel.isVisible(), false);
  await page.locator('[data-mobile-pane-button="conversation"]').click();
  assert.equal(await panel.isVisible(), true);
  await page.locator('[data-mobile-pane-button="progress"]').click();
  assert.equal(await page.locator('#record-heading').isVisible(), true);
  await page.locator('#workspace-organize').click();
  assert.equal(await page.locator('#project-record').isVisible(), false);
  const saved = await page.evaluate(() => localStorage.getItem('forge.workspace-layout.v1'));
  assert.equal(JSON.parse(saved).version, 1);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.waitForFunction(() => !document.querySelector('.workspace').classList.contains('floating-workspace'));
  assert.equal(await workspace.evaluate(node => node.classList.contains('floating-workspace')), false);
  for (const pane of ['conversation', 'preview', 'progress', 'project']) {
    await page.locator(`[data-mobile-pane-button="${pane}"]`).click();
    assert.equal(await page.locator('.workspace > .panel:visible').count(), 1);
  }
  await page.setViewportSize({ width: 1180, height: 820 });
  await page.waitForFunction(() => document.querySelector('.workspace').classList.contains('floating-workspace'));
  await page.locator('[data-mobile-pane-button="conversation"]').click();
  assert.equal(await draft.inputValue(), 'Rascunho da janela flutuante; não enviar.');
  const calls = await page.evaluate(() => window.__floatCalls);
  for (const command of ['send_message', 'connect_agent', 'disconnect_agent', 'interrupt_agent', 'start_login']) {
    assert.ok(!calls.includes(command), `Window gestures must not call ${command}`);
  }
  assert.equal(await page.locator('#messages article').count(), 0);
  if (output) await page.screenshot({ path: output, fullPage: false });
  console.log('PASS: floating move/resize, keyboard, snap, focus, minimize/reopen, organize, local geometry and narrow navigation preserve draft without Send.');
}
module.exports = { checkFloatingWorkspace };
