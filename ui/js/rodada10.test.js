// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 10: o total da tarefa como soma das atividades na tela, as linhas por atividade do painel
// do timer, a etiqueta da reunião nos cartões e no dock, e o "Ajustar hoje" já numa atividade.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { linhasDeAtividade, situacaoDoTimer, timersParaDock, SITUACAO, TRABALHO, REUNIAO } from './formato.js';
import { usarAtividades, linhasAtividades, etiquetaReuniao, linhaDivisao, cartaoTimer, relogioFoco, controles } from './componentes.js';
import { campoAjuste, ajusteDoForm } from './editar.js';

const M = 60_000;
const H = 60 * M;
const ESTUDO = 3;
const ATIVIDADES = [
  { id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivada: false },
  { id: 2, nome: 'Reunião', cor: '#e8a23a', ordem: 1, arquivada: false },
  { id: 3, nome: 'Estudo', cor: '#3fb68b', ordem: 2, arquivada: false },
];
usarAtividades(ATIVIDADES);

/** Cliente A: o Claude das 9:00 às 11:00 e a reunião manual das 10:00, ainda aberta (agora 11:10). */
const cliente_a = (extra = {}) => ({
  id: 7,
  nome: 'Cliente A',
  cor: '#ff6b4a',
  rodando: true,
  situacao: 'reuniao',
  atividade_padrao_id: TRABALHO,
  manual_atividade_id: REUNIAO,
  trechos: [
    { inicio: 9 * H, fim: 11 * H, origem: 'claude', atividade_id: TRABALHO },
    { inicio: 10 * H, fim: null, origem: 'manual', atividade_id: REUNIAO },
  ],
  ajuste_ms: 0,
  ajustes: [],
  sessoes: [],
  ...extra,
});
const AGORA = 11 * H + 10 * M;

test('linhas por atividade: todas as não arquivadas e as arquivadas com tempo, com o estado', () => {
  const v = linhasDeAtividade(cliente_a(), ATIVIDADES, AGORA);
  assert.deepEqual(
    v.map((a) => [a.nome, a.ms, a.rodando, a.manual]),
    [['Trabalho', 120 * M, false, false], ['Reunião', 70 * M, true, true], ['Estudo', 0, false, false]],
  );
  // parado e sem tempo: todas as não arquivadas, zeradas (rodada 19)
  const parado = { id: 1, trechos: [], ajustes: [] };
  assert.deepEqual(linhasDeAtividade(parado, ATIVIDADES, 0).map((a) => [a.id, a.ms]), [[TRABALHO, 0], [REUNIAO, 0], [ESTUDO, 0]]);
  // timer de curso (padrão Estudo) e um ajuste de Estudo noutro: a linha aparece
  assert.deepEqual(linhasDeAtividade({ ...parado, atividade_padrao_id: ESTUDO }, ATIVIDADES, 0).map((a) => a.id), [1, 2, 3]);
  const comEstudo = { ...parado, ajustes: [{ id: 1, delta_ms: 15 * M, motivo: null, atividade_id: ESTUDO }] };
  assert.deepEqual(linhasDeAtividade(comEstudo, ATIVIDADES, 0).map((a) => [a.id, a.ms]), [[1, 0], [2, 0], [3, 15 * M]]);
  // Reunião arquivada sem tempo: some; com tempo hoje, fica (arquivada)
  const arq = ATIVIDADES.map((a) => (a.id === REUNIAO ? { ...a, arquivada: true } : a));
  assert.deepEqual(linhasDeAtividade(parado, arq, 0).map((a) => a.id), [TRABALHO, ESTUDO]);
  assert.equal(linhasDeAtividade(cliente_a(), arq, AGORA)[1].arquivada, true);
  // o Claude rodando acende o Trabalho
  const claude = cliente_a({ trechos: [{ inicio: 9 * H, fim: null, origem: 'claude', atividade_id: TRABALHO }], manual_atividade_id: null, situacao: 'trabalhando' });
  assert.deepEqual(linhasDeAtividade(claude, ATIVIDADES, 10 * H).map((a) => [a.id, a.rodando, a.manual]), [[1, true, false], [2, false, false], [3, false, false]]);
});

test('painel do timer: o relógio grande é o total, e embaixo uma linha por atividade com ▶/⏸ e ✎', () => {
  const t = cliente_a();
  const h = relogioFoco(t, AGORA);
  // 2h + 1h 10min = 3h 10min
  assert.match(h, /data-relogio="7">03:10<span>:00<\/span>/);
  assert.match(h, /class="lats"/);
  assert.doesNotMatch(h, /class="divat"/, 'as linhas tomam o lugar da divisão');
  const l = linhasAtividades(t, AGORA);
  // Trabalho parado: ▶ que começa o trabalho
  assert.match(l, /<div class="lat" style="--a:#5b8def"><i class="pt" aria-hidden="true"><\/i><span class="lic"><svg[^]*?<\/svg><\/span><span class="nm">Trabalho<\/span><span class="t mono" data-at="7:1">2:00:00<\/span><button class="lp" data-acao="play-atividade" data-timer="7" data-atividade="1" title="Começar trabalho"/);
  // Reunião rodando: destacada, "rodando", ⏸ que pausa só ela
  assert.match(l, /<div class="lat on reu" style="--a:#e8a23a">[^]*<span class="nm">Reunião<\/span><em class="st">rodando<\/em><span class="t mono" data-at="7:2">1:10:00<\/span><button class="lp" data-acao="pausar-atividade" data-timer="7" data-atividade="2" title="Pausar reunião"/);
  assert.match(l, /<button class="le" data-acao="ajustar-atividade" data-timer="7" data-atividade="2" title="Ajustar hoje: reunião"/);
  // arquivada com tempo: sem botões
  usarAtividades(ATIVIDADES.map((a) => (a.id === REUNIAO ? { ...a, arquivada: true } : a)));
  assert.doesNotMatch(linhasAtividades(t, AGORA), /data-atividade="2"/);
  usarAtividades(ATIVIDADES);
  // o 👥 continua nos controles
  assert.match(controles(t), /data-acao="parar-reuniao"/);
});

test('cartão com a reunião rodando: a etiqueta "reunião" e o tempo dela, menor; o principal é o total', () => {
  const t = cliente_a();
  assert.equal(etiquetaReuniao(t, AGORA), '<div class="kreu" style="--a:#e8a23a" title="reunião: o tempo dela"><b>reunião</b><span class="mono" data-at-hm="7:2">1:10</span></div>');
  const c = cartaoTimer(t, AGORA, false);
  assert.match(c, /class="kreu"[^]*data-tempo="7">3:10:00</);
  assert.equal(etiquetaReuniao(cliente_a({ manual_atividade_id: TRABALHO }), AGORA), '', 'sem reunião, nada');
  assert.doesNotMatch(cartaoTimer(cliente_a({ manual_atividade_id: null, rodando: false }), AGORA, false), /kreu/);
});

test('dock: "reunião + Claude" com os dois rodando, e a sublinha por atividade no expandido', () => {
  const t = cliente_a({ trechos: [{ inicio: 9 * H, fim: null, origem: 'claude', atividade_id: TRABALHO }, { inicio: 10 * H, fim: null, origem: 'manual', atividade_id: REUNIAO }] });
  assert.equal(situacaoDoTimer(t), 'reuniao_claude');
  assert.equal(situacaoDoTimer(cliente_a()), 'reuniao', 'o Claude já parou');
  assert.equal(situacaoDoTimer({ situacao: 'trabalhando', trechos: t.trechos }), 'trabalhando');
  assert.equal(situacaoDoTimer({ situacao: null }), null);
  assert.deepEqual(SITUACAO.reuniao_claude, { curta: 'reunião + Claude', longa: 'reunião + Claude' });
  const snap = { agora: AGORA, grupos: [{ timers: [t] }] };
  const [d] = timersParaDock(snap, AGORA, true);
  assert.equal(d.situacao, 'reuniao_claude');
  assert.equal(d.ms, 130 * M + 70 * M, 'o total da tarefa: 2h 10min de trabalho + 1h 10min de reunião');
  assert.equal(timersParaDock(snap, AGORA, false)[0].situacao, null);
  assert.match(linhaDivisao(cliente_a(), AGORA, 'dx-at'), /^<div class="dx-at" data-divisao="7" data-chave="2h 00min trabalho · 1h 10min reunião"/);
  assert.match(linhaDivisao({ id: 1, trechos: [{ inicio: 0, fim: H, origem: 'manual' }], ajustes: [] }, H, 'dx-at'), /^<div class="dx-at"[^>]*><\/div>$/, 'uma atividade só: vazia');
});

test('"Ajustar hoje" já numa atividade: escolhida no seletor, ou num campo escondido com uma só', () => {
  const t = cliente_a();
  assert.match(campoAjuste(t, ATIVIDADES, REUNIAO), /<option value="2" selected>Reunião<\/option>/);
  const so = campoAjuste(t, [ATIVIDADES[0]], TRABALHO);
  assert.match(so, /<input type="hidden" name="aj_atividade" value="1">/);
  assert.doesNotMatch(campoAjuste(t, [ATIVIDADES[0]]), /aj_atividade/, 'sem a escolhida: o núcleo usa a padrão');
  const form = { elements: { aj_sinal: { value: '-' }, aj_min: { value: '70', focus() {} }, aj_motivo: { value: '' }, aj_atividade: { value: '2' } } };
  assert.deepEqual(ajusteDoForm(form), { delta_ms: -70 * M, motivo: null, atividade_id: REUNIAO });
});

test('total do grupo: a soma dos timers (horas a lançar), com o tempo real na dica; o dia segue a união', async () => {
  const { somaDosTimers, totalDoGrupo, totalDeTimers } = await import('./formato.js');
  const { cabecalhoGrupo } = await import('./componentes.js');
  // Cliente A: Claude 9:00–11:00 e reunião 10:00–11:10 (3h 10min); Flow 10:30–11:30 (1h)
  const s = cliente_a({ trechos: [{ inicio: 9 * H, fim: 11 * H, origem: 'claude', atividade_id: TRABALHO }, { inicio: 10 * H, fim: 11 * H + 10 * M, origem: 'manual', atividade_id: REUNIAO }] });
  const flow = { id: 8, nome: 'Flow', trechos: [{ inicio: 10 * H + 30 * M, fim: 11 * H + 30 * M, origem: 'manual', atividade_id: TRABALHO }], ajustes: [] };
  const agora = 12 * H;
  assert.equal(somaDosTimers([s, flow], agora), 250 * M, '3h 10min + 1h');
  assert.equal(totalDeTimers([s, flow], agora), 150 * M, 'o tempo real: 9:00–11:30');
  assert.deepEqual(totalDoGrupo([s, flow], agora), { texto: '4h 10min', dica: 'tempo real: 2h 30min' });
  const h = cabecalhoGrupo({ id: 1, nome: 'Empresa', timers: [s, flow] }, agora, true);
  assert.match(h, /<span class="tt mono" data-total-grupo="1" title="tempo real: 2h 30min">4h 10min<\/span>/);
  assert.deepEqual(totalDoGrupo([], 0), { texto: '0min', dica: 'tempo real: 0min' });
});
