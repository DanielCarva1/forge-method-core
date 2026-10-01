import { test } from 'node:test';
import assert from 'node:assert/strict';
import { clampRect, snapRect, defaultLayout, encodeLayout, decodeLayout } from '../ui/workspace-layout.mjs';
test('restored and resized panels stay inside the available stage', () => {
  const bounds = { width: 1000, height: 620 };
  assert.deepEqual(clampRect({ x: -30, y: 800, width: 1600, height: 5 }, bounds), { x: 0, y: 300, width: 1000, height: 320 });
  const left = snapRect('left', bounds), right = snapRect('right', bounds);
  assert.equal(right.x - (left.x + left.width), 16);
  const original = defaultLayout({ width: 1800, height: 1000 });
  const restored = decodeLayout(encodeLayout(original, { width: 1800, height: 1000 }), bounds);
  for (const rect of Object.values(restored)) {
    assert.ok(rect.x >= 0 && rect.y >= 0 && rect.x + rect.width <= bounds.width && rect.y + rect.height <= bounds.height);
  }
});
test('corrupt and future storage cannot erase all usable entry points', () => {
  const bounds = { width: 1000, height: 620 };
  const initial = defaultLayout(bounds);
  assert.deepEqual(decodeLayout({ version: 2, panels: {} }, bounds), initial);
  assert.deepEqual(decodeLayout({ version: 1, panels: { conversation: { x: Infinity } } }, bounds), initial);
  const encoded = encodeLayout(initial, bounds);
  Object.values(encoded.panels).forEach(rect => { rect.minimized = true; });
  assert.equal(decodeLayout(encoded, bounds).conversation.minimized, false);
});
