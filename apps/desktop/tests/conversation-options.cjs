async function openConversationOptions(page) {
  const options = page.locator('#conversation-options');
  if (!await options.evaluate(node => node.open)) await options.locator('summary').click();
}

async function clickConversationAction(page, name) {
  await openConversationOptions(page);
  await page.getByRole('button', { name, exact: true }).click();
}

module.exports = { openConversationOptions, clickConversationAction };
