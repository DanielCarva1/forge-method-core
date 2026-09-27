// The Forge ID identifies a project; the folder name is what a person sees.
export function projectDisplayName(project) {
  const root = project.project_root.replace(/[\\/]+$/, '');
  return root.split(/[\\/]/).pop() || project.project_id;
}
