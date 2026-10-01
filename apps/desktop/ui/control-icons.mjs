// Presentation only: preserve the existing control, label children and listeners.
const paths = {
  home: 'm3 11 9-8 9 8M5 10v11h14V10M9 21v-7h6v7',
  explore: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Zm4 5-3 5-5 3 3-5 5-3Z',
  projects: 'M3 7h7l2-3h9v16H3V7Z',
  workspace: 'M4 4h16v12H9l-5 4V4Z',
  about: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM12 11v6M12 7v1',
  appearance: 'M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18ZM12 3v18',
  copy: 'M8 8h12v13H8V8ZM4 16V3h12',
  folder: 'M3 7h7l2-3h9v16H3V7ZM8 13h8m-3-3 3 3-3 3',
  save: 'M12 3v12m-4-4 4 4 4-4M4 17v4h16v-4',
  refresh: 'M20 8a8 8 0 1 0 0 8M20 3v5h-5',
  expand: 'M4 9V4h5m6 0h5v5M4 15v5h5m6 0h5v-5',
  remove: 'M5 12h14',
  more: 'M4 12h1m6 0h1m6 0h1',
};

export function decorateControl(control, name, label = control?.textContent.trim()) {
  if (!control || control.querySelector('.control-icon')) return;
  const svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
  svg.classList.add('control-icon'); svg.setAttribute('viewBox', '0 0 24 24'); svg.setAttribute('aria-hidden', 'true');
  const path = document.createElementNS(svg.namespaceURI, 'path'); path.setAttribute('d', paths[name]); svg.append(path);
  const text = document.createElement('span'); text.className = 'control-label';
  text.append(...control.childNodes); control.append(svg, text);
  if (!control.hasAttribute('aria-label')) control.setAttribute('aria-label', label);
  control.dataset.tooltip = label;
  control.addEventListener('keydown', event => { if (event.key === 'Escape') control.toggleAttribute('data-tooltip-dismissed', true); });
  for (const event of ['blur', 'pointerleave']) control.addEventListener(event, () => control.removeAttribute('data-tooltip-dismissed'));
}
