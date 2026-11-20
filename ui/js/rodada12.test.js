// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 12: a dica do total do dia (de onde vem o número), a marca de ajuste no cartão do Painel
// e a opção do dock nos Ajustes.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { dicaDoTotalDoDia, totalDeTimers, curto } from './formato.js';
import { cartaoTimer, usarAtividades } from './componentes.js';
import { campoDockModo, campoAoMinimizar } from './ajustes.js';

const S = 1000;
const M = 60 * S;
const H = 60 * M;
usarAtividades([{ id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivada: false }]);

const timer = (id, nome, trechos, ajustes = []) => ({
  id,
  nome,
  cor: '#4f8cff',
  rodando: false,
  atividade_padrao_id: 1,
  trechos,
  ajustes,
  ajuste_ms: ajustes.reduce((s, a) => s + a.delta_ms, 0),
  sessoes: [],
});
const aj = (id, delta_ms) => ({ id, delta_ms, motivo: null, atividade_id: 1, criado_em: 5 * H });

test('dica do total do dia sem ajuste: só o tempo contado', () => {
  const ts = [timer(1, 'Zoho', [{ inicio: 9 * H, fim: 9 * H + 55 * S, origem: 'claude', atividade_id: 1 }]), timer(2, 'Flow', [])];
  assert.equal(dicaDoTotalDoDia(ts, 10 * H), 'tempo contado 0min');
  assert.equal(dicaDoTotalDoDia([], 10 * H), 'tempo contado 0min');
});

test('dica do total do dia com um ajuste: o caso do Cliente C (+30min e 1min contado)', () => {
  const ts = [
    timer(1, 'Zoho', [{ inicio: 9 * H, fim: 9 * H + 70 * S, origem: 'claude', atividade_id: 1 }]),
    timer(2, 'Cliente C', [], [aj(1, 30 * M)]),
  ];
  assert.equal(curto(totalDeTimers(ts, 10 * H)), '31min');
  assert.equal(dicaDoTotalDoDia(ts, 10 * H), 'tempo contado 1min · ajustes +30min (Cliente C)');
  // um ajuste negativo: o que ele tirou do total (nunca mais do que o timer tinha)
  const menos = [timer(1, 'Zoho', [{ inicio: 9 * H, fim: 10 * H, origem: 'manual', atividade_id: 1 }], [aj(2, -90 * M)])];
  assert.equal(dicaDoTotalDoDia(menos, 10 * H), 'tempo contado 1h 00min · ajustes −1h 00min (Zoho)');
});

test('dica do total do dia com cinco ajustes: até 3 nomes e depois "+N"', () => {
  const ts = ['A', 'B', 'C', 'D', 'E'].map((n, i) => timer(i + 1, n, [], [aj(i + 1, 10 * M)]));
  ts.push(timer(9, 'Sem ajuste', [{ inicio: 9 * H, fim: 10 * H, origem: 'manual', atividade_id: 1 }]));
  assert.equal(dicaDoTotalDoDia(ts, 10 * H), 'tempo contado 1h 00min · ajustes +50min (A, B, C +2)');
});

test('o cartão do Painel mostra o ajuste de hoje embaixo do nome', () => {
  const cliente_c = { ...timer(2, 'Cliente C', [], [{ ...aj(1, 30 * M), motivo: 'reunião' }]), zerado_em: 0 };
  assert.match(cartaoTimer(cliente_c, 10 * H, false), /<div class="kx"><em class="mao" title="ajustado à mão: \+30 min · reunião">\+30 min à mão<\/em><\/div>/);
  // com a reunião rodando, a marca curta ao lado da etiqueta dela
  const reu = { ...cliente_c, manual_atividade_id: 2, trechos: [{ inicio: 9 * H, fim: null, origem: 'manual', atividade_id: 2 }] };
  assert.match(cartaoTimer(reu, 10 * H, false), /<div class="kx"><div class="kreu"[^]*<\/div><em class="mao"[^>]*>\+30 min<\/em><\/div>/);
  // sem ajuste, sem a marca
  assert.doesNotMatch(cartaoTimer(timer(3, 'Flow', []), 10 * H, false), /class="mao"/);
  // ajuste feito antes do zerar não entra no número, nem na marca
  assert.doesNotMatch(cartaoTimer({ ...cliente_c, zerado_em: 6 * H }, 10 * H, false), /class="mao"/);
});

test('Ajustes: o dock sempre à vista ou só com pergunta, e o minimizar sem prometer o dock', () => {
  assert.match(campoDockModo(undefined), /value="sempre" checked><span>Sempre à vista/);
  assert.match(campoDockModo('perguntas'), /value="perguntas" checked><span>Só quando houver uma pergunta/);
  assert.match(campoAoMinimizar('dock', 'sempre'), /value="dock" checked><span>Abrir o dock/);
  const escondido = campoAoMinimizar('dock', 'perguntas');
  assert.match(escondido, /value="dock" checked><span>Dock \(escondido\)/);
  assert.doesNotMatch(escondido, /Abrir o dock/);
  assert.match(escondido, /com o dock só nas perguntas, só minimiza/);
});
