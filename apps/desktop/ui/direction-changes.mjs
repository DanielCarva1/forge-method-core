// Compare only the literal wording in consecutive authoritative revisions.
// This is a display aid, not a new decision or an explanation of intent.
export function directionChanges(previous, current) {
  if (!previous) return [];
  const changes = [];
  if (previous.outcome !== current.outcome) {
    changes.push({ label: 'Objetivo anterior', values: [previous.outcome] });
    changes.push({ label: 'Objetivo nesta versão', values: [current.outcome] });
  }
  for (const [field, added, removed] of [
    ['constraints', 'Combinados acrescentados', 'Combinados retirados'],
    ['unacceptable_outcomes', 'Cuidados acrescentados', 'Cuidados retirados'],
  ]) {
    const before = new Set(previous[field]);
    const after = new Set(current[field]);
    const additions = [...after].filter(value => !before.has(value));
    const removals = [...before].filter(value => !after.has(value));
    if (additions.length) changes.push({ label: added, values: additions });
    if (removals.length) changes.push({ label: removed, values: removals });
  }
  return changes;
}
