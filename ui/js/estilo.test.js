// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
const pasta = new URL('../css/', import.meta.url);
const css = readdirSync(pasta).filter((f) => f.endsWith('.css')).map((f) => [f, readFileSync(new URL(f, pasta), 'utf8')]);
test('nenhuma animação contínua no CSS (gastava GPU redesenhando a 60 fps)', () => {
  assert.ok(css.length >= 4);
  for (const [f, t] of css) {
    assert.doesNotMatch(t, /infinite/, f);
    assert.doesNotMatch(t, /@keyframes\s+pulse/, f);
  }
});
test('rodando: bolinha com anel fixo no app e no dock', () => {
  const de = (nome) => css.find(([f]) => f === nome)[1];
  for (const sel of ['.k.on .h i', '.kc.on .h i']) assert.match(de('app.css'), new RegExp(`${sel.replace(/\./g, '\\.')} \\{ box-shadow: 0 0 0 3px`));
  assert.match(de('dock.css'), /\.dx-r\.on i \{ box-shadow: 0 0 0 3px/);
  assert.match(de('dock.css'), /\.dk i \{[^}]*box-shadow: 0 0 0 3px/);
});
