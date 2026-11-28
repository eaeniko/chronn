// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 13: as etiquetas "manual" e "segundo plano" e quanto tempo o "esperando você" fica à
// vista em cada lugar (dock recolhido, dock expandido e app).
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { SITUACAO, ehLembrete, lembreteVisivel, situacaoNaTela, timersParaDock, lembretesDeFora, podeEncerrar, textoDosDeFora, dicaDoLembrete } from './formato.js';
import { cartaoTimer, relogioFoco, usarAtividades, usarEsperaApp, etiquetaSituacao } from './componentes.js';
import { campoEspera, ESPERA_OPCOES } from './ajustes.js';

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
  espera_desde: null,
  espera_ultima: null,
  atividade_padrao_id: 1,
  trechos: [],
  ajustes: [],
  ajuste_ms: 0,
  sessoes: [],
  ...extra,
});

test('"manual" e "segundo plano": as etiquetas e quem é lembrete', () => {
  assert.deepEqual(SITUACAO.manual, { curta: '▶ ligado', longa: 'você deu ▶' });
  assert.deepEqual(SITUACAO.segundo_plano, { curta: 'segundo plano', longa: 'segundo plano' });
  assert.equal(ehLembrete('esperando'), true);
  assert.equal(ehLembrete('segundo_plano'), true);
  assert.equal(ehLembrete('manual'), false);
  assert.equal(podeEncerrar({ rodando: false, situacao: 'segundo_plano' }), true, 'o ■ tira o lembrete');
});

test('o cartão mostra "manual" com o ▶ rodando e "segundo plano" parado', () => {
  usarEsperaApp(0);
  const manual = timer(1, { rodando: true, situacao: 'manual', manual_atividade_id: 1, trechos: [{ inicio: 9 * H, fim: null, origem: 'manual', atividade_id: 1 }] });
  assert.match(cartaoTimer(manual, 10 * H, false), /<div class="kx"><small class="ksit manual" title="▶ ligado"><span class="s-manual">▶ ligado<\/span><\/small><\/div>/);
  const sp = timer(2, { situacao: 'segundo_plano', espera_ultima: 9 * H });
  assert.match(cartaoTimer(sp, 15 * H, false), /<small class="ksit segundo_plano" title="segundo plano"><span class="s-segundo_plano">segundo plano<\/span><\/small>/);
  // no painel do timer, parado, o lembrete fica no lugar do motivo
  assert.match(relogioFoco(sp, 15 * H), /<div class="why"><span class="lembrete segundo_plano">segundo plano<\/span><\/div>/);
  // a reunião tem a etiqueta dela
  const reu = timer(3, { rodando: true, situacao: 'reuniao', manual_atividade_id: 2 });
  assert.equal(etiquetaSituacao(reu, 10 * H), '');
});

test('o "esperando você" no dock recolhido some depois do tempo dele (30 min por padrão)', () => {
  const snap = (cfg) => ({
    agora: 10 * H,
    config: cfg,
    grupos: [{ id: 1, timers: [
      timer(1, { situacao: 'esperando', espera_desde: 9 * H, espera_ultima: 9 * H + 45 * M }),
      timer(2, { situacao: 'esperando', espera_desde: 8 * H, espera_ultima: 8 * H + 10 * M }),
      timer(3, { situacao: 'segundo_plano', espera_desde: 9 * H + 50 * M, espera_ultima: 9 * H + 50 * M }),
    ] }],
  });
  // 30 min: o de 15 min atrás e o "segundo plano" de 10 min atrás ficam; o de 1h 50min atrás sai
  assert.deepEqual(timersParaDock(snap({}), 10 * H, true).map((x) => x.id), [1, 3]);
  assert.deepEqual(timersParaDock(snap({ espera_dock_min: 15 }), 10 * H, true).map((x) => x.id), [3]);
  assert.deepEqual(timersParaDock(snap({ espera_dock_min: 120 }), 10 * H, true).map((x) => x.id), [2, 1, 3]);
  assert.deepEqual(timersParaDock(snap({ espera_dock_min: 0 }), 10 * H, true).map((x) => x.id), [2, 1, 3], 'até a virada');
  assert.equal(lembretesDeFora(snap({ espera_dock_min: 0 }), true), 0);
});

test('o dock recolhido distingue "segundo plano" na dica e no "+N"', () => {
  assert.match(dicaDoLembrete({ nome: 'Cliente C', situacao: 'esperando' }), /^Cliente C: uma sessão do Claude espera sua resposta/);
  assert.match(dicaDoLembrete({ nome: 'Site', situacao: 'segundo_plano' }), /^Site: uma tarefa em segundo plano ainda roda; o Claude continua quando ela terminar/);
  // 4 rodando: os 3 lembretes ficam de fora
  const rodando = [1, 2, 3, 4].map((i) => timer(i, { rodando: true, trechos: [{ inicio: i, fim: null }] }));
  const snap = (lembretes) => ({ agora: 10 * H, config: { espera_dock_min: 0 }, grupos: [{ id: 1, timers: [...rodando, ...lembretes] }] });
  const esp = (id) => timer(id, { situacao: 'esperando', espera_desde: id, espera_ultima: 9 * H });
  const sp = (id) => timer(id, { situacao: 'segundo_plano', espera_desde: id, espera_ultima: 9 * H });
  assert.deepEqual(textoDosDeFora(snap([esp(7), esp(8), sp(9)]), true), { texto: '+2 esperando · +1 segundo plano', dica: 'Mais 2 esperando você e 1 em segundo plano: veja no dock expandido' });
  assert.deepEqual(textoDosDeFora(snap([sp(9)]), true), { texto: '+1 segundo plano', dica: 'Mais 1 em segundo plano: veja no dock expandido' });
  assert.equal(textoDosDeFora(snap([]), true), null);
  assert.equal(lembretesDeFora(snap([esp(7), sp(9)]), true), 2);
});

test('no dock expandido e no app, até a virada por padrão, ou pelo tempo escolhido', () => {
  const t = timer(1, { situacao: 'esperando', espera_ultima: 8 * H });
  assert.equal(situacaoNaTela(t, 23 * H, 0), 'esperando', 'até a virada (o núcleo corta na virada)');
  assert.equal(situacaoNaTela(t, 9 * H - 1, 60), 'esperando');
  assert.equal(situacaoNaTela(t, 9 * H, 60), null);
  assert.equal(lembreteVisivel(timer(2, { situacao: 'esperando' }), 9 * H, 60), false, 'sem o instante: some');
  // o que não é lembrete não some
  assert.equal(situacaoNaTela(timer(3, { situacao: 'manual', rodando: true }), 23 * H, 15), 'manual');
  // no cartão, com o app em 1 h
  usarEsperaApp(60);
  assert.doesNotMatch(cartaoTimer(t, 10 * H, false), /ksit/);
  assert.match(cartaoTimer(t, 8 * H + 30 * M, false), /<small class="ksit esperando"/);
  usarEsperaApp(0);
});

test('Ajustes → "Esperando você": as cinco opções nos dois lugares, com os padrões', () => {
  assert.deepEqual(ESPERA_OPCOES.map(([v]) => v), [15, 30, 60, 120, 0]);
  const h = campoEspera({});
  assert.match(h, /Esperando você, no dock recolhido[\s\S]*name="espera_dock_min" value="30" checked><span>30 min/);
  assert.match(h, /Esperando você, no dock expandido e no app[\s\S]*name="espera_app_min" value="0" checked><span>Até a virada do dia/);
  assert.equal((h.match(/name="espera_dock_min"/g) ?? []).length, 5);
  assert.match(campoEspera({ espera_dock_min: 120, espera_app_min: 15 }), /value="120" checked[\s\S]*value="15" checked/);
});
