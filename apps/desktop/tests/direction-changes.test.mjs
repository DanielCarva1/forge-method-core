import test from 'node:test';
import assert from 'node:assert/strict';
import { directionChanges } from '../ui/direction-changes.mjs';
const first = { outcome: 'Um jardim', constraints: ['Local', 'Gratuito'], unacceptable_outcomes: ['Perder arquivos'] };
test('first visible revision does not invent an earlier direction', () => {
  assert.deepEqual(directionChanges(null, first), []);
});
test('compares exact recorded wording without interpreting approval', () => {
  const current = { outcome: '<script>Um jardim novo</script>', constraints: ['Gratuito', 'Sem cadastro'], unacceptable_outcomes: ['Publicar sem pedir'] };
  assert.deepEqual(directionChanges(first, current), [
    { label: 'Objetivo anterior', values: ['Um jardim'] },
    { label: 'Objetivo nesta versão', values: ['<script>Um jardim novo</script>'] },
    { label: 'Combinados acrescentados', values: ['Sem cadastro'] },
    { label: 'Combinados retirados', values: ['Local'] },
    { label: 'Cuidados acrescentados', values: ['Publicar sem pedir'] },
    { label: 'Cuidados retirados', values: ['Perder arquivos'] },
  ]);
  assert.deepEqual(first.constraints, ['Local', 'Gratuito']);
});
test('reordering or duplicate wording is not a new agreement', () => {
  assert.deepEqual(directionChanges(first, { ...first, constraints: ['Gratuito', 'Local', 'Local'] }), []);
});
