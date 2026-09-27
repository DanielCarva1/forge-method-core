// A UI bookmark, not another history store or Forge project record.
const key = project => `forge.conversation.v1:${JSON.stringify([project.project_id, project.project_root])}`;
const uncertainKey = project => `forge.send-unconfirmed.v1:${JSON.stringify([project.project_id, project.project_root])}`;
export function readReference(storage, project) {
  const value = storage.getItem(key(project));
  if (value !== null && (!value.trim() || value.length > 200)) throw new Error('Invalid conversation reference');
  return value;
}
export function saveReference(storage, project, id) {
  storage.setItem(key(project), id);
}
// Only the thread ID is stored. Codex remains the authority for messages and delivery.
export function readUnconfirmedSend(storage, project) {
  const value = storage.getItem(uncertainKey(project));
  if (value !== null && (!value.trim() || value.length > 200)) throw new Error('Invalid uncertain send reference');
  return value;
}
export function markUnconfirmedSend(storage, project, id) {
  storage.setItem(uncertainKey(project), id);
}
export function clearUnconfirmedSend(storage, project) {
  storage.removeItem(uncertainKey(project));
}
