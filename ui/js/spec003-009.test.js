// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Specs 003 e 009: o total do dia (a soma, o tempo real ou os dois), o "desde ontem" do número do
// timer, as opções do ▶ nos Ajustes e o nome do ▶ ("▶ ligado").
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { definirIdioma } from './textos.js';
import {
  totaisDoDia,
  textoDoTotal,
  totalDoDia,
  totalDoGrupo,
  totalPrincipal,
  dicaDosTotais,
  usarTotal,
  totalAtual,
  desdeDeAntes,
  hojeDoNumero,
  etiquetaComposta,
  partesDoTotal,
} from './formato.js';
import { campoTotalTopo, htmlPlay } from './ajustes.js';
import { dicaDoTempo } from './componentes.js';
import { gatilhos, DICAS } from './dicas.js';

const H = 3_600_000;
const M = 60_000;
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

/** Dois timers de 23/11: A com o ▶ das 9h às 12h, B com o Claude das 10h às 11h (1h em paralelo). */
const A = { id: 1, nome: 'A', trechos: [{ inicio: 9 * H, fim: 12 * H, origem: 'manual', atividade_id: 1 }], ajustes: [] };
const B = { id: 2, nome: 'B', trechos: [{ inicio: 10 * H, fim: 11 * H, origem: 'claude', atividade_id: 1 }], ajustes: [] };

test('spec 003: a soma de hoje, o tempo real e o "em paralelo"', () => {
  const t = totaisDoDia([A, B], 13 * H);
  assert.deepEqual(t, { soma: 4 * H, real: 3 * H });
  assert.equal(dicaDosTotais(t), 'a lançar: 4h 00min · tempo real: 3h 00min · 1h 00min em paralelo');
  // sem paralelo, sem o "em paralelo"
  assert.equal(dicaDosTotais(totaisDoDia([A], 13 * H)), 'a lançar: 3h 00min · tempo real: 3h 00min');
});

test('spec 003: o texto pela escolha, e cada número diz o que é', () => {
  const t = { soma: 12 * H + 2 * M, real: 8 * H + 42 * M };
  assert.equal(textoDoTotal(t, 'os_dois'), '12h 02min · 8h 42min real');
  assert.equal(textoDoTotal(t, 'soma'), '12h 02min a lançar');
  assert.equal(textoDoTotal(t, 'real'), '8h 42min real');
  // "Os dois" iguais (menos de 1 min de diferença): só o número
  assert.equal(textoDoTotal({ soma: 2 * H + 30_000, real: 2 * H }, 'os_dois'), '2h 00min');
  // o número que vale sozinho (dock, "Total geral")
  assert.equal(totalPrincipal(t, 'os_dois'), t.soma);
  assert.equal(totalPrincipal(t, 'soma'), t.soma);
  assert.equal(totalPrincipal(t, 'real'), t.real);
  em('en', () => {
    assert.equal(textoDoTotal(t, 'os_dois'), '12h 02m · 8h 42m real time');
    assert.equal(textoDoTotal(t, 'soma'), '12h 02m to log');
    assert.equal(textoDoTotal(t, 'real'), '8h 42m real time');
  });
});

test('spec 003: a escolha vale no topo e nos grupos (a mesma regra), e fica a de agora', () => {
  assert.equal(totalAtual(), 'os_dois', 'o padrão');
  try {
    usarTotal('real');
    assert.equal(totalDoDia([A, B], 13 * H).texto, '3h 00min real');
    assert.equal(totalDoGrupo([A, B], 13 * H).texto, '3h 00min real');
    usarTotal('soma');
    assert.equal(totalDoDia([A, B], 13 * H).texto, '4h 00min a lançar');
    assert.equal(totalDoGrupo([A, B], 13 * H).texto, '4h 00min a lançar');
    usarTotal('qualquer');
    assert.equal(totalAtual(), 'os_dois', 'desconhecida: o padrão');
    const d = totalDoDia([A, B], 13 * H);
    assert.equal(d.texto, '4h 00min · 3h 00min real');
    assert.equal(d.ms, 4 * H);
    // sem ajuste, o "tempo contado" é o próprio real e não se repete (revisão, M11)
    assert.equal(d.dica, 'a lançar: 4h 00min · tempo real: 3h 00min · 1h 00min em paralelo');
    assert.deepEqual([d.grande, d.pequeno], ['4h 00min', '· 3h 00min real']);
    // com ajuste hoje, a dica de antes vem primeiro
    const comAjuste = { ...A, ajustes: [{ atividade_id: 1, delta_ms: 15 * M, criado_em: 12 * H }], ajuste_ms: 15 * M };
    assert.match(totalDoDia([comAjuste, B], 13 * H).dica, /^tempo contado 3h 00min · ajustes \+15min \(A\)\na lançar: /);
  } finally {
    usarTotal('os_dois');
  }
});

test('spec 003: o topo, os espaços, o Foco e o dock leem a escolha da config', () => {
  for (const nome of ['painel.js', 'dock.js']) assert.match(fonte(nome), /usarTotal\(s\.config\?\.total_topo\)/, nome);
  // o topo: o número grande e, pequeno ao lado, o real ou o que ele é (revisão, I4)
  assert.match(fonte('painel.js'), /partesDoTotalDaAba\(snap, abaDoTopo\(snap, visao\), agora\)/);
  assert.match(fonte('painel.js'), /trocarTexto\(\$\('#total-sub'\), partes\.pequeno\)/);
  assert.match(fonte('../index.html'), /<b class="mono" id="total-dia">0min<\/b><em class="mono" id="total-sub"><\/em>/);
  assert.match(fonte('foco.js'), /totalDoDia\(snap\.grupos\.flatMap/);
  // spec 004: com o tempo de hoje dos arquivados\n  assert.match(fonte('dock.js'), /totaisDoDock\(todosTimers\(\), agora, undefined, escondidosDoDia\(snap\)\)\.ms/);
});

test('spec 003: a dica da primeira vez que a soma e o real divergem, só na janela principal', () => {
  const snap = (timers) => ({ agora: 13 * H, grupos: [{ id: 1, timers }], perguntas: [] });
  assert.ok(gatilhos(snap([A, B])).includes('total_dois'));
  assert.ok(!gatilhos(snap([A])).includes('total_dois'), 'sem paralelo, nada');
  assert.equal(DICAS.total_dois.alvo(), '.day');
  assert.ok(DICAS.total_dois.soNaPrincipal);
  assert.match(fonte('dock.js'), /DICAS\[id\]\?\.soNaPrincipal/);
  try {
    usarTotal('soma');
    assert.ok(!gatilhos(snap([A, B])).includes('total_dois'), 'só com "Os dois"');
  } finally {
    usarTotal('os_dois');
  }
});

test('US 003.3: o "desde ontem" à vista e quanto é de hoje, só com dias de antes', () => {
  const ontem = 9 * H - 24 * H;
  const t = { ...A, zerado_em: ontem, acumulado_antes: [{ atividade_id: 1, ms: 2 * H }] };
  assert.equal(desdeDeAntes(t, 13 * H), 'desde ontem ' + new Date(ontem).toTimeString().slice(0, 5));
  assert.equal(hojeDoNumero(t, 13 * H), '3h 00min hoje');
  // só de hoje: nada
  const hoje = { ...A, zerado_em: 9 * H, acumulado_antes: [] };
  assert.equal(desdeDeAntes(hoje, 13 * H), '');
  assert.equal(hojeDoNumero(hoje, 13 * H), '');
  // nunca zerado (`zerado_em` 0) com dias de antes: sem a data (revisão, I3)
  const nunca = { ...A, zerado_em: 0, acumulado_antes: [{ atividade_id: 1, ms: 5 * H }] };
  assert.equal(desdeDeAntes(nunca, 13 * H), 'inclui dias de antes');
  assert.equal(hojeDoNumero(nunca, 13 * H), '3h 00min hoje');
  em('en', () => assert.equal(desdeDeAntes(nunca, 13 * H), 'includes earlier days'));
  // e a dica do número também diz (re-revisão)
  assert.equal(dicaDoTempo(nunca, 13 * H), 'inclui dias de antes, 3h 00min hoje');
});

test('spec 003: as duas partes do topo pela escolha', () => {
  const t = { soma: 12 * H + 2 * M, real: 8 * H + 42 * M };
  assert.deepEqual(partesDoTotal(t, 'os_dois'), { grande: '12h 02min', pequeno: '· 8h 42min real' });
  assert.deepEqual(partesDoTotal({ soma: H, real: H }, 'os_dois'), { grande: '1h 00min', pequeno: '' });
  assert.deepEqual(partesDoTotal(t, 'soma'), { grande: '12h 02min', pequeno: 'a lançar' });
  assert.deepEqual(partesDoTotal(t, 'real'), { grande: '8h 42min', pequeno: 'real' });
  em('en', () => assert.deepEqual(partesDoTotal(t, 'os_dois'), { grande: '12h 02m', pequeno: '· 8h 42m real time' }));
});

test('Ajustes: o "Total do dia" (Painel) e a seção do ▶, nos dois idiomas', () => {
  const c = campoTotalTopo(undefined);
  assert.match(c, /name="total_topo" value="os_dois" checked/);
  assert.match(c, /Soma dos timers \(a lançar\)/);
  assert.match(c, /Tempo real \(sem contar em dobro\)/);
  assert.match(campoTotalTopo('real'), /value="real" checked/);
  const p = htmlPlay({});
  assert.match(p, /name="varios_play" >/, 'desligado por padrão');
  assert.match(p, /name="parar_play_claude" >/, 'desligado por padrão');
  assert.match(p, /Vários ▶ ao mesmo tempo/);
  assert.match(p, /Parar o ▶ quando o Claude termina e você sai da conversa/);
  assert.match(htmlPlay({ varios_play: true, parar_play_claude: true }), /name="varios_play" checked[^]*name="parar_play_claude" checked/);
  em('en', () => {
    assert.match(campoTotalTopo('os_dois'), /Day total[^]*Both[^]*Sum of the timers \(to log\)/);
    assert.match(htmlPlay({}), /Several ▶ at the same time[^]*Stop the ▶ when Claude finishes and you leave the conversation/);
  });
  const a = fonte('ajustes.js');
  assert.match(a, /secao\('play', TEXTOS\.secoes\.play, htmlPlay\(c\)\)/);
  assert.match(a, /nome === 'varios_play' \|\| nome === 'parar_play_claude'\) \{/);
  assert.match(a, /\['total_topo', c\.total_topo \?\? 'os_dois'\]/);
});

test('spec 009: o ▶ se chama "▶ ligado" e a etiqueta junta com " · "', () => {
  const t = (claude) => ({ rodando: true, situacao: 'manual', situacao_claude: claude, manual_atividade_id: 1, espera_ultima: 9 * H, trechos: [] });
  assert.equal(etiquetaComposta(t('lendo'), 9 * H, 0).texto, '▶ ligado · lendo');
  assert.equal(etiquetaComposta(t(null), 9 * H, 0).texto, '▶ ligado');
  em('en', () => assert.equal(etiquetaComposta(t('lendo'), 9 * H, 0).texto, '▶ on · reading'));
  // nunca "manual" nem "play manual" na tela para o ▶
  const textos = fonte('textos.js');
  assert.doesNotMatch(textos, /'play manual'|'Play manual'|: 'manual'|'Manual'/);
});
