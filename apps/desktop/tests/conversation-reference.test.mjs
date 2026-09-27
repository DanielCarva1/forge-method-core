import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readReference, saveReference, readUnconfirmedSend, markUnconfirmedSend, clearUnconfirmedSend } from '../ui/conversation-reference.mjs';

test('stores only a bookmark scoped to project identity and folder', () => {
  const values = new Map();
  const storage = { getItem: key => values.get(key) ?? null, setItem: (key, value) => values.set(key, value) };
  const project = { project_id: 'one', project_root: 'D:\\one' };
  assert.equal(readReference(storage, project), null);
  saveReference(storage, project, 'thread-one');
  assert.equal(readReference(storage, project), 'thread-one');
  assert.equal(readReference(storage, { ...project, project_root: 'D:\\two' }), null);
  assert.equal(readReference(storage, { ...project, project_id: 'two' }), null);
  assert.deepEqual([...values.values()], ['thread-one']);
});

test('storage failure and corrupt references must not silently create a fresh conversation', () => {
  const project = { project_id: 'one', project_root: 'D:\\one' };
  assert.throws(() => readReference({ getItem() { throw new Error('unavailable'); } }, project));
  assert.throws(() => readReference({ getItem: () => '' }, project));
});

test('uncertain delivery marker stores only a project-scoped thread ID until reviewed', () => {
  const values = new Map();
  const storage = { getItem: key => values.get(key) ?? null, setItem: (key, value) => values.set(key, value), removeItem: key => values.delete(key) };
  const project = { project_id: 'one', project_root: 'D:\\one' };
  markUnconfirmedSend(storage, project, 'thread-one');
  assert.equal(readUnconfirmedSend(storage, project), 'thread-one');
  assert.equal(readUnconfirmedSend(storage, { ...project, project_root: 'D:\\two' }), null);
  assert.deepEqual([...values.values()], ['thread-one']);
  clearUnconfirmedSend(storage, project);
  assert.equal(readUnconfirmedSend(storage, project), null);
  assert.throws(() => readUnconfirmedSend({ getItem: () => '' }, project));
});
