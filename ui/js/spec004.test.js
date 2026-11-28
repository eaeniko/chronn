// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Spec 004: o total do dia com os arquivados, o arquivar com destino, os arquivados nos Ajustes e
// o apagar de verdade.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { definirIdioma } from './textos.js';
import { totalDoDia, escondidosDoDia } from './formato.js';
import { partesDoTotalDaAba } from './espacos.js';
import {
  htmlDestino,
  destinoDoForm,
  oQueContaNele,
  oQueAconteceAoApagar,
  avisoDoApagar,
  diaCurto,
  nomeConfere,
  temHoras,
  htmlArquivados,
} from './arquivados.js';

const H = 3_600_000;
const M = 60_000;
const fonte = (nome) => readFileSync(new URL(nome, import.meta.url), 'utf8');

function em(id, f) {
  definirIdioma(id);
  try {
    return f();
  } finally {
    definirIdioma('pt-BR');
  }
}

const trecho = (de, ate) => ({ inicio: de, fim: ate, origem: 'manual', atividade_id: 1 });
const timer = (id, nome, trechos, espaco_id = 1) => ({ id, nome, trechos, ajustes: [], ajuste_ms: 0, espaco_id });

test('spec 004.3: o total do dia soma o tempo de hoje dos arquivados, e a dica diz quanto é deles', () => {
  const visiveis = [timer(1, 'A', [trecho(9 * H, 11 * H)])];
  const escondidos = [{ id: 9, nome: 'Velho', trechos: [trecho(10 * H, 10 * H + 30 * M)], ajustes: [], ajuste_ms: 0 }];
  const sem = totalDoDia(visiveis, 12 * H, 'soma');
  const com = totalDoDia(visiveis, 12 * H, 'soma', escondidos);
  assert.equal(sem.ms, 2 * H);
  assert.equal(com.ms, 2 * H + 30 * M, 'a soma não encolhe');
  assert.match(com.dica, /inclui 30min de timers arquivados/);
  // o real não conta em dobro a meia hora em paralelo
  assert.equal(totalDoDia(visiveis, 12 * H, 'real', escondidos).ms, 2 * H);
  assert.doesNotMatch(sem.dica, /arquivados/);
  em('en', () => assert.match(totalDoDia(visiveis, 12 * H, 'soma', escondidos).dica, /includes 30m from archived timers/));
  assert.deepEqual(escondidosDoDia({}), []);
});

test('spec 004.3: o tempo dos arquivados entra só no total geral (sem abas ou em "Todos")', () => {
  const g1 = { id: 1, espaco_id: 1, timers: [timer(1, 'A', [trecho(9 * H, 10 * H)])] };
  const g2 = { id: 2, espaco_id: 2, timers: [timer(2, 'B', [trecho(9 * H, 10 * H)], 2)] };
  const escondidos_hoje = [{ id: 9, nome: 'Velho', trechos: [trecho(13 * H, 14 * H)], ajustes: [], ajuste_ms: 0 }];
  const espacos = [
    { id: 1, nome: 'Trabalho', ordem: 0, arquivado: false },
    { id: 2, nome: 'Pessoal', ordem: 1, arquivado: false },
  ];
  const snap = { agora: 15 * H, grupos: [g1, g2], espacos, escondidos_hoje, config: {} };
  assert.equal(partesDoTotalDaAba(snap, null, 15 * H).ms, 3 * H, 'Todos');
  assert.equal(partesDoTotalDaAba(snap, 1, 15 * H).ms, 1 * H, 'a aba de um espaço, não');
  // com um espaço só (sem abas), o topo é o total geral
  const so = { ...snap, grupos: [g1], espacos: [espacos[0]] };
  assert.equal(partesDoTotalDaAba(so, 1, 15 * H).ms, 2 * H);
});

test('spec 004.2: a escolha do destino só aparece com conversas ou regras, e vem pela sugestão', () => {
  const opcoes = [
    { id: 2, nome: 'Sprint 13' },
    { id: 3, nome: 'Outro' },
  ];
  assert.equal(htmlDestino({ conversas: 0, regras: [], sugestao: null }, opcoes), '');
  const uso = { conversas: 3, regras: ['D:\\proj'], sugestao: 2 };
  assert.equal(oQueContaNele(uso), '<b>3 conversas</b> e a regra da pasta <code class="aj-cam">D:\\proj</code>');
  const h = htmlDestino(uso, opcoes);
  assert.match(h, /contam nele\. O que fazer\?/);
  assert.match(h, /value="mover" checked/);
  assert.match(h, /<option value="2" selected>Sprint 13<\/option>/);
  assert.doesNotMatch(h, /value="nao" checked/);
  // sem sugestão: "Não contar mais" marcado, e o seletor começa vazio
  const sem = htmlDestino({ ...uso, sugestao: null }, opcoes);
  assert.match(sem, /value="nao" checked/);
  assert.match(sem, /<option value="" selected disabled>/);
  // sem nenhum outro timer, o "Mover" fica apagado
  assert.match(htmlDestino(uso, []), /value="mover" disabled/);
  // o grupo
  assert.match(htmlDestino({ conversas: 1, regras: ['a', 'b'], sugestao: null }, opcoes, true), /<b>1 conversa<\/b> e as regras de 2 pastas contam nos timers dele/);
  em('en', () => assert.match(htmlDestino(uso, opcoes), /<b>3 conversations<\/b> and the rule for the folder/));
  // revisão M5: a concordância com uma coisa só
  assert.match(htmlDestino({ conversas: 1, regras: [], sugestao: null }, opcoes), /<b>1 conversa<\/b> conta nele\./);
  assert.match(htmlDestino({ conversas: 0, regras: ['D:\\x'], sugestao: null }, opcoes), /D:\\x<\/code> conta nele\./);
  em('en', () => assert.match(htmlDestino({ conversas: 1, regras: [], sugestao: null }, opcoes), /conversation<\/b> counts in it\./));
});

test('spec 004.5 (revisão M5): o apagar diz que as conversas ficam sem timer e que as regras saem', () => {
  assert.equal(oQueAconteceAoApagar({ conversas: 0, regras: [] }), '');
  assert.equal(
    oQueAconteceAoApagar({ conversas: 3, regras: ['D:\\proj'] }),
    'As 3 conversas ficam sem timer. A regra da pasta <code class="aj-cam">D:\\proj</code> sai.',
  );
  assert.equal(oQueAconteceAoApagar({ conversas: 1, regras: ['a', 'b'] }), 'A conversa fica sem timer. As regras de 2 pastas saem.');
});

test('spec 004.2: o destino do formulário', () => {
  const form = (modo, destino) => ({
    querySelector: () => (modo ? { value: modo } : null),
    elements: { destino: destino === undefined ? undefined : { value: destino } },
  });
  assert.equal(destinoDoForm(form('nao', '2')), null);
  assert.equal(destinoDoForm(form(null)), null, 'sem a escolha (sem conversas nem regras)');
  assert.equal(destinoDoForm(form('mover', '2')), 2);
  assert.throws(() => destinoDoForm(form('mover', '')), /Escolha o timer de destino/);
});

test('spec 004.5: o apagar com horas diz o total, os dias e o período; o nome confere sem maiúsculas', () => {
  const uso = { total_ms: 12 * H + 40 * M, dias: 6, primeiro_dia: '2026-11-03', ultimo_dia: '2026-11-21', rodando: false };
  assert.equal(avisoDoApagar(uso), '12h 40min em 6 dias (de 03/11 a 21/11) somem do histórico. Os totais desses dias mudam, e o que já foi lançado deixa de bater.');
  assert.match(avisoDoApagar({ ...uso, dias: 1, ultimo_dia: '2026-11-03' }), /em 1 dia \(em 03\/11\)/);
  em('en', () => assert.match(avisoDoApagar(uso), /over 6 days \(from Nov 3 to Nov 21\)/));
  assert.equal(diaCurto('2026-11-03'), '03/11');
  assert.equal(diaCurto(null), '');
  assert.ok(nomeConfere(' sprint 12 ', 'Sprint 12'));
  assert.ok(!nomeConfere('Sprint 1', 'Sprint 12'));
  assert.ok(!nomeConfere('', ''));
  assert.ok(temHoras(uso));
  assert.ok(temHoras({ total_ms: 0, rodando: true }), 'rodando também pede o nome');
  assert.ok(!temHoras({ total_ms: 0, rodando: false }));
});

test('spec 004.4: a lista dos arquivados, com os grupos e os timers dentro deles', () => {
  assert.match(htmlArquivados(null), /Carregando/);
  assert.match(htmlArquivados({ grupos: [], timers: [] }), /<li class="aj-vazio">Nada arquivado<\/li>/);
  const dados = {
    grupos: [
      {
        id: 5,
        nome: 'Casa',
        cor: '#b07cff',
        espaco_id: 1,
        total_ms: 31 * H,
        timers: [
          { id: 7, nome: 'Reforma', cor: '#b07cff', arquivado: false, total_ms: 31 * H, grupo_nome: null, espaco_id: 1 },
          { id: 8, nome: 'Jardim', cor: '#b07cff', arquivado: true, total_ms: 0, grupo_nome: null, espaco_id: 1 },
        ],
      },
    ],
    timers: [
      { id: 9, nome: 'Sprint 12', cor: '#5bd1c7', arquivado: true, total_ms: 12 * H, grupo_nome: 'Trabalho', espaco_id: 2 },
      { id: 10, nome: 'Teste', cor: '#ffb13b', arquivado: true, total_ms: 0, grupo_nome: null, espaco_id: 1 },
    ],
  };
  const h = htmlArquivados(dados, [{ id: 1, nome: 'Geral', arquivado: false }]);
  assert.match(h, /<span class="aj-n">Casa<\/span><small>2 timers · 31h 00min<\/small><button type="button" class="bt mini" data-aj="desarquivar-grupo" data-id="5">Desarquivar<\/button><button type="button" class="bt mini perigo-txt" data-aj="apagar-grupo" data-id="5">Apagar…<\/button>/);
  assert.match(h, /<li class="aj-arq-sub"[^>]*><i><\/i><span class="aj-n">Jardim<\/span><small>arquivado à parte · 0min<\/small><\/li>/);
  assert.match(h, /Sprint 12<\/span><small>Trabalho · 12h 00min<\/small>/);
  // sem tempo, "Apagar" direto (sem reticências)
  assert.match(h, /data-aj="apagar-timer" data-id="10">Apagar<\/button>/);
  assert.match(h, /Teste<\/span><small>Sem grupo · 0min/);
  // com 2 espaços à vista, cada um diz o espaço
  const dois = htmlArquivados(dados, [
    { id: 1, nome: 'Geral', arquivado: false },
    { id: 2, nome: 'Trabalho', arquivado: false },
  ]);
  assert.match(dois, /Sprint 12<\/span><small>Trabalho · Trabalho · 12h 00min<\/small>/);
  em('en', () => assert.match(htmlArquivados({ grupos: [], timers: [] }), /Nothing archived/));
  // revisão M6: a falha ao ler não vira "Nada arquivado"
  assert.match(htmlArquivados({ erro: true }), /class="aj-erro"/);
  // revisão M5: o grupo com timers, mesmo sem horas, abre um diálogo (reticências)
  const g0 = { grupos: [{ ...dados.grupos[0], total_ms: 0 }], timers: [] };
  assert.match(htmlArquivados(g0), /data-aj="apagar-grupo" data-id="5">Apagar…/);
});

test('spec 004: o editar sem a caixa de arquivar, os menus e os Ajustes ligados', () => {
  const editar = fonte('editar.js');
  assert.doesNotMatch(editar, /caixa\('arquivado'/, 'o arquivar passa pelo diálogo com o destino');
  assert.match(editar, /data-arquivar/);
  assert.match(editar, /data-apagar/);
  const painel = fonte('painel.js');
  assert.match(painel, /b\.dataset\.mt === 'arquivar'\) pegarErro\(arquivarTimer\(snap, timerId\)\)/);
  assert.match(painel, /b\.dataset\.mt === 'apagar'\) pegarErro\(apagarTimer\(snap, t\)\)/);
  assert.match(painel, /b\.dataset\.mg === 'apagar'\) pegarErro\(apagarGrupo\(g\)\)/);
  assert.match(painel, /usarAbrirArquivados\(\(\) => abrirAjustes\('arquivados'\)\)/);
  const ajustes = fonte('ajustes.js');
  assert.match(ajustes, /'grupos', 'arquivados', 'backups'/);
  assert.match(ajustes, /chamar\('arquivados'\)/);
  assert.match(ajustes, /case 'apagar-grupo':/);
});
