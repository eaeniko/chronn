// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Spec 001: a leitura no app do Claude falhando nunca mais fica escondida. A linha dos Ajustes
// (Claude Code) e o aviso do dock, uma vez por falha.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { definirIdioma } from './textos.js';
import { htmlLeituraApp } from './ajustes.js';
import { avisoDaLeituraApp } from './formato.js';

const fonte = (nome) => readFileSync(new URL(nome, import.meta.url), 'utf8');

/** Roda `f` com o idioma `id` e volta ao português (os outros testes contam com ele). */
function em(id, f) {
  definirIdioma(id);
  try {
    return f();
  } finally {
    definirIdioma('pt-BR');
  }
}

const DIA = Date.UTC(2026, 10, 26, 3, 0); // meia-noite de 26/11 em UTC-3
const AS_9H12 = DIA + (9 * 60 + 12) * 60_000;

test('Ajustes: a leitura no app do Claude ok, falhando desde a hora, ou ainda sem uso', () => {
  const ok = htmlLeituraApp({ estado: 'ok', desde: null }, DIA);
  assert.match(ok, /data-leitura-app="ok"/);
  assert.match(ok, /Leitura no app do Claude/);
  assert.match(ok, /aj-estado on/);
  assert.doesNotMatch(ok, /aj-dica/, 'ok: sem ajuda');
  const f = htmlLeituraApp({ estado: 'falhando', desde: AS_9H12 }, DIA);
  assert.match(f, /falhando desde 09:12/);
  assert.match(f, /aj-estado antigo/);
  assert.match(f, /diagnostico-leitura\.json/, 'a ajuda diz onde ficam os números');
  for (const l of [undefined, null, { estado: 'sem_uso', desde: null }, { estado: 'outra coisa' }]) {
    assert.match(htmlLeituraApp(l, DIA), /ainda sem uso/);
  }
  // falhando sem o instante: não inventa a hora
  assert.match(htmlLeituraApp({ estado: 'falhando', desde: null }, DIA), /ainda sem uso/);
  em('en', () => {
    assert.match(htmlLeituraApp({ estado: 'falhando', desde: AS_9H12 }, DIA), /Reading in the Claude app.*failing since 09:12/s);
    assert.match(htmlLeituraApp({ estado: 'ok' }, DIA), />ok</);
  });
});

test('dock: avisa uma vez por falha, com a hora em que começou', () => {
  assert.equal(avisoDaLeituraApp({ estado: 'ok', desde: null }, null, DIA), null);
  assert.equal(avisoDaLeituraApp(undefined, null, DIA), null);
  assert.equal(avisoDaLeituraApp({ estado: 'falhando', desde: null }, null, DIA), null);
  const a = avisoDaLeituraApp({ estado: 'falhando', desde: AS_9H12 }, null, DIA);
  assert.equal(a.chave, String(AS_9H12));
  assert.match(a.texto, /desde 09:12/);
  assert.equal(avisoDaLeituraApp({ estado: 'falhando', desde: AS_9H12 }, a.chave, DIA), null, 'a mesma falha não avisa de novo');
  assert.ok(avisoDaLeituraApp({ estado: 'falhando', desde: AS_9H12 + 3_600_000 }, a.chave, DIA), 'outra falha avisa');
  em('en', () => assert.match(avisoDaLeituraApp({ estado: 'falhando', desde: AS_9H12 }, null, DIA).texto, /since 09:12/));
});

test('a linha fica na seção do Claude Code e o dock confere a cada estado', () => {
  const aj = fonte('./ajustes.js');
  assert.match(aj, /data-bloco="hooks"[^\n]*\n<div data-bloco="leitura-app">/);
  assert.match(aj, /desenharLeituraApp\(\);/);
  const dock = fonte('./dock.js');
  assert.match(dock, /verDicas\(s\);\n\s*verLeituraApp\(s\);/);
  assert.match(dock, /avisoForaDoDock\(a\.texto, TEXTOS\.entendi/);
});
