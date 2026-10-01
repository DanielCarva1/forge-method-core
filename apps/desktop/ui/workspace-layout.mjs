// Pure device-local panel geometry. No project, account or agent state.
export const panelNames = ['conversation', 'preview', 'progress', 'project'];
const clamp = (value, low, high) => Math.max(low, Math.min(high, value));
export function clampRect(rect, bounds) {
  const width = clamp(rect.width, Math.min(380, bounds.width), bounds.width);
  const height = clamp(rect.height, Math.min(320, bounds.height), bounds.height);
  return { x: clamp(rect.x, 0, bounds.width - width), y: clamp(rect.y, 0, bounds.height - height), width, height };
}
export function snapRect(side, bounds) {
  const width = Math.floor((bounds.width - 16) / 2);
  return clampRect({ x: side === 'right' ? bounds.width - width : 0, y: 0, width, height: bounds.height }, bounds);
}
export function defaultLayout(bounds) {
  const conversation = clampRect({ x: 0, y: 0, width: bounds.width * .62, height: bounds.height }, bounds);
  return Object.fromEntries(panelNames.map((name, i) => [name, {
    ...(name === 'conversation' ? conversation : clampRect({
      x: bounds.width * .64, y: (i - 1) * 32,
      width: bounds.width * .36, height: bounds.height * .8,
    }, bounds)), minimized: name !== 'conversation',
  }]));
}
export function encodeLayout(layout, bounds) {
  return { version: 1, panels: Object.fromEntries(panelNames.map(name => {
    const r = layout[name];
    return [name, { x: r.x / bounds.width, y: r.y / bounds.height,
      width: r.width / bounds.width, height: r.height / bounds.height, minimized: r.minimized }];
  })) };
}
export function decodeLayout(value, bounds) {
  const layout = defaultLayout(bounds);
  if (value?.version !== 1) return layout;
  for (const name of panelNames) {
    const r = value.panels?.[name];
    if (!r || !['x', 'y', 'width', 'height'].every(key => Number.isFinite(r[key]) && r[key] >= 0 && r[key] <= 1)
        || r.width === 0 || r.height === 0 || typeof r.minimized !== 'boolean') continue;
    layout[name] = { ...clampRect({ x: r.x * bounds.width, y: r.y * bounds.height,
      width: r.width * bounds.width, height: r.height * bounds.height }, bounds), minimized: r.minimized };
  }
  // A damaged/empty saved workspace must never hide every entry point.
  if (panelNames.every(name => layout[name].minimized)) layout.conversation.minimized = false;
  return layout;
}
