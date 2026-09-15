// A UI bookmark, not another history store or Forge project record.
const key = project => `forge.conversation.v1:${JSON.stringify([project.project_id, project.project_root])}`;
export function readReference(storage, project) {
  const value = storage.getItem(key(project));
  if (value !== null && (!value.trim() || value.length > 200)) throw new Error('Invalid conversation reference');
  return value;
}
export function saveReference(storage, project, id) {
  storage.setItem(key(project), id);
}
