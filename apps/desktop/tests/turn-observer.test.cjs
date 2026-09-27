const { test } = require('node:test');
const assert = require('node:assert/strict');
const { waitForTurn } = require('./turn-observer.cjs');

test('progress extends the idle observation beyond an old fixed cutoff', async () => {
  let time = 0;
  const states = [
    { fingerprint: 'sent' }, { fingerprint: 'sent' },
    { fingerprint: 'tool-1' }, { fingerprint: 'tool-1' },
    { fingerprint: 'file-written' }, { fingerprint: 'file-written' },
    { fingerprint: 'reply', completed: true },
  ];
  const changes = [];
  const result = await waitForTurn({
    inspect: async () => states.shift(),
    sleep: async () => { time += 100; },
    now: () => time,
    pollMs: 100, quietMs: 250, maxMs: 1000,
    onProgress: state => changes.push(state.fingerprint),
  });
  assert.equal(result.completed, true);
  assert.equal(time, 600);
  assert.deepEqual(changes, ['sent', 'tool-1', 'file-written']);
});

test('quiet and explicit budget limits remain distinct from completion', async () => {
  let time = 0;
  await assert.rejects(waitForTurn({
    inspect: async () => ({ fingerprint: 'unchanged' }),
    sleep: async () => { time += 100; }, now: () => time,
    pollMs: 100, quietMs: 250, maxMs: 1000,
  }), /No visible turn or file progress/);
  time = 0;
  await assert.rejects(waitForTurn({
    inspect: async () => ({ fingerprint: `progress-${time}` }),
    sleep: async () => { time += 100; }, now: () => time,
    pollMs: 100, quietMs: 250, maxMs: 450,
  }), /explicit 450 ms test budget/);
});

test('terminal failure stops observation without implying a retry', async () => {
  await assert.rejects(waitForTurn({
    inspect: async () => ({ status: 'Interrompido', failed: true }),
    sleep: async () => { throw new Error('must not sleep'); },
  }), /ended without completion: Interrompido/);
});
