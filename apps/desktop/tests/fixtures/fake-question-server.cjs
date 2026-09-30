// Native protocol fixture: no network, credentials, model or agent execution.
const { appendFileSync } = require('node:fs');
const { createInterface } = require('node:readline');
const marker = process.env.FORGE_QUESTION_MARKER;
if (!marker) process.exit(2);
let count = 0;
let questionId;
const write = value => process.stdout.write(`${JSON.stringify(value)}\n`);
const reply = (id, result) => write({ id, result });
const completed = status => write({ method: 'turn/completed', params: { turn: { id: `turn-${count}`, status } } });
createInterface({ input: process.stdin }).on('line', line => {
  const request = JSON.parse(line);
  if (!request.method) {
    appendFileSync(marker, `${JSON.stringify(request)}\n`);
    if (request.id === questionId && request.result) {
      write({ method: 'item/completed', params: { item: { type: 'agentMessage', id: `answer-${count}`, text: 'Recebi suas escolhas.' } } });
      completed('completed');
    }
    return;
  }
  switch (request.method) {
    case 'initialize': reply(request.id, {}); break;
    case 'initialized': break;
    case 'account/read': reply(request.id, { account: { type: 'chatgpt' } }); break;
    case 'thread/start': reply(request.id, { thread: { id: 'questions-thread', cwd: request.params.cwd, status: { type: 'idle' }, turns: [] } }); break;
    case 'turn/start':
      count++;
      reply(request.id, { turn: { id: `turn-${count}` } });
      write({ method: 'turn/started', params: { turn: { id: `turn-${count}` } } });
      if (count === 1) write({ id: 'approval-test', method: 'item/commandExecution/requestApproval', params: { command: 'never execute this fixture' } });
      questionId = count === 1 ? 900 : `question-${count}`;
      write({ id: questionId, method: 'item/tool/requestUserInput', params: {
        threadId: 'questions-thread', turnId: `turn-${count}`, itemId: `question-item-${count}`, isBlocking: true,
        questions: [
          { id: 'style', header: 'Visual', question: 'Como você quer o visual?', options: [{ label: 'Claro', description: 'Leve e convidativo' }, { label: 'Escuro', description: 'Mais contraste' }] },
          { id: 'name', header: 'Nome', question: 'Qual nome você prefere?', options: null },
        ],
      } });
      break;
    case 'turn/interrupt': reply(request.id, {}); completed('interrupted'); break;
    default: write({ id: request.id, error: { code: -32601, message: 'Fixture method not supported' } });
  }
});
