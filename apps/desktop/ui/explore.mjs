const search = document.querySelector('#category-query');
const status = document.querySelector('#category-status');
const cards = [...document.querySelectorAll('.category-card')];
const composer = document.querySelector('#message-text');
const ideaStatus = document.querySelector('#idea-selection-status');
let lastSuggestedDraft = '';
function clearIdeaStatus() {
  ideaStatus.textContent = '';
  ideaStatus.hidden = true;
}

function normalize(value) {
  return value.normalize('NFD').replace(/[\u0300-\u036f]/g, '').toLocaleLowerCase('pt-BR');
}

function filterCategories() {
  const query = normalize(search.value.trim());
  let visible = 0;
  for (const card of cards) {
    const matches = !query || normalize(card.dataset.category).includes(query);
    card.hidden = !matches;
    if (matches) visible += 1;
  }
  status.textContent = query
    ? `${visible} ${visible === 1 ? 'tema encontrado' : 'temas encontrados'}.`
    : 'Todos os temas estão visíveis.';
}

search.addEventListener('input', filterCategories);
// Search controls can be restored/cleared by the browser without an input event.
// Reconcile card visibility whenever the Explore screen is revisited.
addEventListener('hashchange', () => { if (location.hash === '#explore') filterCategories(); });
document.querySelector('#category-search').addEventListener('submit', event => event.preventDefault());

export function chooseStarter(link) {
  if (composer.value.trim() && composer.value !== lastSuggestedDraft) {
    ideaStatus.textContent = 'Sua ideia escrita foi mantida. Você pode editá-la antes de enviar.';
    ideaStatus.hidden = false;
    return;
  }
  composer.value = link.dataset.starter;
  lastSuggestedDraft = composer.value;
  composer.dispatchEvent(new Event('input', { bubbles: true }));
  clearIdeaStatus();
}

for (const link of document.querySelectorAll('[data-starter]')) {
  link.addEventListener('click', () => chooseStarter(link));
}
composer.addEventListener('input', clearIdeaStatus);
addEventListener('hashchange', () => { if (location.hash !== '#workspace') clearIdeaStatus(); });
