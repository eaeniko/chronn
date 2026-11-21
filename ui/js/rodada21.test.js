// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 21: mover timer e grupo entre espaços, o cabeçalho novo (com o "+ Novo" principal e o
// tema nos Ajustes), a linha do tempo recolhível no Painel e o Foco com todas as atividades.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  itensDoMenuGrupo,
  htmlDoMenuGrupo,
  htmlDoMenuEspacos,
  ITENS_DO_MENU_TIMER,
  htmlDoMenuTimer,
  destinosDoTimer,
  htmlDoMenuDestinos,
  textoDoMovido,
} from './espacos.js';
import { temaResolvido, segueOSistema, TRABALHO } from './formato.js';
import { usarAtividades, cabecalhoGrupo, cartaoTimer, comOAtual } from './componentes.js';
import { campoGrupo, campoEspaco, grupoDoForm, opcoesDeAtividade } from './editar.js';
import { linhaDoRecente } from './primeiro-uso.js';
import { htmlAparencia, htmlPainel } from './ajustes.js';
import { secaoLinhaDoPainel, CHAVE_LINHA } from './linha.js';
import { visaoFoco } from './foco.js';
import { PASSOS, MINI_TOURS, PASSOS_ESSENCIAL } from './tour.js';
import { DICAS } from './dicas.js';

const fonte = (nome) => readFileSync(new URL(`./${nome}`, import.meta.url), 'utf8');
const INDEX = readFileSync(new URL('../index.html', import.meta.url), 'utf8');

const M = 60_000;
const H = 60 * M;
const ATIVIDADES = [
  { id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivada: false },
  { id: 2, nome: 'Reunião', cor: '#e8a23a', ordem: 1, arquivada: false },
  { id: 3, nome: 'Estudo', cor: '#3fb68b', ordem: 2, arquivada: false },
];
usarAtividades(ATIVIDADES);

const timer = (id, nome, espaco, grupo, trechos = []) => ({
  id, nome, cor: '#ff6b4a', grupo_id: grupo, espaco_id: espaco, rodando: trechos.some((t) => t.fim === null),
  atividade_padrao_id: TRABALHO, manual_atividade_id: null, trechos, ajuste_ms: 0, ajustes: [], sessoes: [],
  zerado_em: 0, acumulado_antes: [], atividades_ocultas: [],
});
const AGORA = 12 * H;
const snapDe = () => {
  const obra = timer(10, 'Obra', 2, 200, [{ inicio: 9 * H, fim: 10 * H, origem: 'manual', atividade_id: 1 }]);
  const flow = timer(11, 'Flow', 1, 100);
  const leitura = timer(12, 'Leitura', 2, null);
  return {
    agora: AGORA,
    dia_inicio: 0,
    espacos: [
      { id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivado: false, real_antes_ms: 0 },
      { id: 2, nome: 'Pessoal', cor: '#e86bd0', ordem: 1, arquivado: false, real_antes_ms: 0 },
    ],
    grupos: [
      { id: 100, espaco_id: 1, nome: 'Empresa', cor: '#111111', timers: [flow], real_antes_ms: 0 },
      { id: 200, espaco_id: 2, nome: 'Casa', cor: '#222222', timers: [obra], real_antes_ms: 0 },
      { id: null, espaco_id: 2, nome: 'Sem grupo', cor: '#8b8e98', timers: [leitura], real_antes_ms: 0 },
    ],
    atividades: ATIVIDADES,
    config: { espaco_aba: null, painel_linha: true, corte_min: 0 },
  };
};
const umEspaco = () => {
  const s = snapDe();
  return { ...s, espacos: [s.espacos[0]], grupos: [s.grupos[0], { ...s.grupos[1], espaco_id: 1 }] };
};

test('rodada 21 §1: o menu ⋯ do grupo; com um espaço só, sem "Mover para outro espaço…"', () => {
  const s = snapDe();
  const casa = s.grupos[1];
  // rodada 24: o "Renomear" é o primeiro
  assert.deepEqual(itensDoMenuGrupo(s, casa).map((i) => [i.id, i.rotulo]), [
    ['renomear', 'Renomear'],
    ['editar', 'Editar'],
    ['mover', 'Mover para outro espaço…'],
    ['arquivar', 'Arquivar'],
  ]);
  assert.match(htmlDoMenuGrupo(s, casa), /^<div class="menu-ajuda menu-grupo" role="menu" aria-label="Casa"><button type="button" role="menuitem" data-mg="renomear" data-grupo="200">Renomear<\/button><button type="button" role="menuitem" data-mg="editar" data-grupo="200">Editar<\/button><button type="button" role="menuitem" data-mg="mover" data-grupo="200">Mover para outro espaço…<\/button>/);
  // o segundo passo: os outros espaços
  assert.equal(
    htmlDoMenuEspacos(s, casa),
    '<div class="menu-ajuda menu-mover" role="menu" aria-label="Mover para outro espaço…"><button type="button" role="menuitem" data-mover-grupo="200" data-espaco="1">Trabalho</button></div>',
  );
  // com um espaço só, a opção some
  const so = umEspaco();
  assert.deepEqual(itensDoMenuGrupo(so, so.grupos[1]).map((i) => i.id), ['renomear', 'editar', 'arquivar']);
  // o ⋯ fica no cabeçalho do grupo (não no "Sem grupo") e chama o menu
  assert.match(cabecalhoGrupo(casa, AGORA, true), /<button class="ge" data-acao="menu-grupo" data-grupo="200" title="Mais" aria-label="Mais: Casa" aria-haspopup="menu" aria-expanded="false">⋯<\/button>/);
  assert.doesNotMatch(cabecalhoGrupo(s.grupos[2], AGORA, true), /menu-grupo/);
  const p = fonte('painel.js');
  assert.match(p, /case 'menu-grupo':\s*alternarMenuGrupo\(alvo, grupoId\);/);
  assert.match(p, /abrirMenu\(botao, htmlDoMenuEspacos\(snap, g\), 'esquerda', \(e\) => moverGrupo\(g, Number\(e\.dataset\.espaco\)\)\)/);
});

test('rodada 21 §1: o ⋯ do cartão e o "Mover para…" do timer, por espaço; com um só, só os grupos', () => {
  const s = snapDe();
  const leitura = s.grupos[2].timers[0];
  assert.deepEqual(ITENS_DO_MENU_TIMER.map((i) => i.rotulo), ['Editar', 'Mover para…']);
  assert.match(htmlDoMenuTimer(leitura), /data-mt="mover" data-timer="12">Mover para…/);
  assert.match(cartaoTimer(leitura, AGORA, false), /<button class="km" data-acao="menu-timer" data-timer="12" title="Mais"[^>]*>⋯<\/button>/);
  assert.match(fonte('cartoes.js'), /\$\{botaoMenuTimer\(timer\)\}/);
  // os destinos: por espaço, os grupos e o "Sem grupo" de cada um; onde ele está, desabilitado
  assert.deepEqual(
    destinosDoTimer(s, leitura).map((d) => [d.espaco.nome, d.itens.map((i) => [i.rotulo, i.grupoId, i.espacoId, i.atual])]),
    [
      ['Trabalho', [['Empresa', 100, 1, false], ['Sem grupo', null, 1, false]]],
      ['Pessoal', [['Casa', 200, 2, false], ['Sem grupo', null, 2, true]]],
    ],
  );
  const h = htmlDoMenuDestinos(s, leitura);
  assert.match(h, /<div class="menu-sec" role="presentation"><i style="--e:#5b8def"><\/i>Trabalho<\/div><button type="button" role="menuitem" data-mover-timer="12" data-grupo="100" data-espaco="1">Empresa<\/button>/);
  assert.match(h, /data-grupo="" data-espaco="2" disabled>Sem grupo · aqui<\/button>/);
  // com um espaço só: só os grupos (e o "Sem grupo"), sem o nome do espaço
  const so = umEspaco();
  const d = destinosDoTimer(so, so.grupos[0].timers[0]);
  assert.deepEqual(d.map((x) => [x.espaco, x.itens.map((i) => i.rotulo)]), [[null, ['Empresa', 'Casa', 'Sem grupo']]]);
  assert.doesNotMatch(htmlDoMenuDestinos(so, so.grupos[0].timers[0]), /menu-sec/);
});

test('rodada 21 §1: o campo Grupo com os optgroup de todos os espaços e o "Sem grupo" de cada um', () => {
  const s = snapDe();
  const h = campoGrupo(s, null, 2);
  assert.match(
    h,
    /<select name="grupo"><optgroup label="Trabalho"><option value="100">Empresa<\/option><option value="sem:1">Sem grupo<\/option><\/optgroup><optgroup label="Pessoal"><option value="200">Casa<\/option><option value="sem:2" selected>Sem grupo<\/option><\/optgroup><option value="novo">\+ Novo grupo…<\/option><\/select>/,
  );
  // o "Sem grupo" de um espaço vira o espaço do timer
  assert.deepEqual(grupoDoForm('sem:2', '', ''), { id: null, espaco: 2 });
  assert.deepEqual(grupoDoForm('100', '', ''), { id: 100 });
  // com um espaço só: como antes, sem optgroup
  const so = umEspaco();
  assert.doesNotMatch(campoGrupo(so, null, 1), /optgroup|sem:/);
  assert.match(campoGrupo(so, null, 1), /<option value="" selected>Sem grupo<\/option>/);
});

test('rodada 21 §1: o aviso de mover, com o Desfazer por 10 s', () => {
  const s = snapDe();
  assert.equal(
    textoDoMovido(s, 'Obra', { espacoId: 2, espacoAntes: 1 }),
    'Obra foi para Pessoal. As horas dele, também as de dias anteriores, agora contam em Pessoal.',
  );
  assert.equal(
    textoDoMovido(s, 'Flow', { grupoId: 200, espacoId: 2, espacoAntes: 1 }),
    'Flow foi para Casa. As horas dele, também as de dias anteriores, agora contam em Pessoal.',
  );
  assert.equal(textoDoMovido(s, 'Leitura', { grupoId: 200, espacoId: 2, espacoAntes: 2 }), 'Leitura foi para Casa.');
  const p = fonte('painel.js');
  assert.match(p, /const DESFAZER_MOVER_MS = 10_000;/);
  assert.match(p, /agir\('mover_grupo', \{ grupo_id: g\.id, espaco_id: espacoId \}\)/);
  assert.match(p, /agir\('mover_timer', \{ timer_id: t\.id, grupo_id: grupoId, espaco_id: espacoId \}\)/);
  assert.equal((p.match(/chamar\('desfazer_mover', \{ movido: m \}\)/g) ?? []).length, 2);
});

test('rodada 21 §2: o cabeçalho em duas linhas, com o "+ Novo" principal (a ordem nova está na rodada 22)', () => {
  const l2 = INDEX.slice(INDEX.indexOf('<div class="top-l2">'), INDEX.indexOf('</header>'));
  // o "+ Novo" é o botão principal; o tema saiu do cabeçalho
  assert.match(INDEX, /<button class="novo principal" id="bt-novo"/);
  assert.doesNotMatch(INDEX, /bt-tema|data-acao="tema"/);
  const css = readFileSync(new URL('../css/app.css', import.meta.url), 'utf8');
  assert.match(css, /\.novo\.principal \{[^}]*background: var\(--acento\); color: #fff;/);
  assert.match(css, /\.top-l2 \{[^}]*border-top: 1px solid var\(--ln\);/);
  // o total é o maior texto do cabeçalho
  const px = (sel) => Number(css.match(new RegExp(`${sel} \\{[^}]*font-size: (\\d+)px`))[1]);
  assert.ok(px('\\.day b') > px('\\.logo') && px('\\.day b') > px('\\.novo'), 'o total é o maior');
  // sem espaço, as ações da linha 2 viram ícones antes de a linha quebrar (o "+ Novo" mantém o texto)
  assert.match(css, /\.top\.compacta \.acao \{ width: 30px;[^}]*\}\s*\.top\.compacta \.acao \.rot \{ display: none; \}/);
  assert.doesNotMatch(css, /\.novo[^{]*\.rot|compacta \.novo|l1-[a-z-]+ \.novo/);
  assert.match(fonte('painel.js'), /if \(acoes\.offsetTop > telas\.offsetTop \+ 4\) top\.classList\.add\('compacta'\);/);
  // cada ação tem o texto na dica (o ícone sozinho continua dizendo o que faz)
  for (const id of ['bt-pausar-tudo', 'bt-zerar-todos']) assert.match(l2, new RegExp(`id="${id}"[^>]*title="`), id);
  // "N rodando" e "Pausar tudo" saíram do corpo do Painel
  assert.doesNotMatch(fonte('painel.js'), /barraRodando/);
});

test('rodada 21 §2: o tema em Ajustes → Aparência; Sistema segue o prefers-color-scheme', () => {
  assert.equal(temaResolvido('sistema', true), 'escuro');
  assert.equal(temaResolvido('sistema', false), 'claro');
  assert.equal(temaResolvido('windows', true), 'escuro', 'o nome de antes vale como Sistema');
  assert.equal(temaResolvido('claro', true), 'claro');
  assert.equal(temaResolvido('escuro', false), 'escuro');
  assert.equal(segueOSistema('sistema'), true);
  assert.equal(segueOSistema('claro'), false);
  const h = htmlAparencia('sistema');
  assert.deepEqual([...h.matchAll(/value="(\w+)"[^>]*>\s*<span>(\w+)/g)].map((m) => [m[1], m[2]]), [
    ['claro', 'Claro'],
    ['escuro', 'Escuro'],
    ['sistema', 'Sistema'],
  ]);
  assert.match(h, /value="sistema" checked/);
  assert.match(htmlAparencia('windows'), /value="sistema" checked/);
  assert.match(htmlAparencia('claro'), /value="claro" checked/);
  assert.match(fonte('painel.js'), /temaResolvido\(escolhido, escuroDoWindows\.matches\)/);
});

test('rodada 21 §2: os alvos do tour, dos mini-tours e das dicas existem no cabeçalho novo', () => {
  const alvos = [...PASSOS, ...PASSOS_ESSENCIAL, ...MINI_TOURS.painel]
    .map((p) => p.alvo)
    .concat(Object.values(DICAS).map((d) => d.alvo?.()))
    .filter((a) => typeof a === 'string' && a.startsWith('#bt-') || a === '#total-dia' || a === '#abas .abas');
  assert.ok(alvos.includes('#bt-novo') && alvos.includes('#bt-pausar-tudo') && alvos.includes('#bt-zerar-todos'));
  for (const a of alvos) {
    const id = a.split(' ')[0].slice(1);
    assert.match(INDEX, new RegExp(`id="${id}"`), a);
  }
  assert.ok(MINI_TOURS.painel.some((p) => p.alvo === '#bt-pausar-tudo'));
  assert.ok(MINI_TOURS.painel.some((p) => p.alvo === '#abas .abas'));
});

test('rodada 21 §3: a linha do tempo do Painel recolhe, lembra e desliga', () => {
  const s = snapDe();
  const aberta = secaoLinhaDoPainel(s.grupos, 0, AGORA, { mostrar: true, aberta: true });
  assert.match(aberta, /^<section class="grp lt-painel">\s*<div class="gh"><button class="gd" data-acao="dobrar" data-grupo="linha-do-tempo" aria-expanded="true"><span class="fd" aria-hidden="true">▾<\/span><b>Linha do tempo<\/b><\/button><\/div>\s*<div class="tl">/);
  // recolhida: só o cabeçalho, com a seta dos grupos
  const fechada = secaoLinhaDoPainel(s.grupos, 0, AGORA, { mostrar: true, aberta: false });
  assert.match(fechada, /class="grp lt-painel fechado"/);
  assert.match(fechada, /aria-expanded="false"><span class="fd" aria-hidden="true">▸<\/span>/);
  assert.doesNotMatch(fechada, /class="tl"/);
  // desligada em Ajustes → Painel: nada
  assert.equal(secaoLinhaDoPainel(s.grupos, 0, AGORA, { mostrar: false, aberta: true }), '');
  assert.match(htmlPainel({ painel_linha: true }), /<input type="checkbox" name="painel_linha" checked ?>/);
  assert.match(htmlPainel({}), /name="painel_linha" checked/, 'ligado por padrão');
  assert.doesNotMatch(htmlPainel({ painel_linha: false }), /checked/);
  // lembrar: o estado fica nos grupos fechados (gravados), pela chave da linha
  assert.equal(CHAVE_LINHA, 'linha-do-tempo');
  const p = fonte('painel.js');
  assert.match(p, /\{ mostrar: snap\.config\.painel_linha !== false, aberta: !fechados\.has\(CHAVE_LINHA\) \}/);
  assert.match(p, /case 'dobrar': \{[^}]*gravarLocal\(CHAVE_FECHADOS, \[\.\.\.fechados\]\);/);
  assert.match(fonte('ajustes.js'), /nome === 'painel_linha'/);
});

test('rodada 21 §4: o Foco mostra todas as atividades (Estudo e Reunião), com ▶/⏸ e o tempo', () => {
  const s = snapDe();
  const obra = s.grupos[1].timers[0];
  const h = visaoFoco(s, AGORA, new Set(), obra, s.grupos[1], 0);
  for (const nome of ['Trabalho', 'Reunião', 'Estudo']) assert.match(h, new RegExp(`<span class="nm">${nome}</span>`), nome);
  assert.match(h, /data-acao="play-atividade" data-timer="10" data-atividade="3"/);
  assert.match(h, /data-at="10:1">1:00:00</);
  // as ocultas valem no Foco também
  const oculto = { ...obra, atividades_ocultas: [3] };
  assert.doesNotMatch(visaoFoco(s, AGORA, new Set(), oculto, s.grupos[1], 0), /<span class="nm">Estudo<\/span>/);
});

// ---- correção da revisão da rodada 21 ----

// as opções marcadas do primeiro seletor do HTML (o campo Grupo traz o Espaço escondido depois)
const marcadas = (h) => [...h.slice(0, h.indexOf('</select>')).matchAll(/<option value="([^"]*)" selected>([^<]*)</g)].map((m) => [m[1], m[2]]);

test('revisão 21 I1: comOAtual marca o valor de agora; fora da lista, entra primeiro, marcado', () => {
  const ops = '<option value="1">A</option><option value="2" selected>B</option>';
  assert.equal(comOAtual(ops, 1, 'X'), '<option value="1" selected>A</option><option value="2">B</option>');
  assert.equal(comOAtual(ops, 9, 'Velho (arquivado)'), '<option value="9" selected>Velho (arquivado)</option><option value="1">A</option><option value="2">B</option>');
  assert.equal(comOAtual(ops, null, 'X'), ops, 'sem valor, como veio');
  assert.equal(comOAtual('<option value="sem:1">S</option>', 'sem:1', 'X'), '<option value="sem:1" selected>S</option>');
  assert.equal(comOAtual('<option value="1">A&amp;B</option>', 2, 'C<D'), '<option value="2" selected>C&lt;D</option><option value="1">A&amp;B</option>');
});

test('revisão 21 I1: o "Sem grupo" de um espaço arquivado vem marcado, com o nome do espaço', () => {
  const s = snapDe();
  s.espacos.push({ id: 3, nome: 'Antigo', cor: '#333333', ordem: 2, arquivado: true, real_antes_ms: 0 });
  const h = campoGrupo(s, null, 3);
  assert.deepEqual(marcadas(h), [['sem:3', 'Sem grupo · Antigo (arquivado)']]);
  assert.match(h, /<select name="grupo"><option value="sem:3" selected>/, 'primeiro, antes dos espaços ativos');
  assert.deepEqual(grupoDoForm('sem:3', '', ''), { id: null, espaco: 3 }, 'salvar sem mexer não muda o espaço');
  // com um espaço ativo só (o outro arquivado), também
  const so = umEspaco();
  so.espacos.push({ id: 3, nome: 'Antigo', cor: '#333333', ordem: 2, arquivado: true, real_antes_ms: 0 });
  assert.deepEqual(marcadas(campoGrupo(so, null, 3)), [['sem:3', 'Sem grupo · Antigo (arquivado)']]);
  assert.deepEqual(marcadas(campoGrupo(so, null, 1)), [['', 'Sem grupo']]);
  // o grupo arquivado e o de agora: um só marcado
  assert.deepEqual(marcadas(campoGrupo(s, 999, 1, 'Velho')), [['999', 'Velho (arquivado)']]);
  assert.deepEqual(marcadas(campoGrupo(s, 200, 2)), [['200', 'Casa']]);
  assert.deepEqual(marcadas(campoGrupo(s, null, 1)), [['sem:1', 'Sem grupo']]);
  // o campo Espaço, a atividade e o timer da pasta recente: o mesmo helper
  assert.deepEqual(marcadas(campoEspaco(s, 3)), [['3', 'Antigo (arquivado)']]);
  assert.deepEqual(marcadas(campoEspaco(s, 2)), [['2', 'Pessoal']]);
  assert.deepEqual(marcadas(opcoesDeAtividade(ATIVIDADES.slice(0, 2), 2)), [['2', 'Reunião']]);
  const timers = [{ id: 10, nome: 'Obra' }];
  assert.deepEqual(marcadas(linhaDoRecente({ nome: 'p', pasta: 'C:\p', usado_em: 0, timer_id: 10 }, 0, timers, 0)), [['10', 'Obra']]);
  assert.deepEqual(marcadas(linhaDoRecente({ nome: 'p', pasta: 'C:\p', usado_em: 0, timer_id: null }, 0, timers, 0)), [['', '—']]);
  assert.equal(marcadas(linhaDoRecente({ nome: 'p', pasta: 'C:\p', usado_em: 0, timer_id: 77 }, 0, timers, 0)).length, 1);
  // todo seletor do diálogo passa pelo helper (nenhum marca "selected" à mão)
  for (const f of ['editar.js', 'primeiro-uso.js']) assert.doesNotMatch(fonte(f), /\? ' selected' :/, f);
});

test('revisão 21 M1: a linha do tempo do Painel mostra o cabeçalho sempre; sem tempo, "Nada contado hoje"', () => {
  const s = snapDe();
  const vazio = s.grupos.map((g) => ({ ...g, timers: g.timers.map((t) => ({ ...t, trechos: [] })) }));
  const aberta = secaoLinhaDoPainel(vazio, 0, AGORA, { mostrar: true, aberta: true });
  assert.match(aberta, /<b>Linha do tempo<\/b>/);
  assert.match(aberta, /<p class="vazio">Nada contado hoje<\/p>/);
  const fechada = secaoLinhaDoPainel(vazio, 0, AGORA, { mostrar: true, aberta: false });
  assert.match(fechada, /<b>Linha do tempo<\/b>/);
  assert.doesNotMatch(fechada, /Nada contado/);
  assert.doesNotMatch(secaoLinhaDoPainel(s.grupos, 0, AGORA, { mostrar: true, aberta: true }), /Nada contado/);
});

test('revisão 21 M2: o Desfazer do mover que não age mostra a recusa na faixa', () => {
  const ed = fonte('editar.js');
  assert.match(ed, /await aoClicar\(\);[\s\S]*catch \(e\) \{[\s\S]*textContent = texto\(e\);/);
});

test('revisão 21 M3: compacto, o "N rodando" fica só o número (com a dica); o nome da atividade antes do "rodando"', () => {
  // (rodada 22: com o ponto verde antes do número)
  assert.match(INDEX, /<span class="rodando-n" id="rodando" hidden><i class="pt-on" aria-hidden="true"><\/i><b class="rn mono"><\/b><span class="rt"><\/span><\/span>/);
  const css = readFileSync(new URL('../css/app.css', import.meta.url), 'utf8');
  assert.match(css, /\.top\.compacta \.rodando-n \.rt \{ display: none; \}/);
  assert.doesNotMatch(css, /\.top\.compacta \.rodando-n \{ display: none/);
  const p = fonte('painel.js');
  assert.match(p, /trocarTexto\(n\.querySelector\('\.rn'\), String\(rodando\)\);/);
  assert.match(p, /trocarTitulo\(n, TEXTOS\.rodando\(rodando\)\);/);
  const flex = (sel) => css.match(new RegExp(`\\.lat \\.${sel} \\{ flex: 0 (\\d+) auto;`))?.[1];
  assert.ok(Number(flex('st')) > Number(flex('nm')), 'o "rodando" encolhe antes do nome');
  assert.match(css, /\.lat \.nm \{[^}]*min-width: 3\.2em;/);
});

test('revisão 21 M4: o Sistema resolvido no <head>, antes de pintar (sem piscar o escuro)', () => {
  for (const pagina of ['index.html', 'dock.html', 'aviso.html']) {
    const h = readFileSync(new URL(`../${pagina}`, import.meta.url), 'utf8');
    const cab = h.slice(0, h.indexOf('<link'));
    assert.match(cab, /worktimer\.tema-escolhido/, pagina);
    assert.match(cab, /matchMedia\('\(prefers-color-scheme: dark\)'\)/, pagina);
  }
  assert.match(fonte('painel.js'), /gravarLocal\(CHAVE_TEMA_ESCOLHIDO, escolhido\);/);
  assert.match(fonte('dock.js'), /snap\?\.config\.tema \?\? 'sistema'/);
});

test('revisão 21 M5: o "Mover para…" põe o foco na primeira opção habilitada', () => {
  // (rodada 22: a opção marcada, como a tela de agora no menu das telas, vem antes)
  assert.match(fonte('painel.js'), /const habilitadas = \[\.\.\.menu\.querySelectorAll\('button:not\(:disabled\)'\)\];[\s\S]*?\?\? habilitadas\[0\];\s*inicial\?\.focus\(\);/);
});

test('re-revisão 21 R1: o editar timer confere tudo (o ajuste e o lugar) antes de gravar qualquer coisa', () => {
  const ed = fonte('editar.js');
  const envio = ed.slice(ed.indexOf("await chamar('checar_editar_timer'"));
  const ordem = ["await chamar('checar_editar_timer'", 'await gravarAjuste(form, t.id)', 'await grupoDoTimer(form, snap, feito)', "await chamar('editar_timer'"].map((m) =>
    envio.indexOf(m),
  );
  assert.ok(ordem.every((x) => x >= 0), String(ordem));
  assert.deepEqual([...ordem].sort((a, b) => a - b), ordem, 'conferir vem antes de gravar');
  assert.match(envio, /ajuste,\s*grupo_novo_espaco: g\.novo \? espacoDoForm\(form\) : null,/);
  // sem trocar o lugar, o timer vai no lugar de agora (o Sem grupo do espaço arquivado vale)
  const s = snapDe();
  s.espacos[1].arquivado = true;
  assert.deepEqual(marcadas(campoGrupo(s, null, 2)), [['sem:2', 'Sem grupo · Pessoal (arquivado)']]);
  assert.deepEqual(grupoDoForm('sem:2', '', ''), { id: null, espaco: 2 });
});

test('re-revisão 21 R2: comOAtual não mexe no texto das opções ("Bugs selected for QA")', () => {
  const ops = '<option value="1">Bugs selected for QA</option><option value="2" selected>Outro selected</option>';
  assert.equal(
    comOAtual(ops, 1, 'X'),
    '<option value="1" selected>Bugs selected for QA</option><option value="2">Outro selected</option>',
  );
  assert.equal(
    comOAtual(ops, 9, 'Velho selected (arquivado)'),
    '<option value="9" selected>Velho selected (arquivado)</option><option value="1">Bugs selected for QA</option><option value="2">Outro selected</option>',
  );
  const s = snapDe();
  s.grupos[0] = { ...s.grupos[0], nome: 'Bugs selected for QA' };
  const h = campoGrupo(s, 200, 2);
  assert.match(h, /<option value="100">Bugs selected for QA<\/option>/);
  assert.deepEqual(marcadas(h), [['200', 'Casa']]);
});
