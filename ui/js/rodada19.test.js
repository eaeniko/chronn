// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 19: todas as atividades no painel do timer, as dicas que não cortam, o zerar por
// atividade e a saída da importação da versão anterior.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { linhasDeAtividade, quebrarDica, zeradoDaAtividade, contasAcumuladas, msDoTimer, msDeHoje, totalDeTimers, hojeDesdeZerar, itensDoMenuZerar, TRABALHO, REUNIAO } from './formato.js';
import { usarAtividades, linhasAtividades, andarAtividades, htmlDoMenuZerar, botaoZerar, controles, DICA_ZERAR, DICA_ZERAR_MENU } from './componentes.js';
import { textoDaFaixaZeradoAtividade, DESFAZER_ZERAR_MS } from './editar.js';
import { vigiarTitulos, trocarTitulo, LIMITE_DICA } from './titulos.js';

const fonte = (nome) => readFileSync(new URL(`./${nome}`, import.meta.url), 'utf8');

const M = 60_000;
const H = 60 * M;
const ESTUDO = 3;
const ATIVIDADES = [
  { id: 1, nome: 'Trabalho', cor: '#5b8def', ordem: 0, arquivada: false },
  { id: 2, nome: 'Reunião', cor: '#e8a23a', ordem: 1, arquivada: false },
  { id: 3, nome: 'Estudo', cor: '#3fb68b', ordem: 2, arquivada: false },
  { id: 4, nome: 'Curso antigo', cor: '#999999', ordem: 3, arquivada: true },
];
usarAtividades(ATIVIDADES);

/** Um timer com 2h de trabalho e 30min de reunião, os dois fechados (agora 11:00). */
const timer = (extra = {}) => ({
  id: 7,
  nome: 'Cliente A',
  cor: '#ff6b4a',
  rodando: false,
  atividade_padrao_id: TRABALHO,
  manual_atividade_id: null,
  trechos: [
    { inicio: 8 * H, fim: 10 * H, origem: 'manual', atividade_id: TRABALHO },
    { inicio: 10 * H, fim: 10 * H + 30 * M, origem: 'manual', atividade_id: REUNIAO },
  ],
  ajuste_ms: 0,
  ajustes: [],
  sessoes: [],
  zerado_em: 0,
  ...extra,
});
const AGORA = 11 * H;

test('rodada 19 §1: o Estudo aparece sem tempo; a arquivada sem tempo não aparece', () => {
  const v = linhasDeAtividade(timer(), ATIVIDADES, AGORA);
  assert.deepEqual(v.map((a) => [a.nome, a.ms]), [['Trabalho', 2 * H], ['Reunião', 30 * M], ['Estudo', 0]]);
  // a arquivada com tempo desde o zerar volta, sem botões
  const comCurso = timer({ trechos: [...timer().trechos, { inicio: 10 * H + 30 * M, fim: 10 * H + 40 * M, origem: 'manual', atividade_id: 4 }] });
  assert.deepEqual(linhasDeAtividade(comCurso, ATIVIDADES, AGORA).map((a) => [a.id, a.arquivada]), [[1, false], [2, false], [ESTUDO, false], [4, true]]);
  const h = linhasAtividades(comCurso, AGORA);
  assert.match(h, /<span class="nm">Estudo<\/span>/);
  assert.match(h, /data-acao="play-atividade" data-timer="7" data-atividade="3"/, 'o Estudo tem o ▶');
  assert.doesNotMatch(h, /data-atividade="4"/, 'a arquivada fica sem botões');
});

test('rodada 19 §2: quebrarDica quebra entre palavras, no limite, e mantém as quebras', () => {
  const linhas = (t, max) => quebrarDica(t, max).split('\n');
  // entre palavras, cada linha no máximo `max`
  assert.deepEqual(linhas('um dois tres quatro cinco seis', 10), ['um dois', 'tres', 'quatro', 'cinco seis']);
  for (const l of linhas(DICA_ZERAR, 60)) assert.ok(l.length <= 60, l);
  assert.equal(quebrarDica(DICA_ZERAR).replaceAll('\n', ' '), DICA_ZERAR, 'nada se perde, só os espaços viram quebras');
  // exatamente no limite: fica
  assert.equal(quebrarDica('abcde fghij', 11), 'abcde fghij');
  assert.equal(quebrarDica('abcde fghij', 10), 'abcde\nfghij');
  // as quebras que já existem continuam
  assert.equal(quebrarDica('primeira linha\num dois tres quatro', 10), 'primeira\nlinha\num dois\ntres\nquatro');
  // a palavra maior que o limite fica inteira, numa linha só
  assert.deepEqual(linhas('ver C:\\Users\\alguem\\projetos\\muito-longo agora', 12), ['ver', 'C:\\Users\\alguem\\projetos\\muito-longo', 'agora']);
  // curta: igual; já quebrada: igual (sem laço no observador)
  assert.equal(quebrarDica('curta'), 'curta');
  const q = quebrarDica(DICA_ZERAR);
  assert.equal(quebrarDica(q), q);
  assert.equal(LIMITE_DICA, 60);
});

/** Um DOM de mentira: elementos com `title` e um MutationObserver que entrega os registros na mão. */
function domDeMentira() {
  const fila = [];
  class El {
    constructor(title = null, filhos = []) {
      this.nodeType = 1;
      this.attrs = title === null ? {} : { title };
      this.filhos = filhos;
      this.escritas = 0;
    }
    getAttribute(n) { return this.attrs[n] ?? null; }
    setAttribute(n, v) {
      this.attrs[n] = String(v);
      this.escritas++;
      fila.push({ type: 'attributes', target: this, attributeName: n });
    }
    get title() { return this.attrs.title ?? ''; }
    set title(v) { this.setAttribute('title', v); }
    querySelectorAll() {
      const v = [];
      const andar = (e) => { for (const f of e.filhos) { if (f.attrs.title !== undefined) v.push(f); andar(f); } };
      andar(this);
      return v;
    }
    acrescentar(e) { this.filhos.push(e); fila.push({ type: 'childList', target: this, addedNodes: [e] }); }
  }
  class Observador {
    constructor(cb) { this.cb = cb; Observador.ultimo = this; }
    observe(raiz, op) { this.raiz = raiz; this.op = op; }
  }
  /** Entrega os registros até a fila esvaziar; devolve quantas voltas deu. */
  const entregar = () => {
    let voltas = 0;
    while (fila.length && voltas < 50) {
      Observador.ultimo.cb(fila.splice(0));
      voltas++;
    }
    return voltas;
  };
  return { El, Observador, entregar, fila };
}

test('rodada 19 §2: o observador quebra os title longos, os que entram depois, e não entra em laço', () => {
  const { El, Observador, entregar, fila } = domDeMentira();
  const longo = new El(DICA_ZERAR);
  const curto = new El('Editar');
  const raiz = new El(null, [longo, curto]);
  const obs = vigiarTitulos(raiz, Observador);
  assert.deepEqual(obs.op, { subtree: true, childList: true, attributes: true, attributeFilter: ['title'] });
  // os que já estão: o longo quebrado, o curto intocado
  assert.equal(longo.getAttribute('title'), quebrarDica(DICA_ZERAR));
  assert.equal(curto.escritas, 0);
  // a escrita do próprio observador volta como registro e não escreve de novo
  assert.ok(entregar() <= 1);
  assert.equal(longo.escritas, 1);
  assert.equal(fila.length, 0);
  // um title que muda depois
  curto.setAttribute('title', `Encerrar: ${DICA_ZERAR}`);
  const voltas = entregar();
  assert.ok(voltas <= 2, `voltas: ${voltas}`);
  assert.equal(curto.escritas, 2, 'a mudança e a quebra, uma vez');
  assert.ok(curto.getAttribute('title').split('\n').every((l) => l.length <= 60));
  // um elemento que entra depois, com um filho de dica longa
  const neto = new El(DICA_ZERAR);
  raiz.acrescentar(new El(null, [neto]));
  entregar();
  assert.equal(neto.escritas, 1);
  assert.equal(fila.length, 0, 'sem laço');
  // o laço de cada segundo põe a dica já quebrada, e não reescreve quando não mudou
  const total = new El('');
  trocarTitulo(total, DICA_ZERAR);
  trocarTitulo(total, DICA_ZERAR);
  assert.equal(total.escritas, 1);
  // sem MutationObserver (o Node), não faz nada
  assert.equal(vigiarTitulos(raiz, undefined), null);
});

test('rodada 19 §2: as três páginas ligam o observador das dicas', () => {
  for (const p of ['painel.js', 'dock.js', 'aviso.js']) assert.match(fonte(p), /^vigiarTitulos\(\);$/m, p);
  // os laços de cada segundo põem o title quebrado (não reescrevem a cada volta)
  for (const p of ['painel.js', 'dock.js']) assert.doesNotMatch(fonte(p), /\.title !== /, p);
});

test('rodada 19 §3: cada atividade conta desde o seu zerar; o timer é a soma', () => {
  // a reunião zerada às 10:15: o trabalho fica, a reunião conta 15 min
  const t = timer({ zerados_atividade: [{ atividade_id: REUNIAO, zerado_em: 10 * H + 15 * M }] });
  assert.equal(zeradoDaAtividade(t)(REUNIAO), 10 * H + 15 * M);
  assert.equal(zeradoDaAtividade(t)(TRABALHO), 0);
  assert.deepEqual([...contasAcumuladas(t, AGORA).porAtividade], [[TRABALHO, 2 * H], [REUNIAO, 15 * M]]);
  assert.equal(msDoTimer(t, AGORA), 2 * H + 15 * M);
  // o total do dia e a linha do tempo não mudam (a união de hoje)
  assert.equal(totalDeTimers([t], AGORA), 2 * H + 30 * M);
  assert.equal(msDeHoje(t, AGORA), 2 * H + 30 * M);
  // o ajuste da reunião feito antes do zerar dela sai do número; o do trabalho fica
  const comAjustes = timer({
    zerados_atividade: [{ atividade_id: REUNIAO, zerado_em: 10 * H + 15 * M }],
    ajustes: [
      { id: 1, delta_ms: 5 * M, atividade_id: REUNIAO, criado_em: 10 * H },
      { id: 2, delta_ms: 7 * M, atividade_id: TRABALHO, criado_em: 10 * H },
    ],
  });
  assert.equal(msDoTimer(comAjustes, AGORA), 2 * H + 7 * M + 15 * M);
  // o zerar do timer mais novo vence o da atividade
  assert.equal(msDoTimer({ ...t, zerado_em: 10 * H + 20 * M }, AGORA), 10 * M);
  // sem zerar nenhum: o timer como veio
  const semZerar = timer({ zerado_em: null });
  assert.equal(hojeDesdeZerar(semZerar), semZerar);
});

test('rodada 19 §3: o menu do ↺ com uma atividade (zera direto) e com várias', () => {
  // só trabalho com tempo: sem menu
  const so = timer({ trechos: [timer().trechos[0]] });
  assert.deepEqual(itensDoMenuZerar(so, ATIVIDADES, AGORA), []);
  // nenhuma com tempo: sem menu
  assert.deepEqual(itensDoMenuZerar(timer({ trechos: [] }), ATIVIDADES, AGORA), []);
  // trabalho e reunião: o timer todo e uma por atividade, na ordem da lista
  const itens = itensDoMenuZerar(timer(), ATIVIDADES, AGORA);
  assert.deepEqual(itens, [
    { zerar: 'timer', rotulo: 'O timer todo' },
    { zerar: 'atividade', id: TRABALHO, rotulo: 'Só Trabalho' },
    { zerar: 'atividade', id: REUNIAO, rotulo: 'Só Reunião' },
  ]);
  // a reunião zerada agora: sai do menu (sem tempo desde o zerar dela), e o ↺ volta a zerar direto
  assert.deepEqual(itensDoMenuZerar(timer({ zerados_atividade: [{ atividade_id: REUNIAO, zerado_em: AGORA }] }), ATIVIDADES, AGORA), []);
  const h = htmlDoMenuZerar(timer(), itens);
  assert.match(h, /^<div class="menu-ajuda menu-zerar" role="menu" aria-label="Zerar Cliente A">/);
  assert.match(h, /<button type="button" role="menuitem" data-zerar="timer" data-timer="7">O timer todo<\/button>/);
  assert.match(h, /<button type="button" role="menuitem" data-zerar="atividade" data-timer="7" data-atividade="2">Só Reunião<\/button>/);
  // o painel: menu com itens, zerar direto sem; os dois zeram pelo núcleo com a faixa
  const p = fonte('painel.js');
  assert.match(p, /if \(itens\.length\) alternarMenuZerar\(alvo, timer, itens\);\s*else await zerarTimer\(id\);/);
  assert.match(p, /agir\('zerar_atividade', \{ timer_id: id, atividade_id: atividade \}\)/);
  assert.match(p, /faixaZerado\(z, textoDaFaixaZeradoAtividade\(nome, nomeAt\)\)/);
  assert.equal(textoDaFaixaZeradoAtividade('Cliente A', 'Reunião'), 'Cliente A: só Reunião voltou a 0:00.');
  assert.equal(DESFAZER_ZERAR_MS, 10_000, 'o Desfazer por 10 s, como o do ↺');
});

test('rodada 19 §3: o ↺ pequeno de cada linha de atividade, só com tempo', () => {
  const h = linhasAtividades(timer(), AGORA);
  assert.match(h, /<button class="lz" data-acao="zerar-atividade" data-timer="7" data-atividade="2" title="Zerar só Reunião" aria-label="Zerar só Reunião — Cliente A">↺<\/button><button class="le" data-acao="ajustar-atividade" data-timer="7" data-atividade="2"/);
  // o Estudo, sem tempo: o ↺ está lá, escondido (guarda o lugar)
  assert.match(h, /<button class="lz" data-acao="zerar-atividade" data-timer="7" data-atividade="3" title="Zerar só Estudo" aria-label="Zerar só Estudo — Cliente A" hidden>↺<\/button>/);
  assert.doesNotMatch(h, /data-atividade="1"[^>]*title="Zerar só Trabalho"[^>]* hidden/);
  // o laço de cada segundo mostra o ↺ quando a atividade passa a ter tempo
  const lz = { hidden: true };
  const el = { dataset: { at: '7:3' }, textContent: '0:00:00', parentElement: { querySelector: (s) => (s === '.lz' ? lz : null) } };
  const raiz = { querySelectorAll: () => [el] };
  const comEstudo = timer({ trechos: [...timer().trechos, { inicio: 10 * H + 30 * M, fim: null, origem: 'manual', atividade_id: ESTUDO }] });
  andarAtividades(raiz, [comEstudo], AGORA);
  assert.equal(el.textContent, '0:30:00');
  assert.equal(lz.hidden, false);
  // o CSS guarda o lugar do ↺ escondido
  const css = readFileSync(new URL('../css/app.css', import.meta.url), 'utf8');
  assert.match(css, /\.lat \.lz\[hidden\] \{ display: grid; visibility: hidden; \}/);
});

test('revisão da rodada 19 (M3): com o menu, a dica do ↺ diz que abre a escolha, com aria-haspopup', () => {
  // duas atividades com tempo: abre o menu
  const menu = botaoZerar(timer(), '', AGORA);
  assert.equal(DICA_ZERAR_MENU, 'Zerar: escolher o timer todo ou uma atividade. Nada é apagado');
  assert.match(menu, /^<button data-acao="zerar" data-timer="7" title="Zerar: escolher o timer todo ou uma atividade\. Nada é apagado" aria-label="Zerar Cliente A" aria-haspopup="menu" aria-expanded="false">↺<\/button>$/);
  // uma só: zera direto, com a dica de antes e sem o popup
  const direto = botaoZerar(timer({ trechos: [timer().trechos[0]] }), 'mi', AGORA);
  assert.match(direto, /^<button class="mi" data-acao="zerar" data-timer="7" title="Zerar: o número volta a 0:00/);
  assert.doesNotMatch(direto, /aria-haspopup|aria-expanded/);
  assert.ok(direto.includes(DICA_ZERAR));
  // o painel do timer, o Foco e os cartões usam o mesmo botão
  assert.ok(controles(timer(), AGORA).includes(menu));
  assert.match(fonte('cartoes.js'), /\$\{botaoZerar\(timer, 'mi', agora\)\}/);
  assert.match(fonte('foco.js'), /\$\{controles\(timer, agora\)\}/);
  assert.match(readFileSync(new URL('../index.html', import.meta.url), 'utf8'), /id="bt-tour"[^>]*aria-haspopup="menu" aria-expanded="false"/);
});

test('revisão da rodada 19 (M4): o menu fecha por qualquer caminho tirando os ouvintes, e o 2º clique só fecha', () => {
  const p = fonte('painel.js');
  const abrir = p.slice(p.indexOf('function abrirMenu('), p.indexOf('/**', p.indexOf('function abrirMenu(')));
  // um menu aberto é fechado pelo fechar dele (os ouvintes saem); o mesmo dono não reabre
  assert.match(abrir, /if \(menuAberto\) \{\s*const era = menuAberto\.dono;\s*menuAberto\.fechar\(\);\s*if \(era === dono\) return;/);
  assert.doesNotMatch(abrir, /aberto\.remove\(\)/, 'nada de tirar o menu sem tirar os ouvintes');
  // o fechar tira os dois ouvintes, esquece o menu e desmarca o aria-expanded
  assert.match(abrir, /const fechar = \(\) => \{\s*menu\.remove\(\);\s*document\.removeEventListener\('mousedown', fora, true\);\s*document\.removeEventListener\('keydown', tecla, true\);\s*if \(menuAberto\?\.fechar === fechar\) menuAberto = null;\s*marcarExpandido\(dono, false\);/);
  // o clique fora ignora o botão do dono pelo dono (ação e timer), não pelo elemento: vale depois
  // de o painel ser redesenhado
  assert.match(abrir, /const b = ev\.target\.closest\?\.\('\[data-acao\]'\);\s*if \(b && donoDoMenu\(b\) === dono\) return;/);
  assert.match(p, /const donoDoMenu = \(b\) => `\$\{b\.dataset\.acao\}:\$\{b\.dataset\.timer \?\? ''\}:\$\{b\.dataset\.grupo \?\? ''\}`;/);
  // o redesenho com o menu aberto mantém o aria-expanded
  assert.match(p, /if \(menuAberto\) marcarExpandido\(menuAberto\.dono, true\);/);
  // o menu do ? usa o mesmo caminho
  assert.match(p, /function alternarMenuAjuda\(botao\) \{\s*abrirMenu\(botao, htmlDoMenuAjuda\(visao\), 'esquerda'/);
});
