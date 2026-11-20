// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { botaoEncerrar, textoEncerrado, marcaEncerrado, cartaoTimer, controles, opcoesDaPergunta, textoDaPergunta } from './componentes.js';
test('pergunta de sessão nova: "+ Novo timer…" fica na página, como o "Outro timer…"', () => {
  const p = {
    id: 4,
    sessao_id: 's"1',
    opcoes: [['ligar:2:regra', 'Projeto A · sempre nesta pasta'], ['outro', 'Outro timer…'], ['novo', '+ Novo timer…'], ['nao', 'Não contar']],
  };
  const h = opcoesDaPergunta(p);
  assert.match(h, /^<div class="o"><button class="y largo" title="Projeto A · sempre nesta pasta" data-acao="responder" data-pergunta="4" data-valor="ligar:2:regra">Projeto A · sempre nesta pasta<\/button>/);
  assert.match(h, /<button title="Outro timer…" data-acao="outro" data-sessao="s&quot;1" data-pergunta="4">Outro timer…<\/button>/);
  assert.match(h, /<button title="\+ Novo timer…" data-acao="novo-timer" data-sessao="s&quot;1" data-pergunta="4">\+ Novo timer…<\/button>/);
  assert.match(h, /<button class="largo" title="Não contar" data-acao="responder" data-pergunta="4" data-valor="nao">Não contar/);
  assert.doesNotMatch(h, /data-valor="novo"/, 'nunca vai ao núcleo como resposta');
});
test('grade da pergunta: destacado na linha dele, os outros aos pares, o que sobra largo', () => {
  const classes = (opcoes) =>
    [...opcoesDaPergunta({ id: 1, opcoes }).matchAll(/<button(?: class="([^"]*)")?/g)].map((m) => m[1] ?? '');
  const o = (n) => Array.from({ length: n }, (_, i) => [`v${i}`, `r${i}`]);
  assert.deepEqual(classes(o(4)), ['y largo', '', '', 'largo']);
  assert.deepEqual(classes(o(3)), ['y largo', '', '']);
  assert.deepEqual(classes(o(2)), ['y', ''], 'dois: lado a lado');
  assert.deepEqual(classes(o(1)), ['y largo']);
});
test('texto da pergunta de sessão nova: a pasta numa linha, inteira na dica', () => {
  const p = { tipo: 'sessao_nova', texto: 'x', pasta: 'G:/Projeto <Solution>/Flow', pasta_curta: 'Projeto <Solution>/Flow' };
  const h = textoDaPergunta(p);
  assert.match(h, /<div class="ask-pasta" title="G:\/Projeto &lt;Solution&gt;\/Flow">Projeto &lt;Solution&gt;\/Flow<\/div>/);
  assert.equal(textoDaPergunta({ tipo: 'app_fechado', texto: 'Contar <já>?' }), '<div class="ask-t">Contar &lt;já&gt;?</div>');
});
const timer = (x) => ({ id: 3, nome: 'Cliente <F>', cor: '#a00', rodando: false, situacao: null, encerrado: false, trechos: [], ajuste_ms: 0, ...x });
test('botaoEncerrar: só com o que encerrar; fixo fica apagado no lugar', () => {
  assert.equal(botaoEncerrar(timer(), 'pe'), '');
  const b = botaoEncerrar(timer({ rodando: true }), 'pe');
  assert.match(b, /data-acao="encerrar" data-timer="3"/);
  assert.match(b, /aria-label="Encerrar Cliente &lt;F&gt;"/);
  assert.doesNotMatch(b, /disabled/);
  assert.match(botaoEncerrar(timer({ situacao: 'esperando' }), 'p'), /data-acao="encerrar"/);
  const apagado = botaoEncerrar(timer({ encerrado: true }), 'ec', true);
  assert.match(apagado, / disabled>/);
  assert.match(apagado, /title="Encerrado: volta/);
});
test('cartão e controles levam o ■; o texto de encerrado só com o timer parado', () => {
  assert.match(cartaoTimer(timer({ rodando: true }), 0, false), /data-acao="encerrar"/);
  assert.doesNotMatch(cartaoTimer(timer(), 0, false), /data-acao="encerrar"/);
  assert.match(controles(timer()), /data-acao="encerrar"[^>]* disabled>/);
  assert.equal(textoEncerrado(timer({ encerrado: true })), 'encerrado · volta com uma mensagem sua ou com o play');
  assert.equal(textoEncerrado(timer({ encerrado: true, rodando: true })), '');
  assert.match(marcaEncerrado(timer({ encerrado: true })), /^<span class="encerrado">encerrado · /);
  assert.equal(marcaEncerrado(timer()), '');
});
test('volta da inatividade: "Retomar agora" destacado na linha dele, depois "Contar o tempo fora" e "Deixar pausado"', () => {
  const p = {
    id: 9,
    tipo: 'volta_inatividade',
    texto: 'Você ficou 26 min sem mexer no PC. Retomar Cliente A (reunião)?',
    opcoes: [['retomar', 'Retomar agora'], ['contar', 'Contar o tempo fora'], ['pausado', 'Deixar pausado']],
  };
  const h = opcoesDaPergunta(p);
  assert.match(h, /^<div class="o"><button class="y largo" title="Retomar agora" data-acao="responder" data-pergunta="9" data-valor="retomar">Retomar agora<\/button>/);
  assert.match(h, /<button title="Contar o tempo fora" data-acao="responder" data-pergunta="9" data-valor="contar">/);
  assert.match(h, /<button title="Deixar pausado" data-acao="responder" data-pergunta="9" data-valor="pausado">Deixar pausado<\/button><\/div>$/);
});
test('rodada 15: conversa nova numa pasta com regra, Certo destacado, Outro e Novo na página', () => {
  const p = {
    id: 9,
    tipo: 'conversa_nova',
    sessao_id: 's9',
    texto: 'Conversa nova: Flow · contando em Projeto A pela pasta',
    opcoes: [['certo', 'Certo'], ['outro', 'Outro timer…'], ['novo', '+ Novo timer…']],
  };
  const h = opcoesDaPergunta(p);
  assert.match(h, /<button class="y largo" title="Certo" data-acao="responder" data-pergunta="9" data-valor="certo">Certo<\/button>/);
  assert.match(h, /data-acao="outro" data-sessao="s9"/);
  assert.match(h, /data-acao="novo-timer" data-sessao="s9"/);
  assert.equal(textoDaPergunta(p), '<div class="ask-t">Conversa nova: Flow · contando em Projeto A pela pasta</div>');
});
test('rodada 15: cada sessão ligada tem o ✎ e o ✕ (desligar) continua', async () => {
  const { destino } = await import('./componentes.js');
  const h = destino({ id: 4, link_card: null, sessoes: [{ id: 's"1', nome: 'Flow', cwd: 'G:/Flow', estado: 'parada' }] }, null);
  assert.match(h, /<button class="x" data-acao="editar-sessao" data-sessao="s&quot;1" data-timer="4" title="Ver e mudar a sessão" aria-label="Ver e mudar Flow">✎<\/button>/);
  assert.match(h, /data-acao="desligar" data-sessao="s&quot;1"/);
});
