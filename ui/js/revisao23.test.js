// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Revisão da rodada 23: a data dos backups pelo idioma (I1), o foco ao trocar o idioma pelo
// teclado (M2) e a redação do inglês (M4 a M7).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { DICIONARIOS, TEXTOS, definirIdioma } from './textos.js';
import { dataDoBackup } from './formato.js';

const PAINEL = readFileSync(new URL('./painel.js', import.meta.url), 'utf8');
const EN = DICIONARIOS.en;

/** Roda `f` com o idioma `id` e volta ao português (os outros testes contam com ele). */
function em(id, f) {
  definirIdioma(id);
  try {
    return f();
  } finally {
    definirIdioma('pt-BR');
  }
}

/** Todos os textos fixos de um dicionário (as funções, chamadas com parâmetros de exemplo). */
function textos(d) {
  const v = [];
  const andar = (x) => {
    if (typeof x === 'string') v.push(x);
    else if (typeof x === 'function') {
      try {
        andar(x(...Array.from({ length: x.length }, (_, i) => `P${i}`)));
      } catch {
        /* funções que esperam outro tipo de parâmetro ficam de fora */
      }
    } else if (Array.isArray(x)) x.forEach(andar);
    else if (x && typeof x === 'object' && !(x instanceof RegExp)) Object.values(x).forEach(andar);
  };
  andar(d);
  return v;
}

test('revisão 23 I1: a data dos backups vem em ISO e sai pelo idioma (dia e mês sem ambiguidade em inglês)', async () => {
  assert.equal(dataDoBackup('2026-11-05T09:30'), '05/11/2026 09:30');
  em('en', () => assert.equal(dataDoBackup('2026-11-05T09:30'), 'Nov 5, 2026 09:30'));
  assert.equal(dataDoBackup('qualquer'), 'qualquer', 'o que não é ISO passa como veio');
  const { htmlBackups, corpoRestaurar } = await import('./ajustes.js');
  const b = { nome: '2026-11-05_0930.db', quando: '2026-11-05T09:30', bytes: 2048 };
  em('en', () => {
    assert.match(htmlBackups([b]), />Nov 5, 2026 09:30<\/span>/);
    assert.match(corpoRestaurar(b), /how it was on <b>Nov 5, 2026 09:30<\/b>/);
  });
});

test('revisão 23 M2: trocar o idioma remonta os Ajustes com a rolagem e o foco de volta no rádio escolhido', () => {
  const f = PAINEL.slice(PAINEL.indexOf('async function remontarAjustesNoIdioma'));
  assert.match(f, /const rolagem = el\.querySelector\('\.aj-corpo'\)\?\.scrollTop/);
  assert.match(f, /const noIdioma = ativo\?\.name === 'idioma' && el\.contains\(ativo\);/);
  assert.match(f, /await montarAjustes\(el,/);
  assert.match(f, /corpo\.scrollTop = rolagem;/);
  assert.match(f, /if \(noIdioma\) el\.querySelector\('input\[name="idioma"\]:checked'\)\?\.focus\(\{ preventScroll: true \}\);/);
});

test('revisão 23 M4: as frases do inglês que estavam quebradas ou estranhas', () => {
  assert.equal(EN.fora.dica(['2 paused', '1 waiting for you']), 'Also 2 paused and 1 waiting for you: see the expanded dock');
  assert.equal(EN.aMaisOuAMenos, 'Add or subtract');
  assert.deepEqual([EN.aMais, EN.aMenos, EN.ajusteDe], ['add', 'subtract', 'on']);
  assert.equal(EN.tempoDela('Meeting'), 'Time in Meeting');
  assert.equal(EN.reuniaoDicaTroca('Meeting', 'Meeting'), 'Meeting: count the manual time as Meeting from now');
  for (const s of textos(EN)) assert.doesNotMatch(s, /more or less/i, s);
});

test('revisão 23 M5: o painel do timer não é "dashboard", e o card do Zoho diz que é do Zoho', () => {
  assert.equal(EN.atividadesNoPainel, 'Activities on the timer panel');
  assert.equal(EN.semCard, 'No Zoho card set');
  assert.equal(EN.cardPadrao, 'Default Zoho card');
  assert.equal(EN.cardDoTimer('X'), 'Zoho card: X');
  assert.equal(EN.linkDoCard, 'Zoho card link');
  // o cartão do timer continua "card" (a tela Cards e o tour)
  assert.equal(EN.telaCartoes, 'Cards');
});

test('revisão 23 M6: a redação revista do inglês', () => {
  em('en', () => {
    assert.equal(TEXTOS.idiomaSistemaDica('English'), 'Windows language: English.');
    assert.equal(TEXTOS.digaMinutos, 'Enter the number of minutes.');
    assert.equal(TEXTOS.comoEntrou, 'How it was linked');
    assert.equal(TEXTOS.descartarTitulo('X'), 'Discard today’s time for X?');
    assert.equal(TEXTOS.descartado('X'), 'Today’s time for X discarded.');
    assert.match(TEXTOS.textoEncerrado, /comes back when you send a message or press play/);
    assert.match(TEXTOS.tempos[2][2], /is marked as stopped/);
    assert.match(TEXTOS.dicaInatividade(10), /You can change it in Settings → Times/);
    assert.equal(TEXTOS.motivo('manual'), 'manual');
  });
  for (const s of textos(EN)) assert.doesNotMatch(s, /manual play/i, s);
});

test('revisão 23 M7: o inglês usa só as aspas curvas e "E.g." com maiúscula', () => {
  for (const s of textos(EN)) {
    assert.doesNotMatch(s, /"/, `aspas retas: ${s}`);
    assert.doesNotMatch(s, /(^|[^A-Za-z])e\.g\./, `"e.g." minúsculo: ${s}`);
  }
});
