// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 24: o espaço no dock recolhido (um ponto só; "Só o timer" por padrão, "Espaço em texto"
// e "Cor do espaço" nos Ajustes) e o "Renomear" no ⋯ do grupo, com o campo no lugar do nome.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { DOCK_ESPACO, espacoNoRecolhido, pontoRecolhido, espacoEmTexto, itensDoMenuGrupo } from './espacos.js';
import { campoDockEspaco } from './ajustes.js';
import { nomeNovo, ligarRenomear, depoisDoClique, JANELA_DO_CLIQUE } from './renomear.js';
import { GRUPO_NOME_MAX } from './editar.js';
import { DICIONARIOS, definirIdioma } from './textos.js';

const fonte = (nome) => readFileSync(new URL(`./${nome}`, import.meta.url), 'utf8');

/** Roda `f` com o idioma `id` e volta ao português (os outros testes contam com ele). */
function em(id, f) {
  definirIdioma(id);
  try {
    return f();
  } finally {
    definirIdioma('pt-BR');
  }
}

const ESPACOS = [
  { id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivado: false },
  { id: 2, nome: 'Pessoal', cor: '#e86bd0', ordem: 1, arquivado: false },
];
const snapDe = (dock_espaco, espacos = ESPACOS) => ({ espacos, config: { dock_espaco }, grupos: [] });
const LEITURA = { id: 20, nome: 'Leitura', cor: '#ff6b4a', espaco_id: 2 };

test('rodada 24: as três formas do espaço no dock recolhido, com 2 espaços', () => {
  assert.deepEqual(DOCK_ESPACO, ['timer', 'texto', 'cor']);
  // A, o padrão (também sem o campo, numa config de antes): o ponto do timer, o espaço na dica
  for (const escolha of ['timer', undefined, 'estragado']) {
    const esp = espacoNoRecolhido(snapDe(escolha), LEITURA);
    assert.equal(esp.modo, 'timer');
    assert.equal(esp.espaco.nome, 'Pessoal');
    assert.equal(pontoRecolhido(LEITURA, esp), '<i title="Leitura · espaço Pessoal" aria-label="Leitura · espaço Pessoal"></i>');
    assert.equal(espacoEmTexto(esp), '');
  }
  // B: o ponto do timer, sem dica; o nome do espaço na linha da situação, na cor dele
  const b = espacoNoRecolhido(snapDe('texto'), LEITURA);
  assert.equal(pontoRecolhido(LEITURA, b), '<i></i>');
  assert.equal(espacoEmTexto(b), '<span class="sesp" style="--e:#e86bd0">Pessoal</span>');
  // C: um ponto só, na cor do espaço (o --c dele: o mesmo halo de rodando), com a dica
  const c = espacoNoRecolhido(snapDe('cor'), LEITURA);
  assert.equal(pontoRecolhido(LEITURA, c), '<i style="--c:#e86bd0" title="Leitura · espaço Pessoal" aria-label="Leitura · espaço Pessoal"></i>');
  assert.equal(espacoEmTexto(c), '');
  // o timer sem espaço gravado é do primeiro
  assert.equal(espacoNoRecolhido(snapDe('cor'), { ...LEITURA, espaco_id: undefined }).espaco.nome, 'Trabalho');
  // em inglês, a dica também
  em('en', () => assert.match(pontoRecolhido(LEITURA, c), /title="Leitura · space Pessoal"/));
});

test('rodada 24: com um espaço só, as três formas ficam iguais à "Só o timer", sem espaço', () => {
  const um = [ESPACOS[0], { ...ESPACOS[1], arquivado: true }];
  for (const escolha of DOCK_ESPACO) {
    const esp = espacoNoRecolhido(snapDe(escolha, um), LEITURA);
    assert.deepEqual(esp, { modo: 'timer', espaco: null }, escolha);
    assert.equal(pontoRecolhido(LEITURA, esp), '<i></i>');
    assert.equal(espacoEmTexto(esp), '');
  }
});

test('rodada 24: o dock recolhido usa um ponto só, e a linha do "Espaço em texto" corta inteira', () => {
  const d = fonte('dock.js');
  const rec = d.slice(d.indexOf('function timerRecolhido'), d.indexOf('function recolhido('));
  assert.match(rec, /const esp = espacoNoRecolhido\(snap, /);
  assert.match(rec, />\$\{pontoRecolhido\(t, esp\)\}\$\{nomeRecolhido\(t, sit, esp\)\}/);
  assert.doesNotMatch(rec, /<i><\/i>|pontoDoEspaco/, 'sem o segundo ponto');
  // B: "Pessoal · escrevendo" (sem situação, só "Pessoal"), numa linha só com as reticências
  const nome = d.slice(d.indexOf('function nomeRecolhido'), d.indexOf('function corDaReuniao'));
  assert.match(nome, /if \(!sit && !espaco\) return nome;/);
  assert.match(nome, /const resto = e \? `<span class="mais"> · <\/span>\$\{htmlEtiqueta\(e, 16\)\}` : '';/);
  assert.match(nome, /<small class="sit\$\{e \? ` \$\{e\.p1\.k\}` : ''\}" title="\$\{dica\}">\$\{espaco\}\$\{resto\}<\/small>/);
  const css = fonte('../css/dock.css');
  assert.match(css, /\.dk \.sit \{[^}]*overflow: hidden; text-overflow: ellipsis; white-space: nowrap;/);
  // o nome do espaço com o mesmo ajuste de contraste do nome do timer (revisão da rodada 24)
  assert.match(css, /\.dk \.sit \.sesp \{ color: color-mix\(in srgb, var\(--e\), #000 var\(--texto-cor\)\); font-weight: 600; \}/);
  assert.match(css, /\[style\*="--c"\] \{ --ct: color-mix\(in srgb, var\(--c\), #000 var\(--texto-cor\)\); \}/, 'a mesma regra do --ct');
  // o expandido não muda: o ponto do espaço continua lá
  assert.match(d, /<i><\/i>\$\{pontoDoEspaco\(snap, t\)\}<span class="nm"/);
});

test('rodada 24: Ajustes → Dock: "Espaço no dock recolhido", com as três prévias e a dica, nos dois idiomas', () => {
  const h = campoDockEspaco(undefined);
  assert.match(h, /<span id="aj-dock-espaco">Espaço no dock recolhido<\/span>/);
  assert.match(h, /role="radiogroup" aria-labelledby="aj-dock-espaco"/);
  assert.deepEqual([...h.matchAll(/name="dock_espaco" value="(\w+)"/g)].map((m) => m[1]), ['timer', 'texto', 'cor']);
  assert.match(h, /value="timer" checked>/, 'sem escolha, "Só o timer"');
  assert.match(h, /<em class="dkp" aria-hidden="true"><i><\/i><b><strong>Relatório<\/strong><\/b><\/em>Só o timer/);
  assert.match(h, /<em class="dkp" aria-hidden="true"><i><\/i><b><small><u>Pessoal<\/u> · escrevendo<\/small><strong>Relatório<\/strong><\/b><\/em>Espaço em texto/);
  assert.match(h, /<em class="dkp" aria-hidden="true"><i class="e"><\/i><b><strong>Relatório<\/strong><\/b><\/em>Cor do espaço/);
  assert.match(h, /com um só, as três ficam iguais a "Só o timer"/);
  assert.match(campoDockEspaco('cor'), /value="cor" checked>/);
  em('en', () => {
    const e = campoDockEspaco('texto');
    assert.match(e, /Space on the collapsed dock/);
    assert.match(e, /value="texto" checked>/);
    assert.match(e, /<u>Personal<\/u> · writing<\/small><strong>Report<\/strong>/);
    assert.match(e, /Timer only[\s\S]*Space as text[\s\S]*Space color/);
    assert.match(e, /with just one, all three look like “Timer only”/);
  });
  // na seção Dock, gravado como os outros rádios e acompanhando o estado
  const a = fonte('ajustes.js');
  assert.match(a, /\$\{campoDockEspaco\(c\.dock_espaco\)\}`,/);
  assert.match(a, /nome === 'dock_espaco' \|\|/);
  assert.match(a, /\['dock_espaco', c\.dock_espaco \?\? 'timer'\],/);
});

test('rodada 24: as chaves novas existem nos dois idiomas', () => {
  for (const k of ['dockEspacoRotulo', 'dockEspacoOps', 'dockEspacoDica', 'dockEspacoPrevia', 'dicaPontoEspaco', 'renomear', 'renomearGrupoRotulo']) {
    assert.ok(DICIONARIOS['pt-BR'][k] !== undefined && DICIONARIOS.en[k] !== undefined, k);
  }
  assert.equal(DICIONARIOS.en.renomear, 'Rename');
});

test('rodada 24: "Renomear" é o primeiro item do ⋯ do grupo', () => {
  const s = { espacos: [ESPACOS[0]], grupos: [] };
  assert.deepEqual(itensDoMenuGrupo(s, { id: 5, nome: 'Casa' }).map((i) => i.id), ['renomear', 'editar', 'arquivar', 'apagar']);
  em('en', () => assert.equal(itensDoMenuGrupo(s, { id: 5 })[0].rotulo, 'Rename'));
  const p = fonte('painel.js');
  assert.match(p, /if \(b\.dataset\.mg === 'renomear'\) renomearGrupo\(grupoId\);\s*else if \(b\.dataset\.mg === 'editar'\)/);
});

test('rodada 24: o nome a gravar: vazio ou igual volta ao anterior', () => {
  assert.equal(nomeNovo('Casa', 'Lar'), 'Lar');
  assert.equal(nomeNovo('Casa', '  Lar  '), 'Lar', 'sem os espaços das pontas');
  assert.equal(nomeNovo('Casa', 'Casa'), null);
  assert.equal(nomeNovo('Casa', ' Casa '), null);
  assert.equal(nomeNovo('Casa', ''), null);
  assert.equal(nomeNovo('Casa', '   '), null);
});

/** Um campo de mentira: um EventTarget com valor e `isConnected`. */
function campo(valor) {
  const c = new EventTarget();
  c.value = valor;
  c.isConnected = true;
  return c;
}
function tecla(c, key) {
  const ev = new Event('keydown', { cancelable: true });
  ev.key = key;
  let parou = false;
  ev.stopPropagation = () => (parou = true);
  c.dispatchEvent(ev);
  return { cancelado: ev.defaultPrevented, parou };
}

test('rodada 24: o campo de renomear: Enter grava, Esc cancela, perder o foco grava, e acaba uma vez só', () => {
  const fim = [];
  const ligar = (valor) => {
    const c = campo(valor);
    ligarRenomear(c, 'Casa', (nome, comTeclado) => fim.push([nome, comTeclado]));
    return c;
  };
  // Enter
  let c = ligar('Casa');
  c.value = 'Lar';
  assert.deepEqual(tecla(c, 'Enter'), { cancelado: true, parou: true });
  c.dispatchEvent(new Event('blur'));
  assert.deepEqual(fim, [['Lar', true]], 'o blur depois do Enter não grava de novo');
  // Esc: volta sem gravar, mesmo com o texto mudado
  fim.length = 0;
  c = ligar('Casa');
  c.value = 'Outro';
  assert.deepEqual(tecla(c, 'Escape'), { cancelado: true, parou: true });
  assert.deepEqual(fim, [[null, true]]);
  // perder o foco grava
  fim.length = 0;
  c = ligar('Casa');
  c.value = 'Lar';
  c.dispatchEvent(new Event('blur'));
  assert.deepEqual(fim, [['Lar', false]]);
  // vazio ou igual: volta sem gravar
  fim.length = 0;
  c = ligar('Casa');
  c.value = '';
  tecla(c, 'Enter');
  c = ligar('Casa');
  c.dispatchEvent(new Event('blur'));
  assert.deepEqual(fim, [[null, true], [null, false]]);
  // outras teclas são do campo
  fim.length = 0;
  c = ligar('Casa');
  assert.deepEqual(tecla(c, 'a'), { cancelado: false, parou: false });
  // o campo que saiu da página (a tela redesenhou) não grava: o painel recria o campo
  c.isConnected = false;
  c.dispatchEvent(new Event('blur'));
  assert.deepEqual(fim, []);
});

test('rodada 24: o painel põe o campo no lugar do nome e o mantém nos redesenhos', () => {
  const p = fonte('painel.js');
  // o redesenho guarda o texto, a seleção e o foco, e devolve o campo depois
  assert.match(p, /function preservando\(fn\) \{\s*guardarRenomear\(\);/);
  assert.match(p, /if \(chave && !document\.contains\(ativo\)\) focarPelaChave\(chave\);\s*campoDeRenomear\(\);\s*\}/);
  // começa com o nome todo selecionado
  assert.match(p, /renomeando = \{ grupoId, original: g\.nome, valor: g\.nome, ini: 0, fim: g\.nome\.length, foco: true \};/);
  // no lugar do botão que abre e fecha o grupo (um campo dentro de um botão não recebe as teclas)
  assert.match(p, /gd\.replaceWith\(caixa\);/);
  assert.match(p, /campo\.setSelectionRange\(r\.ini, r\.fim\);/);
  // grava pelo editar_grupo, com o resto do grupo como está; com o teclado, o foco volta ao ⋯
  assert.match(p, /if \(nome && g\) await agir\('editar_grupo', \{ grupo: grupoParaSalvar\(\{ \.\.\.g, nome \}\) \}\);/);
  assert.match(p, /if \(comTeclado\) document\.querySelector\(`\.corpo \[data-acao="menu-grupo"\]\[data-grupo="\$\{r\.grupoId\}"\]`\)\?\.focus\(\);/);
  // o "Sem grupo" não tem ⋯: segue sem renomear
  assert.match(fonte('componentes.js'), /grupo\.id === null\s*\? ''\s*: `<button class="ge" data-acao="menu-grupo"/);
});

test('revisão da rodada 24: a janela perdendo o foco (Alt+Tab) não termina o renomear', () => {
  const fim = [];
  const antes = globalThis.document;
  let comFoco = false;
  globalThis.document = { hasFocus: () => comFoco };
  try {
    const c = campo('Casa');
    ligarRenomear(c, 'Casa', (nome, comTeclado) => fim.push([nome, comTeclado]));
    c.value = 'La';
    // a janela saiu: o campo segue aberto, com o texto pela metade
    c.dispatchEvent(new Event('blur'));
    assert.deepEqual(fim, []);
    // de volta à janela, o foco saindo do campo dentro da página grava
    comFoco = true;
    c.value = 'Lar';
    c.dispatchEvent(new Event('blur'));
    assert.deepEqual(fim, [['Lar', false]]);
  } finally {
    if (antes === undefined) delete globalThis.document;
    else globalThis.document = antes;
  }
});

test('revisão da rodada 24: o fim do renomear espera o clique que tirou o foco', async () => {
  // sem um mousedown recente (o foco saiu pelo Tab): segue já
  const doc = new EventTarget();
  let seguiu = false;
  await depoisDoClique(doc, -Infinity, () => 1000).then(() => (seguiu = true));
  assert.ok(seguiu);
  // com um mousedown agora: só depois do click (as ações da página rodam antes)
  seguiu = false;
  const espera = depoisDoClique(doc, 1000, () => 1000 + JANELA_DO_CLIQUE).then(() => (seguiu = true));
  await new Promise((ok) => setTimeout(ok, 20));
  assert.equal(seguiu, false, 'espera o clique');
  doc.dispatchEvent(new Event('click'));
  await espera;
  assert.ok(seguiu);
  // soltou fora do alvo (mouseup sem click): segue logo depois
  seguiu = false;
  const solta = depoisDoClique(doc, 1000, () => 1000).then(() => (seguiu = true));
  doc.dispatchEvent(new Event('mouseup'));
  await solta;
  assert.ok(seguiu);
  // no painel: o cabeçalho volta já, só ele; a gravação e o redesenho vêm depois do clique
  const p = fonte('painel.js');
  const fim = p.slice(p.indexOf('async function terminarRenomear'), p.indexOf('// ---- desenho ----'));
  assert.match(fim, /r\.caixa\.replaceWith\(r\.cabecalho\);\s*\}\s*if \(!comTeclado\) await depoisDoClique\(document, ultimoMousedown\);/);
  assert.ok(fim.indexOf('depoisDoClique') < fim.indexOf("agir('editar_grupo'") && fim.indexOf("agir('editar_grupo'") < fim.indexOf('desenhar();'));
  assert.match(p, /document\.addEventListener\('mousedown', \(\) => \(ultimoMousedown = performance\.now\(\)\), true\);/);
});

test('revisão da rodada 24: o mesmo limite do nome do grupo no Renomear, no "Novo grupo" e no "Editar grupo"', () => {
  assert.equal(GRUPO_NOME_MAX, 60);
  assert.match(fonte('painel.js'), /<input class="g-ren" type="text" maxlength="\$\{GRUPO_NOME_MAX\}"/);
  const e = fonte('editar.js');
  assert.match(e, /campoTexto\('nome', TEXTOS\.nome, '', '', `autofocus maxlength="\$\{GRUPO_NOME_MAX\}"`\) \+\s*campoEspaco\(snap, espacoId\)/, 'novo grupo');
  assert.match(e, /campoTexto\('nome', TEXTOS\.nome, g\.nome, '', `autofocus maxlength="\$\{GRUPO_NOME_MAX\}"`\) \+\s*campoEspaco\(snap, g\.espaco_id/, 'editar grupo');
  assert.match(e, /campoTexto\('grupo_nome', TEXTOS\.nomeDoGrupoNovo, '', '', `maxlength="\$\{GRUPO_NOME_MAX\}" /, 'o grupo novo do editar timer');
  // e o núcleo, com o mesmo número
  const rs = readFileSync(new URL('../../src-tauri/src/estado_app.rs', import.meta.url), 'utf8');
  assert.match(rs, new RegExp(`pub const GRUPO_NOME_MAX: usize = ${GRUPO_NOME_MAX};`));
});
