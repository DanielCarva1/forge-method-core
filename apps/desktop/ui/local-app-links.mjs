// A cited address is an entry point, not proof that a server is running.
export function localAppLinks(text) {
  const found = new Map();
  const plain = text.replace(/```[^\n]*\n[\s\S]*?```/g, '');
  for (const match of plain.matchAll(/https?:\/\/(?:\[[0-9a-f:]+\]|[^\s<>"`\[\])}])+/gi)) {
    const address = match[0].replace(/[.,;!?]+$/, '');
    if (address.length > 1024 || /[\\\u0000-\u001f\u007f]/.test(address)) continue;
    try {
      const url = new URL(address);
      if (!['localhost', '127.0.0.1', '[::1]'].includes(url.hostname) || url.username || url.password) continue;
      found.set(url.href, url.href);
      if (found.size >= 5) break;
    } catch { /* Not an address that can be opened. */ }
  }
  return [...found.values()];
}
