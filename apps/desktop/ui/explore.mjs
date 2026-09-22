const search = document.querySelector('#category-query');
const status = document.querySelector('#category-status');
const cards = [...document.querySelectorAll('.category-card')];
const composer = document.querySelector('#message-text');

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
document.querySelector('#category-search').addEventListener('submit', event => event.preventDefault());

for (const link of document.querySelectorAll('[data-starter]')) {
  link.addEventListener('click', () => {
    composer.value = link.dataset.starter;
  });
}
