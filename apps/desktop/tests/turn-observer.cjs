// Bounded observation for costly native provider tests. A fixed wait alone
// must not classify an actively progressing turn as stalled.
async function waitForTurn({ inspect, sleep, now = Date.now, pollMs = 5000, quietMs = 7 * 60_000, maxMs = 20 * 60_000, onProgress = () => {} }) {
  const started = now();
  let lastProgress = started;
  let previous;
  for (;;) {
    const state = await inspect();
    if (state.completed) return state;
    if (state.failed) throw new Error(`Codex turn ended without completion: ${state.status}`);
    if (state.fingerprint !== previous) {
      previous = state.fingerprint;
      lastProgress = now();
      onProgress(state);
    }
    if (now() - started >= maxMs) throw new Error(`Codex turn exceeded the explicit ${maxMs} ms test budget; inspect the live thread and files before another send`);
    if (now() - lastProgress >= quietMs) throw new Error(`No visible turn or file progress for ${quietMs} ms; inspect the live thread and files before another send`);
    await sleep(pollMs);
  }
}

module.exports = { waitForTurn };
