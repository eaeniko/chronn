// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { htmlDoAviso } from './formato.js';

test('rodada 15: o aviso fora do dock é só a faixa, com o botão se houver', () => {
  assert.equal(
    htmlDoAviso({ id: 1, texto: 'Flow <x> encerrado.', botao: 'Desfazer' }),
    '<div class="faixa" role="status"><span>Flow &lt;x&gt; encerrado.</span><button type="button" data-qual="1">Desfazer</button></div>',
  );
  assert.equal(htmlDoAviso({ id: 2, texto: 'Não deu certo.', botao: null }), '<div class="faixa" role="status"><span>Não deu certo.</span></div>');
});

test('rodada 15: no dock, as faixas vão para fora (usarFaixa)', async () => {
  const { faixa, faixaEncerrado, usarFaixa, DESFAZER_ENCERRAR_MS } = await import('./editar.js');
  const vistos = [];
  usarFaixa((...a) => vistos.push(a));
  try {
    faixa('Não deu certo.');
    faixaEncerrado(7, 'Flow');
  } finally {
    usarFaixa(null);
  }
  assert.deepEqual(vistos[0], ['Não deu certo.', null, null, 4000]);
  assert.equal(vistos[1][0], 'Flow encerrado.');
  assert.equal(vistos[1][1], 'Desfazer');
  assert.equal(typeof vistos[1][2], 'function');
  assert.equal(vistos[1][3], DESFAZER_ENCERRAR_MS);
});

test('rodada 15: mover a conversa nova pelo "Outro timer…" vem desde o início, mesmo depois de 15 min', async () => {
  const { desdeInicioPadrao } = await import('./editar.js');
  const agora = 10 * 3_600_000;
  assert.equal(desdeInicioPadrao({ timer_id: 3, criada_em: agora - 40 * 60_000 }, agora), false, 'mover uma sessão antiga');
  assert.equal(desdeInicioPadrao({ timer_id: 3, criada_em: agora - 40 * 60_000, conversa_nova: true }, agora), true);
});

test('revisão da rodada 15: o "desde o início" do Mover do ✎ e da conversa nova diz que leva só hoje', async () => {
  const { textoDesdeInicio } = await import('./editar.js');
  assert.equal(textoDesdeInicio('Flow', false), 'O tempo que a sessão já contou vem para Flow.');
  assert.equal(textoDesdeInicio('Flow', true), 'O tempo que a sessão contou hoje vem para Flow. Os dias anteriores ficam onde estão.');
});
