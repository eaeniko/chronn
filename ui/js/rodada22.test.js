// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 22 (0.3.1): o cabeçalho "opção A". Linha 1: o nome, o "+ Novo" e o "Lançar o dia", o ? e
// o ⚙; à direita, sozinho, o total. Linha 2: as telas num menu, as abas e as ações em fantasma.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { ITENS_DAS_TELAS, htmlDoMenuTelas, rotuloDaTela, proximoNoMenu } from './telas.js';
import { PASSOS, PASSOS_ESSENCIAL, MINI_TOURS } from './tour.js';
import { DICAS } from './dicas.js';
import { TEXTOS } from './textos.js';

const INDEX = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
const CSS = readFileSync(new URL('../css/app.css', import.meta.url), 'utf8');
const PAINEL = readFileSync(new URL('./painel.js', import.meta.url), 'utf8');
const L1 = INDEX.slice(INDEX.indexOf('<div class="top-l1">'), INDEX.indexOf('<div class="top-l2">'));
const L2 = INDEX.slice(INDEX.indexOf('<div class="top-l2">'), INDEX.indexOf('</header>'));
const ordem = (trecho, marcas) => marcas.map((m) => trecho.indexOf(m));
const emOrdem = (trecho, marcas) => {
  const p = ordem(trecho, marcas);
  assert.ok(p.every((x) => x >= 0), `${marcas} → ${p}`);
  assert.deepEqual([...p].sort((a, b) => a - b), p, String(marcas));
};

test('rodada 22: a linha 1 na ordem: nome, "+ Novo", "Lançar o dia", ? e ⚙; o total por último', () => {
  emOrdem(L1, ['class="logo"', 'id="bt-novo"', 'id="bt-lancar"', 'id="bt-tour"', 'id="bt-ajustes"', 'class="sp"', 'class="day"', 'id="data"', 'id="total-dia"']);
  // o "+ Novo" e o "Lançar o dia" juntos; o ? e o ⚙ juntos, depois de um respiro
  assert.match(L1, /<div class="l1-criar">\s*<button class="novo principal" id="bt-novo"[\s\S]*?id="bt-lancar"[\s\S]*?<\/div>\s*<div class="l1-ajuda">/);
  assert.match(CSS, /\.logo \{[^}]*margin-right: 30px;/);
  assert.match(CSS, /\.l1-ajuda \{[^}]*margin-left: 18px;/);
  // nada disso ficou na linha 2
  for (const id of ['bt-novo', 'bt-lancar', 'bt-tour', 'bt-ajustes', 'total-dia']) assert.doesNotMatch(L2, new RegExp(`id="${id}"`), id);
});

test('rodada 22: a área da hora tem só a data e o total', () => {
  const day = L1.slice(L1.indexOf('<div class="day">'));
  assert.equal(day.trim(), '<div class="day"><small id="data"></small><b class="mono" id="total-dia">0min</b></div>\n    </div>'.trim());
  // depois do espaço que empurra, só o total
  const depois = L1.slice(L1.indexOf('<div class="sp"></div>') + '<div class="sp"></div>'.length);
  assert.doesNotMatch(depois, /<button/);
  // o total é o maior texto da tela (mono), com a data por cima, fora da linha do meio
  assert.match(CSS, /\.day b \{ font-size: 30px;/);
  assert.match(CSS, /\.day small \{ position: absolute; right: 0; bottom: 100%;/);
});

test('rodada 22: o "+ Novo" é o principal (preenchido, 36 px, ícone + texto); o "Lançar o dia", contorno com "em breve"', () => {
  assert.match(L1, /<button class="novo principal" id="bt-novo" data-acao="novo-menu"[^>]*aria-haspopup="menu"[^>]*><svg class="bi"[^>]*>[\s\S]*?<\/svg><span data-texto="novoRotulo">Novo<\/span><\/button>/);
  assert.match(CSS, /\.novo \{[^}]*height: 36px;/);
  assert.match(CSS, /\.novo\.principal \{[^}]*background: var\(--acento\); color: #fff;/);
  assert.match(CSS, /\.lancar \{[^}]*height: 36px;[^}]*border: 1px solid/);
  assert.match(L1, /<button class="lancar" id="bt-lancar" disabled><svg class="bi"[^>]*>[\s\S]*?<\/svg><span class="rot" data-texto="lancarDia">Lançar o dia<\/span><span class="breve" data-texto="emBreve">em breve<\/span><\/button>/);
  // sem lugar na linha 1: primeiro sai o "em breve", depois o texto do "Lançar"; o "+ Novo" nunca
  assert.match(CSS, /\.top\.l1-apertada \.lancar \.breve \{ display: none; \}/);
  assert.match(CSS, /\.top\.l1-sem-texto \.lancar \.rot \{ display: none; \}/);
  assert.doesNotMatch(CSS, /l1-[a-z-]+ \.novo|compacta \.novo/);
  assert.match(PAINEL, /if \(sobra\(\)\) top\.classList\.add\('l1-apertada'\);\s*if \(sobra\(\)\) top\.classList\.add\('l1-sem-texto'\);/);
  // os textos do cabeçalho vêm do TEXTOS (o HTML traz os mesmos, para a primeira pintura)
  for (const m of INDEX.matchAll(/data-texto="(\w+)">([^<]*)</g)) assert.equal(TEXTOS[m[1]], m[2], m[1]);
  for (const m of INDEX.matchAll(/title="([^"]*)" data-dica="(\w+)"/g)) assert.equal(TEXTOS[m[2]], m[1], m[2]);
  assert.match(PAINEL, /for \(const el of document\.querySelectorAll\('\[data-texto\]'\)\) trocarTexto\(el, TEXTOS\[el\.dataset\.texto\]\);/);
});

test('rodada 22: a linha 2: o menu das telas, as abas e, à direita, as ações em fantasma', () => {
  emOrdem(L2, ['class="telas"', 'id="bt-telas"', 'id="abas-sep"', 'id="abas"', 'class="sp"', 'id="rodando"', 'class="pt-on"', 'id="bt-pausar-tudo"', 'id="bt-zerar-todos"']);
  assert.doesNotMatch(L2, /data-visao=/, 'as telas saíram da linha (estão no menu)');
  assert.match(L2, /<button class="telas-bt" id="bt-telas" data-acao="telas-menu"[^>]*aria-haspopup="menu" aria-expanded="false"><span class="tn">Painel<\/span><span class="seta" aria-hidden="true">▾<\/span><\/button>/);
  // fantasma: sem fundo nem borda até o hover
  const acao = CSS.match(/\n\.acao \{[^}]*\}/)[0];
  assert.doesNotMatch(acao, /[^-]background:|border:/);
  assert.match(CSS, /\.rodando-n \.pt-on \{[^}]*background: #2fd19b;/);
  // compacto: ícones com dica antes de quebrar; o ponto e o número ficam
  assert.match(CSS, /\.top\.compacta \.acao \.rot \{ display: none; \}/);
  assert.match(CSS, /\.top\.compacta \.rodando-n \.rt \{ display: none; \}/);
});

test('rodada 22: o menu das telas: a atual marcada, na ordem, e o botão diz a tela', () => {
  assert.deepEqual(ITENS_DAS_TELAS.map((t) => [t.id, t.rotulo]), [
    ['painel', 'Painel'],
    ['foco', 'Foco'],
    ['cartoes', 'Cartões'],
    ['linha', 'Linha do tempo'],
  ]);
  const h = htmlDoMenuTelas('cartoes');
  assert.match(h, /^<div class="menu-ajuda menu-telas" role="menu" aria-label="Telas">/);
  assert.deepEqual([...h.matchAll(/aria-checked="(true|false)" data-visao="(\w+)"/g)].map((m) => [m[2], m[1]]), [
    ['painel', 'false'],
    ['foco', 'false'],
    ['cartoes', 'true'],
    ['linha', 'false'],
  ]);
  assert.match(h, /data-visao="cartoes" class="on"><span class="mt-ok" aria-hidden="true">✓<\/span><span class="mt-nome">Cartões<\/span><\/button>/);
  assert.doesNotMatch(h, /<kbd>/, 'nenhuma tela tem atalho hoje');
  assert.equal(rotuloDaTela('linha'), 'Linha do tempo');
  assert.equal(rotuloDaTela('ajustes'), 'Ajustes');
  assert.equal(rotuloDaTela('xyz'), 'Painel');
  assert.match(PAINEL, /case 'telas-menu':\s*alternarMenuTelas\(alvo\);/);
  assert.match(PAINEL, /abrirMenu\(\s*botao,\s*htmlDoMenuTelas\(visao\),\s*'esquerda',\s*\(b\) => \{\s*if \(b\.dataset\.visao\) trocarVisao\(b\.dataset\.visao\);\s*\},\s*foco,\s*\);/);
  // revisão M4: o "visao" solto (as telas antigas da linha) saiu
  assert.doesNotMatch(PAINEL, /case 'visao':/);
  assert.match(PAINEL, /trocarTexto\(\$\('#bt-telas \.tn'\), rotuloDaTela\(visao\)\);/);
});

test('rodada 22: o menu abre (clique, Enter, espaço), fecha com Esc e com o clique fora, e as setas andam', () => {
  // abre: é um <button> (Enter e espaço clicam), com aria-haspopup e aria-expanded
  assert.match(L2, /<button class="telas-bt"[^>]*aria-haspopup="menu" aria-expanded="false"/);
  // o foco vai na tela de agora; aberto pelas setas, na primeira ou na última
  assert.match(PAINEL, /foco === 'primeira' \? habilitadas\[0\] : foco === 'ultima' \? habilitadas\.at\(-1\) : menu\.querySelector\('\[aria-checked="true"\]:not\(:disabled\)'\) \?\? habilitadas\[0\];/);
  // Esc fecha e devolve o foco ao botão (só se o foco estava no menu); o clique fora fecha
  assert.match(PAINEL, /if \(ev\.key === 'Escape'\) \{\s*ev\.preventDefault\(\);\s*fechar\(\);\s*if \(dentro\) botaoDoDono\(dono\)\?\.focus\(\);\s*return;/);
  assert.match(PAINEL, /const fora = \(ev\) => \{\s*if \(menu\.contains\(ev\.target\)\) return;/);
  // as setas: ↓ e ↑ andam e dão a volta; Home e End vão às pontas
  assert.match(PAINEL, /const i = proximoNoMenu\(opcoes\.length, opcoes\.indexOf\(document\.activeElement\), ev\.key\);/);
  assert.equal(proximoNoMenu(4, 0, 'ArrowDown'), 1);
  assert.equal(proximoNoMenu(4, 3, 'ArrowDown'), 0);
  assert.equal(proximoNoMenu(4, 0, 'ArrowUp'), 3);
  assert.equal(proximoNoMenu(4, 2, 'ArrowUp'), 1);
  assert.equal(proximoNoMenu(4, -1, 'ArrowDown'), 0);
  assert.equal(proximoNoMenu(4, -1, 'ArrowUp'), 3);
  assert.equal(proximoNoMenu(4, 2, 'Home'), 0);
  assert.equal(proximoNoMenu(4, 0, 'End'), 3);
  assert.equal(proximoNoMenu(4, 1, 'a'), null);
  assert.equal(proximoNoMenu(0, -1, 'ArrowDown'), null);
});

test('revisão 22 M1: as teclas só agem com o foco dentro do menu; Tab ou o foco saindo fecham', () => {
  // fora do menu (num campo), setas, Home e End são do campo; o foco nunca é puxado
  assert.match(PAINEL, /const dentro = menu\.contains\(document\.activeElement\);/);
  assert.match(PAINEL, /if \(!dentro\) return;\s*\/\/ Tab fecha[^\n]*\n\s*if \(ev\.key === 'Tab'\) \{\s*fechar\(\);\s*botaoDoDono\(dono\)\?\.focus\(\);\s*return;\s*\}/);
  // o foco indo para fora do menu fecha (menos para o botão do dono, cujo clique só fecha)
  assert.match(PAINEL, /menu\.addEventListener\('focusout', \(ev\) => \{\s*const para = ev\.relatedTarget;\s*if \(!para \|\| menu\.contains\(para\) \|\| !menu\.isConnected\) return;/);
  assert.match(PAINEL, /donoDoMenu\(para\.closest\('\[data-acao\]'\)\) === dono\) return;\s*fechar\(\);/);
});

test('revisão 22 M2: ↓ ou ↑ no "Painel ▾" fechado abre o menu na primeira ou na última opção', () => {
  assert.match(
    PAINEL,
    /if \(\(ev\.key === 'ArrowDown' \|\| ev\.key === 'ArrowUp'\) && ev\.target\.matches\?\.\('#bt-telas'\) && ev\.target\.getAttribute\('aria-expanded'\) !== 'true'\) \{\s*ev\.preventDefault\(\);\s*alternarMenuTelas\(ev\.target, ev\.key === 'ArrowDown' \? 'primeira' : 'ultima'\);/,
  );
});

test('revisão 22 M3: a opção focada nos menus tem contorno visível (os dois temas usam o destaque)', () => {
  assert.match(CSS, /\.menu-ajuda button:focus-visible \{ outline: 2px solid color-mix\(in srgb, var\(--acento\) 75%, transparent\); outline-offset: -2px; \}/);
  assert.doesNotMatch(CSS, /\.menu-ajuda button:focus-visible \{[^}]*outline: none/);
  assert.doesNotMatch(CSS, /\.menu-ajuda button:hover, \.menu-ajuda button:focus-visible \{[^}]*outline: none/);
});

test('rodada 22: os alvos do tour, dos mini-tours e das dicas existem; o passo das telas aponta o menu', () => {
  const telas = PASSOS.find((p) => p.id === 'telas');
  assert.equal(telas.alvo, '#bt-telas');
  assert.equal(telas.titulo, TEXTOS.tourTelasTitulo);
  const alvos = [...PASSOS, ...PASSOS_ESSENCIAL, ...Object.values(MINI_TOURS).flat()]
    .map((p) => p.alvo)
    .concat(Object.values(DICAS).map((d) => d.alvo?.({ grupos: [] })))
    .filter((a) => typeof a === 'string' && /^#(bt-|total-dia|abas)/.test(a));
  for (const a of ['#bt-telas', '#bt-novo', '#bt-zerar-todos', '#bt-pausar-tudo', '#bt-ajustes', '#bt-tour', '#total-dia']) assert.ok(alvos.includes(a), a);
  for (const a of alvos) assert.match(INDEX, new RegExp(`id="${a.split(' ')[0].slice(1)}"`), a);
});
