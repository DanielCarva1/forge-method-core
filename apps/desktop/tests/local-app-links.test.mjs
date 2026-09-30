import { test } from 'node:test';
import assert from 'node:assert/strict';
import { localAppLinks } from '../ui/local-app-links.mjs';
test('only local cited addresses, never credentials, remote hosts or code examples', () => {
  assert.deepEqual(localAppLinks('Teste [o app](http://localhost:5173/). Também http://127.0.0.1:3000/demo e http://[::1]:4000/'), ['http://localhost:5173/', 'http://127.0.0.1:3000/demo', 'http://[::1]:4000/']);
  assert.deepEqual(localAppLinks('https://localhost.evil.com/ http://user:pass@localhost:80/ javascript:alert(1)\n```js\n"http://localhost:9999/"\n```'), []);
  assert.equal(localAppLinks(Array.from({ length: 10 }, (_, n) => `http://localhost:${3000 + n}/`).join(' ')).length, 5);
});
