// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Dock flutuante em duas etapas. Recolhido: os timers rodando (e os parados com uma sessão
// esperando você), o total do dia e ⤢ (a janela acompanha a largura do conteúdo). Expandido: o
// total, os timers por grupo, as perguntas, a sessão que você está lendo (ou escrevendo) e "Abrir o app"; recolhe
// pelo traço do topo ou ao clicar fora do dock. Com a opção "Mostrar o que cada timer está
// fazendo", cada timer leva a etiqueta da situação: curta no recolhido (largura fixa, reticências),
// por extenso no expandido.
// O ■ (encerrar) fica em cada linha do expandido e, no recolhido, só no lembrete "esperando você",
// no lugar que já era reservado ao botão (nos rodando ele alargaria o dock). O 👥 (reunião manual)
// fica só no expandido, ao lado do play; no recolhido, a etiqueta diz "reunião" (ou "reunião +
// Claude", com o Claude trabalhando junto), na cor da atividade. O tempo é sempre o número do
// timer: o total da tarefa (a soma das atividades) desde o último zerar, com "desde …" na dica; no
// expandido, com mais de uma atividade, a linha do timer ganha uma sublinha com o tempo de cada uma.
// O total do dia ("hoje") é o de hoje.
// Duplo clique em qualquer etapa abre o app.
// Posição, tamanho e etapa ficam no núcleo (dock_guardar / dock_etapa).
import { chamar, aoMudar } from './api.js';
import { curto, horaMin, relogio, totalDeTimers, timersParaDock, textoDosDeFora, dicaDoLembrete, recolherAoPerderFoco, trocarTexto, etiquetaComposta, dicaDoTotalDoDia, REUNIAO } from './formato.js';
import {
  esc,
  cor,
  botaoPlay,
  botaoEncerrar,
  botaoReuniao,
  msDoTimer,
  podeEncerrar,
  usarAtividades,
  usarCorte,
  usarEsperaApp,
  esperaAppAtual,
  htmlEtiqueta,
  atividadePorId,
  linhaDivisao,
  andarAtividades,
  dicaDoTempo,
  tituloDoTempo,
  htmlDaPergunta,
  mudarEscolha,
  esquecerEscolhas,
  sempreDoBotao,
  pedidoDeLigar,
} from './componentes.js';
import { faixa, faixaEncerrado, usarFaixa } from './editar.js';
import { DICAS, gatilhos, criarFila } from './dicas.js';
import { nomearPagina, marcaDeTeste } from './nome.js';
import { vigiarTitulos, trocarTitulo } from './titulos.js';
import { pontoDoEspaco, nomeDoGrupo, espacoNoRecolhido, pontoRecolhido, espacoEmTexto } from './espacos.js';
import { TEXTOS, definirIdioma, idiomaDoEstado, idiomaInicial, aplicarTextos, CHAVE_IDIOMA } from './textos.js';

// as dicas longas em linhas curtas (rodada 19)
vigiarTitulos();

const raiz = document.getElementById('dock');
const janela = () => window.__TAURI__.window.getCurrentWindow();
const escuroDoWindows = window.matchMedia('(prefers-color-scheme: dark)');

let snap = null;
let expandido = null;
let lado = { direita: true, embaixo: false };
/** A alça do canto foi apertada e o redimensionamento ainda não acabou. */
let redimensionando = false;
/** Logo depois de um ■, os cliques esperam: a linha some e o 2º clique de um duplo clique cairia noutro botão. */
let travaAte = 0;
const TRAVA_MS = 400;

const todosTimers = () => (snap ? snap.grupos.flatMap((g) => g.timers) : []);
const lerSnap = () => snap;
const totalDoDia = (agora) => curto(totalDeTimers(todosTimers(), agora));
/** "tempo contado 1min · ajustes +30min (Cliente C)": de onde vem o total do dia (rodada 12). */
const dicaDoTotal = (agora) => esc(dicaDoTotalDoDia(todosTimers(), agora));
const comSituacao = () => snap?.config.dock_situacao !== false;

async function agir(nome, args) {
  try {
    return await chamar(nome, args);
  } catch (e) {
    console.error(`worktimer: ${nome} falhou`, e);
    // o aviso fica fora do dock (rodada 15): aparece também no recolhido
    faixa(typeof e === 'string' ? e : TEXTOS.naoDeuCerto);
    return null;
  }
}

// ---- tema e lado da tela ----

function aplicarTema() {
  const t = snap?.config.tema ?? 'sistema';
  const tema = t === 'claro' || t === 'escuro' ? t : escuroDoWindows.matches ? 'escuro' : 'claro';
  document.documentElement.dataset.tema = tema;
}
escuroDoWindows.addEventListener('change', () => snap && aplicarTema());

function aplicarLado(l) {
  if (!l) return;
  lado = l;
  document.documentElement.dataset.ladoX = l.direita ? 'dir' : 'esq';
  document.documentElement.dataset.ladoY = l.embaixo ? 'baixo' : 'cima';
}
aplicarLado(lado);

// ---- desenho ----

const ICONE_EXPANDIR = '⤢';
// um traço, como o "minimizar" das janelas: o ⤡ era quase igual ao ⤢ e ninguém achava
const ICONE_RECOLHER =
  '<svg viewBox="0 0 12 12" width="12" height="12" aria-hidden="true"><path d="M2.5 6h7" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>';

/**
 * Nome do timer no recolhido. Com a situação ligada, a etiqueta fica em cima do nome, numa coluna
 * de largura fixa (o dock não muda de largura quando a etiqueta troca) e sempre com a linha dela,
 * mesmo vazia (a altura não pula quando uma etiqueta aparece). No modo "Espaço em texto" (rodada
 * 24), a linha começa pelo nome do espaço ("Pessoal · escrevendo"; sem situação, só "Pessoal"), e
 * as reticências cortam a linha inteira.
 */
function nomeRecolhido(t, sit, esp = { modo: 'timer', espaco: null }) {
  const nome = `<span class="nm" title="${esc(t.nome)}">${esc(t.nome)}</span>`;
  const espaco = espacoEmTexto(esp);
  if (!sit && !espaco) return nome;
  // a coluna fixa de 104 px: com mais de 16 letras, a segunda parte vira um ponto (rodada 14)
  const e = sit ? t.etiqueta : null;
  if (espaco) {
    const dica = esc(e ? `${esp.espaco.nome} · ${e.texto}` : esp.espaco.nome);
    const resto = e ? `<span class="mais"> · </span>${htmlEtiqueta(e, 16)}` : '';
    return `<span class="tx"><small class="sit${e ? ` ${e.p1.k}` : ''}" title="${dica}">${espaco}${resto}</small>${nome}</span>`;
  }
  const etiqueta = e ? `<small class="sit ${e.p1.k}" title="${esc(e.texto)}">${htmlEtiqueta(e, 16)}</small>` : '<small class="sit"></small>';
  return `<span class="tx">${etiqueta}${nome}</span>`;
}

/** A cor da Reunião (`--a`), para a etiqueta "reunião" na cor da atividade. */
function corDaReuniao() {
  const r = atividadePorId(REUNIAO);
  return r ? `;--a:${cor(r.cor)}` : '';
}

/**
 * Um timer no recolhido. O lembrete "esperando você" (timer parado) não tem play: um clique ali
 * ligaria um play manual por engano. No lugar dele fica o ■, que tira o lembrete do dock (sem
 * mudar a largura: o espaço já era do botão). A dica diz onde responder.
 */
function timerRecolhido(t, sit) {
  // o pausado (rodada 14) tem o ▶ para retomar
  const fim = t.rodando || t.pausado
    ? botaoPlay(t, 'p')
    : podeEncerrar(t)
      ? botaoEncerrar(t, 'p')
      : '<span class="p lembrete" aria-hidden="true"></span>';
  const dica = t.rodando ? '' : ` title="${esc(dicaDoLembrete(t))}"`;
  // com 2 ou mais espaços, o espaço do timer como os Ajustes pedem (rodada 24): um ponto só
  const esp = espacoNoRecolhido(snap, snap.grupos.flatMap((g) => g.timers).find((x) => x.id === t.id) ?? t);
  return `<div class="dk${t.rodando ? '' : ' parado'}" style="--c:${cor(t.cor)}${corDaReuniao()}"${dica}>${pontoRecolhido(t, esp)}${nomeRecolhido(t, sit, esp)}<span class="t mono" data-hm="${t.id}"${t.desde ? ` title="${esc(t.desde)}"` : ''}>${horaMin(t.ms)}</span>${fim}</div>`;
}

function recolhido(agora) {
  const sit = comSituacao();
  const fora = textoDosDeFora(snap, sit, agora);
  const timers =
    timersParaDock(snap, agora, sit)
      .map((t) => timerRecolhido(t, sit))
      .join('') +
    (fora ? `<span class="dmais" title="${esc(fora.dica)}">${esc(fora.texto)}</span>` : '');
  const dois = snap.config.dock_botoes === 'dois'
    ? `<button class="dbt" data-acao="abrir" title="${TEXTOS.abrirOApp}" aria-label="${TEXTOS.abrirOApp}">⧉</button>`
    : '';
  const p = snap.perguntas[0];
  // a pergunta de sessão nova tem o cartão mais largo (rodada 27, opção A: 400 px)
  const ask = p ? `<div class="ask${p.tipo === 'sessao_nova' ? ' sn' : ''}">${htmlDaPergunta(p)}</div>` : '';
  const total = `<div class="dtot" title="${dicaDoTotal(agora)}">${TEXTOS.hojeMin} <b class="mono" data-total-dia>${totalDoDia(agora)}</b></div>`;
  return `<div class="pilula">${marcaDeTeste()}${timers}${total}<button class="dbt" data-acao="expandir" title="${TEXTOS.expandir}" aria-label="${TEXTOS.expandirODock}">${ICONE_EXPANDIR}</button>${dois}</div>${ask}${avisoDeLigacao('dav')}`;
}

/** Os avisos de ligada sozinha que ainda valem (o núcleo diz até quando). */
const avisosAgora = () => (snap?.avisos ?? []).filter((a) => Date.now() < a.ate);

/**
 * "Ligada a Flow pela pasta · Trocar": uma regra de pasta (ou a conversa anterior) acabou de ligar
 * uma sessão sozinha. Uma linha discreta por ~15 s; "Trocar" abre o "Ligar a um timer" dela.
 */
function avisoDeLigacao(classe) {
  // o mais recente, se houver mais de um
  const a = avisosAgora().reduce((x, y) => (x && x.ate >= y.ate ? x : y), null);
  if (!a) return '';
  const dica = `${a.sessao_nome}${a.cwd ? ` — ${a.cwd}` : ''}`;
  return `<div class="${classe}" role="status" title="${esc(dica)}"><span>${esc(a.texto)}</span> · <button data-acao="trocar" data-sessao="${esc(a.sessao_id)}">${TEXTOS.trocar}</button></div>`;
}

/** Redesenha quando o aviso de ligada sozinha vence. */
let avisoVenceId = null;
function vencerAvisos() {
  clearTimeout(avisoVenceId);
  const ates = avisosAgora().map((a) => a.ate);
  if (ates.length) avisoVenceId = setTimeout(desenhar, Math.min(...ates) - Date.now() + 50);
}

/** Sessões ligadas a timers, sem repetir (o snapshot as traz dentro de cada timer). */
function sessoesLigadas() {
  const m = new Map();
  for (const t of todosTimers()) for (const s of t.sessoes) m.set(s.id, s);
  return m;
}

/**
 * A conversa aberta no Claude que está contando leitura (o núcleo reconhece pelo título):
 * "lendo", ou "escrevendo" com o foco na caixa de mensagem dela.
 */
function lendoAgora() {
  if (!snap.lendo) return '';
  const nome = sessoesLigadas().get(snap.lendo)?.nome ?? TEXTOS.umaSessaoDoClaude;
  return `<div class="dx-lendo" title="${esc(nome)}">${snap.escrevendo ? TEXTOS.escrevendoEm : TEXTOS.lendoEm}: <b>${esc(nome)}</b></div>`;
}

/**
 * Situação por extenso numa linha do expandido ("Claude trabalhando", "você lendo", "manual +
 * esperando você", "segundo plano"...). O lembrete fica pelo tempo do app (Ajustes → "Lembretes
 * no dock"; até a virada do dia por padrão). Com mais de 20 letras, a segunda parte vira um ponto.
 */
function situacaoExpandido(t) {
  const e = comSituacao() ? etiquetaComposta(t, Date.now(), esperaAppAtual(), 'longa') : null;
  return e ? `<span class="sit ${e.p1.k}" title="${esc(e.texto)}">${htmlEtiqueta(e, 20)}</span>` : '';
}

function expandidoHtml(agora) {
  const n = snap.rodando;
  const grupos = snap.grupos
    .filter((g) => g.timers.length)
    .map(
      (g) =>
        `<div class="dx-g">${esc(nomeDoGrupo(snap, g))}</div>` +
        g.timers
          .map(
            (t) =>
              `<div class="dx-r${t.rodando ? ' on' : ''}" style="--c:${cor(t.cor)}${corDaReuniao()}"><i></i>${pontoDoEspaco(snap, t)}<span class="nm" title="${esc(t.nome)}">${esc(t.nome)}</span>${situacaoExpandido(t)}<span class="t mono" data-tempo="${t.id}"${tituloDoTempo(t, agora)}>${relogio(msDoTimer(t, agora))}</span>${botaoEncerrar(t, 'p', true)}${botaoReuniao(t, 'p')}${botaoPlay(t, 'p')}</div>` +
              // com mais de uma atividade no dia: o tempo de cada uma, numa sublinha pequena
              linhaDivisao(t, agora, 'dx-at'),
          )
          .join(''),
    )
    .join('');
  // uma pergunta por vez (como no recolhido): a lista de timers fica sempre à vista
  const [p, ...resto] = snap.perguntas;
  const mais = resto.length
    ? `<small class="dx-mais">${TEXTOS.perguntasDepois(resto.length)}</small>`
    : '';
  const pergunta = p ? `<div class="dx-ask${p.tipo === 'sessao_nova' ? ' sn' : ''}">${htmlDaPergunta(p)}${mais}</div>` : '';
  // o aviso de ligada sozinha vem antes da pergunta: dura só ~15 s e não pode ficar rolado para fora
  const asks = avisoDeLigacao('dx-aviso') + pergunta + lendoAgora();
  // com algo rodando, o "Pausar tudo" toma o lugar da contagem (as linhas acesas já mostram quais)
  const pausarTudo = n ? `<button class="dpa" data-acao="pausar-tudo" title="${esc(TEXTOS.rodando(n))}">${TEXTOS.pausarTudo}</button>` : '';
  return `<div class="dx-top">${marcaDeTeste()}<b class="mono" data-total-dia title="${dicaDoTotal(agora)}">${totalDoDia(agora)}</b><small>${TEXTOS.hojeMin}</small><span class="sp"></span>
  ${pausarTudo}
  <button class="dbt" data-acao="recolher" title="${TEXTOS.recolher}" aria-label="${TEXTOS.recolherODock}">${ICONE_RECOLHER}</button>
  <button class="dbt" data-acao="ajustes" title="${TEXTOS.ajustesDica}" aria-label="${TEXTOS.ajustesDica}">⚙&#xFE0E;</button></div>
${asks ? `<div class="dx-asks" data-tauri-drag-region="false">${asks}</div>` : ''}
<div class="dx-lista" data-tauri-drag-region="false">${grupos || `<p class="dx-vazio">${TEXTOS.nenhumTimerAinda}</p>`}</div>
<button class="dx-open" data-acao="abrir">${TEXTOS.abrirOApp}</button>
<button class="grip" data-acao="redimensionar" title="${TEXTOS.redimensionar}" aria-label="${TEXTOS.redimensionarODock}" tabindex="-1"></button>`;
}

function desenhar() {
  if (!snap || expandido === null) return;
  const agora = Date.now();
  const lista = raiz.querySelector('.dx-lista');
  const rolagem = lista ? lista.scrollTop : 0;
  raiz.className = expandido ? 'exp' : 'rec';
  raiz.innerHTML = expandido ? expandidoHtml(agora) : recolhido(agora);
  const nova = raiz.querySelector('.dx-lista');
  if (nova) nova.scrollTop = rolagem;
  vencerAvisos();
}

// ---- avisos fora do dock (rodada 15) ----
// As faixas do dock (o "Flow encerrado. Desfazer", um erro) aparecem numa janela pequena só
// delas, logo abaixo do dock (ou acima), sem pegar o foco; com o dock escondido, na janela
// principal. O "Desfazer" continua sendo do dock: o clique volta para cá (`aviso-clicado`).
// Um aviso de cada vez: o novo toma o lugar.
let avisoAtual = null;

// não chamar o parâmetro de `aoClicar`: esse nome é o tratador dos cliques do dock.
// `extra`: o segundo botão e o que ele faz (`botao2`, `acao2`) e `aoSair(motivo)`, que roda uma
// vez quando o aviso sai: "venceu" (o prazo), "clique" ou "interrompido" (outro aviso no lugar).
function avisoForaDoDock(msg, botao = null, acao = null, ms = 4000, extra = {}) {
  if (avisoAtual) sairDoAviso(avisoAtual, 'interrompido');
  const id = Date.now() * 1000 + Math.floor(Math.random() * 1000);
  const vence = setTimeout(() => fecharAviso(id), ms);
  avisoAtual = { id, acao, vence, acao2: extra.acao2 ?? null, aoSair: extra.aoSair ?? null };
  const tema = document.documentElement.dataset.tema;
  const aviso = { id, texto: msg, botao, botao2: extra.botao2 ?? null, ms, tema, lang: document.documentElement.lang };
  chamar('aviso_mostrar', { aviso }).catch((e) => console.error('worktimer: aviso_mostrar', e));
  return null;
}

function sairDoAviso(a, motivo) {
  clearTimeout(a.vence);
  const f = a.aoSair;
  a.aoSair = null;
  f?.(motivo);
}

function fecharAviso(id, motivo = 'venceu') {
  if (avisoAtual?.id === id) {
    sairDoAviso(avisoAtual, motivo);
    avisoAtual = null;
  }
  chamar('aviso_fechar', { id }).catch(() => {});
}

window.__TAURI__.event.listen('aviso-clicado', async (ev) => {
  const a = avisoAtual;
  const { id, qual } = typeof ev.payload === 'object' && ev.payload ? ev.payload : { id: ev.payload, qual: 1 };
  const acao = qual === 2 ? a?.acao2 : a?.acao;
  if (!a || a.id !== id || !acao) return;
  clearTimeout(a.vence);
  try {
    await acao();
    fecharAviso(a.id, 'clique');
  } catch (e) {
    console.error('worktimer: aviso', e);
    avisoForaDoDock(typeof e === 'string' ? e : TEXTOS.naoDeuCerto);
  }
});
usarFaixa(avisoForaDoDock);

// ---- dicas (rodada 16) ----
// O que acontece no dock (com ele à vista) mostra a dica na janela de aviso, por 15 s ou até o
// clique. A janela principal à vista pede antes (a do dock espera 1,5 s). O núcleo dá a vez, uma
// dica por vez no app inteiro, e segura as dicas durante o tour ou um diálogo na principal.
const DICA_NO_DOCK_MS = 15_000;
const ESPERA_DA_PRINCIPAL_MS = 1_500;

/** A dica à vista na janela de aviso: `{ id, aviso }` (para o núcleo fechar no meio). */
let dicaAtual = null;

const dicaNoAviso = (id, texto) =>
  new Promise((ok) => {
    let saiu = false;
    const sair = (r) => {
      if (!saiu) {
        saiu = true;
        if (dicaAtual?.id === id) dicaAtual = null;
        ok(r);
      }
    };
    avisoForaDoDock(texto, TEXTOS.entendi, async () => sair('entendi'), DICA_NO_DOCK_MS, {
      botao2: TEXTOS.naoMostrarDicas,
      acao2: async () => {
        await chamar('dicas_ligar', { ligadas: false });
        sair('desligar');
      },
      // o prazo conta como vista; outro aviso no lugar, não (volta depois)
      aoSair: (motivo) => sair(motivo === 'venceu' ? 'venceu' : motivo === 'clique' ? 'entendi' : 'interrompida'),
    });
    dicaAtual = { id, aviso: avisoAtual?.id };
  });

const dicas = criarFila({
  ligadas: () => snap?.config?.dicas_ligadas !== false,
  vistas: () => snap?.config?.dicas_vistas ?? [],
  // o dock à vista e sem outro aviso na janela de aviso
  podeMostrar: () => !document.hidden && !avisoAtual,
  reivindicar: (id) => chamar('dica_reivindicar', { id }).catch(() => 'esperar'),
  mostrar: (id) => dicaNoAviso(id, DICAS[id].texto(snap)),
  concluir: (id, vista) => chamar('dica_saiu', { id, vista }).catch((e) => console.error('worktimer: dica_saiu', e)),
});
// o núcleo fechou a dica à vista (a janela principal ficou ocupada: o tour, um diálogo)
window.__TAURI__.event.listen('dica-fechar', ({ payload }) => {
  if (dicaAtual && dicaAtual.id === payload) fecharAviso(dicaAtual.aviso, 'interrompido');
});
// a página do dock (re)começou: a dica que ela mostrava antes de recarregar não prende a vez
chamar('dicas_pagina_comecou').catch(() => {});
/** As dicas a oferecer daqui a pouco (a janela principal à vista pede antes). */
const agendadas = new Set();
/**
 * As dicas do estado de agora. A cada estado: a vista e a que já está na fila não entram de novo,
 * e "Mostrar as dicas de novo" vale na hora (revisão).
 */
function verDicas(s) {
  for (const id of gatilhos(s)) {
    if (agendadas.has(id)) continue;
    agendadas.add(id);
    setTimeout(() => {
      agendadas.delete(id);
      dicas.oferecer(id);
    }, ESPERA_DA_PRINCIPAL_MS);
  }
}
setInterval(() => dicas.tentar(), 2_000);

// o passo do dock no tour da janela principal (rodada 16): uma borda suave pisca, sem foco
window.__TAURI__.event.listen('piscar', (ev) => document.body.classList.toggle('piscando', Boolean(ev.payload)));

/** A cada segundo: só os textos dos números, e só os que mudaram (nada de refazer o HTML). */
function andar() {
  if (!snap) return;
  const agora = Date.now();
  const total = totalDoDia(agora);
  const dica = dicaDoTotalDoDia(todosTimers(), agora);
  for (const el of raiz.querySelectorAll('[data-total-dia]')) {
    trocarTexto(el, total);
    // no recolhido, a dica fica no "hoje 31min" inteiro
    const alvo = el.closest('.dtot') ?? el;
    trocarTitulo(alvo, dica);
  }
  for (const t of todosTimers()) {
    // o número do timer (desde o zerar), com "desde …" na dica
    const ms = msDoTimer(t, agora);
    const dica = dicaDoTempo(t, agora);
    for (const el of raiz.querySelectorAll(`[data-tempo="${t.id}"], [data-hm="${t.id}"]`)) {
      trocarTexto(el, el.dataset.hm !== undefined ? horaMin(ms) : relogio(ms));
      if (dica) trocarTitulo(el, dica);
    }
  }
  // as sublinhas por atividade do expandido (só quando o texto muda)
  andarAtividades(raiz, todosTimers(), agora);
}

// ---- tamanho da janela ----

// O recolhido mede o conteúdo e pede a largura/altura certas (o núcleo segura a borda do lado
// em que o dock está). No expandido o tamanho é o do usuário.
let medidaEnviada = '';
let medirId = null;
function medir() {
  clearTimeout(medirId);
  medirId = setTimeout(async () => {
    if (expandido !== false) return;
    const r = raiz.getBoundingClientRect();
    const larg = Math.ceil(r.width);
    const alt = Math.ceil(r.height);
    const chave = `${larg}x${alt}`;
    if (!larg || !alt || chave === medidaEnviada) return;
    medidaEnviada = chave;
    aplicarLado(await agir('dock_ajustar', { larg, alt }));
  }, 60);
}
new ResizeObserver(medir).observe(raiz);

let guardarId = null;
function guardar() {
  clearTimeout(guardarId);
  guardarId = setTimeout(async () => {
    try {
      // meio segundo sem mexer: o redimensionamento (se havia) acabou
      redimensionando = false;
      const w = janela();
      const [p, s] = await Promise.all([w.outerPosition(), w.outerSize()]);
      aplicarLado(await chamar('dock_guardar', { x: p.x, y: p.y, larg: s.width, alt: s.height }));
    } catch (e) {
      console.error('worktimer: dock_guardar falhou', e);
    }
  }, 500);
}

async function mudarEtapa(exp) {
  if (exp === expandido) return;
  expandido = exp;
  medidaEnviada = '';
  desenhar();
  aplicarLado(await agir('dock_etapa', { expandido: exp }));
  if (!exp) medir();
  // a primeira vez que o dock expande: arrastar, recolher, esconder (rodada 16)
  else dicas.oferecer('dock_expande');
}

// ---- cliques ----

/**
 * "+ Novo timer…" (da pergunta ou do "Ligar a um timer"): o diálogo abre na janela principal. `d`:
 * o `dataset` do botão, com o "Sempre que for na pasta" da pergunta de sessão nova (rodada 27).
 */
const novoTimerDaSessao = (sessaoId, d = {}) => agir('pedir_novo_timer', { sessao_id: sessaoId, ...sempreDoBotao(d) });

/** "Ligar a um timer" da sessão; no recolhido, o dock expande enquanto o diálogo está aberto. */
/**
 * O "Ligar a um timer" (o "Escolher timer…", o "Outro timer…", o "Trocar") abre na janela
 * principal, que vem para a frente; o dock não abre diálogos (rodada 16).
 */
const escolherTimer = (pedido) => agir('pedir_ligar_sessao', { pedido });

async function aoClicar(ev) {
  if (ev.target.closest('.dlg-fundo, .faixa')) return;
  const alvo = ev.target.closest('[data-acao]');
  if (!alvo || !snap || Date.now() < travaAte) return;
  const d = alvo.dataset;
  switch (d.acao) {
    case 'play':
      await agir('play', { timer_id: Number(d.timer) });
      break;
    case 'pausar':
      await agir('pausar', { timer_id: Number(d.timer) });
      break;
    case 'reuniao':
      await agir('play_atividade', { timer_id: Number(d.timer), atividade_id: REUNIAO });
      break;
    case 'parar-reuniao':
      await agir('parar_atividade', { timer_id: Number(d.timer), atividade_id: REUNIAO });
      break;
    case 'encerrar': {
      const id = Number(d.timer);
      const nome = todosTimers().find((t) => t.id === id)?.nome;
      travaAte = Date.now() + TRAVA_MS;
      if (!(await agir('encerrar', { timer_id: id }))) break;
      // recolhido ou expandido: o "Desfazer" fica no aviso fora do dock (rodada 15)
      faixaEncerrado(id, nome);
      break;
    }
    case 'pausar-tudo':
      await agir('pausar_tudo');
      break;
    case 'expandir':
      await mudarEtapa(true);
      break;
    case 'recolher':
      await mudarEtapa(false);
      break;
    case 'abrir':
      await agir('abrir_app');
      break;
    case 'ajustes': // os ajustes moram na janela principal
      await agir('abrir_ajustes');
      break;
    case 'responder':
      await agir('responder', { pergunta_id: Number(d.pergunta), valor: d.valor });
      break;
    case 'novo-timer':
      // a janela principal abre com o "Novo timer" já preenchido; a pergunta fica até salvar
      await novoTimerDaSessao(d.sessao, d);
      break;
    case 'outro':
      await escolherTimer(pedidoDeLigar(d));
      break;
    case 'trocar': {
      // a sessão já tem timer (sem pergunta): o "+ Novo timer…" não entra. O aviso diz o timer
      // dela, para o caso de ela não vir na lista de sessões recentes
      const a = (snap.avisos ?? []).find((x) => x.sessao_id === d.sessao);
      await escolherTimer(pedidoDeLigar(d, a));
      break;
    }
  }
}

/** A alça do canto redimensiona o expandido. */
function aoApertar(ev) {
  if (ev.button === 0 && ev.target.closest('.grip')) {
    ev.preventDefault();
    redimensionando = true;
    janela().startResizeDragging('SouthEast').catch((e) => console.error('worktimer: redimensionar', e));
  }
}

// Duplo clique fora dos botões abre o app. Vale o `dblclick`, não o segundo `mousedown`:
// o primeiro clique começa o arrastar da janela e o Windows engole o segundo apertar.
function aoClicarDuas(ev) {
  if (ev.target.closest('button, label, input, .dlg-fundo, .faixa')) return;
  agir('abrir_app');
}

// Clicar fora do dock (a janela perde o foco) recolhe o expandido.
function aoMudarFoco({ payload: focado }) {
  if (focado) return;
  // o dock não abre diálogos (rodada 16): nada segura o expandido além do redimensionar
  const recolher = recolherAoPerderFoco({ expandido, dialogo: false, redimensionando, soPeloDialogo: false });
  if (recolher) mudarEtapa(false);
}


// ---- início ----

window.addEventListener('mousedown', aoApertar, true);
document.addEventListener('dblclick', aoClicarDuas);
document.addEventListener('click', aoClicar);
// o "Sempre que for na pasta" da pergunta de sessão nova (rodada 27): a caixa e a pasta mudam o
// que os botões respondem; o dock se redesenha e o foco volta ao mesmo lugar
document.addEventListener('change', (ev) => {
  if (!mudarEscolha(ev.target)) return;
  const d = ev.target.dataset;
  desenhar();
  const sel = d.pgSempre !== undefined ? `[data-pg-sempre][data-pergunta="${d.pergunta}"]` : `[data-pg-pasta][data-pergunta="${d.pergunta}"]:checked`;
  raiz.querySelector(sel)?.focus();
});
janela().onMoved(guardar);
janela().onResized(guardar);
janela().onFocusChanged(aoMudarFoco);
// o mouse em cima do dock segura o "Mostrar o dock agora" da bandeja (modo escondido, rodada 12)
const avisarMouse = (dentro) => () => {
  if (snap?.config.dock_modo === 'perguntas') chamar('dock_mouse', { dentro }).catch(() => {});
};
document.documentElement.addEventListener('mouseenter', avisarMouse(true));
document.documentElement.addEventListener('mouseleave', avisarMouse(false));
// o mouse anda na página sem botão apertado: o redimensionamento (se havia) acabou
document.addEventListener(
  'pointermove',
  (ev) => {
    if (!ev.buttons) redimensionando = false;
  },
  { passive: true },
);

let relogioId = null;
function ligarRelogio() {
  clearInterval(relogioId);
  relogioId = document.hidden ? null : setInterval(andar, 1000);
  if (!document.hidden) andar();
}
document.addEventListener('visibilitychange', ligarRelogio);
nomearPagina(document, ' — dock');

/** Lê o idioma guardado pela página (a primeira pintura, antes do estado). */
function lerIdiomaGuardado() {
  try {
    return JSON.parse(localStorage.getItem(CHAVE_IDIOMA));
  } catch {
    return null;
  }
}
definirIdioma(idiomaInicial(lerIdiomaGuardado(), navigator.language));
aplicarTextos(document);

aoMudar((s) => {
  snap = s;
  esquecerEscolhas((s.perguntas ?? []).map((p) => p.id));
  // o idioma do estado (rodada 23): trocado, o dock se redesenha nele logo abaixo
  const idioma = idiomaDoEstado(s, navigator.language);
  if (definirIdioma(idioma)) aplicarTextos(document);
  usarAtividades(s.atividades);
  usarCorte(s.config?.corte_min);
  usarEsperaApp(s.config?.espera_app_min);
  aplicarTema();
  if (expandido === null) expandido = Boolean(s.config.dock_expandido);
  desenhar();
  verDicas(s);
});
ligarRelogio();
