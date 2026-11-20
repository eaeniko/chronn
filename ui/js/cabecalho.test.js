// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 18, seção 5: todo arquivo-fonte de app/ (o código, os testes e os gerados, como o modelo
// do instalador em gen/) começa com a linha de copyright, no comentário da linguagem.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const APP = fileURLToPath(new URL('../../', import.meta.url));
const FORA = new Set(['target', 'node_modules', '.git']);
const TEXTO = 'Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.';
const LINHA = {
  '.rs': `// ${TEXTO}`,
  '.js': `// ${TEXTO}`,
  '.css': `/* ${TEXTO} */`,
  '.html': `<!-- ${TEXTO} -->`,
  '.nsi': `; ${TEXTO}`,
};
const extensao = (nome) => nome.slice(nome.lastIndexOf('.'));

function fontes(dir) {
  const v = [];
  for (const nome of readdirSync(dir)) {
    if (FORA.has(nome)) continue;
    const p = join(dir, nome);
    if (statSync(p).isDirectory()) v.push(...fontes(p));
    else if (LINHA[extensao(nome)]) v.push(p);
  }
  return v;
}

test('autoria: todo arquivo-fonte de app/ começa com o copyright', () => {
  const lista = fontes(APP);
  assert.ok(lista.length > 70, `poucos arquivos lidos (${lista.length})`);
  const sem = [];
  for (const p of lista) {
    // a primeira linha; no HTML, a logo depois do <!doctype html>
    const linhas = readFileSync(p, 'utf8').split(/\r?\n/);
    const i = extensao(p) === '.html' && /^<!doctype/i.test(linhas[0]) ? 1 : 0;
    if (linhas[i] !== LINHA[extensao(p)]) sem.push(p.slice(APP.length));
  }
  assert.deepEqual(sem, []);
});
