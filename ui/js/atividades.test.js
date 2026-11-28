// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Atividades por tarefa (rodada 9): totais por atividade, o 👥, a divisão no painel, a linha do
// tempo, o "Ajustar hoje", o editar timer e os Ajustes → Atividades.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { contasDoTimer, msDoTimer, totalDeTimers, realAjustado, tempoPorAtividade, divisaoPorAtividade, trechosDoDia, atividadesDosTrechos, SITUACAO, TRABALHO, REUNIAO } from './formato.js';
import { usarAtividades, botaoReuniao, divisaoDoTimer, linhaDivisao, cartaoTimer, controles, relogioFoco } from './componentes.js';
import { campoAjuste, ajusteDoForm, campoAtividadePadrao, resumoDoTimer, opcoesDeAtividade } from './editar.js';
import { htmlAtividades } from './ajustes.js';
import { linhaCompacta, visaoLinha } from './linha.js';

const M = 60_000;
const H = 60 * M;
const ESTUDO = 3;
const ATIVIDADES = [
  { id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivada: false },
  { id: 2, nome: 'Reunião', cor: '#e8a23a', ordem: 1, arquivada: false },
  { id: 3, nome: 'Estudo', cor: '#3fb68b', ordem: 2, arquivada: false },
];
usarAtividades(ATIVIDADES);

/** Cliente A: o Claude das 9:00 às 11:00 (trabalho) e uma reunião manual das 10:00 às 11:10. */
const cliente_a = (extra = {}) => ({
  id: 7,
  nome: 'Cliente A',
  cor: '#ff6b4a',
  rodando: false,
  atividade_padrao_id: TRABALHO,
  manual_atividade_id: null,
  trechos: [
    { inicio: 9 * H, fim: 11 * H, origem: 'claude', atividade_id: TRABALHO },
    { inicio: 10 * H, fim: 11 * H + 10 * M, origem: 'manual', atividade_id: REUNIAO },
  ],
  ajuste_ms: 0,
  ajustes: [],
  sessoes: [],
  ...extra,
});

test('totais: cada atividade a sua união, o timer a soma delas, o dia a união de tudo', () => {
  const t = cliente_a();
  const c = contasDoTimer(t, 12 * H);
  assert.equal(c.uniao, 130 * M);
  assert.equal(c.real, 130 * M);
  assert.deepEqual([...c.porAtividade.entries()], [[TRABALHO, 120 * M], [REUNIAO, 70 * M]]);
  assert.equal(msDoTimer(t, 12 * H), 190 * M, '2h + 1h 10min: dois lançamentos');
  assert.equal(totalDeTimers([t], 12 * H), 130 * M, 'o dia: 2h 10min');
  const outro = { trechos: [{ inicio: 10 * H + 30 * M, fim: 12 * H, origem: 'manual', atividade_id: TRABALHO }], ajustes: [] };
  assert.equal(totalDeTimers([t, outro], 12 * H), 3 * H, '9:00 às 12:00');
  // trecho sem atividade (de antes): Trabalho
  assert.deepEqual([...contasDoTimer({ trechos: [{ inicio: 0, fim: H }] }, H).porAtividade.keys()], [TRABALHO]);
});

test('ajustes por atividade somam na atividade e no timer, nunca negativos', () => {
  const aj = (atividade_id, delta_ms) => ({ id: 1, delta_ms, motivo: null, atividade_id });
  let t = cliente_a({ ajustes: [aj(REUNIAO, 20 * M)], ajuste_ms: 20 * M });
  assert.equal(contasDoTimer(t, 12 * H).porAtividade.get(REUNIAO), 90 * M);
  assert.equal(msDoTimer(t, 12 * H), 210 * M, '2h + 1h 30min');
  assert.equal(totalDeTimers([t], 12 * H), 150 * M);
  // tirar 2h da reunião: ela para em zero, e o timer fica com as 2h do Claude, que existiram
  t = cliente_a({ ajustes: [aj(REUNIAO, -120 * M)], ajuste_ms: -120 * M });
  assert.equal(contasDoTimer(t, 12 * H).porAtividade.get(REUNIAO), 0);
  assert.equal(msDoTimer(t, 12 * H), 120 * M);
  assert.equal(totalDeTimers([t], 12 * H), 120 * M);
  // tirar tudo das duas (sobrepostas): o timer para em zero, não abaixo
  t = cliente_a({ ajustes: [aj(REUNIAO, -70 * M), aj(TRABALHO, -120 * M)], ajuste_ms: -190 * M });
  assert.equal(msDoTimer(t, 12 * H), 0);
  assert.equal(totalDeTimers([t], 12 * H), 0);
  // ajuste de Estudo num timer sem estudo: positivo entra, negativo não tira nada
  t = cliente_a({ ajustes: [aj(ESTUDO, -30 * M)], ajuste_ms: -30 * M });
  assert.equal(msDoTimer(t, 12 * H), 190 * M);
  assert.equal(totalDeTimers([t], 12 * H), 130 * M);
  t = cliente_a({ ajustes: [aj(ESTUDO, 15 * M)], ajuste_ms: 15 * M });
  assert.equal(msDoTimer(t, 12 * H), 205 * M);
  assert.equal(totalDeTimers([t], 12 * H), 145 * M);
  // só ajuste_ms, sem a lista (como antes): vale em Trabalho
  assert.equal(msDoTimer({ trechos: [{ inicio: 0, fim: H }], ajuste_ms: -90 * M }, H), 0);
});

test('a decisão da rodada 10: timer 3h 10min, dia 2h 10min; com −1h 10min em Reunião, 0, 2h e 2h', () => {
  const t = cliente_a();
  assert.equal(msDoTimer(t, 12 * H), 190 * M);
  assert.equal(totalDeTimers([t], 12 * H), 130 * M);
  const menos = cliente_a({ ajustes: [{ id: 1, delta_ms: -70 * M, motivo: null, atividade_id: REUNIAO }], ajuste_ms: -70 * M });
  assert.equal(contasDoTimer(menos, 12 * H).porAtividade.get(REUNIAO), 0);
  assert.equal(msDoTimer(menos, 12 * H), 120 * M);
  assert.equal(totalDeTimers([menos], 12 * H), 120 * M);
});

test('o caso da revisão: −1h 10min na reunião não tira as horas do Claude', () => {
  const t = cliente_a({ ajustes: [{ id: 1, delta_ms: -70 * M, motivo: null, atividade_id: REUNIAO }], ajuste_ms: -70 * M });
  const c = contasDoTimer(t, 12 * H);
  assert.deepEqual([c.porAtividade.get(REUNIAO), c.porAtividade.get(TRABALHO)], [0, 120 * M]);
  assert.equal(msDoTimer(t, 12 * H), 120 * M);
  const outro = { trechos: [{ inicio: 12 * H, fim: 13 * H, origem: 'manual', atividade_id: TRABALHO }], ajustes: [] };
  // o dia era 3h 10min (9:00–11:10 e 12:00–13:00): perde só os 10 min que eram só da reunião
  assert.equal(totalDeTimers([t, outro], 14 * H), 180 * M);
  assert.equal(realAjustado(130 * M, -70 * M, 120 * M), 120 * M);
  assert.equal(realAjustado(10 * M, -60 * M, 0), 0);
});

test('divisão por atividade: só com mais de uma, na ordem da lista', () => {
  const t = cliente_a();
  assert.deepEqual(tempoPorAtividade(t, ATIVIDADES, 12 * H).map((a) => [a.nome, a.ms]), [['Trabalho', 120 * M], ['Reunião', 70 * M]]);
  assert.equal(divisaoPorAtividade(t, ATIVIDADES, 12 * H), '2h 00min trabalho · 1h 10min reunião');
  const so = { trechos: [{ inicio: 9 * H, fim: 10 * H, origem: 'claude', atividade_id: TRABALHO }], ajustes: [] };
  assert.equal(divisaoPorAtividade(so, ATIVIDADES, 12 * H), '', 'quem não usa atividades não vê nada');
  assert.equal(divisaoDoTimer(so, 12 * H).html, '');
  const d = divisaoDoTimer(t, 12 * H);
  assert.equal(d.chave, '2h 00min trabalho · 1h 10min reunião');
  assert.match(d.html, /<i style="--a:#e8a23a"><\/i>1h 10min reunião/);
  assert.match(linhaDivisao(t, 12 * H), /data-divisao="7" data-chave="2h 00min trabalho · 1h 10min reunião"/);
  assert.match(relogioFoco(t, 12 * H), /class="lats"/, 'o painel do timer tem as linhas por atividade');
});

test('👥: ao lado do ▶, aceso com a reunião, some sem a atividade Reunião ou quando ela é a padrão', () => {
  const parado = cliente_a({ trechos: [] });
  assert.match(botaoReuniao(parado, 'pe'), /class="pe reu" data-acao="reuniao" data-timer="7" title="Reunião: conta este tempo como reunião neste timer"[^>]*aria-pressed="false" style="--a:#e8a23a"/);
  assert.match(botaoReuniao(cliente_a({ rodando: true, manual_atividade_id: TRABALHO }), 'pe'), /title="Reunião: troca o ▶ por reunião agora"/);
  const on = botaoReuniao(cliente_a({ rodando: true, manual_atividade_id: REUNIAO }), 'pe');
  assert.match(on, /class="pe reu on" data-acao="parar-reuniao"[^>]*aria-pressed="true"/);
  assert.equal(botaoReuniao(cliente_a({ atividade_padrao_id: REUNIAO }), 'pe'), '', 'o ▶ já é reunião');
  usarAtividades(ATIVIDADES.map((a) => (a.id === REUNIAO ? { ...a, arquivada: true } : a)));
  assert.equal(botaoReuniao(parado, 'pe'), '', 'Reunião arquivada');
  usarAtividades(ATIVIDADES);
  // no cartão, no canto de cima (ao lado do nome); nos controles do painel e do Foco, antes do ▶ grande
  assert.match(cartaoTimer(parado, 0, false), /<div class="h"><i><\/i><span class="nm">Cliente A<\/span><button class="km" data-acao="menu-timer"[^>]*>⋯<\/button><button class="pe reu" data-acao="reuniao"/);
  assert.match(controles(parado), /data-acao="zerar"[\s\S]*data-acao="reuniao"[\s\S]*class="big"/);
  assert.deepEqual(SITUACAO.reuniao, { curta: 'reunião', longa: 'reunião' });
});

test('linha do tempo: a faixa da atividade e a legenda só com mais de uma atividade', () => {
  const t = cliente_a();
  const b = trechosDoDia(t, 0, 12 * H, 8, 19, ATIVIDADES);
  assert.deepEqual(b.map((x) => x.atividade), [TRABALHO, REUNIAO]);
  assert.equal(b[1].titulo, '10:00–11:10 · você deu ▶ · 1h 10min · reunião');
  assert.equal(b[0].titulo, '09:00–11:00 · Claude trabalhando · 2h 00min', 'a padrão não repete');
  assert.deepEqual(atividadesDosTrechos([t], ATIVIDADES).map((a) => a.id), [TRABALHO, REUNIAO]);
  const snap = { grupos: [{ id: 1, nome: 'Agendas', timers: [t] }] };
  const v = visaoLinha(snap, 0, 12 * H);
  assert.match(v, /<b class="m at" title="10:00–11:10 · você deu ▶ · 1h 10min · reunião" style="[^"]*;--a:#e8a23a"><\/b>/);
  assert.match(v, /<span><i class="sw at" style="--a:#e8a23a"><\/i>reunião<\/span>/);
  assert.match(linhaCompacta(snap.grupos, 0, 12 * H), /class="tl-leg"/);
  const so = { ...t, trechos: [t.trechos[0]] };
  const v1 = visaoLinha({ grupos: [{ id: 1, nome: 'Agendas', timers: [so] }] }, 0, 12 * H);
  assert.doesNotMatch(v1, /class="[^"]*\bat\b/);
  assert.doesNotMatch(linhaCompacta([{ timers: [so] }], 0, 12 * H), /tl-leg/);
});

test('"Ajustar hoje" com a atividade (a padrão do timer já escolhida) e o editar com a padrão', () => {
  const h = campoAjuste(cliente_a({ atividade_padrao_id: ESTUDO }));
  assert.match(h, /<select name="aj_atividade"[^>]*><option value="1">Trabalho<\/option><option value="2">Reunião<\/option><option value="3" selected>Estudo<\/option><\/select>/);
  assert.doesNotMatch(campoAjuste(cliente_a(), [ATIVIDADES[0]]), /aj_atividade/, 'uma atividade só: sem seletor');
  const form = { elements: { aj_sinal: { value: '+' }, aj_min: { value: '30', focus() {} }, aj_motivo: { value: '' }, aj_atividade: { value: '2' } } };
  assert.deepEqual(ajusteDoForm(form), { delta_ms: 30 * M, motivo: null, atividade_id: REUNIAO });
  assert.match(campoAtividadePadrao(cliente_a()), /<span>Atividade padrão<\/span><select name="atividade_padrao"><option value="1" selected>Trabalho/);
  assert.equal(campoAtividadePadrao(cliente_a(), [ATIVIDADES[0]]), '');
  assert.equal(resumoDoTimer({ link_card: 'x', atividade_padrao_id: ESTUDO, ajuste_ms: 0, ajustes: [] }), 'card do Zoho · estudo');
  assert.equal(resumoDoTimer({ atividade_padrao_id: TRABALHO, ajuste_ms: 0, ajustes: [] }), '');
  // a escolhida arquivada continua no seletor, marcada
  usarAtividades(ATIVIDADES.map((a) => (a.id === ESTUDO ? { ...a, arquivada: true } : a)));
  assert.match(opcoesDeAtividade(ATIVIDADES.slice(0, 2), ESTUDO), /^<option value="3" selected>Estudo \(arquivada\)<\/option>/);
  usarAtividades(ATIVIDADES);
});

test('Ajustes → Atividades: renomear, cor, arquivar (nunca a última) e desarquivar', () => {
  const snap = { atividades: ATIVIDADES, grupos: [{ id: 1, timers: [cliente_a({ atividade_padrao_id: ESTUDO })] }] };
  const h = htmlAtividades(snap);
  assert.match(h, /<input type="color" name="at_cor" data-id="2" value="#e8a23a" aria-label="Cor de Reunião"><input type="text" class="aj-at-nome" name="at_nome" data-id="2" value="Reunião"/);
  assert.match(h, /data-id="3" value="Estudo"[^>]*><small>padrão de 1 timer<\/small>/);
  assert.match(h, /data-aj="nova-atividade">\+ Acrescentar atividade/);
  const so = htmlAtividades({ atividades: [ATIVIDADES[0], { ...ATIVIDADES[1], arquivada: true }], grupos: [] });
  assert.match(so, /data-aj="arquivar-atividade" data-id="1" disabled title="Precisa sobrar pelo menos uma atividade"/);
  assert.match(so, /<li class="aj-at arq">[\s\S]*data-aj="desarquivar-atividade" data-id="2"/);
});
