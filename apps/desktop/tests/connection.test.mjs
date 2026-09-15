import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readAppInfo } from '../ui/connection.mjs';

test('a browser cannot pretend to be the native app', async () => {
  assert.equal((await readAppInfo(undefined)).state, 'unavailable');
});

test('reads the native app identity without claiming an agent connection', async () => {
  const calls = [];
  const result = await readAppInfo(async (command) => {
    calls.push(command);
    return { name: 'Forge', version: '0.1.0' };
  });
  assert.deepEqual(calls, ['app_info']);
  assert.deepEqual(result, { state: 'ready', version: '0.1.0' });
});

test('does not expose raw native errors to the screen', async () => {
  const result = await readAppInfo(async () => { throw new Error('private path'); });
  assert.deepEqual(result, { state: 'unavailable' });
});

test('rejects malformed native responses', async () => {
  for (const response of [null, {}, { name: 'Other', version: '1' }]) {
    assert.equal((await readAppInfo(async () => response)).state, 'unavailable');
  }
});

test('an unresponsive native bridge does not leave initialization pending', async () => {
  const result = await Promise.race([
    readAppInfo(() => new Promise(() => {}), 5),
    new Promise(resolve => setTimeout(() => resolve({ state: 'stuck' }), 50)),
  ]);
  assert.deepEqual(result, { state: 'unavailable' });
});
