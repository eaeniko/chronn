// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 14: a etiqueta do manual junto com o Claude ("manual + esperando você"), o pausado no
// dock recolhido e o dock que cresce para o aviso caber inteiro.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { etiquetaComposta, timersParaDock, textoDosDeFora, dicaDoLembrete, REUNIAO, TRABALHO } from './formato.js';
import { htmlEtiqueta, etiquetaSituacao, cartaoTimer, relogioFoco, usarAtividades, usarEsperaApp } from './componentes.js';
import { campoEspera, PAUSADO_OPCOES } from './ajustes.js';

const M = 60_000;
const H = 60 * M;
usarAtividades([
  { id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivada: false },
  { id: 2, nome: 'Reunião', cor: '#e8a23a', ordem: 1, arquivada: false },
]);

const timer = (id, extra = {}) => ({
  id,
  nome: `T${id}`,
  cor: '#4f8cff',
  rodando: false,
  situacao: null,
  situacao_claude: null,
  manual_atividade_id: null,
  espera_desde: null,
  espera_ultima: null,
  pausado_em: null,
  encerrado: false,
  atividade_padrao_id: TRABALHO,
  trechos: [],
  ajustes: [],
  ajuste_ms: 0,
  sessoes: [],
  ...extra,
});
const manual = (claude, extra = {}) =>
  timer(1, { rodando: true, situacao: claude === 'trabalhando' ? 'trabalhando' : 'manual', situacao_claude: claude, manual_atividade_id: TRABALHO, espera_ultima: 9 * H, trechos: [{ inicio: 8 * H, fim: null, origem: 'manual', atividade_id: TRABALHO }], ...extra });

test('o manual junto com cada estado do Claude', () => {
  const texto = (claude) => etiquetaComposta(manual(claude), 9 * H + M, 0)?.texto;
  assert.equal(texto(null), 'manual');
  assert.equal(texto('trabalhando'), 'manual + Claude');
  assert.equal(texto('esperando'), 'manual + esperando você');
  assert.equal(texto('segundo_plano'), 'manual + segundo plano');
  assert.equal(texto('lendo'), 'manual + lendo');
  assert.equal(texto('escrevendo'), 'manual + escrevendo');
  // a reunião manual: o mesmo padrão
  const reu = (claude) => etiquetaComposta(manual(claude, { manual_atividade_id: REUNIAO, situacao: 'reuniao' }), 9 * H + M, 0)?.texto;
  assert.equal(reu(null), 'reunião');
  assert.equal(reu('trabalhando'), 'reunião + Claude');
  assert.equal(reu('esperando'), 'reunião + esperando você');
  assert.equal(reu('segundo_plano'), 'reunião + segundo plano');
  assert.equal(reu('lendo'), 'reunião + lendo');
  // as partes, para a cor de cada uma
  assert.deepEqual(etiquetaComposta(manual('esperando'), 9 * H, 0), { p1: { k: 'manual', texto: 'manual' }, p2: { k: 'esperando', texto: 'esperando você' }, texto: 'manual + esperando você' });
  // sem o manual, a etiqueta de sempre, numa parte
  assert.equal(etiquetaComposta(timer(2, { situacao: 'esperando', situacao_claude: 'esperando', espera_ultima: 9 * H }), 9 * H, 0).texto, 'esperando você');
  assert.equal(etiquetaComposta(timer(2, { situacao: 'trabalhando', situacao_claude: 'trabalhando' }), 9 * H, 0, 'longa').texto, 'Claude trabalhando');
  assert.equal(etiquetaComposta(timer(2), 9 * H, 0), null);
});

test('o "esperando você" junto do manual some no tempo de cada lugar', () => {
  const t = manual('esperando'); // terminou às 9:00
  assert.equal(etiquetaComposta(t, 9 * H + 29 * M, 30).texto, 'manual + esperando você');
  assert.equal(etiquetaComposta(t, 9 * H + 30 * M, 30).texto, 'manual', 'venceu: fica só o manual');
  assert.equal(etiquetaComposta(t, 23 * H, 0).texto, 'manual + esperando você', 'até a virada');
});

test('cada parte na cor dela e, sem espaço, a segunda vira um ponto com a dica', () => {
  const e = etiquetaComposta(manual('esperando'), 9 * H, 0);
  assert.equal(htmlEtiqueta(e), '<span class="s-manual">manual</span><span class="mais"> + </span><span class="s-esperando">esperando você</span>');
  assert.equal(htmlEtiqueta(e, 16), '<span class="s-manual">manual</span><span class="pt2 s-esperando" title="manual + esperando você"></span>');
  // cabe: as duas partes
  assert.match(htmlEtiqueta(etiquetaComposta(manual('trabalhando'), 9 * H, 0), 16), /manual<\/span><span class="mais"> \+ <\/span><span class="s-trabalhando">Claude/);
  // no cartão: estreito, o ponto
  usarEsperaApp(0);
  assert.match(cartaoTimer(manual('esperando'), 9 * H, false), /<small class="ksit manual_esperando" title="manual \+ esperando você"><span class="s-manual">manual<\/span><span class="pt2 s-esperando"/);
  // na visão Cartões (sem limite) e no painel do timer, por extenso
  assert.match(etiquetaSituacao(manual('segundo_plano'), 9 * H), /<span class="s-segundo_plano">segundo plano<\/span>/);
  assert.match(relogioFoco({ ...manual('esperando'), motivo: 'play manual' }, 9 * H), /<div class="why"><span class="etq"><span class="s-manual">manual<\/span><span class="mais"> \+ <\/span><span class="s-esperando">esperando você<\/span><\/span><\/div>/);
  // a reunião no cartão: a etiqueta dela e a segunda parte depois
  const reu = manual('esperando', { manual_atividade_id: REUNIAO, situacao: 'reuniao' });
  assert.match(cartaoTimer(reu, 9 * H, false), /<div class="kreu"[^]*<\/div><small class="ksit reuniao_esperando" title="reunião \+ esperando você"><span class="mais">\+ <\/span><span class="s-esperando">esperando você<\/span><\/small>/);
});

test('o pausado fica no dock recolhido 10 min, depois dos rodando e antes dos lembretes', () => {
  const rodando = timer(1, { rodando: true, situacao: 'manual', trechos: [{ inicio: 8 * H, fim: null }] });
  const pausado = (id, em) => timer(id, { pausado_em: em });
  const espera = timer(9, { situacao: 'esperando', espera_desde: 9 * H, espera_ultima: 9 * H + 55 * M });
  const snap = (ts, cfg = {}) => ({ agora: 10 * H, config: cfg, grupos: [{ id: 1, timers: ts }] });
  const r = timersParaDock(snap([espera, pausado(3, 9 * H + 52 * M), rodando, pausado(4, 9 * H + 58 * M), pausado(5, 9 * H + 49 * M)]), 10 * H, true);
  // o de 9:49 já passou dos 10 min; os outros, do ⏸ mais recente ao mais antigo
  assert.deepEqual(r.map((x) => [x.id, x.pausado]), [[1, false], [4, true], [3, true], [9, false]]);
  assert.deepEqual(r[1].etiqueta, { p1: { k: 'pausado', texto: 'pausado' }, p2: null, texto: 'pausado' });
  // pausado com uma sessão esperando você: as duas partes, e o lembrete não repete (revisão)
  const comEspera = timer(7, { pausado_em: 9 * H + 57 * M, situacao: 'esperando', situacao_claude: 'esperando', espera_desde: 9 * H + 56 * M, espera_ultima: 9 * H + 56 * M });
  const r2 = timersParaDock(snap([comEspera]), 10 * H, true);
  assert.deepEqual(r2.map((x) => [x.id, x.pausado, x.etiqueta.texto]), [[7, true, 'pausado + esperando você']]);
  assert.deepEqual(r2[0].etiqueta.p2, { k: 'esperando', texto: 'esperando você' });
  // "segundo plano" também; o lembrete vencido (30 min do recolhido) sai, e fica só "pausado"
  const sp = timer(8, { pausado_em: 9 * H + 57 * M, situacao: 'segundo_plano', situacao_claude: 'segundo_plano', espera_ultima: 9 * H + 50 * M });
  assert.equal(timersParaDock(snap([sp]), 10 * H, true)[0].etiqueta.texto, 'pausado + segundo plano');
  const velho = timer(8, { pausado_em: 9 * H + 57 * M, situacao: 'esperando', situacao_claude: 'esperando', espera_ultima: 9 * H + 20 * M });
  assert.equal(timersParaDock(snap([velho]), 10 * H, true)[0].etiqueta.texto, 'pausado');
  assert.equal(dicaDoLembrete(r[1]), 'T4: pausado. ▶ para retomar.');
  // 0 = sai na hora; 30 min: o de 9:49 volta
  assert.deepEqual(timersParaDock(snap([pausado(3, 9 * H + 52 * M)], { pausado_dock_min: 0 }), 10 * H, true), []);
  assert.deepEqual(timersParaDock(snap([pausado(5, 9 * H + 49 * M)], { pausado_dock_min: 30 }), 10 * H, true).map((x) => x.id), [5]);
  // o ■ (encerrado) não fica
  assert.deepEqual(timersParaDock(snap([timer(6, { pausado_em: 9 * H + 59 * M, encerrado: true })]), 10 * H, true), []);
  // o limite de 4 e o "+N", com o pausado contado à parte
  const muitos = [1, 2, 3].map((i) => timer(10 + i, { rodando: true, trechos: [{ inicio: i, fim: null }] }));
  const fora = textoDosDeFora(snap([...muitos, pausado(3, 9 * H + 59 * M), pausado(4, 9 * H + 58 * M), espera], { espera_dock_min: 0 }), true);
  assert.deepEqual(fora, { texto: '+1 pausado · +1 esperando', dica: 'Mais 1 pausado e 1 esperando você: veja no dock expandido' });
});

test('Ajustes → "Lembretes no dock" com o pausado', () => {
  assert.deepEqual(PAUSADO_OPCOES.map(([v]) => v), [0, 5, 10, 30, 60]);
  assert.match(campoEspera({}), /Pausado, no dock recolhido[\s\S]*name="pausado_dock_min" value="10" checked><span>10 min/);
  assert.match(campoEspera({ pausado_dock_min: 0 }), /name="pausado_dock_min" value="0" checked><span>Sai na hora/);
});
