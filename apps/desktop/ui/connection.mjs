export async function readAppInfo(invoke, timeoutMs = 3000) {
  if (typeof invoke !== 'function') return { state: 'unavailable' };
  let timer;
  try {
    const info = await Promise.race([
      invoke('app_info'),
      new Promise(resolve => { timer = setTimeout(() => resolve(null), timeoutMs); }),
    ]);
    if (info?.name !== 'Forge' || typeof info.version !== 'string' || !info.version) {
      return { state: 'unavailable' };
    }
    return { state: 'ready', version: info.version };
  } catch {
    return { state: 'unavailable' };
  } finally {
    clearTimeout(timer);
  }
}
