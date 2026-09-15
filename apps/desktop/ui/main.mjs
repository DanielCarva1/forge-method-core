import { readAppInfo } from './connection.mjs';

const status = document.querySelector('#native-status');
const retry = document.querySelector('#retry');

async function refresh() {
  retry.disabled = true;
  status.textContent = 'Verificando o aplicativo…';
  const result = await readAppInfo(globalThis.__TAURI__?.core?.invoke);
  status.textContent = result.state === 'ready'
    ? `Aplicativo iniciado · versão ${result.version}`
    : 'Não foi possível acessar a parte nativa. Abra esta tela pelo aplicativo Forge.';
  retry.disabled = false;
}

retry.addEventListener('click', refresh);
refresh();
