const screens = new Map(
  [...document.querySelectorAll('[data-screen]')].map(screen => [screen.dataset.screen, screen]),
);

const homeFragments = new Set(['', 'home', 'about', 'start-ideas']);

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

  if (!moveFocus) return;
  const fragment = location.hash.slice(1);
  const target = document.getElementById(fragment);
  const heading = screens.get(route)?.querySelector('h1');
  requestAnimationFrame(() => {
    if (target && target !== screens.get(route)) target.scrollIntoView({ block: 'start' });
    else heading?.focus({ preventScroll: false });
  });
}

renderRoute(false);
addEventListener('hashchange', () => renderRoute(true));
