// Native IPC fixture only: models Codex's device-login completion notification.
// No network, credentials, project writes or real Codex account are involved.
const { existsSync, writeFileSync } = require('node:fs');
const { createInterface } = require('node:readline');
const marker = process.env.FORGE_FAKE_AUTH_MARKER;
if (!marker) process.exit(2);
const reply = (id, result) => process.stdout.write(`${JSON.stringify({ id, result })}\n`);
createInterface({ input: process.stdin }).on('line', line => {
  let request;
  try { request = JSON.parse(line); } catch { return; }
  if (typeof request.id !== 'number') return;
  switch (request.method) {
    case 'initialize': reply(request.id, {}); break;
    case 'account/read': reply(request.id, { account: existsSync(marker) ? { type: 'chatgpt' } : null, requiresOpenaiAuth: true }); break;
    case 'account/login/start':
      if (process.env.FORGE_FAKE_LOGIN_STARTED) writeFileSync(process.env.FORGE_FAKE_LOGIN_STARTED, 'started');
      reply(request.id, { type: 'chatgptDeviceCode', loginId: 'fixture-login', userCode: 'TEST-1234', verificationUrl: 'https://auth.openai.com/codex/device' });
      if (process.env.FORGE_AUTH_CANCEL !== '1') setTimeout(() => {
        writeFileSync(marker, 'completed');
        process.stdout.write(`${JSON.stringify({ method: 'account/login/completed', params: { loginId: 'fixture-login', success: true } })}\n`);
      }, 6000);
      break;
    case 'account/login/cancel': reply(request.id, { status: 'canceled' }); break;
    case 'thread/start':
      if (process.env.FORGE_FAKE_SKILL_MARKER)
        writeFileSync(process.env.FORGE_FAKE_SKILL_MARKER, JSON.stringify(request.params));
      reply(request.id, { thread: { id: 'fixture-thread', cwd: request.params.cwd, status: { type: 'idle' }, turns: [] } });
      break;
    case 'turn/start': reply(request.id, { turn: { id: 'fixture-turn' } }); break;
    default: process.stdout.write(`${JSON.stringify({ id: request.id, error: { code: -32601, message: 'Fixture method not supported' } })}\n`);
  }
});
