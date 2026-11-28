// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 11: o número do timer é o acumulado desde o último zerar (não volta a zero na virada do
// dia), com "desde …"; o total do dia continua o de hoje; o ↺ só zera e o descarte de hoje foi para
// o "Ajustar hoje".
import { test } from 'node:test'; import assert from 'node:assert/strict';
import {
  msDoTimer,
  msDeHoje,
  hojeDesdeZerar,
  contasAcumuladas,
  desdeTexto,
  totalDeTimers,
  totalDoGrupo,
  linhasDeAtividade,
  timersParaDock,
  TRABALHO,
  REUNIAO,
} from './formato.js';
import { usarAtividades, usarCorte, cartaoTimer, relogioFoco, controles, cabecalhoGrupo, marcaAjuste } from './componentes.js';
import { campoAjuste, textoDaFaixaZerado, TEXTO_ZERADOS_TODOS } from './editar.js';

const M = 60_000;
const H = 60 * M;
const ATIVIDADES = [
  { id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivada: false },
  { id: 2, nome: 'Reunião', cor: '#e8a23a', ordem: 1, arquivada: false },
];
usarAtividades(ATIVIDADES);

// 18/11/2026 no relógio local de quem roda o teste (as datas do "desde" são locais)
const local = (dia, hh, mm = 0) => new Date(2026, 10, dia, hh, mm).getTime();
const AGORA = local(18, 11, 0);

/**
 * Cliente A hoje: o Claude das 9:00 às 10:00 e a reunião das 10:00, aberta. Antes de hoje, 3h de
 * Trabalho e 1h de Reunião (o que o núcleo manda em `acumulado_antes`), zerado ontem às 14:20.
 */
const cliente_a = (extra = {}) => ({
  id: 7,
  nome: 'Cliente A',
  cor: '#ff6b4a',
  rodando: true,
  atividade_padrao_id: TRABALHO,
  manual_atividade_id: REUNIAO,
  trechos: [
    { inicio: local(18, 9), fim: local(18, 10), origem: 'claude', atividade_id: TRABALHO },
    { inicio: local(18, 10), fim: null, origem: 'manual', atividade_id: REUNIAO },
  ],
  ajuste_ms: 0,
  ajustes: [],
  sessoes: [],
  zerado_em: local(17, 14, 20),
  acumulado_antes: [
    { atividade_id: TRABALHO, ms: 3 * H },
    { atividade_id: REUNIAO, ms: H },
  ],
  ...extra,
});

test('"desde …": hoje, ontem, outra data e outro ano', () => {
  assert.equal(desdeTexto(local(18, 9, 0), AGORA), 'desde hoje 09:00');
  assert.equal(desdeTexto(local(17, 14, 20), AGORA), 'desde ontem 14:20');
  assert.equal(desdeTexto(local(15, 10, 5), AGORA), 'desde 15/11 10:05');
  assert.equal(desdeTexto(new Date(2025, 11, 31, 23, 59).getTime(), AGORA), 'desde 31/12/2025 23:59');
  // virada do mês: o dia 1º, e ontem o 31 do mês anterior
  assert.equal(desdeTexto(new Date(2026, 9, 31, 8, 0).getTime(), new Date(2026, 10, 1, 9, 0).getTime()), 'desde ontem 08:00');
  assert.equal(desdeTexto(0, AGORA), '', 'sem zerar (desde sempre): nada');
  assert.equal(desdeTexto(undefined, AGORA), '');
});

test('"desde hoje/ontem" segue o dia de trabalho (o corte), não o calendário', () => {
  const C = 180; // corte às 03:00
  // 01:30 de 19/11 é do dia de trabalho 18: às 10:00 do 19 é "ontem"
  assert.equal(desdeTexto(local(19, 1, 30), local(19, 10, 0), C), 'desde ontem 01:30');
  // e às 02:00 do 19 (ainda o dia 18), "hoje"
  assert.equal(desdeTexto(local(19, 1, 30), local(19, 2, 0), C), 'desde hoje 01:30');
  // 03:10 do 18 e 02:50 do 18 (dia 17), vistos às 10:00 do 19
  assert.equal(desdeTexto(local(18, 3, 10), local(19, 10, 0), C), 'desde ontem 03:10');
  assert.equal(desdeTexto(local(18, 2, 50), local(19, 10, 0), C), 'desde 18/11 02:50', 'anteontem: a data do relógio');
  // sem corte (o padrão), o calendário
  assert.equal(desdeTexto(local(19, 1, 30), local(19, 10, 0)), 'desde hoje 01:30');
  // o corte do snapshot chega à dica dos cartões e do dock
  usarCorte(C);
  const t = cliente_a({ zerado_em: local(19, 1, 30) });
  assert.match(cartaoTimer(t, local(19, 10, 0), false), /title="desde ontem 01:30, [^"]+ hoje"/);
  const [d] = timersParaDock({ agora: local(19, 10, 0), config: { corte_min: C }, grupos: [{ id: 1, timers: [t] }] }, local(19, 10, 0));
  assert.equal(d.desde, 'desde ontem 01:30');
  usarCorte(0);
});

test('o número do timer: os dias fechados mais hoje; o total do dia continua o de hoje', () => {
  const t = cliente_a();
  // hoje: 1h de Trabalho e 1h de Reunião (10:00 às 11:00)
  assert.equal(msDeHoje(t, AGORA), 2 * H);
  assert.equal(msDoTimer(t, AGORA), 2 * H + 4 * H);
  const c = contasAcumuladas(t, AGORA);
  assert.deepEqual([c.porAtividade.get(TRABALHO), c.porAtividade.get(REUNIAO)], [4 * H, 2 * H]);
  assert.equal(totalDeTimers([t], AGORA), 2 * H, 'o dia: a união de hoje');
  // sem os campos novos (um snapshot antigo): o de hoje, como antes
  const { zerado_em, acumulado_antes, ...velho } = t;
  assert.equal(msDoTimer(velho, AGORA), 2 * H);
});

test('zerado hoje: o trecho que atravessa o zerar conta a partir dele, e os ajustes só os feitos depois', () => {
  const t = cliente_a({
    zerado_em: local(18, 9, 30),
    acumulado_antes: [],
    ajustes: [
      { id: 1, delta_ms: 30 * M, motivo: 'antes', atividade_id: TRABALHO, criado_em: local(18, 9, 10) },
      { id: 2, delta_ms: 15 * M, motivo: 'depois', atividade_id: TRABALHO, criado_em: local(18, 9, 40) },
    ],
    ajuste_ms: 45 * M,
  });
  const h = hojeDesdeZerar(t);
  assert.deepEqual(h.trechos.map((x) => [x.inicio, x.fim]), [[local(18, 9, 30), local(18, 10)], [local(18, 10), null]]);
  assert.deepEqual([h.ajustes.map((a) => a.id), h.ajuste_ms], [[2], 15 * M]);
  // 30 min de Trabalho + 15 do ajuste, e 1h de Reunião
  assert.equal(msDoTimer(t, AGORA), 45 * M + H);
  // o dia não olha o zerar: 2h de trechos e os dois ajustes
  assert.equal(msDeHoje(t, AGORA), 2 * H + 45 * M);
  // o "ajustado à mão" do painel do timer mostra o que entra no número
  assert.match(marcaAjuste(t), /ajustado à mão: \+15 min/);
  // zerado agora com tudo rodando: 0:00
  assert.equal(msDoTimer(cliente_a({ zerado_em: AGORA, acumulado_antes: [] }), AGORA), 0);
  // trecho que acabou antes do zerar some do número
  assert.equal(hojeDesdeZerar(cliente_a({ zerado_em: local(18, 10, 30) })).trechos.length, 1);
});

test('linhas por atividade: o acumulado de cada uma desde o zerar', () => {
  const v = linhasDeAtividade(cliente_a(), ATIVIDADES, AGORA);
  assert.deepEqual(v.map((a) => [a.nome, a.ms, a.rodando]), [['Trabalho', 4 * H, false], ['Reunião', 2 * H, true]]);
});

test('a soma do grupo no período e o tempo real na dica', () => {
  // Flow: 1h hoje (9:30–10:30), zerado hoje às 9:00; o grupo teve 3h 30min de tempo real antes
  const flow = {
    id: 8,
    nome: 'Flow',
    cor: '#4f8cff',
    rodando: false,
    trechos: [{ inicio: local(18, 9, 30), fim: local(18, 10, 30), origem: 'claude', atividade_id: TRABALHO }],
    ajuste_ms: 0,
    ajustes: [],
    sessoes: [],
    zerado_em: local(18, 9),
    acumulado_antes: [],
  };
  const s = cliente_a();
  // soma: Cliente A 6h + Flow 1h; tempo real: 3h 30min antes + hoje a união de 9:00 às 11:00
  // spec 003: "Os dois" (o padrão), a soma e o real ao lado quando é diferente
  assert.deepEqual(totalDoGrupo([s, flow], AGORA, 3 * H + 30 * M), { texto: '7h 00min · 5h 30min real', dica: 'a lançar: 7h 00min · tempo real: 5h 30min · 1h 30min em paralelo' });
  // o tempo real de hoje conta cada timer desde o zerar dele: Cliente A zerado às 10:30 deixa 9:30–11:00
  const z = cliente_a({ zerado_em: local(18, 10, 30), acumulado_antes: [] });
  assert.deepEqual(totalDoGrupo([z, flow], AGORA), { texto: '1h 30min', dica: 'a lançar: 1h 30min · tempo real: 1h 30min' });
  // o cabeçalho usa o `real_antes_ms` do grupo
  const g = { id: 1, nome: 'Empresa', cor: '#111', timers: [s, flow], real_antes_ms: 3 * H + 30 * M };
  assert.match(cabecalhoGrupo(g, AGORA, true), /title="a lançar: 7h 00min · tempo real: 5h 30min · 1h 30min em paralelo">7h 00min · 5h 30min real</);
});

test('"desde …" no painel do timer, na dica dos cartões e do dock', () => {
  const t = cliente_a();
  assert.match(relogioFoco(t, AGORA), /<div class="clk mono" data-relogio="7">06:00<span>:00<\/span><\/div>\n<div class="desde" data-desde="7">desde ontem 14:20<\/div>/);
  assert.match(cartaoTimer(t, AGORA, false), /<span class="tm mono" data-tempo="7" title="desde ontem 14:20, [^"]+ hoje">6:00:00<\/span>/);
  // US 003.3: o "desde ontem" à vista no cartão, com dias de antes no número
  assert.match(cartaoTimer(t, AGORA, false), /<div class="desde kd" data-desde-k="7">desde ontem 14:20<\/div>/);
  const [d] = timersParaDock({ agora: AGORA, grupos: [{ id: 1, timers: [t] }] }, AGORA);
  assert.deepEqual([d.ms, d.desde], [6 * H, 'desde ontem 14:20']);
  // sem zerar: sem a etiqueta nem a dica
  assert.doesNotMatch(cartaoTimer(cliente_a({ zerado_em: 0 }), AGORA, false), /title="desde/);
});

test('↺ Zerar sem diálogo, "Zerar todos" e o descarte de hoje no "Ajustar hoje"', () => {
  // só uma atividade com tempo: o ↺ zera direto (com duas ou mais, abre o menu: rodada19.test.js)
  const so = cliente_a({ trechos: cliente_a().trechos.filter((t) => t.atividade_id === TRABALHO), acumulado_antes: [] });
  const c = controles(so, AGORA);
  assert.match(c, /<button data-acao="zerar" data-timer="7" title="Zerar: o número volta a 0:00 agora[^"]*Nada é apagado[^"]*" aria-label="Zerar Cliente A">↺<\/button>/);
  assert.equal(textoDaFaixaZerado('Cliente A'), 'Cliente A zerado.');
  assert.equal(TEXTO_ZERADOS_TODOS, 'Todos os timers zerados.');
  assert.match(campoAjuste(cliente_a(), ATIVIDADES), /<button type="button" class="am-desc" data-descartar>Descartar o tempo de hoje…<\/button>\n<\/fieldset>/);
});
