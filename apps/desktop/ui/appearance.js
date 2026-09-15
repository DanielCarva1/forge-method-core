// Appearance belongs to this device's UI, never to Forge project state.
(() => {
  const key = 'forge.appearance.v1';
  const root = document.documentElement;
  const system = matchMedia('(prefers-color-scheme: dark)');
  let preference = { theme: 'system', contrast: false };
  try {
    const saved = JSON.parse(localStorage.getItem(key));
    if (saved && ['system', 'light', 'dark'].includes(saved.theme) && typeof saved.contrast === 'boolean') preference = saved;
  } catch { /* Storage may be unavailable; system appearance remains usable. */ }
  function apply() {
    root.dataset.theme = preference.theme === 'system' ? (system.matches ? 'dark' : 'light') : preference.theme;
    root.toggleAttribute('data-high-contrast', preference.contrast);
  }
  apply();
  system.addEventListener('change', apply);
  document.addEventListener('DOMContentLoaded', () => {
    const theme = document.getElementById('appearance-theme');
    const contrast = document.getElementById('appearance-contrast');
    const status = document.getElementById('appearance-status');
    theme.value = preference.theme;
    contrast.checked = preference.contrast;
    function update() {
      preference = { theme: theme.value, contrast: contrast.checked };
      apply();
      try {
        localStorage.setItem(key, JSON.stringify(preference));
        status.textContent = 'Preferência salva neste aplicativo.';
      } catch { status.textContent = 'A aparência mudou, mas não foi possível salvar. Você pode continuar usando o aplicativo.'; }
    }
    theme.addEventListener('change', update);
    contrast.addEventListener('change', update);
  });
})();
