// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { NOME_APP, nomearPagina } from './nome.js';

test('o nome de exibição vem da constante do núcleo', () => {
  const fonte = readFileSync(new URL('../../crates/nucleo/src/lib.rs', import.meta.url), 'utf8');
  assert.ok(NOME_APP.length > 0);
  assert.ok(fonte.includes(`pub const APP_NOME: &str = "${NOME_APP}";`));
});

test('nenhum texto das páginas traz o nome antigo escrito à mão', () => {
  const pastas = [new URL('./', import.meta.url), new URL('../', import.meta.url)];
  for (const pasta of pastas) {
    for (const f of readdirSync(pasta).filter((f) => /\.(js|html)$/.test(f) && !f.endsWith('.test.js'))) {
      assert.doesNotMatch(readFileSync(new URL(f, pasta), 'utf8'), /Work Timer/, f);
    }
  }
});

test('nomearPagina põe o nome no título e nos lugares marcados', () => {
  const els = [{ textContent: '' }, { textContent: '' }];
  const doc = { title: '', querySelectorAll: (sel) => (sel === '[data-nome-app]' ? els : []) };
  nomearPagina(doc, ' — dock');
  assert.equal(doc.title, `${NOME_APP} — dock`);
  assert.deepEqual(els.map((e) => e.textContent), [NOME_APP, NOME_APP]);
});
