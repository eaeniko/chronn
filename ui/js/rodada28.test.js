// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 28: a pergunta "De qual espaço é esta conta?", a lista de ligar em três faixas (as contas
// deste espaço, as sem conta e as de outro espaço recolhidas), "Contas do Claude" nos Ajustes e
// "Claude Code no WSL".
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { TEXTOS, definirIdioma } from './textos.js';
import { perguntaContaNova, perguntaGrande, htmlDaPergunta } from './componentes.js';
import { faixasDeLigar, ordemDeLigar, linhaDaSessao } from './editar.js';
import { htmlContas, htmlWsl } from './ajustes.js';

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

const conta = 'a1b2c3d4e5f60718';
const pergunta = (o = {}) => ({
  id: 2 ** 52 + 7,
  tipo: 'conta_nova',
  texto: 'De qual espaço é esta conta?',
  opcoes: [
    [`conta:${conta}:espaco:1`, 'Trabalho'],
    [`conta:${conta}:espaco:2`, 'Pessoal'],
    [`conta:${conta}:depois`, 'Depois'],
  ],
  cores: ['#5b8def', '#e86bd0', null],
  sessao_id: null,
  pasta: 'D:\\Projetos\\site',
  pasta_curta: 'Projetos\\site',
  sessao_nome: null,
  origem: 'cli',
  pastas: [],
  ...o,
});

test('a pergunta da conta nova: o sobre, o título, Vem de e Pasta, um botão por espaço com a cor e o Depois discreto', () => {
  const h = perguntaContaNova(pergunta());
  assert.match(h, /<div class="sn-sobre">Conta nova do Claude<\/div><div class="sn-titulo">De qual espaço é esta conta\?<\/div>/);
  assert.match(h, /<dt>Vem de<\/dt><dd>terminal<\/dd>/);
  assert.match(h, /<dt>Pasta<\/dt><dd class="sn-pasta mono" title="D:\\Projetos\\site">Projetos\\site<\/dd>/);
  assert.match(h, /<button class="cn-esp" title="Trabalho" style="--c:#5b8def" data-acao="responder" data-pergunta="4503599627370503" data-valor="conta:a1b2c3d4e5f60718:espaco:1"><i><\/i><span>Trabalho<\/span><\/button>/);
  assert.match(h, /class="cn-esp" title="Pessoal" style="--c:#e86bd0"/);
  assert.match(h, /<button class="largo sn-nao" title="Depois" data-acao="responder"[^>]*data-valor="conta:a1b2c3d4e5f60718:depois"><span>Depois<\/span><\/button><\/div>$/);
  // três espaços: o último sozinho ocupa a linha
  const tres = perguntaContaNova(
    pergunta({ opcoes: [['conta:x:espaco:1', 'A'], ['conta:x:espaco:2', 'B'], ['conta:x:espaco:3', 'C'], ['conta:x:depois', 'Depois']], cores: ['#111', '#222', '#333', null] }),
  );
  assert.match(tres, /class="cn-esp largo" title="C"/);
  assert.doesNotMatch(tres, /class="cn-esp largo" title="A"/);
  // no Chat: sem a pasta; sem origem: sem o bloco
  assert.doesNotMatch(perguntaContaNova(pergunta({ pasta: null, pasta_curta: null, origem: 'claude-desktop' })), /<dt>Pasta<\/dt>/);
  assert.doesNotMatch(perguntaContaNova(pergunta({ pasta: null, pasta_curta: null, origem: null })), /sn-info/);
  // a cor que não é cor vira a neutra
  assert.match(perguntaContaNova(pergunta({ cores: ['red;x', null, null] })), /--c:#8b8e98/);
  // no dock: o cartão grande, como a sessão nova
  assert.equal(perguntaGrande(pergunta()), true);
  assert.equal(perguntaGrande({ tipo: 'sessao_nova' }), true);
  assert.equal(perguntaGrande({ tipo: 'app_fechado' }), false);
  assert.equal(htmlDaPergunta(pergunta()), perguntaContaNova(pergunta()));
  // em inglês
  const en = em('en', () => perguntaContaNova(pergunta()));
  assert.match(en, /New Claude account<\/div><div class="sn-titulo">Which space is this account in\?/);
  assert.match(en, /<dt>From<\/dt><dd>terminal<\/dd>/);
});

const s = (id, timer_id, conta_espaco, conta = conta_espaco === null ? null : `c${conta_espaco}`) => ({ id, nome: id, cwd: '', estado: 'parada', timer_id, origem: 'cli', conta, conta_espaco });

test('a lista de ligar em três faixas: deste espaço, sem conta e as de outro espaço, cada uma na ordem de hoje', () => {
  // a ordem do núcleo: a mais recente primeiro
  const lista = [
    s('outro-livre', null, 2),
    s('sem-conta', null, null, null),
    s('deste-ligada', 9, 1),
    s('deste-livre', null, 1),
    s('conta-sem-espaco', 5, null, 'c9'),
    s('outro-em-outro', 5, 2),
    s('deste-em-outro', 5, 1),
  ];
  const f = faixasDeLigar(lista, 9, 1, 2);
  const nomes = (ids) => ids.map((i) => lista[i].id);
  assert.deepEqual(nomes(f[0]), ['deste-livre', 'deste-em-outro', 'deste-ligada'], 'as livres, as de outro timer e as já ligadas a este');
  assert.deepEqual(nomes(f[1]), ['sem-conta', 'conta-sem-espaco']);
  assert.deepEqual(nomes(f[2]), ['outro-livre', 'outro-em-outro']);
  // sem isso, nulo: a lista fica como hoje
  assert.equal(faixasDeLigar(lista, 9, 1, 1), null, 'um espaço só');
  assert.equal(faixasDeLigar(lista.map((x) => ({ ...x, conta_espaco: null })), 9, 1, 2), null, 'nenhuma conta com espaço');
  assert.equal(faixasDeLigar(lista, 9, null, 2), null, 'timer sem espaço conhecido');
  assert.deepEqual(ordemDeLigar(lista, 9).length, lista.length);
  // a linha de cada sessão diz "terminal (WSL)" para o WSL
  assert.match(linhaDaSessao({ origem: 'wsl', cwd: '\\\\wsl.localhost\\Ubuntu\\home\\u\\p' }), /terminal \(WSL\)/);
  assert.match(em('en', () => linhaDaSessao({ origem: 'wsl', cwd: '' })), /terminal \(WSL\)/);
});

test('a lista de ligar: a faixa recolhida "Outras contas (N) ›" abre no lugar, sem diálogo novo', () => {
  const ed = fonte('./editar.js');
  assert.match(ed, /data-outras="1" aria-expanded="false"><span class="it-n">\$\{esc\(TEXTOS\.outrasContas\(faixas\[2\]\.length\)\)\} ›<\/span>/);
  assert.match(ed, /if \(b\.dataset\.outras\) \{\n\s+outrasAbertas = true;\n\s+el\.innerHTML = itensDe\(\);/);
  assert.match(ed, /let outrasAbertas = false;/, 'a escolha vale só para esta lista');
  assert.equal(TEXTOS.outrasContas(3), 'Outras contas (3)');
  assert.equal(em('en', () => TEXTOS.outrasContas(3)), 'Other accounts (3)');
});

const espacos = [
  { id: 1, nome: 'Trabalho', cor: '#5b8def', arquivado: false },
  { id: 2, nome: 'Pessoal', cor: '#e86bd0', arquivado: false },
  { id: 3, nome: 'Velho', cor: '#999999', arquivado: true },
];

test('Ajustes → Contas do Claude: só com conta; Conta ·1a2b, a origem e a data, o seletor com Sem espaço e o Esquecer', () => {
  assert.equal(htmlContas([], espacos), '');
  assert.equal(htmlContas(null, espacos), '');
  const visto = new Date(2026, 10, 21, 14, 5).getTime();
  const h = htmlContas([{ id: conta, curta: 'a1b2', origem: 'claude-desktop', visto_em: visto, espaco_id: 2 }, { id: 'ffff000011112222', curta: 'ffff', origem: null, visto_em: visto, espaco_id: null }], espacos);
  assert.match(h, /<h4 class="aj-sub">Contas do Claude<\/h4>/);
  assert.match(h, /<li style="--c:#e86bd0"><i><\/i><span class="aj-n">Conta ·a1b2<\/span><small>app do Claude · vista 21\/11\/2026 14:05<\/small>/);
  assert.match(h, /<select class="aj-conta-esp" data-conta="a1b2c3d4e5f60718" aria-label="Espaço da conta Conta ·a1b2"><option value="">Sem espaço<\/option><option value="1">Trabalho<\/option><option value="2" selected>Pessoal<\/option><\/select>/);
  assert.doesNotMatch(h, /Velho/, 'o arquivado só aparece se for o da conta');
  assert.match(h, /<li><i><\/i><span class="aj-n">Conta ·ffff<\/span><small>vista 21\/11\/2026 14:05<\/small>/);
  assert.match(h, /<option value="" selected>Sem espaço<\/option>/);
  assert.match(h, /data-aj="conta-esquecer" data-conta="ffff000011112222"[^>]*>Esquecer<\/button>/);
  assert.match(h, /O Chronn guarda só um resumo do id da conta, nunca o id\./);
  assert.doesNotMatch(h, new RegExp('5d1c0e7a'), 'nunca o id');
  const en = em('en', () => htmlContas([{ id: conta, curta: 'a1b2', origem: 'cli', visto_em: visto, espaco_id: null }], espacos));
  assert.match(en, /Claude accounts/);
  assert.match(en, /Account ·a1b2<\/span><small>terminal · seen /);
  assert.match(en, /No space/);
  assert.match(en, /Forget/);
  assert.match(en, /Chronn keeps only a digest of the account id, never the id\./);
  // o seletor não é da config: vai direto ao comando da conta
  const aj = fonte('./ajustes.js');
  assert.match(aj, /if \(el\.matches\?\.\('select\[data-conta\]'\)\) \{\n\s+try \{\n\s+await chamar\('conta_definir', \{ conta: el\.dataset\.conta, espaco_id: el\.value === '' \? null : Number\(el\.value\) \}\);/);
  assert.match(aj, /await chamar\('conta_esquecer', \{ conta: alvo\.dataset\.conta \}\);/);
});

test('Claude Code no WSL: só com distro; ligada com Desligar, parada sem botão e sem Claude Code sem botão', () => {
  assert.equal(htmlWsl([]), '');
  assert.equal(htmlWsl(undefined), '', 'ainda sem leitura');
  // revisão M10: a leitura falhou
  assert.match(htmlWsl(null), /<h4 class="aj-sub">Claude Code no WSL<\/h4><small class="aj-dica" role="note">Não deu para ler as distros do WSL\.<\/small>/);
  assert.match(em('en', () => htmlWsl(null)), /Couldn’t read the WSL distros\./);
  const h = htmlWsl([
    { nome: 'Ubuntu', estado: 'ligada' },
    { nome: 'Debian', estado: 'parada' },
    { nome: 'Alpine', estado: 'sem_claude' },
    { nome: 'Kali', estado: 'desligada' },
    { nome: 'Fedora', estado: 'desatualizado' },
  ]);
  assert.match(h, /<h4 class="aj-sub">Claude Code no WSL<\/h4>/);
  assert.match(h, /<li data-wsl="Ubuntu"><span class="aj-n">Ubuntu<\/span><span class="aj-estado on"><i><\/i>ligada<\/span><small><\/small><button type="button" class="bt mini" data-aj="wsl-desligar" data-distro="Ubuntu">Desligar<\/button><\/li>/);
  // 0.6.1: a parada tem o "Ligar", que liga a distro só no clique (`data-subir`)
  assert.match(h, /<li data-wsl="Debian"><span class="aj-n">Debian<\/span><span class="aj-estado ausente"><i><\/i>parada<\/span><small><\/small><button type="button" class="bt mini" data-aj="wsl-ligar" data-subir="1" data-distro="Debian" title="Liga o Debian agora, só neste clique, para mostrar a prévia">Ligar<\/button><\/li>/);
  assert.match(h, /<li data-wsl="Alpine">[^]*Claude Code não encontrado<\/span><small><\/small><\/li>/);
  assert.match(h, /data-aj="wsl-ligar" data-distro="Kali">Ligar<\/button>/);
  assert.match(h, /<span class="aj-estado antigo"><i><\/i>hook desatualizado<\/span><small><\/small><button type="button" class="bt mini" data-aj="wsl-desligar" data-distro="Fedora">/);
  assert.match(h, /O Chronn lê a fila da distro pelo Windows\. Nada no Linux precisa rodar programas do Windows\./);
  // uma ligada que está parada segue ligada (o Desligar), sem abrir a distro
  assert.match(htmlWsl([{ nome: 'Ubuntu', estado: 'parada', ligada: true }]), /parada<\/span><small><\/small><button type="button" class="bt mini" data-aj="wsl-desligar"/);
  const en = em('en', () => htmlWsl([{ nome: 'Ubuntu', estado: 'ligada' }, { nome: 'Debian', estado: 'parada' }]));
  assert.match(en, /Claude Code in WSL/);
  assert.match(en, /connected<\/span>/);
  assert.match(en, /stopped<\/span><small><\/small><button type="button" class="bt mini" data-aj="wsl-ligar" data-subir="1"/);
  assert.match(en, /Chronn reads the distro’s queue from Windows\./);
  // nos Ajustes e no diálogo de conectar
  const aj = fonte('./ajustes.js');
  assert.match(aj, /<div data-bloco="wsl">\$\{htmlWsl\(dados\.wsl\)\}<\/div>/);
  // revisão I2: nem o diálogo nem os Ajustes esperam o WSL
  assert.match(aj, /corpo: `\$\{corpoPrevia\(previa, settings, pastas, atualizar\)\}<div data-wsl-dialogo><\/div>`,/);
  assert.match(aj, /lerWsl\(\)\.then\(\(wsl\) => \{/);
  assert.doesNotMatch(aj, /Promise\.all(Settled)?\(\[[^\]]*lerWsl\(\)/);
  assert.match(aj, /el\.innerHTML = html\(snap\);\n  atualizarWsl\(\);/);
});

test('rodada 28: as chaves novas existem e mudam nos dois idiomas', () => {
  for (const k of ['contaNovaSobre', 'contaNovaTitulo', 'contasDoClaude', 'semEspaco', 'esquecer', 'contasAjuda', 'wslTitulo', 'wslAjuda', 'wslLigar', 'wslDesligar', 'wslNaoDeuLer']) {
    assert.equal(typeof TEXTOS[k], 'string', k);
    assert.notEqual(TEXTOS[k], em('en', () => TEXTOS[k]), k);
  }
  for (const e of ['ligada', 'sem_claude', 'parada', 'desatualizado', 'desligada']) {
    assert.notEqual(TEXTOS.wslEstado(e), em('en', () => TEXTOS.wslEstado(e)), e);
  }
});
