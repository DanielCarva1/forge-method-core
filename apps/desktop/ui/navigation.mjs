import { decorateControl } from './control-icons.mjs';

for (const link of document.querySelectorAll('.primary-nav a')) decorateControl(link, link.dataset.route || 'about');
decorateControl(document.querySelector('.appearance summary'), 'appearance');
for (const [id, icon] of Object.entries({ 'copy-preview-path': 'copy', 'reveal-result-file': 'folder', 'save-result-copy': 'save', 'refresh-result-files': 'refresh', 'open-preview': 'expand' })) {
  const control = document.getElementById(id);
  decorateControl(control, icon); control?.classList.add('secondary-control');
}

const screens = new Map(
  [...document.querySelectorAll('[data-screen]')].map(screen => [screen.dataset.screen, screen]),
);

const homeFragments = new Set(['', 'home', 'about', 'start-ideas', 'updates']);

function routeForHash() {
  const fragment = location.hash.slice(1);
  if (homeFragments.has(fragment)) return 'home';
  return screens.has(fragment) ? fragment : 'home';
}

function renderRoute(moveFocus) {
  const route = routeForHash();
  for (const [name, screen] of screens) screen.hidden = name !== route;
  for (const link of document.querySelectorAll('[data-route]')) {
    if (link.dataset.route === route) link.setAttribute('aria-current', 'page');
    else link.removeAttribute('aria-current');
  }

  const fragment = location.hash.slice(1);
  const target = document.getElementById(fragment);
  if ((fragment === 'about' || fragment === 'updates') && document.getElementById('about')) document.getElementById('about').open = true;
  if (!moveFocus) return;
  const heading = screens.get(route)?.querySelector('h1');
  requestAnimationFrame(() => {
    if (target && target !== screens.get(route)) {
      target.scrollIntoView({ block: 'start' });
      if (fragment === 'about') target.querySelector('summary')?.focus({ preventScroll: true });
      if (fragment === 'updates') target.querySelector('h3')?.focus({ preventScroll: true });
    }
    else {
      heading?.focus({ preventScroll: true });
      scrollTo({ top: 0, left: 0 });
    }
  });
}

renderRoute(false);
addEventListener('hashchange', () => renderRoute(true));
document.querySelector('.skip').addEventListener('click', event => {
  event.preventDefault();
  const main = document.getElementById('main');
  main.focus({ preventScroll: true });
  main.scrollIntoView({ block: 'start' });
});
