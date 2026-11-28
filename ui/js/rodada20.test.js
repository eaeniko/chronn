// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 20: os espaços (as abas, "Todos" com cabeçalhos, o total da aba, o ponto no dock), o
// "+ Novo" global (o menu, o grupo pré-escolhido, a tecla N, o grupo vazio) e as atividades que
// aparecem no painel de cada timer.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  espacosAtivos,
  comAbas,
  abaAtual,
  gruposDaAba,
  timersDaAba,
  htmlAbas,
  secoesDaAba,
  totalDaAba,
  dicaDoTotalDaAba,
  textoZerarTodos,
  textoPausarTudo,
  textoZeradosDaAba,
  grupoPreEscolhido,
  espacoPreEscolhido,
  ITENS_DO_MENU_NOVO,
  htmlDoMenuNovo,
  teclaDoNovo,
  pontoDoEspaco,
  gruposPorEspaco,
  topoDaTela,
  espacoDoBotao,
  podeApagarEspaco,
  destinoAoApagar,
  textoAoApagar,
  nomeDoGrupo,
} from './espacos.js';
import { linhasDeAtividade, TRABALHO, REUNIAO } from './formato.js';
import { usarAtividades, secaoGrupo, linhaGrupoVazio, barraRodando, chaveGrupo } from './componentes.js';
import { campoGrupo, campoEspaco, campoAtividadesNoPainel, atividadesOcultasDoForm, corpoLigarATimer, grupoDoForm, opcoesDeAtividade } from './editar.js';
import { TEXTOS } from './textos.js';

const fonte = (nome) => readFileSync(new URL(`./${nome}`, import.meta.url), 'utf8');

const M = 60_000;
const H = 60 * M;
const ESTUDO = 3;
const ATIVIDADES = [
  { id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivada: false },
  { id: 2, nome: 'Reunião', cor: '#e8a23a', ordem: 1, arquivada: false },
  { id: 3, nome: 'Estudo', cor: '#3fb68b', ordem: 2, arquivada: false },
];
usarAtividades(ATIVIDADES);

const timer = (id, nome, espaco, grupo, trechos = [], extra = {}) => ({
  id,
  nome,
  cor: '#ff6b4a',
  grupo_id: grupo,
  espaco_id: espaco,
  rodando: trechos.some((t) => t.fim === null),
  atividade_padrao_id: TRABALHO,
  manual_atividade_id: null,
  trechos,
  ajuste_ms: 0,
  ajustes: [],
  sessoes: [],
  zerado_em: 0,
  acumulado_antes: [],
  atividades_ocultas: [],
  ...extra,
});
const tr = (de, ate, atividade_id = TRABALHO) => ({ inicio: de, fim: ate, origem: 'manual', atividade_id });
const AGORA = 12 * H;

/** Trabalho (1): Empresa com Flow (9h–11h) e o Sem grupo dele com Solto; Pessoal (2): Casa (vazio)
 *  e o Sem grupo dele com Leitura (10h–rodando). */
const snapDe = (extra = {}) => {
  const flow = timer(10, 'Flow', 1, 100, [tr(9 * H, 11 * H)]);
  const solto = timer(11, 'Solto', 1, null);
  const leitura = timer(20, 'Leitura', 2, null, [tr(10 * H, null)]);
  return {
    agora: AGORA,
    espacos: [
      { id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivado: false, real_antes_ms: 0 },
      { id: 2, nome: 'Pessoal', cor: '#e86bd0', ordem: 1, arquivado: false, real_antes_ms: 0 },
      { id: 3, nome: 'Velho', cor: '#888888', ordem: 2, arquivado: true, real_antes_ms: 0 },
    ],
    grupos: [
      { id: 100, espaco_id: 1, nome: 'Empresa', cor: '#111111', timers: [flow], real_antes_ms: 0 },
      { id: null, espaco_id: 1, nome: 'Sem grupo', cor: '#8b8e98', timers: [solto], real_antes_ms: 0 },
      { id: 200, espaco_id: 2, nome: 'Casa', cor: '#222222', timers: [], real_antes_ms: 0 },
      { id: null, espaco_id: 2, nome: 'Sem grupo', cor: '#8b8e98', timers: [leitura], real_antes_ms: 0 },
    ],
    atividades: ATIVIDADES,
    config: { espaco_aba: null },
    ...extra,
  };
};
const umEspaco = () => {
  const s = snapDe();
  return { ...s, espacos: [s.espacos[0]], grupos: s.grupos.filter((g) => g.espaco_id === 1) };
};

test('rodada 20 §1: as abas aparecem só com 2 ou mais espaços não arquivados', () => {
  assert.deepEqual(espacosAtivos(snapDe()).map((e) => e.nome), ['Trabalho', 'Pessoal'], 'o arquivado fica fora');
  assert.equal(comAbas(snapDe()), true);
  const h = htmlAbas(snapDe(), null);
  assert.match(h, /^<div class="abas" role="tablist" aria-label="Espaços">/);
  assert.deepEqual([...h.matchAll(/data-espaco="(\d*)"[^>]*>(?:<i[^>]*><\/i>)?([^<]+)<\/button>/g)].map((m) => [m[1], m[2]]), [
    ['', 'Todos'],
    ['1', 'Trabalho'],
    ['2', 'Pessoal'],
  ]);
  assert.match(h, /class="aba on" data-acao="aba" data-espaco="" aria-selected="true">Todos/);
  assert.match(htmlAbas(snapDe(), 2), /class="aba on" data-acao="aba" data-espaco="2" aria-selected="true"><i style="--e:#e86bd0"/);
  // com um só: nada muda na tela
  assert.equal(comAbas(umEspaco()), false);
  assert.equal(htmlAbas(umEspaco(), null), '');
  assert.equal(abaAtual(umEspaco()), null);
  // as três telas desenham as abas
  // rodada 21: as abas moram no cabeçalho (#abas), para as três telas
  assert.match(fonte('painel.js'), /const abas = TELAS_COM_ABAS\.includes\(visao\) \? htmlAbas\(snap, abaDaTela\(\)\) : '';/);
  for (const f of ['cartoes.js', 'linha.js']) assert.doesNotMatch(fonte(f), /htmlAbas/, f);
});

test('rodada 20 §1: "Todos" mostra tudo, com o nome do espaço como cabeçalho acima dos grupos dele', () => {
  const s = snapDe();
  const h = secoesDaAba(s, null, (g) => `<g:${g.id ?? `sem-${g.espaco_id}`}>`, AGORA);
  assert.match(
    h,
    /^<section class="esp" data-espaco="1"><div class="esp-h" style="--e:#5b8def">.*<b>Trabalho<\/b>.*<\/div><g:100><g:sem-1><\/section><section class="esp" data-espaco="2"><div class="esp-h" style="--e:#e86bd0">.*<b>Pessoal<\/b>.*<\/div><g:200><g:sem-2><\/section>$/,
  );
  // o cabeçalho do espaço: a soma dos timers dele (Pessoal: 2h da Leitura), o tempo real na dica
  assert.match(h, /data-total-espaco="2" title="a lançar: 2h 00min · tempo real: 2h 00min">2h 00min</);
  // na aba de um espaço: só os grupos dele, sem cabeçalho
  assert.equal(secoesDaAba(s, 2, (g) => `<g:${g.id}>`, AGORA), '<g:200><g:null>');
  assert.deepEqual(gruposDaAba(s, 1).map((g) => g.id), [100, null]);
  assert.deepEqual(timersDaAba(s, 2).map((t) => t.nome), ['Leitura']);
  // cada espaço tem o seu "Sem grupo" (as chaves não se misturam)
  assert.deepEqual(s.grupos.map(chaveGrupo), ['100', 'sem', '200', 'sem-2']);
});

test('rodada 20 §1: a aba lembrada na config; um espaço que saiu volta a "Todos"', () => {
  assert.equal(abaAtual(snapDe({ config: { espaco_aba: 2 } })), 2);
  assert.equal(abaAtual(snapDe({ config: { espaco_aba: 3 } })), null, 'arquivado');
  assert.equal(abaAtual(snapDe({ config: { espaco_aba: 99 } })), null, 'apagado');
  assert.equal(abaAtual(snapDe({ config: {} })), null);
  // o clique na aba grava a escolha na config (salvar_config)
  const p = fonte('painel.js');
  assert.match(p, /case 'aba': \{[^]*?snap\.config = \{ \.\.\.snap\.config, espaco_aba: id \};[^]*?agir\('salvar_config', \{ config: snap\.config \}\)/);
});

test('rodada 20 §1: o total do dia é o da aba; a dica traz o geral e o de cada espaço', () => {
  const s = snapDe();
  assert.equal(totalDaAba(s, 1, AGORA), 2 * H);
  assert.equal(totalDaAba(s, 2, AGORA), 2 * H);
  assert.equal(totalDaAba(s, null, AGORA), 3 * H, 'a união de hoje: 9h às 12h');
  // spec 003: a soma e o real; o geral e os espaços no número que vale sozinho (a soma, em "Os dois")
  assert.equal(dicaDoTotalDaAba(s, 2, AGORA), 'a lançar: 2h 00min · tempo real: 2h 00min\nTotal geral 4h 00min · Trabalho 2h 00min · Pessoal 2h 00min');
  // com um espaço só, a dica de antes
  assert.equal(dicaDoTotalDaAba(umEspaco(), null, AGORA), 'a lançar: 2h 00min · tempo real: 2h 00min');
  // o "Zerar todos" e o "Pausar tudo" dizem a aba
  assert.equal(textoZerarTodos(s, 2), 'Zerar todos (Pessoal)');
  assert.equal(textoZerarTodos(s, null), 'Zerar todos');
  assert.equal(textoPausarTudo(s, 1), 'Pausar tudo (Trabalho)');
  assert.equal(textoZeradosDaAba(s, 2), 'Todos os timers de Pessoal zerados.');
  assert.equal(textoZeradosDaAba(s, null), null);
  assert.match(barraRodando(1, textoPausarTudo(s, 2), 2), /data-acao="pausar-tudo" data-espaco="2">Pausar tudo \(Pessoal\)</);
  // e vão ao núcleo com o espaço da aba
  const p = fonte('painel.js');
  assert.match(p, /agir\('pausar_tudo', \{ espaco_id: espacoDoBotao\(alvo\) \}\)/);
  assert.match(p, /agir\('zerar_todos', \{ espaco_id: a \}\)/);
});

test('rodada 20 §1: o ponto do espaço no dock, com o nome do espaço na dica', () => {
  const s = snapDe();
  const leitura = s.grupos[3].timers[0];
  assert.equal(pontoDoEspaco(s, leitura), '<i class="desp" style="--e:#e86bd0" title="Espaço: Pessoal" aria-label="Espaço: Pessoal"></i>');
  assert.equal(pontoDoEspaco(umEspaco(), leitura), '', 'com um só espaço, sem ponto');
  const d = fonte('dock.js');
  // rodada 24: o recolhido segue os Ajustes (espacoNoRecolhido); o expandido fica com o ponto
  assert.equal((d.match(/pontoDoEspaco\(snap, /g) ?? []).length, 1, 'só no expandido');
  // o dock mostra o que roda em todos os espaços: o total do dock não usa a aba
  assert.doesNotMatch(d, /abaAtual|espaco_aba/);
});

test('rodada 20 §1: "Outro timer…" lista os timers por espaço e grupo', () => {
  const s = snapDe();
  assert.deepEqual(gruposPorEspaco(s).map((x) => [x.espaco.nome, x.grupos.map((g) => g.id)]), [
    ['Trabalho', [100, null]],
    ['Pessoal', [200, null]],
  ]);
  const h = corpoLigarATimer(s, { id: 's1', nome: 'Sessão', cwd: null, timer_id: null });
  assert.match(h, /<div class="lista-e" style="--e:#5b8def"><i><\/i>Trabalho<\/div><div class="lista-g">Empresa<\/div>[^]*Flow[^]*<div class="lista-g">Sem grupo<\/div>[^]*Solto[^]*<div class="lista-e" style="--e:#e86bd0"><i><\/i>Pessoal<\/div><div class="lista-g">Sem grupo<\/div>[^]*Leitura/);
  assert.doesNotMatch(corpoLigarATimer(umEspaco(), { id: 's1', nome: 'x', cwd: null, timer_id: null }), /lista-e/);
});

test('rodada 20 §2: o menu do "+ Novo" tem Timer, Grupo e Espaço', () => {
  assert.deepEqual(ITENS_DO_MENU_NOVO.map((i) => [i.id, i.rotulo]), [['timer', 'Timer'], ['grupo', 'Grupo'], ['espaco', 'Espaço']]);
  assert.equal(
    htmlDoMenuNovo(),
    '<div class="menu-ajuda menu-novo" role="menu" aria-label="Novo"><button type="button" role="menuitem" data-novo="timer">Timer</button><button type="button" role="menuitem" data-novo="grupo">Grupo</button><button type="button" role="menuitem" data-novo="espaco">Espaço</button></div>',
  );
  // o botão no topo, ao lado do total
  const idx = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
  assert.match(idx, /<button class="novo principal" id="bt-novo" data-acao="novo-menu"[^>]*aria-haspopup="menu" aria-expanded="false"><svg class="bi"[^>]*>.*?<\/svg><span data-texto="novoRotulo">Novo<\/span><\/button>/);
  // os cartões tracejados e o "+ Novo grupo" solto saíram
  for (const f of ['componentes.js', 'painel.js', 'cartoes.js', 'foco.js']) assert.doesNotMatch(fonte(f), /novoTimerBotao|novoGrupoBotao|k-novo/, f);
});

test('rodada 20 §2: o grupo pré-escolhido do timer novo em cada caso', () => {
  const s = snapDe();
  const flow = s.grupos[0].timers[0];
  const leitura = s.grupos[3].timers[0];
  // o timer aberto no painel: o grupo dele (ou o "Sem grupo" do espaço dele)
  assert.deepEqual(grupoPreEscolhido(s, null, flow), { grupoId: 100, espacoId: 1 });
  assert.deepEqual(grupoPreEscolhido(s, 2, leitura), { grupoId: null, espacoId: 2 });
  // sem timer aberto: o "Sem grupo" do espaço da aba; em "Todos", do primeiro espaço
  assert.deepEqual(grupoPreEscolhido(s, 2, null), { grupoId: null, espacoId: 2 });
  assert.deepEqual(grupoPreEscolhido(s, null, null), { grupoId: null, espacoId: 1 });
  // o grupo novo: o espaço da aba
  assert.equal(espacoPreEscolhido(s, 2), 2);
  assert.equal(espacoPreEscolhido(s, null), 1);
  // o campo Grupo do diálogo: o grupo marcado; sem grupo, o espaço do "Sem grupo" à vista
  const comGrupo = campoGrupo(s, 100, 1);
  assert.match(comGrupo, /<optgroup label="Trabalho"><option value="100" selected>Empresa<\/option><option value="sem:1">Sem grupo<\/option><\/optgroup><optgroup label="Pessoal"><option value="200">Casa<\/option><option value="sem:2">Sem grupo<\/option><\/optgroup>/);
  assert.match(comGrupo, /<div class="timer-espaco" hidden>/);
  const semGrupo = campoGrupo(s, null, 2);
  assert.match(semGrupo, /<option value="sem:2" selected>Sem grupo<\/option>/);
  assert.match(semGrupo, /<div class="timer-espaco" hidden><label class="campo"><span>Espaço<\/span><select name="espaco"><option value="1">Trabalho<\/option><option value="2" selected>Pessoal<\/option><\/select>/);
  // o "Novo grupo" e o editar grupo têm o campo Espaço
  assert.match(campoEspaco(s, 2), /<option value="2" selected>Pessoal/);
  assert.doesNotMatch(campoEspaco(s, 1), /Velho/, 'o arquivado não é oferecido');
  assert.match(campoEspaco(s, 3), /<option value="3" selected>Velho/, 'a não ser o do próprio grupo');
  const e = fonte('editar.js');
  assert.match(e, /campoEspaco\(snap, espacoId\) \+/);
  assert.match(e, /campoEspaco\(snap, g\.espaco_id \?\? ESPACO_PRIMEIRO\) \+/);
  // o painel usa o timer aberto (só no Painel) e a aba
  assert.match(fonte('painel.js'), /const aberto = visao === 'painel' \? timerSelecionado\(\) : null;\s*const \{ grupoId, espacoId \} = grupoPreEscolhido\(snap, a, aberto\);/);
});

test('rodada 20 §2: a tecla N abre o "+ Novo", mas não com o foco num campo', () => {
  const ev = (key, extra = {}) => ({ key, ctrlKey: false, altKey: false, metaKey: false, repeat: false, ...extra });
  const corpo = { tagName: 'BODY', isContentEditable: false };
  assert.equal(teclaDoNovo(ev('n'), corpo), true);
  assert.equal(teclaDoNovo(ev('N'), corpo), true);
  assert.equal(teclaDoNovo(ev('n'), null), true);
  for (const tag of ['INPUT', 'TEXTAREA', 'SELECT']) assert.equal(teclaDoNovo(ev('n'), { tagName: tag }), false, tag);
  assert.equal(teclaDoNovo(ev('n'), { tagName: 'DIV', isContentEditable: true }), false);
  assert.equal(teclaDoNovo(ev('n', { ctrlKey: true }), corpo), false, 'Ctrl+N não');
  assert.equal(teclaDoNovo(ev('n', { repeat: true }), corpo), false);
  assert.equal(teclaDoNovo(ev('m'), corpo), false);
  assert.equal(teclaDoNovo(ev('n'), corpo, true), false, 'com um diálogo aberto, não');
  assert.match(fonte('painel.js'), /teclaDoNovo\(ev, document\.activeElement, dialogoAberto\(\) \|\| tourAtivo\(\)\)/);
});

test('rodada 20 §2: o grupo vazio mostra só "Grupo vazio · + Timer"', () => {
  const s = snapDe();
  const casa = s.grupos[2];
  assert.equal(
    linhaGrupoVazio(casa),
    '<div class="g-vazio">Grupo vazio · <button type="button" data-acao="novo-timer" data-grupo="200" data-espaco="2">+ Timer</button></div>',
  );
  const h = secaoGrupo(casa, AGORA, true);
  assert.ok(h.includes(linhaGrupoVazio(casa)));
  assert.doesNotMatch(h, /class="grid"/);
  // fechado, nada; com timers, a grade sem cartão tracejado
  assert.doesNotMatch(secaoGrupo(casa, AGORA, false), /g-vazio/);
  assert.doesNotMatch(secaoGrupo(s.grupos[0], AGORA, true), /g-vazio|Novo timer/);
  // o clique abre o "Novo timer" já nesse grupo (e no espaço dele)
  assert.match(fonte('painel.js'), /novoTimer\(snap, grupoId, null, \{ espacoId: Number\(alvo\.dataset\.espaco \|\| 1\) \}\)/);
});

test('rodada 20 §3: linhasDeAtividade com ocultas; a oculta com tempo ou rodando aparece', () => {
  const t = (extra) => timer(1, 'A', 1, null, [], extra);
  const ids = (x) => linhasDeAtividade(x, ATIVIDADES, AGORA).map((a) => a.id);
  assert.deepEqual(ids(t({})), [TRABALHO, REUNIAO, ESTUDO]);
  // ocultas sem tempo: somem
  assert.deepEqual(ids(t({ atividades_ocultas: [REUNIAO, ESTUDO] })), [TRABALHO]);
  // oculta com tempo desde o zerar: aparece
  assert.deepEqual(ids(t({ atividades_ocultas: [REUNIAO, ESTUDO], trechos: [tr(9 * H, 9 * H + 30 * M, ESTUDO)] })), [TRABALHO, ESTUDO]);
  // oculta com tempo só antes do zerar: some
  assert.deepEqual(ids(t({ atividades_ocultas: [ESTUDO], trechos: [tr(9 * H, 9 * H + 30 * M, ESTUDO)], zerado_em: 10 * H })), [TRABALHO, REUNIAO]);
  // oculta rodando: aparece (rodando)
  const rodando = linhasDeAtividade(t({ atividades_ocultas: [REUNIAO], trechos: [tr(AGORA, null, REUNIAO)] }), ATIVIDADES, AGORA);
  assert.deepEqual(rodando.map((a) => [a.id, a.rodando]), [[TRABALHO, false], [REUNIAO, true], [ESTUDO, false]]);
  // a padrão nunca é ocultada (mesmo vindo na lista)
  assert.deepEqual(ids(t({ atividades_ocultas: [TRABALHO, REUNIAO] })), [TRABALHO, ESTUDO]);
  assert.deepEqual(ids(t({ atividade_padrao_id: ESTUDO, atividades_ocultas: [ESTUDO, REUNIAO] })), [TRABALHO, ESTUDO]);
});

test('rodada 20 §3: "Atividades no painel" no editar timer; a padrão não oculta', () => {
  const t = timer(1, 'A', 1, null, [], { atividades_ocultas: [ESTUDO] });
  const h = campoAtividadesNoPainel(t, ATIVIDADES);
  assert.match(h, /^<fieldset class="campo at-painel-g"><legend>Atividades no painel<\/legend>/);
  // a padrão: marcada, desabilitada, com a dica
  assert.match(h, /<label class="chk at-painel" title="A atividade padrão do timer aparece sempre"><input type="checkbox" name="at_painel" value="1" checked disabled>/);
  assert.match(h, /<input type="checkbox" name="at_painel" value="2" checked><span><i style="--a:#e8a23a"><\/i>Reunião/);
  assert.match(h, /<input type="checkbox" name="at_painel" value="3"><span>/, 'a oculta vem desmarcada');
  // o padrão é todas marcadas
  assert.equal((campoAtividadesNoPainel(timer(1, 'A', 1, null), ATIVIDADES).match(/ checked/g) ?? []).length, 3);
  // o que vai ao núcleo: as desmarcadas, nunca a padrão
  const form = {
    querySelectorAll: () => [
      { value: '1', checked: false, disabled: true },
      { value: '2', checked: false, disabled: false },
      { value: '3', checked: true, disabled: false },
    ],
  };
  assert.deepEqual(atividadesOcultasDoForm(form), [2]);
  assert.equal(atividadesOcultasDoForm({ querySelectorAll: () => [] }), null, 'sem a seção, ficam as de antes');
  assert.match(fonte('editar.js'), /const ocultas = atividadesOcultasDoForm\(form\);[\s\S]*chamar\('editar_timer', \{[\s\S]*?ocultas,\s*\}\);/);
});

test('rodada 20: os textos novos da interface ficam num lugar só', () => {
  assert.equal(TEXTOS.novo, '+ Novo');
  assert.equal(TEXTOS.grupoVazio, 'Grupo vazio');
  assert.equal(TEXTOS.atividadesNoPainel, 'Atividades no painel');
  for (const f of ['espacos.js', 'componentes.js', 'editar.js', 'ajustes.js', 'tour.js']) assert.match(fonte(f), /from '\.\/textos\.js'/, f);
});

// ---- revisão da rodada 20 ----

test('revisão da rodada 20 (I2): o escopo do "Zerar todos" e do "Pausar tudo" vem da mesma fonte do texto, em toda tela', () => {
  const s = snapDe({ config: { espaco_aba: 2 } });
  // a pessoa está na aba Pessoal do Painel e troca de tela
  const painel = topoDaTela(s, 'painel');
  assert.deepEqual(painel, { espaco: 2, texto: 'Zerar todos (Pessoal)', dica: TEXTOS.dicaZerarTodosDoEspaco('Pessoal'), pausar: 'Pausar tudo (Pessoal)' });
  for (const visao of ['cartoes', 'linha', 'ajustes']) assert.deepEqual(topoDaTela(s, visao), painel, `${visao}: a aba lembrada e o texto dela`);
  // o Foco não tem abas: "Todos", e o texto diz isso
  assert.deepEqual(topoDaTela(s, 'foco'), { espaco: null, texto: 'Zerar todos', dica: null, pausar: 'Pausar tudo' });
  // o clique lê o escopo gravado no botão junto com o texto: nunca outro
  const botao = (topo) => ({ dataset: { espaco: String(topo.espaco ?? '') }, textContent: topo.texto });
  for (const visao of ['painel', 'cartoes', 'linha', 'ajustes', 'foco']) {
    const topo = topoDaTela(s, visao);
    const b = botao(topo);
    const escopo = espacoDoBotao(b);
    assert.equal(escopo, topo.espaco, visao);
    assert.equal(b.textContent.includes('(Pessoal)'), escopo === 2, `${visao}: o texto e o clique concordam`);
  }
  assert.match(barraRodando(1, 'Pausar tudo (Pessoal)', 2), /data-acao="pausar-tudo" data-espaco="2">Pausar tudo \(Pessoal\)</);
  assert.equal(espacoDoBotao({ dataset: { espaco: '' } }), null);
  const p = fonte('painel.js');
  // o topo é redesenhado também nos Ajustes, e o botão leva o escopo do mesmo objeto do texto
  assert.match(p, /if \(visao === 'ajustes'\) \{[^}]*desenharTopoDaAba\(\);/);
  assert.match(p, /const topo = topoDaTela\(snap, visao\);\s*zt\.dataset\.espaco = topo\.espaco \?\? '';\s*trocarTexto\(zt\.querySelector\('\.rot'\), topo\.texto\);/);
  assert.match(p, /const a = espacoDoBotao\(alvo\);\s*const z = await agir\('zerar_todos', \{ espaco_id: a \}\);/);
});

test('revisão da rodada 20 (I1): apagar um espaço só sem nada à vista; os arquivados vão para outro', () => {
  const s = snapDe();
  const [trabalho, pessoal, velho] = s.espacos;
  // com grupo ou timer à vista: desabilitado, com a dica
  assert.deepEqual(podeApagarEspaco(s, { ...pessoal, ativos: 2, arquivados: 0 }), { pode: false, dica: 'Mova os grupos para outro espaço antes' });
  // só com arquivados: pode, e a confirmação diz para onde eles vão
  const soArquivados = { ...pessoal, ativos: 0, arquivados: 3 };
  assert.deepEqual(podeApagarEspaco(s, soArquivados), { pode: true, dica: '' });
  assert.equal(destinoAoApagar(s, pessoal.id).nome, 'Trabalho');
  assert.equal(textoAoApagar(s, soArquivados), '3 itens arquivados vão para Trabalho.');
  assert.equal(textoAoApagar(s, { ...soArquivados, arquivados: 1 }), '1 item arquivado vai para Trabalho.');
  assert.equal(textoAoApagar(s, { ...soArquivados, arquivados: 0 }), 'O espaço não tem nada dentro.');
  assert.equal(destinoAoApagar(s, trabalho.id).nome, 'Pessoal');
  // o último não arquivado nunca; um arquivado pode (vai para o que sobra)
  const so = umEspaco();
  assert.deepEqual(podeApagarEspaco(so, { ...so.espacos[0], ativos: 0, arquivados: 0 }), { pode: false, dica: TEXTOS.ultimoEspaco });
  assert.equal(podeApagarEspaco(s, { ...velho, ativos: 0, arquivados: 1 }).pode, true);
  // os Ajustes usam a mesma regra e o mesmo texto
  const aj = fonte('ajustes.js');
  assert.match(aj, /const ap = podeApagarEspaco\(snap, e\);/);
  assert.match(aj, /corpo: `<p class="dlg-p">\$\{esc\(textoAoApagar\(lerSnap\(\), e\)\)\}<\/p>`/);
});

test('revisão da rodada 20 (M1, M3, M5): o rodando de espaço arquivado em "Todos"; sem campo Espaço com um só; "Sem grupo · Pessoal"', () => {
  // M1: um timer rodando num espaço arquivado aparece em "Todos", depois dos outros
  const s = snapDe();
  const comArquivado = { ...s, grupos: [...s.grupos, { id: null, espaco_id: 3, nome: 'Sem grupo', cor: '#8b8e98', timers: [timer(30, 'Antigo', 3, null, [tr(11 * H, null)])], real_antes_ms: 0 }] };
  const h = secoesDaAba(comArquivado, null, (g) => `<g:${g.id ?? `sem-${g.espaco_id}`}>`, AGORA);
  assert.match(h, /<section class="esp" data-espaco="3">[^]*<b>Velho<\/b>[^]*<g:sem-3><\/section>$/);
  assert.equal(totalDaAba(comArquivado, null, AGORA), 3 * H, 'no total do dia');
  // M3: com um espaço só, nenhum campo Espaço
  const so = umEspaco();
  assert.equal(campoEspaco(so, 1), '');
  assert.doesNotMatch(campoGrupo(so, null, 1), /name="espaco"/);
  assert.match(campoGrupo(s, null, 2), /name="espaco"/);
  // M5: "Sem grupo · <espaço>" no Foco e no dock expandido, com 2 ou mais espaços
  assert.equal(nomeDoGrupo(s, s.grupos[3]), 'Sem grupo · Pessoal');
  assert.equal(nomeDoGrupo(s, s.grupos[0]), 'Empresa');
  assert.equal(nomeDoGrupo(so, so.grupos[1]), 'Sem grupo');
  assert.match(fonte('foco.js'), /cabecalhoGrupo\(\{ \.\.\.g, nome: nomeDoGrupo\(snap, g\) \}/);
  assert.match(fonte('dock.js'), /<div class="dx-g">\$\{esc\(nomeDoGrupo\(snap, g\)\)\}<\/div>/);
  // M4: sem o espaço 1 fixo como padrão nos diálogos
  assert.doesNotMatch(fonte('editar.js'), /espacoId = ESPACO_PRIMEIRO|padrao = ESPACO_PRIMEIRO/);
});

test('re-revisão da rodada 20 (R1): o seletor nunca troca em silêncio um valor fora da lista', () => {
  const s = snapDe();
  // o timer rodando de um grupo arquivado (no "Sem grupo"): o grupo dele fica como opção marcada
  const h = campoGrupo(s, 999, 1, 'Casa velha');
  assert.match(h, /<select name="grupo"><option value="999" selected>Casa velha \(arquivado\)<\/option>/);
  assert.equal((h.match(/ selected/g) ?? []).length, 2, 'só o grupo e o espaço (escondido) marcados');
  assert.match(h, /<div class="timer-espaco" hidden>/);
  // salvar sem mexer no seletor mantém o grupo
  assert.deepEqual(grupoDoForm('999', '', ''), { id: 999 });
  // sem o nome (não deveria), ainda marca o atual
  assert.match(campoGrupo(s, 999, 1), /<option value="999" selected>Grupo \(arquivado\)<\/option>/);
  // o grupo à vista não ganha opção a mais
  assert.doesNotMatch(campoGrupo(s, 100, 1), /arquivado/);
  // o espaço arquivado também fica marcado, dizendo que é arquivado
  assert.match(campoEspaco(s, 3), /<option value="3" selected>Velho \(arquivado\)<\/option>/);
  // a atividade padrão arquivada também (a regra de antes, conferida)
  const arq = [...ATIVIDADES.slice(0, 2), { ...ATIVIDADES[2], arquivada: true }];
  usarAtividades(arq);
  assert.match(opcoesDeAtividade(arq.filter((a) => !a.arquivada), ESTUDO), /<option value="3" selected>Estudo \(arquivada\)<\/option>/);
  usarAtividades(ATIVIDADES);
  // o editar timer passa o nome do grupo arquivado que o núcleo manda
  assert.match(fonte('editar.js'), /campoGrupo\(snap, t\.grupo_id, t\.espaco_id \?\? ESPACO_PRIMEIRO, t\.grupo_arquivado \?\? null\)/);
});

test('re-revisão da rodada 20 (R3): "Sem grupo · <espaço>" com dois "Sem grupo" à vista, mesmo sem abas', () => {
  const so = umEspaco();
  // um espaço ativo e o "Sem grupo" de um arquivado com um timer rodando: sem abas, mas dois "Sem grupo"
  const comArquivado = {
    ...so,
    espacos: [...so.espacos, { id: 3, nome: 'Velho', cor: '#888888', ordem: 2, arquivado: true, real_antes_ms: 0 }],
    grupos: [...so.grupos, { id: null, espaco_id: 3, nome: 'Sem grupo', cor: '#8b8e98', timers: [], real_antes_ms: 0 }],
  };
  assert.equal(comAbas(comArquivado), false);
  assert.deepEqual(comArquivado.grupos.filter((g) => g.id === null).map((g) => nomeDoGrupo(comArquivado, g)), ['Sem grupo · Trabalho', 'Sem grupo · Velho']);
  // um "Sem grupo" só: o nome de antes
  assert.equal(nomeDoGrupo(so, so.grupos[1]), 'Sem grupo');
});
