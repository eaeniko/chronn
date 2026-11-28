// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { relogio, curto, uniaoMs, posicaoNaLinha, relogioGrande, horaMin, dataDoDia, trechosDoDia, reguaDoDia, horaDoDia, timersParaDock, linhas, msDoTimer, totalDeTimers, textoDoAjuste, deltaDoAjuste, codigosDoLink, rotuloDoDestino, lembretesDeFora, linkDoZohoSprints, minutosDeSegundos, segundosDeMinutos, faltamCriar, tituloDoTrecho, recolherAoPerderFoco, trocarTexto, podeEncerrar, SITUACAO } from './formato.js';
const M = 60_000;
test('relogio', () => { assert.equal(relogio(2*3600e3 + 23*M + 16e3), '2:23:16'); assert.equal(relogio(0), '0:00:00'); });
test('curto', () => { assert.equal(curto(6*3600e3 + 12*M), '6h 12min'); assert.equal(curto(45*M), '45min'); assert.equal(curto(2*3600e3 + 1*M), '2h 01min'); assert.equal(curto(8*3600e3), '8h 00min'); assert.equal(curto(0), '0min'); });
test('tituloDoTrecho: horário, origem e duração', () => {
  const H = 3600e3;
  assert.equal(tituloDoTrecho({ inicio: 9*H + 30*M, fim: 11*H, origem: 'claude' }, 0, 12*H), '09:30–11:00 · Claude trabalhando · 1h 30min');
  assert.equal(tituloDoTrecho({ inicio: 9*H, fim: null, origem: 'manual' }, 0, 9*H + 20*M), '09:00–agora · você deu ▶ · 20min');
  assert.equal(tituloDoTrecho({ inicio: 14*H, fim: 14*H + 5*M, origem: 'leitura' }, 0, 15*H), '14:00–14:05 · você lendo · 5min');
  const [b] = trechosDoDia({ trechos: [{ inicio: 9*H, fim: 10*H, origem: 'manual' }] }, 0, 12*H);
  assert.equal(b.titulo, '09:00–10:00 · você deu ▶ · 1h 00min');
});
test('uniaoMs nao conta em dobro e estende aberto', () => {
  assert.equal(uniaoMs([{inicio: 0, fim: 60*M}, {inicio: 30*M, fim: 90*M}], 0), 90*M);
  assert.equal(uniaoMs([{inicio: 0, fim: null}], 10*M), 10*M);
});
test('posicaoNaLinha', () => { const d = 0; const r = posicaoNaLinha(d + 9*3600e3, d + 10*3600e3 + 6*M, d); assert.equal(Math.round(r.left), 9); assert.equal(Math.round(r.width), 10); });
test('relogioGrande, horaMin e dataDoDia', () => {
  assert.deepEqual(relogioGrande(58*M + 10e3), { hm: '00:58', s: '10' });
  assert.equal(horaMin(3600e3 + 32*M + 59e3), '1:32');
  assert.equal(dataDoDia('2026-11-17'), 'Terça, 17 nov');
});
test('trechosDoDia ordena e marca manual', () => {
  const H = 3600e3;
  const timer = { trechos: [
    { inicio: 13*H, fim: null, origem: 'claude' },
    { inicio: 9*H, fim: 10*H, origem: 'manual' },
    { inicio: 8*H + 30*M, fim: 9*H, origem: 'leitura' },
  ] };
  const r = trechosDoDia(timer, 0, 13*H + 33*M);
  assert.equal(r.length, 3);
  assert.deepEqual(r.map((x) => x.manual), [false, true, false]);
  assert.deepEqual(r.map((x) => Math.round(x.left * 100) / 100), [4.55, 9.09, 45.45]);
  assert.deepEqual(r.map((x) => Math.round(x.width * 100) / 100), [4.55, 9.09, 5]);
  assert.deepEqual(trechosDoDia({ trechos: [] }, 0, 0), []);
  // régua estendida: 6h–20h
  const [b] = trechosDoDia({ trechos: [{ inicio: 6*H, fim: 7*H, origem: 'manual' }] }, 0, 0, 6, 20);
  assert.equal(b.left, 0); assert.equal(Math.round(b.width * 100) / 100, 7.14); assert.equal(b.manual, true);
});
test('reguaDoDia vai das 8h às 19h e estende para os trechos de fora', () => {
  const H = 3600e3;
  assert.deepEqual(reguaDoDia([], 0, 10*H), { ini: 8, fim: 19 });
  assert.deepEqual(reguaDoDia([{ inicio: 6*H + 40*M, fim: 7*H }], 0, 10*H), { ini: 6, fim: 19 });
  assert.deepEqual(reguaDoDia([{ inicio: 18*H, fim: null }], 0, 21*H + 5*M), { ini: 8, fim: 22 });
  assert.deepEqual(reguaDoDia([{ inicio: 23*H, fim: 27*H }], 0, 27*H), { ini: 8, fim: 27 });
});
test('horaDoDia', () => {
  assert.equal(horaDoDia(9*3600e3 + 12*M, 0), '09:12');
  assert.equal(horaDoDia(25*3600e3 + 5*M, 0), '01:05');
  assert.equal(horaDoDia(-40*M, 0), '23:20', 'antes da meia-noite');
});
test('timersParaDock ordena por início e limita a 4', () => {
  const H = 3600e3;
  const t = (id, rodando, trechos) => ({ id, nome: `T${id}`, cor: `#00000${id}`, rodando, trechos });
  const snap = {
    agora: 12*H,
    grupos: [
      { id: 1, timers: [
        t(1, true, [{ inicio: 11*H, fim: null }]),
        t(2, false, [{ inicio: 8*H, fim: 9*H }]),
        t(3, true, [{ inicio: 8*H, fim: 9*H }, { inicio: 10*H, fim: null }]),
      ] },
      { id: null, timers: [
        t(4, true, [{ inicio: 11*H + 30*M, fim: null }]),
        t(5, true, [{ inicio: 9*H, fim: null }, { inicio: 9*H + 30*M, fim: null }]),
        t(6, true, [{ inicio: 11*H + 45*M, fim: null }]),
      ] },
    ],
  };
  const r = timersParaDock(snap);
  assert.deepEqual(r.map((x) => x.id), [5, 3, 1, 4]);
  assert.deepEqual(r[0], { id: 5, nome: 'T5', cor: '#000005', ms: 3*H, desde: '', rodando: true, situacao: null, pausado: false, etiqueta: null });
  assert.equal(r[1].ms, 3*H, 'soma o trecho fechado e o aberto');
  assert.equal(timersParaDock(snap, 13*H)[2].ms, 2*H, 'agora opcional');
  assert.deepEqual(timersParaDock({ agora: 0, grupos: [] }), []);
});
test('linhas: uma por linha, sem vazias nem repetidas', () => {
  assert.deepEqual(linhas('  claude \r\n\ncode\nClaude\n code '), ['claude', 'code']);
  assert.deepEqual(linhas(''), []);
});
test('minutos dos campos de tempo', () => {
  assert.equal(minutosDeSegundos(180), '3');
  assert.equal(minutosDeSegundos(90), '1.5');
  assert.equal(segundosDeMinutos('10'), 600);
  assert.equal(segundosDeMinutos(' 1,5 '), 90);
  for (const ruim of ['', '0', '-1', 'abc', '1441', '0.001', '0.5', '0,99', '1e9', 'Infinity', '9'.repeat(400)]) assert.equal(segundosDeMinutos(ruim), null, ruim);
  assert.equal(segundosDeMinutos('1'), 60);
  assert.equal(segundosDeMinutos('1440'), 86400);
});
test('faltamCriar: só os timers que ainda não foram criados', () => {
  assert.deepEqual(faltamCriar(['Projeto A', 'Reuniões', 'Estudo'], ['projeto a']), ['Reuniões', 'Estudo']);
  assert.deepEqual(faltamCriar(['A', 'B'], ['A', 'B']), []);
  assert.deepEqual(faltamCriar(['A'], []), ['A']);
});
test('recolherAoPerderFoco: só o expandido livre recolhe ao clicar fora', () => {
  const livre = { expandido: true, dialogo: false, redimensionando: false, soPeloDialogo: false };
  assert.equal(recolherAoPerderFoco(livre), true);
  assert.equal(recolherAoPerderFoco({ ...livre, expandido: false }), false);
  // antes do primeiro snapshot a etapa ainda não é conhecida
  assert.equal(recolherAoPerderFoco({ ...livre, expandido: null }), false);
  assert.equal(recolherAoPerderFoco({ ...livre, dialogo: true }), false);
  assert.equal(recolherAoPerderFoco({ ...livre, redimensionando: true }), false);
  // expandiu só para o diálogo de escolher o timer: recolhe quando ele fechar, não pelo foco
  assert.equal(recolherAoPerderFoco({ ...livre, soPeloDialogo: true }), false);
});
test('trocarTexto só escreve quando o texto muda', () => {
  let escritas = 0;
  let t = '1:32';
  const el = { get textContent() { return t; }, set textContent(v) { escritas += 1; t = v; } };
  trocarTexto(el, '1:32');
  assert.equal(escritas, 0);
  trocarTexto(el, '1:33');
  assert.equal(escritas, 1);
  assert.equal(t, '1:33');
});
test('situação "escrevendo": a etiqueta da leitura com o foco na caixa de mensagem', () => {
  assert.deepEqual(SITUACAO.escrevendo, { curta: 'escrevendo', longa: 'você escrevendo' });
  // conta como leitura: rodando, dá para encerrar
  assert.equal(podeEncerrar({ rodando: true, situacao: 'escrevendo' }), true);
});
test('timersParaDock com a situação: os rodando e depois os que esperam você', () => {
  const H = 3600e3;
  const t = (id, rodando, situacao, trechos = []) => ({ id, nome: `T${id}`, cor: '#000', rodando, situacao, trechos });
  // o lembrete à vista até a virada (o corte pelo tempo tem teste próprio, rodada 13)
  const snap = {
    agora: 12*H,
    config: { espera_dock_min: 0 },
    grupos: [{ id: 1, timers: [
      t(1, false, 'esperando', [{ inicio: 9*H, fim: 10*H }]),
      t(2, true, 'trabalhando', [{ inicio: 11*H, fim: null }]),
      t(3, false, null),
      t(4, true, 'lendo', [{ inicio: 10*H, fim: null }]),
    ] }],
  };
  const r = timersParaDock(snap, 12*H, true);
  assert.deepEqual(r.map((x) => [x.id, x.rodando, x.situacao]), [[4, true, 'lendo'], [2, true, 'trabalhando'], [1, false, 'esperando']]);
  assert.equal(r[2].ms, H, 'o parado mostra o tempo de hoje');
  // sem a opção: só os rodando, sem etiqueta
  assert.deepEqual(timersParaDock(snap, 12*H, false).map((x) => [x.id, x.situacao]), [[4, null], [2, null]]);
  // no máximo 4, os rodando primeiro
  const muitos = { agora: 0, config: { espera_dock_min: 0 }, grupos: [{ id: 1, timers: [t(9, false, 'esperando'), ...[1, 2, 3, 4].map((i) => t(i, true, null, [{ inicio: i, fim: null }]))] }] };
  assert.deepEqual(timersParaDock(muitos, 10, true).map((x) => x.id), [1, 2, 3, 4]);
});
test('tempo do timer e total com os ajustes à mão, nunca abaixo de zero', () => {
  const H = 3600e3;
  const a = { trechos: [{ inicio: 9*H, fim: 10*H }], ajuste_ms: 15*M };
  const b = { trechos: [{ inicio: 9*H + 30*M, fim: 10*H + 30*M }], ajuste_ms: -20*M };
  const c = { trechos: [{ inicio: 11*H, fim: 11*H + 10*M }], ajuste_ms: -30*M };
  assert.equal(msDoTimer(a, 0), 75*M);
  assert.equal(msDoTimer(b, 0), 40*M);
  assert.equal(msDoTimer(c, 0), 0, 'nunca abaixo de zero');
  assert.equal(msDoTimer({ trechos: [] }, 0), 0, 'sem ajuste_ms');
  // união 1h30 + 15 − 20; o C só tira os 10 min que tem
  assert.equal(totalDeTimers([a, b], 0), 85*M);
  assert.equal(totalDeTimers([a, b, c], 0), 85*M);
  assert.equal(totalDeTimers([{ trechos: [], ajuste_ms: 45*M }], 0), 45*M, 'timer sem trecho com ajuste');
  assert.equal(totalDeTimers([], 0), 0);
});
test('texto do ajuste e o "Ajustar hoje"', () => {
  assert.equal(textoDoAjuste(15*M), '+15 min');
  assert.equal(textoDoAjuste(-65*M), '−1h 05min');
  assert.equal(textoDoAjuste(120*M), '+2h 00min');
  assert.equal(deltaDoAjuste('+', '15'), 15*M);
  assert.equal(deltaDoAjuste('-', ' 30 '), -30*M);
  assert.equal(deltaDoAjuste('+', '1440'), 1440*M);
  for (const ruim of ['', '0', '1441', '1,5', '-5', 'abc', '15min']) assert.equal(deltaDoAjuste('+', ruim), null, ruim);
});
test('código e nome do destino: o link do Zoho Sprints só traz códigos', () => {
  const card = 'https://sprints.zoho.com/workspace/exemplo#P146/itemdetails/I7';
  const proj = 'https://sprints.zoho.com/workspace/exemplo#P146';
  assert.deepEqual(codigosDoLink(card), { projeto: 'P146', item: 'I7' });
  assert.deepEqual(codigosDoLink(proj), { projeto: 'P146', item: null });
  assert.deepEqual(codigosDoLink('https://sprints.zoho.com/workspace/exemplo#P146/kanban'), { projeto: 'P146', item: null });
  assert.deepEqual(codigosDoLink('https://exemplo.com/Pasta/I7x'), { projeto: null, item: null });
  assert.equal(rotuloDoDestino(card), 'I7', 'não mostra mais "exemplo"');
  assert.equal(rotuloDoDestino(proj), 'P146');
  assert.equal(rotuloDoDestino(card, ' INV - Inovação '), 'INV - Inovação · I7');
  assert.equal(rotuloDoDestino('https://exemplo.com/a/meu-card'), 'meu card', 'outro link: o fim do caminho');
  assert.equal(rotuloDoDestino('https://exemplo.com/a/meu-card', 'Nome'), 'Nome');
  assert.equal(rotuloDoDestino('texto solto'), 'texto solto');
});
test('minutos com limite: a inatividade vai até 30 min no modo dev', () => {
  assert.equal(segundosDeMinutos('30', 30), 1800);
  assert.equal(segundosDeMinutos('31', 30), null);
  assert.equal(segundosDeMinutos('1440'), 86400, 'padrão: 24 h');
});
test('total do dia nunca abaixo do maior timer (timers sobrepostos com ajuste negativo)', () => {
  const H = 3600e3;
  const a = { trechos: [{ inicio: 9*H, fim: 10*H }], ajuste_ms: -60*M };
  const b = { trechos: [{ inicio: 9*H, fim: 10*H }] };
  assert.equal(msDoTimer(a, 0), 0);
  assert.equal(msDoTimer(b, 0), 60*M);
  assert.equal(totalDeTimers([a, b], 0), 60*M, 'não 0');
  assert.equal(totalDeTimers([a, { ...b, ajuste_ms: 15*M }], 0), 75*M);
});
test('links de outro site não viram código do Zoho', () => {
  assert.equal(linkDoZohoSprints('https://sprints.zoho.com/workspace/exemplo#P146'), true);
  assert.equal(linkDoZohoSprints('https://sprints.zoho.eu/workspace/x#P1/itemdetails/I2'), true);
  assert.equal(linkDoZohoSprints('https://exemplo.com/docs/p10'), false);
  assert.equal(linkDoZohoSprints('https://zoho.com.exemplo.com/P1'), false);
  assert.equal(linkDoZohoSprints('não é link'), false);
  assert.deepEqual(codigosDoLink('https://exemplo.com/docs/p10'), { projeto: null, item: null });
  assert.equal(rotuloDoDestino('https://exemplo.com/docs/p10'), 'p10', 'o fim do caminho, não "P10"');
});
test('lembretes do dock: do que terminou antes, e quantos ficaram de fora', () => {
  const t = (id, rodando, situacao, espera_desde = null) => ({ id, nome: `T${id}`, cor: '#000', rodando, situacao, espera_desde, trechos: rodando ? [{ inicio: id, fim: null }] : [] });
  const snap = { agora: 100, config: { espera_dock_min: 0 }, grupos: [{ id: 1, timers: [
    t(1, true, 'trabalhando'), t(2, true, null), t(3, true, null),
    t(7, false, 'esperando', 50), t(8, false, 'esperando', 20), t(9, false, 'esperando', 30),
  ] }] };
  assert.deepEqual(timersParaDock(snap, 100, true).map((x) => x.id), [1, 2, 3, 8], 'o que espera há mais tempo');
  assert.equal(lembretesDeFora(snap, true), 2);
  assert.equal(lembretesDeFora(snap, false), 0);
  const poucos = { agora: 0, config: { espera_dock_min: 0 }, grupos: [{ id: 1, timers: [t(7, false, 'esperando', 50), t(8, false, 'esperando', 20)] }] };
  assert.deepEqual(timersParaDock(poucos, 0, true).map((x) => x.id), [8, 7]);
  assert.equal(lembretesDeFora(poucos, true), 0);
});
test('tamanho do arquivo de backup', async () => {
  const { tamanho } = await import('./formato.js');
  assert.equal(tamanho(820), '820 B');
  assert.equal(tamanho(49152), '48 KB');
  assert.equal(tamanho(1.25 * 1024 * 1024), '1,3 MB');
  assert.equal(tamanho(12 * 1024 * 1024), '12 MB');
});
test('podeEncerrar: rodando ou com o lembrete "esperando você"', () => {
  assert.equal(podeEncerrar({ rodando: true, situacao: null }), true);
  assert.equal(podeEncerrar({ rodando: false, situacao: 'esperando' }), true);
  assert.equal(podeEncerrar({ rodando: false, situacao: null, encerrado: true }), false);
  assert.equal(podeEncerrar({ rodando: false, situacao: 'lendo' }), false);
  assert.equal(podeEncerrar(null), false);
});
