// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 27, parte 2: a origem da sessão ("Vem de"), a pergunta "Em qual timer contar?" (opção A:
// o cartão de 400 px com as linhas Sessão, Vem de e Pasta, o "Sempre que for na pasta" com a lista
// das pastas) e a lista de sessões para ligar (as disponíveis primeiro).
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { TEXTOS, definirIdioma } from './textos.js';
import {
  perguntaSessaoNova,
  htmlDaPergunta,
  escolhaDaPergunta,
  mudarEscolha,
  esquecerEscolhas,
  sempreDoBotao,
  pedidoDeLigar,
  textoDaPergunta,
  opcoesDaPergunta,
} from './componentes.js';
import { camposDaSessao, campoRegra, linhaDaSessao, ordemDeLigar } from './editar.js';

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

test('o rótulo da origem nos dois idiomas; sem origem, nada', () => {
  const pt = ['claude-desktop', 'cli', 'claude-vscode', 'chat', 'sdk-ts'].map((o) => TEXTOS.origemDaSessao(o));
  assert.deepEqual(pt, ['app do Claude', 'terminal', 'VS Code', 'Chat do app do Claude', 'outro programa']);
  const en = em('en', () => ['claude-desktop', 'cli', 'claude-vscode', 'chat', 'sdk-ts'].map((o) => TEXTOS.origemDaSessao(o)));
  assert.deepEqual(en, ['Claude app', 'terminal', 'VS Code', 'Claude app Chat', 'another program']);
  assert.equal(TEXTOS.origemDaSessao(null), null);
  assert.equal(TEXTOS.origemDaSessao(''), null);
});

// ---- a pergunta de sessão nova (opção A) ----

const PASTAS = [
  { pasta: 'D:/Projetos', curta: 'D:/Projetos', comecou: true, agora: false },
  { pasta: 'D:/Projetos/cliente', curta: 'Projetos/cliente', comecou: false, agora: false },
  { pasta: 'D:/Projetos/cliente/app', curta: 'cliente/app', comecou: false, agora: true },
];
const pergunta = (o = {}) => ({
  id: 7,
  tipo: 'sessao_nova',
  texto: 'x',
  sessao_id: 's9',
  sessao_nome: 'Plano do app <novo>',
  origem: 'claude-desktop',
  pasta: 'D:/Projetos/cliente/app',
  pasta_curta: 'cliente/app',
  pastas: PASTAS,
  opcoes: [
    ['ligar:3', 'Contar em App mobile'],
    ['outro', 'Outro timer…'],
    ['novo', '+ Novo timer…'],
    ['nao', 'Não contar'],
  ],
  ...o,
});
const SEM_SUGESTAO = [
  ['outro', 'Escolher o timer…'],
  ['novo', '+ Novo timer…'],
  ['nao', 'Não contar'],
];
/** Marca a caixa (ou escolhe a pasta) como a pessoa: o `change` de um input de mentira. */
const marcar = (id, caixa, valor = null) =>
  mudarEscolha({ dataset: caixa ? { pergunta: String(id), pgSempre: '' } : { pergunta: String(id), pgPasta: '' }, checked: true, value: valor });

test('a pergunta de sessão nova: em cima o "Sessão nova do Claude", a pergunta, e as linhas Sessão, Vem de e Pasta', () => {
  esquecerEscolhas([]);
  const h = perguntaSessaoNova(pergunta());
  assert.match(h, /^<div class="sn-sobre">Sessão nova do Claude<\/div><div class="sn-titulo">Em qual timer contar\?<\/div>/);
  assert.match(h, /<dt>Sessão<\/dt><dd class="sn-nome">Plano do app &lt;novo&gt;<\/dd>/);
  assert.match(h, /<dt>Vem de<\/dt><dd>app do Claude<\/dd>/);
  assert.match(h, /<dt>Pasta<\/dt><dd class="sn-pasta mono" title="D:\/Projetos\/cliente\/app">cliente\/app<\/dd>/);
  // o principal: "Contar em X" e, menor, "só esta sessão"; responde só esta sessão
  assert.match(h, /<button class="y largo sn-principal"[^>]*data-valor="ligar:3"><span class="sn-bt">Contar em App mobile<\/span><small>só esta sessão<\/small><\/button>/);
  assert.match(h, /data-acao="outro" data-sessao="s9" data-pergunta="7" data-sempre="0">Outro timer…/);
  assert.match(h, /data-acao="novo-timer" data-sessao="s9" data-pergunta="7" data-sempre="0">\+ Novo timer…/);
  assert.match(h, /<button class="largo sn-nao"[^>]*data-valor="nao">Não contar<\/button>/);
  // embaixo, a caixa desmarcada, sem a lista, com a ajuda
  assert.match(h, /<div class="sn-sempre" data-tauri-drag-region="false">/);
  assert.match(h, /<input type="checkbox" data-pg-sempre data-pergunta="7" data-pasta="D:\/Projetos\/cliente\/app"><span>Sempre que for na pasta<\/span>/);
  assert.doesNotMatch(h, /sn-pastas/);
  assert.match(h, /Marcado, as próximas sessões nessa pasta contam sozinhas no timer que você escolher\./);
  assert.equal(htmlDaPergunta(pergunta()), h);
  // sem nome ainda; sem origem, a linha "Vem de" não aparece
  const sem = perguntaSessaoNova(pergunta({ sessao_nome: null, origem: null }));
  assert.match(sem, /<dd class="sn-nome"><i>sem nome ainda<\/i><\/dd>/);
  assert.doesNotMatch(sem, /Vem de/);
});

test('com a caixa marcada: a lista das pastas, a de agora escolhida, e o principal leva a pasta', () => {
  esquecerEscolhas([]);
  assert.ok(marcar(7, true));
  let h = perguntaSessaoNova(pergunta());
  assert.match(h, /data-pg-sempre data-pergunta="7" data-pasta="D:\/Projetos\/cliente\/app" checked>/);
  assert.match(h, /<div class="sn-pastas" role="radiogroup"/);
  const itens = [...h.matchAll(/<label class="sn-pi">.*?<\/label>/g)].map((m) => m[0]);
  assert.equal(itens.length, 3);
  assert.match(itens[0], /value="D:\/Projetos"[^>]*><span class="mono" title="D:\/Projetos">D:\/Projetos<\/span><small>onde começou<\/small>/);
  assert.doesNotMatch(itens[1], /<small>/);
  assert.match(itens[2], /value="D:\/Projetos\/cliente\/app"[^>]* checked>.*<small>onde está agora<\/small>/);
  assert.match(h, /data-valor="ligar:3:pasta:D:\/Projetos\/cliente\/app"><span class="sn-bt">Contar em App mobile<\/span><small>esta e as próximas sessões na pasta<\/small>/);
  assert.match(h, /data-acao="outro"[^>]*data-sempre="1" data-pasta="D:\/Projetos\/cliente\/app">/);
  assert.doesNotMatch(h, /A pasta de cima/);
  // a pasta de cima: o aviso de que ela pega todos os projetos
  assert.ok(marcar(7, false, 'D:/Projetos'));
  h = perguntaSessaoNova(pergunta());
  assert.match(h, /data-valor="ligar:3:pasta:D:\/Projetos"/);
  assert.match(h, /<b>A pasta de cima pega todos os projetos dentro dela\.<\/b>/);
  assert.deepEqual(escolhaDaPergunta(pergunta()).pasta, PASTAS[0]);
  // a pasta que saiu da lista volta para a de agora; a pergunta que saiu esquece a escolha
  assert.deepEqual(escolhaDaPergunta(pergunta({ pastas: PASTAS.slice(1) })).pasta, PASTAS[2]);
  esquecerEscolhas([8]);
  assert.equal(escolhaDaPergunta(pergunta()).sempre, false);
  // só uma pasta: as duas marcas nela, e sem o aviso da de cima
  marcar(7, true);
  const uma = perguntaSessaoNova(pergunta({ pastas: [{ pasta: 'D:/app', curta: 'D:/app', comecou: true, agora: true }] }));
  assert.match(uma, /<small>onde começou · onde está agora<\/small>/);
  assert.doesNotMatch(uma, /A pasta de cima/);
  esquecerEscolhas([]);
  // o que não é da pergunta não muda nada
  assert.equal(mudarEscolha({ dataset: { pergunta: '7' } }), false);
  assert.equal(mudarEscolha({ dataset: {} }), false);
});

test('sem sugestão, "Escolher o timer…" é o principal; o Chat não tem a Pasta nem o "Sempre que for na pasta"', () => {
  esquecerEscolhas([]);
  const sem = perguntaSessaoNova(pergunta({ opcoes: SEM_SUGESTAO }));
  assert.match(sem, /<button class="y largo sn-principal" title="Escolher o timer…" data-acao="outro"[^>]*>Escolher o timer…<\/button>/);
  assert.match(sem, /<button class="largo" title="\+ Novo timer…" data-acao="novo-timer"/);
  const chat = perguntaSessaoNova(
    pergunta({ sessao_id: 'chat:0a1b', sessao_nome: 'Receita', origem: 'chat', pasta: null, pasta_curta: null, pastas: [], opcoes: SEM_SUGESTAO }),
  );
  assert.match(chat, /<dt>Vem de<\/dt><dd>Chat do app do Claude<\/dd>/);
  assert.doesNotMatch(chat, /<dt>Pasta<\/dt>|sn-sempre|data-sempre/);
  // em inglês
  const en = em('en', () => perguntaSessaoNova(pergunta()));
  assert.match(en, /New Claude session<\/div><div class="sn-titulo">Which timer should count it\?/);
  assert.match(en, /<dt>Session<\/dt>.*<dt>From<\/dt><dd>Claude app<\/dd>.*<dt>Folder<\/dt>/);
  assert.match(en, /<small>just this session<\/small>/);
  assert.match(en, /Always when it’s in the folder/);
  // a faixa da janela principal: o Chat sem a linha da pasta
  assert.equal(textoDaPergunta({ tipo: 'sessao_nova', texto: 'x', pasta: null }), '<div class="ask-t">Sessão nova do Claude. Em qual timer contar?</div>');
});

test('revisão M5: na faixa da janela principal, o "Outro timer…" e o "+ Novo timer…" da sessão nova vêm desmarcados', () => {
  const opcoes = [['ligar:3', 'Contar em X'], ['outro', 'Outro timer…'], ['novo', '+ Novo timer…'], ['nao', 'Não contar']];
  const h = opcoesDaPergunta({ id: 7, tipo: 'sessao_nova', sessao_id: 's1', opcoes });
  assert.equal([...h.matchAll(/data-sempre="0"/g)].length, 2, 'só o "outro" e o "novo"');
  assert.match(h, /data-acao="outro" data-sessao="s1" data-pergunta="7" data-sempre="0"/);
  assert.match(h, /data-acao="novo-timer" data-sessao="s1" data-pergunta="7" data-sempre="0"/);
  assert.deepEqual(sempreDoBotao({ sessao: 's1', pergunta: '7', sempre: '0' }), { sempre: false, pasta: null });
  // a conversa nova (pasta com regra) não muda
  assert.doesNotMatch(opcoesDaPergunta({ id: 8, tipo: 'conversa_nova', sessao_id: 's1', opcoes }), /data-sempre/);
  // a janela principal passa a caixa ao "Ligar a um timer" e ao "+ Novo timer…"
  const painel = fonte('./painel.js');
  assert.match(painel, /case 'outro': \{\n[^\n]*\n      const sempre = sempreDoBotao\(alvo\.dataset\);\n      await ligarATimer\(lerSnap, alvo\.dataset\.sessao, \(sessaoId\) => agir\('pedir_novo_timer', \{ sessao_id: sessaoId, \.\.\.sempre \}\), \{\}, sempre\);/);
  assert.match(painel, /await agir\('pedir_novo_timer', \{ sessao_id: alvo\.dataset\.sessao, \.\.\.sempreDoBotao\(alvo\.dataset\) \}\);/);
});

test('revisão M6: ao marcar a caixa, a pasta escolhida fica fixa e não pula com a de agora', () => {
  const pastas = (agora) =>
    ['D:/P', 'D:/P/a', 'D:/P/a/b'].map((x, i) => ({ pasta: x, curta: x, comecou: i === 0, agora: i === agora }));
  const p = { id: 91, tipo: 'sessao_nova', sessao_id: 's1', opcoes: [['ligar:3', 'Contar em X']], pastas: pastas(1) };
  // a caixa leva a pasta à vista
  assert.match(perguntaSessaoNova(p), /data-pg-sempre data-pergunta="91" data-pasta="D:\/P\/a"/);
  mudarEscolha({ dataset: { pergunta: '91', pgSempre: '', pasta: 'D:/P/a' }, checked: true });
  // a sessão desceu de pasta: a escolhida continua a que você viu
  assert.equal(escolhaDaPergunta({ ...p, pastas: pastas(2) }).pasta.pasta, 'D:/P/a');
  assert.match(perguntaSessaoNova({ ...p, pastas: pastas(2) }), /data-valor="ligar:3:pasta:D:\/P\/a"/);
  esquecerEscolhas([]);
});

test('o "Outro timer…" e o "+ Novo timer…" levam a caixa e a pasta; o "Novo timer" abre como estava', () => {
  assert.deepEqual(sempreDoBotao({ sempre: '1', pasta: 'D:/x' }), { sempre: true, pasta: 'D:/x' });
  assert.deepEqual(sempreDoBotao({ sempre: '0', pasta: 'D:/x' }), { sempre: false, pasta: null });
  assert.deepEqual(sempreDoBotao({}), { sempre: null, pasta: null });
  const p = pedidoDeLigar({ sessao: 's9', pergunta: '7', sempre: '1', pasta: 'D:/Projetos/cliente' });
  assert.deepEqual([p.sempre, p.pasta, p.com_novo], [true, 'D:/Projetos/cliente', true]);
  // o "Novo timer": a caixa como no dock, na pasta escolhida; o Chat (sem pasta), sem a caixa
  const pedido = { sessao_id: 's9', nome: 'cliente', pasta: 'D:/Projetos/cliente', pasta_mae: false, outro_timer_nome: null };
  assert.match(camposDaSessao({ ...pedido, sempre: true }), /name="regra" checked><span>Sempre que for nesta pasta<small>D:\/Projetos\/cliente<\/small>/);
  assert.doesNotMatch(camposDaSessao({ ...pedido, sempre: false }), /checked/);
  assert.match(camposDaSessao({ ...pedido, sempre: null }), /checked/, 'de outro lugar: como antes');
  // revisão I2: numa pasta que não pode ter regra (a pasta do usuário), sem a caixa
  const semRegra = camposDaSessao({ ...pedido, pasta: 'C:/Users/voce', sempre: false, pode_regra: false });
  assert.doesNotMatch(semRegra, /name="regra"/);
  assert.match(semRegra, /C:\/Users\/voce/, 'a pasta da sessão continua dita');
  assert.equal(camposDaSessao({ ...pedido, pasta: '' }), '');
  // marcada vence o padrão (também numa pasta-mãe, com o aviso)
  assert.match(campoRegra('D:/x', { pasta_mae: true }, 'T', true), /checked[\s\S]*dlg-aviso/);
});

test('o dock: o cartão de 400 px só para a sessão nova, a mesma estrutura no expandido e o redesenho na troca', () => {
  const css = fonte('../css/dock.css');
  assert.match(css, /\.ask\.sn \{ width: 400px;/);
  assert.match(css, /\.ask \{\n  width: 320px;/, 'as outras perguntas seguem com 320 px');
  const dock = fonte('./dock.js');
  // rodada 28: o cartão grande também na pergunta da conta nova (`perguntaGrande`)
  assert.match(dock, /<div class="ask\$\{perguntaGrande\(p\) \? ' sn' : ''\}">\$\{htmlDaPergunta\(p\)\}<\/div>/);
  assert.match(dock, /<div class="dx-ask\$\{perguntaGrande\(p\) \? ' sn' : ''\}">\$\{htmlDaPergunta\(p\)\}\$\{mais\}<\/div>/);
  assert.match(dock, /document\.addEventListener\('change', \(ev\) => \{\n  if \(!mudarEscolha\(ev\.target\)\) return;/);
  // as chaves novas existem nos dois idiomas
  for (const k of ['sessaoNovaSobre', 'emQualTimer', 'rotuloSessao', 'rotuloVemDe', 'rotuloPasta', 'semNomeAinda', 'soEstaSessao', 'estaEProximas', 'sempreQueForNaPasta', 'ondeComecou', 'ondeEstaAgora', 'ajudaSempreNaPasta', 'ajudaPastaDeCima', 'pastasDaRegra']) {
    assert.equal(typeof TEXTOS[k], 'string', k);
    assert.equal(typeof em('en', () => TEXTOS[k]), 'string', k);
    assert.notEqual(TEXTOS[k], em('en', () => TEXTOS[k]), k);
  }
});

// ---- a lista de sessões para ligar ----

test('cada linha da lista de ligar mostra de onde a sessão veio e a pasta', () => {
  assert.equal(linhaDaSessao({ origem: 'chat', cwd: '' }), '<small class="it-p">Chat do app do Claude</small>');
  assert.equal(linhaDaSessao({ origem: 'cli', cwd: 'D:/x' }), '<small class="it-p" title="D:/x">terminal · D:/x</small>');
  assert.equal(linhaDaSessao({ origem: null, cwd: 'D:/x' }), '<small class="it-p" title="D:/x">D:/x</small>');
  assert.equal(linhaDaSessao({ origem: null, cwd: '' }), '');
  assert.match(fonte('./editar.js'), /<span class="it-e">\$\{esc\(estadoDaSessao\(s\.estado\)\)\}<\/span>\n  \$\{linhaDaSessao\(s\)\}/);
});

test('a lista de ligar: as livres, depois as de outro timer e as já ligadas a este por último', () => {
  const s = (id, timer_id) => ({ id, timer_id });
  // a ordem do núcleo: livres primeiro, as ligadas misturadas (a mais recente primeiro)
  const v = [s('a', null), s('b', null), s('c', 5), s('d', 9), s('e', 5), s('f', 9)];
  assert.deepEqual(ordemDeLigar(v, 5).map((i) => v[i].id), ['a', 'b', 'd', 'f', 'c', 'e']);
  assert.deepEqual(ordemDeLigar(v, 9).map((i) => v[i].id), ['a', 'b', 'c', 'e', 'd', 'f']);
  assert.deepEqual(ordemDeLigar([], 5), []);
});
