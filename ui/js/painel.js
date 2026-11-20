// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Janela principal: as quatro visões (Painel, Foco, Cartões, Linha do tempo), trocadas pelo topo.
// O evento "estado-mudou" pede snapshot novo e redesenha a visão; a cada segundo só os números
// andam, e as réguas da linha do tempo são redesenhadas uma vez por minuto.
import { chamar, aoMudar } from './api.js';
import { curto, relogio, relogioGrande, horaMin, totalDeTimers, totalDoGrupo, dataDoDia, trocarTexto, msDeHoje, desdeTexto, dicaDoTotalDoDia, itensDoMenuZerar, REUNIAO } from './formato.js';
import {
  secaoGrupo,
  relogioFoco,
  destino,
  msDoTimer,
  chaveGrupo,
  cor,
  semTimerNenhum,
  barraRodando,
  usarAtividades,
  usarCorte,
  usarEsperaApp,
  andarAtividades,
  dicaDoTempo,
  textoDaPergunta,
  opcoesDaPergunta,
  htmlDoMenuZerar,
} from './componentes.js';
import { linhaCompacta, visaoLinha } from './linha.js';
import {
  abaAtual,
  comAbas,
  gruposDaAba,
  timersDaAba,
  htmlAbas,
  secoesDaAba,
  totalDaAba,
  dicaDoTotalDaAba,
  textoPausarTudo,
  textoZeradosDaAba,
  topoDaTela,
  espacoDoBotao,
  abaDoTopo,
  grupoPreEscolhido,
  espacoPreEscolhido,
  htmlDoMenuNovo,
  teclaDoNovo,
} from './espacos.js';
import { visaoCartoes } from './cartoes.js';
import { visaoFoco } from './foco.js';
import {
  novoGrupo,
  novoEspaco,
  editarGrupo,
  novoTimer,
  editarTimer,
  faixa,
  faixaZerado,
  textoDaFaixaZerado,
  textoDaFaixaZeradoAtividade,
  TEXTO_ZERADOS_TODOS,
  ligarSessao,
  ligarATimer,
  editarSessao,
  ajustarHoje,
  aoPerderFoco,
  perguntarAoFechar,
  perguntarAoMinimizar,
  faixaEncerrado,
  quandoLivre,
  dialogoAberto,
  quandoSemDialogo,
  sessaoDeReserva,
  decidirPedido,
  ligarAoExistente,
  TEXTO_PEDIDO_DESCARTADO,
} from './editar.js';
import { montarAjustes, atualizarAjustes, esperarGravacoes, ligarAoClaude, faixaDosHooks } from './ajustes.js';
import { inicioSemTimers, semTimers, primeiroUso } from './primeiro-uso.js';
import { NOME_APP, nomearPagina, linhaDoConsole } from './nome.js';
import { vigiarTitulos, trocarTitulo } from './titulos.js';
import { iniciarTour, mostrarFaixaTour, tourAtivo, passosDoTour, htmlDoMenuAjuda, redesenharTour } from './tour.js';
import { DICAS, gatilhos, criarFila, balaoDeDica, fecharDicaAVista } from './dicas.js';

// as dicas longas em linhas curtas (rodada 19)
vigiarTitulos();

const $ = (sel) => document.querySelector(sel);
const CHAVE_FECHADOS = 'worktimer.grupos-fechados';
const CHAVE_TEMA = 'worktimer.tema';
const VISOES = ['painel', 'foco', 'cartoes', 'linha'];

let snap = null;
let selecionado = null;
let visao = null;
/** Visão de onde se abriu os Ajustes (para onde "Voltar" leva). */
let visaoAntes = 'painel';
let primeiroUsoVisto = false;
let relogioId = null;
let minutoDesenhado = null;
let temaNativo;
const fechados = new Set(lerLocal(CHAVE_FECHADOS, []));
const escuroDoWindows = window.matchMedia('(prefers-color-scheme: dark)');

function lerLocal(chave, padrao) {
  try {
    const v = localStorage.getItem(chave);
    return v === null ? padrao : JSON.parse(v);
  } catch {
    return padrao;
  }
}

function gravarLocal(chave, valor) {
  try {
    localStorage.setItem(chave, JSON.stringify(valor));
  } catch {
    /* sem armazenamento: só não lembra */
  }
}

// ---- tema ----

function temaResolvido(tema) {
  if (tema === 'claro' || tema === 'escuro') return tema;
  return escuroDoWindows.matches ? 'escuro' : 'claro';
}

function aplicarTema() {
  const escolhido = snap?.config.tema ?? 'escuro';
  const tema = temaResolvido(escolhido);
  document.documentElement.dataset.tema = tema;
  // barra de título nativa acompanha (null = segue o Windows)
  const nativo = escolhido === 'windows' ? null : tema === 'claro' ? 'light' : 'dark';
  if (nativo !== temaNativo) {
    temaNativo = nativo;
    window.__TAURI__?.window?.getCurrentWindow().setTheme(nativo).catch(() => {});
  }
  gravarLocal(CHAVE_TEMA, tema);
  const bt = $('#bt-tema');
  bt.textContent = tema === 'escuro' ? '☀︎' : '☾';
  bt.title = tema === 'escuro' ? 'Modo claro' : 'Modo escuro';
}

async function alternarTema() {
  if (!snap) return;
  const tema = temaResolvido(snap.config.tema) === 'escuro' ? 'claro' : 'escuro';
  snap.config = { ...snap.config, tema };
  aplicarTema();
  await agir('salvar_config', { config: snap.config });
}

escuroDoWindows.addEventListener('change', () => snap && aplicarTema());

// ---- visões ----

function mostrarVisao() {
  $('#bt-ajustes').classList.toggle('on', visao === 'ajustes');
  for (const b of document.querySelectorAll('.views [data-visao]')) {
    const on = b.dataset.visao === visao;
    b.classList.toggle('on', on);
    if (on) b.setAttribute('aria-current', 'page');
    else b.removeAttribute('aria-current');
  }
  for (const el of document.querySelectorAll('.vis')) {
    const on = el.dataset.v === visao;
    el.hidden = !on;
    // a visão escondida fica vazia: os números que andam são procurados só na que aparece
    if (!on && el.dataset.v !== 'painel') el.innerHTML = '';
  }
  if (visao !== 'painel') for (const id of ['#grupos', '#linha', '#foco']) $(id).innerHTML = '';
}

/** Ajustes ocupam o corpo como uma visão, mas não ficam salvos como a última escolhida. */
/** `secao`: a seção dos Ajustes a mostrar (o "Sobre o …" da bandeja pede "sobre"; rodada 18). */
function abrirAjustes(secao = null) {
  const irPara = () => secao && $(`#v-ajustes #aj-${secao}`)?.scrollIntoView({ block: 'start' });
  if (!snap) return;
  if (visao === 'ajustes') return irPara();
  // a dica à vista fecha ao ir para os Ajustes (sem contar como vista: volta depois)
  fecharDicaAVista();
  visaoAntes = visao ?? 'painel';
  visao = 'ajustes';
  mostrarVisao();
  montarAjustes($('#v-ajustes'), { lerSnap, voltar: () => trocarVisao(visaoAntes) }).then(irPara);
}

async function trocarVisao(v) {
  if (!VISOES.includes(v) || v === visao) return;
  // a dica à vista fecha ao trocar de tela (sem contar como vista: volta depois)
  fecharDicaAVista();
  visao = v;
  mostrarVisao();
  desenhar();
  if (snap && snap.config.visao !== v) {
    snap.config = { ...snap.config, visao: v };
    await agir('salvar_config', { config: snap.config });
  }
}

// ---- dados ----

const todosTimers = () => (snap ? snap.grupos.flatMap((g) => g.timers) : []);
const grupoDe = (timer) => snap.grupos.find((g) => g.timers.includes(timer)) ?? null;
const lerSnap = () => snap;
// a régua conta as horas desde a meia-noite, mesmo com o corte do dia depois dela
const meiaNoite = () => snap.dia_inicio - snap.config.corte_min * 60_000;

/** As telas com as abas de espaço (rodada 20). */
const TELAS_COM_ABAS = ['painel', 'cartoes', 'linha'];

/** A aba de espaço desta tela (nulo: "Todos", e sempre nas telas sem abas). */
const abaDaTela = () => (snap && TELAS_COM_ABAS.includes(visao) ? abaAtual(snap) : null);

/** O timer do painel: o escolhido, se está na aba; senão, o primeiro rodando ou o primeiro da aba. */
function timerSelecionado() {
  const ts = snap ? timersDaAba(snap, abaDaTela()) : [];
  return ts.find((t) => t.id === selecionado) ?? ts.find((t) => t.rodando) ?? ts[0] ?? null;
}

async function agir(nome, args) {
  try {
    return await chamar(nome, args);
  } catch (e) {
    avisar(typeof e === 'string' ? e : 'Não deu certo. Tente de novo.');
    console.error(`worktimer: ${nome} falhou`, e);
    return null;
  }
}

let avisoId = null;
function avisar(texto) {
  const el = $('#aviso');
  el.textContent = texto;
  el.hidden = false;
  clearTimeout(avisoId);
  avisoId = setTimeout(() => (el.hidden = true), 4000);
}

// ---- foco do teclado e rolagem entre desenhos ----

/** Identidade de um elemento que sobrevive ao redesenho (play e pausar contam igual). */
function chaveDoFoco(el) {
  if (!(el instanceof HTMLElement) || !(el.dataset.acao || el.dataset.timer)) return null;
  const d = el.dataset;
  const acao =
    d.acao === 'pausar'
      ? 'play'
      : d.acao === 'parar-reuniao'
        ? 'reuniao'
        : d.acao === 'pausar-atividade'
          ? 'play-atividade'
          : (d.acao ?? '');
  return [acao, d.timer ?? '', d.grupo ?? '', d.sessao ?? '', d.atividade ?? '', el.classList[0] ?? ''].join('|');
}

function focarPelaChave(chave) {
  if (!chave) return;
  for (const el of document.querySelectorAll('.corpo [data-acao], .corpo [data-timer][tabindex]')) {
    if (chaveDoFoco(el) === chave) {
      el.focus({ preventScroll: true });
      return;
    }
  }
}

aoPerderFoco((antigo) => focarPelaChave(chaveDoFoco(antigo)));

/** Redesenha `fn()` guardando o foco do teclado e a rolagem das listas. */
function preservando(fn) {
  const ativo = document.activeElement;
  const chave = ativo?.closest('.corpo') ? chaveDoFoco(ativo) : null;
  const rolagem = {};
  for (const el of document.querySelectorAll('.corpo [data-rolagem]')) rolagem[el.dataset.rolagem] = el.scrollTop;
  fn();
  for (const el of document.querySelectorAll('.corpo [data-rolagem]')) el.scrollTop = rolagem[el.dataset.rolagem] ?? 0;
  if (chave && !document.contains(ativo)) focarPelaChave(chave);
}

// ---- desenho ----

/**
 * As perguntas do núcleo (sessão nova, volta da inatividade, app fechado) também na janela
 * principal, numa faixa sob o topo: com o dock escondido ("Só quando houver uma pergunta") nada se
 * perde, e responder aqui ou no dock resolve as duas (rodada 12). Uma por vez, como no dock.
 */
function desenharPerguntas() {
  const el = $('#perguntas');
  if (!el) return;
  const [p, ...resto] = snap.perguntas ?? [];
  const html = p
    ? `${textoDaPergunta(p)}${opcoesDaPergunta(p)}${resto.length ? `<small class="pg-mais">+${resto.length} ${resto.length === 1 ? 'pergunta' : 'perguntas'} depois desta</small>` : ''}`
    : '';
  if (el.dataset.html !== html) {
    el.dataset.html = html;
    el.innerHTML = html;
  }
  el.hidden = !p;
}

function desenhar() {
  if (!snap || !visao) return;
  desenharPerguntas();
  if (visao === 'ajustes') {
    $('#data').textContent = dataDoDia(snap.dia);
    // o topo dos Ajustes vale para a aba lembrada (revisão da rodada 20)
    desenharTopoDaAba();
    atualizarAjustes(snap);
    andar();
    return;
  }
  const agora = Date.now();
  const atual = timerSelecionado();
  selecionado = atual?.id ?? null;
  $('#data').textContent = dataDoDia(snap.dia);

  preservando(() => {
    if (visao === 'painel') desenharPainel(atual, agora);
    else if (visao === 'cartoes') $('#v-cartoes').innerHTML = visaoCartoes(snap, agora, fechados, selecionado, abaDaTela());
    else if (visao === 'foco') desenharFoco(atual, agora);
    else $('#v-linha').innerHTML = visaoLinha(snap, meiaNoite(), agora, abaDaTela());
  });
  desenharTopoDaAba();
  minutoDesenhado = Math.floor(agora / 60_000);
  // o botão redesenhado com o menu dele aberto segue marcado (revisão da rodada 19)
  if (menuAberto) marcarExpandido(menuAberto.dono, true);
  andar();
}

/**
 * O "Zerar todos" do topo diz a aba ("↺ Zerar todos (Pessoal)", rodada 20). O escopo vai no botão
 * (`data-espaco`), da mesma fonte do texto (`topoDaTela`): o clique lê dali.
 */
function desenharTopoDaAba() {
  const zt = $('#bt-zerar-todos');
  if (!zt || !snap) return;
  if (zt.dataset.padrao === undefined) zt.dataset.padrao = zt.title;
  const topo = topoDaTela(snap, visao);
  zt.dataset.espaco = topo.espaco ?? '';
  trocarTexto(zt, topo.texto);
  trocarTitulo(zt, topo.dica ?? zt.dataset.padrao);
}

function desenharPainel(atual, agora) {
  const a = abaDaTela();
  const grupos = gruposDaAba(snap, a);
  const rodando = grupos.flatMap((g) => g.timers).filter((t) => t.rodando).length;
  $('#grupos').innerHTML =
    htmlAbas(snap, a) +
    barraRodando(rodando, textoPausarTudo(snap, a), a) +
    (grupos.length
      ? secoesDaAba(snap, a, (g) => secaoGrupo(g, agora, !fechados.has(chaveGrupo(g)), selecionado), agora)
      : semTimerNenhum());
  const foco = $('#foco');
  if (atual) {
    const g = grupoDe(atual);
    foco.innerHTML = relogioFoco(atual, agora, g) + destino(atual, g);
    foco.style.setProperty('--c', cor(atual.cor));
  } else {
    foco.innerHTML = '';
  }
  $('#linha').innerHTML = linhaCompacta(grupos, meiaNoite(), agora);
}

function desenharFoco(atual, agora) {
  $('#v-foco').innerHTML = visaoFoco(snap, agora, fechados, atual, atual ? grupoDe(atual) : null, meiaNoite());
}

/** Uma vez por minuto: as réguas e o que mostra minutos (blocos abertos crescem). */
function desenharMinuto(agora) {
  preservando(() => {
    if (visao === 'painel') $('#linha').innerHTML = linhaCompacta(gruposDaAba(snap, abaDaTela()), meiaNoite(), agora);
    else if (visao === 'linha') $('#v-linha').innerHTML = visaoLinha(snap, meiaNoite(), agora, abaDaTela());
    else if (visao === 'foco') desenharFoco(timerSelecionado(), agora);
    else if (visao === 'cartoes') $('#v-cartoes').innerHTML = visaoCartoes(snap, agora, fechados, selecionado, abaDaTela());
  });
}

/** A cada segundo: só os textos dos números, e só os que mudaram (nada de refazer o HTML). */
function andar() {
  if (!snap) return;
  const agora = Date.now();
  const minuto = Math.floor(agora / 60_000);
  if (minuto !== minutoDesenhado) {
    minutoDesenhado = minuto;
    desenharMinuto(agora);
  }
  const ts = todosTimers();
  // o total do dia é o da aba (rodada 20); de onde vem o número, o geral e o de cada espaço na dica
  const total = curto(totalDaAba(snap, abaDoTopo(snap, visao), agora));
  const dicaTotal = dicaDoTotalDaAba(snap, abaDoTopo(snap, visao), agora);
  for (const el of [$('#total-dia'), ...document.querySelectorAll('[data-total-dia]')]) {
    trocarTexto(el, total);
    trocarTitulo(el, dicaTotal);
  }
  for (const t of ts) {
    // o número do timer (o acumulado desde o zerar), com "desde …" na dica
    const ms = msDoTimer(t, agora);
    const dica = dicaDoTempo(t, agora);
    for (const el of document.querySelectorAll(`[data-tempo="${t.id}"]`)) {
      trocarTexto(el, relogio(ms));
      if (dica) trocarTitulo(el, dica);
    }
    for (const el of document.querySelectorAll(`[data-desde="${t.id}"]`)) trocarTexto(el, desdeTexto(t.zerado_em, agora, snap.config.corte_min));
    // a linha do tempo é do dia
    for (const el of document.querySelectorAll(`[data-hm-hoje="${t.id}"]`)) trocarTexto(el, horaMin(msDeHoje(t, agora)));
  }
  for (const g of snap.grupos) {
    // a soma dos timers do grupo; o tempo real (a união) do mesmo período na dica
    const { texto, dica } = totalDoGrupo(g.timers, agora, g.real_antes_ms ?? 0);
    for (const el of document.querySelectorAll(`[data-total-grupo="${chaveGrupo(g)}"]`)) {
      trocarTexto(el, texto);
      trocarTitulo(el, dica);
    }
  }
  // o cabeçalho de cada espaço em "Todos": a soma dos timers dele, com o tempo real na dica
  for (const e of snap.espacos ?? []) {
    const els = document.querySelectorAll(`[data-total-espaco="${e.id}"]`);
    if (!els.length) continue;
    const { texto, dica } = totalDoGrupo(timersDaAba(snap, e.id), agora, e.real_antes_ms ?? 0);
    for (const el of els) {
      trocarTexto(el, texto);
      trocarTitulo(el, dica);
    }
  }
  // os tempos por atividade (as linhas do painel do timer e a etiqueta da reunião nos cartões)
  andarAtividades(document, ts, agora);
  const atual = ts.find((t) => t.id === selecionado);
  const clk = atual && document.querySelector(`[data-relogio="${atual.id}"]`);
  if (clk) {
    const { hm, s } = relogioGrande(msDoTimer(atual, agora));
    // o relógio grande é "HH:MM" (texto) + "<span>:SS</span>": troca só os dois textos
    const seg = clk.querySelector('span');
    if (seg && clk.firstChild?.nodeType === Node.TEXT_NODE) {
      if (clk.firstChild.nodeValue !== hm) clk.firstChild.nodeValue = hm;
      trocarTexto(seg, `:${s}`);
    } else {
      clk.innerHTML = `${hm}<span>:${s}</span>`;
    }
  }
}

function ligarRelogio() {
  clearInterval(relogioId);
  relogioId = document.hidden ? null : setInterval(andar, 1000);
  if (!document.hidden) andar();
}

// ---- cliques ----

function selecionar(id) {
  selecionado = id;
  desenhar();
}

async function aoClicar(ev) {
  if (ev.target.closest('.dlg-fundo, .faixa')) return;
  const alvo = ev.target.closest('[data-acao], .corpo [data-timer][tabindex]');
  if (!alvo || !snap) return;
  const acao = alvo.dataset.acao;
  const id = Number(alvo.dataset.timer);
  const grupoId = alvo.dataset.grupo === '' || alvo.dataset.grupo === undefined ? null : Number(alvo.dataset.grupo);
  switch (acao) {
    case 'visao':
      await trocarVisao(alvo.dataset.visao);
      break;
    case 'play':
      selecionado = id;
      await agir('play', { timer_id: id });
      break;
    case 'pausar':
      await agir('pausar', { timer_id: id });
      break;
    case 'reuniao':
      selecionado = id;
      await agir('play_atividade', { timer_id: id, atividade_id: REUNIAO });
      break;
    case 'parar-reuniao':
      await agir('parar_atividade', { timer_id: id, atividade_id: REUNIAO });
      break;
    // as linhas por atividade do painel do timer: ▶/⏸ e ✎ ("Ajustar hoje" já nela)
    case 'play-atividade':
      selecionado = id;
      await agir('play_atividade', { timer_id: id, atividade_id: Number(alvo.dataset.atividade) });
      break;
    case 'pausar-atividade':
      await agir('pausar_atividade', { timer_id: id, atividade_id: Number(alvo.dataset.atividade) });
      break;
    case 'ajustar-atividade':
      ajustarHoje(snap, id, Number(alvo.dataset.atividade));
      break;
    case 'encerrar': {
      const nome = snap.grupos.flatMap((g) => g.timers).find((t) => t.id === id)?.nome;
      if (await agir('encerrar', { timer_id: id })) faixaEncerrado(id, nome);
      break;
    }
    // "Pausar tudo" vale para a aba (rodada 20; em "Todos", para tudo)
    case 'pausar-tudo':
      // o escopo é o do botão (o mesmo do texto dele)
      await agir('pausar_tudo', { espaco_id: espacoDoBotao(alvo) });
      break;
    // as abas de espaço: a escolhida fica na config
    case 'aba': {
      const id = alvo.dataset.espaco === '' ? null : Number(alvo.dataset.espaco);
      if (id === abaDaTela()) break;
      snap.config = { ...snap.config, espaco_aba: id };
      desenhar();
      await agir('salvar_config', { config: snap.config });
      break;
    }
    // o "+ Novo" do topo: o menu com Timer, Grupo e Espaço
    case 'novo-menu':
      alternarMenuNovo(alvo);
      break;
    case 'selecionar':
      selecionar(id);
      break;
    case 'dobrar': {
      const k = alvo.dataset.grupo;
      if (fechados.has(k)) fechados.delete(k);
      else fechados.add(k);
      gravarLocal(CHAVE_FECHADOS, [...fechados]);
      desenhar();
      break;
    }
    case 'desligar':
      await agir('desligar_sessao', { sessao_id: alvo.dataset.sessao });
      break;
    case 'editar-sessao':
      editarSessao(lerSnap, Number(alvo.dataset.timer), alvo.dataset.sessao);
      break;
    case 'tema':
      await alternarTema();
      break;
    // ↺: só grava o instante do zerar (nada é apagado), sem diálogo; a faixa tem o "Desfazer".
    // Com mais de uma atividade com tempo, abre o menu "O timer todo" / "Só <atividade>" (rodada 19).
    case 'zerar': {
      const timer = snap.grupos.flatMap((g) => g.timers).find((t) => t.id === id);
      const itens = timer ? itensDoMenuZerar(timer, snap.atividades, Date.now()) : [];
      if (itens.length) alternarMenuZerar(alvo, timer, itens);
      else await zerarTimer(id);
      break;
    }
    // o ↺ pequeno de uma linha de atividade do painel do timer (rodada 19)
    case 'zerar-atividade':
      await zerarAtividade(id, Number(alvo.dataset.atividade));
      break;
    // "Zerar todos" da barra do topo: todos os timers no mesmo instante, sem diálogo
    case 'zerar-todos': {
      // o escopo é o do botão, gravado junto com o texto dele (revisão da rodada 20)
      const a = espacoDoBotao(alvo);
      const z = await agir('zerar_todos', { espaco_id: a });
      if (z) {
        faixaZerado(z, textoZeradosDaAba(snap, a) ?? TEXTO_ZERADOS_TODOS);
        dicas.oferecer('zerar');
      }
      break;
    }
    case 'editar':
      editarTimer(snap, id);
      break;
    case 'editar-grupo':
      editarGrupo(snap, grupoId);
      break;
    // as perguntas na janela principal: a resposta vai ao núcleo, como no dock
    case 'responder':
      await agir('responder', { pergunta_id: Number(alvo.dataset.pergunta), valor: alvo.dataset.valor });
      break;
    case 'outro':
      await ligarATimer(lerSnap, alvo.dataset.sessao, (sessaoId) => agir('pedir_novo_timer', { sessao_id: sessaoId }));
      break;
    case 'novo-timer': {
      // o "+ Novo timer…" de uma pergunta de sessão nova: o pedido volta a esta janela (`novo-timer`)
      if (alvo.dataset.sessao !== undefined) {
        await agir('pedir_novo_timer', { sessao_id: alvo.dataset.sessao });
        break;
      }
      // a linha "Grupo vazio · + Timer": o timer novo já nesse grupo
      const novo = await novoTimer(snap, grupoId, null, { espacoId: Number(alvo.dataset.espaco || 1) });
      if (novo !== null) selecionar(novo);
      break;
    }
    case 'novo-grupo':
      await novoGrupo(snap);
      break;
    case 'ligar-sessao':
      await ligarSessao(lerSnap, id);
      break;
    case 'ajustes':
      if (visao === 'ajustes') await trocarVisao(visaoAntes);
      else abrirAjustes();
      break;
    case 'tour':
      // o ? abre o menu: "Como começar", "Esta tela: …" e "Tour completo" (rodada 17)
      alternarMenuAjuda(alvo);
      break;
    default:
      if (!acao && id) selecionar(id);
  }
}

/**
 * O ⚙ do dock pediu os Ajustes ("ajustes"), ou o "Sobre o …" da bandeja ("sobre")? Antes do
 * primeiro snapshot, só anota.
 */
let querAjustes = null;
async function pedirAjustes() {
  let pedido;
  try {
    pedido = await chamar('ajustes_pedidos');
  } catch (e) {
    console.error('worktimer: ajustes_pedidos', e);
    return;
  }
  if (!pedido) return;
  if (snap && visao) abrirAjustes(secaoDoPedido(pedido));
  else querAjustes = pedido;
}

/** A seção do pedido: "sobre" vai direto ao Sobre; "ajustes", ao topo. */
const secaoDoPedido = (pedido) => (pedido === 'sobre' ? 'sobre' : null);

/** `acao`: "perguntar", "bandeja" ou "sair" (a config `ao_fechar`). */
async function aoPedirFechar({ acao }) {
  if (acao === 'perguntar') {
    chamar('fechar_visto').catch((e) => console.error('worktimer: fechar_visto', e));
    perguntarAoFechar({ antes: esperarGravacoes });
    return;
  }
  try {
    await esperarGravacoes();
    await chamar('fechar_principal', { acao, lembrar: false });
  } catch (e) {
    // um erro do núcleo diz o motivo; sem resposta, o núcleo fecha sozinho logo depois
    console.error('worktimer: fechar_principal', e);
    if (typeof e === 'string') avisar(e);
  }
}

/** O minimizar da janela: `acao` "perguntar" ou "bandeja" (a config `ao_minimizar`). */
async function aoPedirMinimizar({ acao }) {
  if (acao === 'perguntar') {
    chamar('minimizar_visto').catch((e) => console.error('worktimer: minimizar_visto', e));
    perguntarAoMinimizar({ antes: esperarGravacoes });
    return;
  }
  try {
    await esperarGravacoes();
    await chamar('minimizar_principal', { acao, lembrar: false });
  } catch (e) {
    // sem resposta, o núcleo fecha sozinho logo depois
    console.error('worktimer: minimizar_principal', e);
  }
}

// ---- faixa dos hooks da versão antiga ----
// O app nunca regrava o settings.json sozinho: a faixa leva à mesma prévia exata dos Ajustes.

let faixaHooksEscondida = false;
let estadoHooks = null;
/** O Claude Code existe neste PC? (a pasta `~\.claude`; rodada 18) */
let claudeEncontrado = true;

/**
 * A faixa aparece com os hooks da versão antiga ("Atualizar") ou desligados com timers ("Ligar",
 * revisão da rodada 16), e nunca com um diálogo aberto: atrás da sombra, o clique cairia nela e
 * fecharia o diálogo (um passo do primeiro uso, um editar). Sem argumento, só reconfere (um diálogo
 * abriu ou fechou, o estado mudou).
 */
function mostrarFaixaHooks(...estado) {
  const el = $('#faixa-hooks');
  if (!el) return;
  if (estado.length) estadoHooks = estado[0];
  const f = faixaDosHooks(estadoHooks, Boolean(snap) && !semTimers(snap), claudeEncontrado);
  if (f) {
    el.querySelector('span').textContent = f.texto;
    $('#bt-hooks-atualizar').textContent = f.botao;
    const esc = $('#bt-hooks-esconder');
    esc.textContent = f.esconder;
    esc.className = f.esconder === '×' ? 'esconder' : 'esconder-txt';
    el.dataset.atualizar = f.atualizar ? '1' : '';
  }
  el.hidden = !f || faixaHooksEscondida || Boolean(document.querySelector('.dlg-fundo'));
}

async function verHooks() {
  try {
    const c = await chamar('conexao_claude');
    claudeEncontrado = c.encontrado !== false;
    mostrarFaixaHooks(c.estado);
  } catch (e) {
    console.error('worktimer: conexao_claude', e);
  }
}

function ligarFaixaHooks() {
  $('#bt-hooks-atualizar')?.addEventListener('click', async () => {
    await ligarAoClaude({ atualizar: $('#faixa-hooks').dataset.atualizar === '1' });
    await verHooks();
  });
  $('#bt-hooks-esconder')?.addEventListener('click', () => {
    faixaHooksEscondida = true;
    mostrarFaixaHooks(null);
  });
  // os Ajustes ligaram, atualizaram ou removeram os hooks
  document.addEventListener('hooks-mudaram', (ev) => mostrarFaixaHooks(ev.detail));
  // um diálogo abriu ou fechou
  new MutationObserver(() => mostrarFaixaHooks()).observe(document.body, { childList: true });
  // a faixa só aparece depois do início (o primeiro uso aberto, ou nada a abrir)
  inicioResolvido.then(verHooks);
}

// ---- tour (rodada 16) ----

/**
 * Um tour: `essencial` (o padrão, 5 passos), `completo` (8) ou `tela` (o mini-tour da tela de
 * agora). O essencial e o completo rodam no Painel (troca de visão antes); o da tela, onde a
 * pessoa está. A dica à vista fecha.
 */
async function verTour(tipo = 'essencial') {
  fecharDicaAVista();
  const daTela = tipo === 'tela';
  if (!daTela && visao !== 'painel') await trocarVisao('painel');
  const passos = passosDoTour(tipo, visao, { abas: comAbas(snap) });
  const dockEscondido = snap?.config?.dock_modo === 'perguntas';
  // o passo das pastas troca para "Conectar o Claude Code" com os hooks desligados; relido a cada
  // desenho, e o tour se redesenha depois do "Ligar" (revisão)
  const claudeLigado = () => estadoHooks === null || estadoHooks === 'ligado';
  const aoAcao = (acao) => {
    if (acao === 'ligar-claude') ligarAoClaude().then(verHooks).then(redesenharTour);
  };
  quandoLivre(() => setTimeout(() => iniciarTour({ passos, dockEscondido, claudeLigado, marcarVisto: !daTela, aoAcao }), 50));
}

/** ↺ do timer todo: o zerar de hoje, com a faixa do "Desfazer". */
async function zerarTimer(id) {
  const nome = snap.grupos.flatMap((g) => g.timers).find((t) => t.id === id)?.nome;
  const z = await agir('zerar', { timer_id: id });
  if (z) {
    faixaZerado(z, textoDaFaixaZerado(nome));
    dicas.oferecer('zerar');
  }
}

/** ↺ "Só <atividade>" (rodada 19): só ela volta a 0:00; a faixa tem o "Desfazer". */
async function zerarAtividade(id, atividade) {
  const nome = snap.grupos.flatMap((g) => g.timers).find((t) => t.id === id)?.nome;
  const nomeAt = (snap.atividades ?? []).find((a) => a.id === atividade)?.nome ?? 'a atividade';
  const z = await agir('zerar_atividade', { timer_id: id, atividade_id: atividade });
  if (z) {
    faixaZerado(z, textoDaFaixaZeradoAtividade(nome, nomeAt));
    dicas.oferecer('zerar');
  }
}

/** O menu do ↺ (rodada 19), abaixo do botão, como o do ?. */
function alternarMenuZerar(botao, timer, itens) {
  abrirMenu(botao, htmlDoMenuZerar(timer, itens), 'esquerda', (b) => {
    if (b.dataset.zerar === 'timer') zerarTimer(timer.id);
    else if (b.dataset.zerar === 'atividade') zerarAtividade(timer.id, Number(b.dataset.atividade));
  });
}

/** O menu do "+ Novo" (rodada 20): Timer, Grupo e Espaço. */
function alternarMenuNovo(botao) {
  abrirMenu(botao, htmlDoMenuNovo(), 'direita', (b) => criarNovo(b.dataset.novo));
}

/**
 * O que o "+ Novo" cria. O timer vem no grupo do timer aberto no painel, ou no "Sem grupo" do
 * espaço da aba (em "Todos", do primeiro espaço); o grupo, no espaço da aba.
 */
async function criarNovo(tipo) {
  if (!snap) return;
  const a = abaDaTela();
  if (tipo === 'timer') {
    const aberto = visao === 'painel' ? timerSelecionado() : null;
    const { grupoId, espacoId } = grupoPreEscolhido(snap, a, aberto);
    const novo = await novoTimer(snap, grupoId, null, { espacoId });
    if (novo !== null) selecionar(novo);
  } else if (tipo === 'grupo') {
    await novoGrupo(snap, espacoPreEscolhido(snap, a));
  } else if (tipo === 'espaco') {
    await novoEspaco(snap);
  }
}

/** O menu do ? (abre e fecha; Esc e o clique fora fecham). */
function alternarMenuAjuda(botao) {
  abrirMenu(botao, htmlDoMenuAjuda(visao), 'direita', (b) => {
    if (b.dataset.ajuda) verTour(b.dataset.ajuda);
  });
}

/** O menu aberto (o do ↺ ou o do ?): `{ dono, fechar }`, ou nulo. */
let menuAberto = null;

/** Quem abriu o menu: a ação e o timer do botão (o botão pode ser redesenhado com o menu aberto). */
const donoDoMenu = (b) => `${b.dataset.acao}:${b.dataset.timer ?? ''}`;

/** O botão de agora do dono (o redesenho troca o elemento). */
const botaoDoDono = (dono) => [...document.querySelectorAll('[data-acao]')].find((b) => donoDoMenu(b) === dono) ?? null;

/** `aria-expanded` dos botões do dono que abrem menu. */
function marcarExpandido(dono, aberto) {
  for (const b of document.querySelectorAll('[aria-haspopup="menu"]')) if (donoDoMenu(b) === dono) b.setAttribute('aria-expanded', String(aberto));
}

/**
 * Um menu pequeno (`.menu-ajuda`) junto ao `botao`: abre e fecha; Esc e o clique fora fecham.
 * `lado`: "direita" alinha a borda direita à do botão (o ? no canto); "esquerda", a esquerda,
 * dentro da janela. Abaixo do botão, ou acima se não couber. `aoEscolher(botao)` com o item
 * clicado, depois de fechar. Fechar por qualquer caminho tira os ouvintes; o clique no botão do
 * dono (mesmo redesenhado) com o menu aberto só fecha.
 */
function abrirMenu(botao, html, lado, aoEscolher) {
  const dono = donoDoMenu(botao);
  if (menuAberto) {
    const era = menuAberto.dono;
    menuAberto.fechar();
    if (era === dono) return;
  }
  const tmp = document.createElement('div');
  tmp.innerHTML = html;
  const menu = tmp.firstElementChild;
  document.body.append(menu);
  const r = botao.getBoundingClientRect();
  const m = menu.getBoundingClientRect();
  const cabeEmbaixo = r.bottom + 6 + m.height <= window.innerHeight - 8;
  const top = cabeEmbaixo ? r.bottom + 6 : Math.max(8, r.top - 6 - m.height);
  if (lado === 'direita') {
    Object.assign(menu.style, { top: `${Math.round(top)}px`, right: `${Math.round(window.innerWidth - r.right)}px` });
  } else {
    const left = Math.min(Math.max(8, r.left), Math.max(8, window.innerWidth - m.width - 8));
    Object.assign(menu.style, { top: `${Math.round(top)}px`, left: `${Math.round(left)}px` });
  }
  menu.querySelector('button')?.focus();
  const fechar = () => {
    menu.remove();
    document.removeEventListener('mousedown', fora, true);
    document.removeEventListener('keydown', tecla, true);
    if (menuAberto?.fechar === fechar) menuAberto = null;
    marcarExpandido(dono, false);
  };
  const fora = (ev) => {
    if (menu.contains(ev.target)) return;
    // o botão do dono: o clique dele fecha (pelo `abrirMenu`), sem reabrir
    const b = ev.target.closest?.('[data-acao]');
    if (b && donoDoMenu(b) === dono) return;
    fechar();
  };
  const tecla = (ev) => {
    if (ev.key === 'Escape') {
      ev.preventDefault();
      fechar();
      botaoDoDono(dono)?.focus();
    }
  };
  menuAberto = { dono, fechar };
  marcarExpandido(dono, true);
  menu.addEventListener('click', (ev) => {
    const b = ev.target.closest('button');
    if (!b) return;
    fechar();
    aoEscolher(b);
  });
  document.addEventListener('mousedown', fora, true);
  document.addEventListener('keydown', tecla, true);
}

/**
 * A faixa "Novo: um tour de 1 minuto…" para quem já usa o app e nunca viu o tour: decidida uma vez,
 * na abertura. "Ver" e "Agora não" a tiram para sempre (gravam `tour_visto`).
 */
function ligarFaixaTour(s) {
  const f = $('#faixa-tour');
  if (!f || !mostrarFaixaTour(s.config, !semTimers(s))) return;
  f.hidden = false;
  const responder = (ver) => {
    f.hidden = true;
    chamar('marcar_tour_visto').catch((e) => console.error('worktimer: marcar_tour_visto', e));
    if (ver) verTour();
  };
  $('#bt-tour-ver').addEventListener('click', () => responder(true), { once: true });
  $('#bt-tour-agora-nao').addEventListener('click', () => responder(false), { once: true });
}
// o "Conectar" da linha "O Claude Code não está conectado" dos diálogos de ligar (rodada 18)
document.addEventListener('conectar-claude', () => ligarAoClaude().then(verHooks));
// o primeiro uso ("Fazer o tour") e os Ajustes ("Como começar", "Tour completo") pedem o tour
// aqui; sem tipo, o essencial
document.addEventListener('pedir-tour', (ev) => verTour(ev.detail?.tipo ?? 'essencial'));

// ---- dicas (rodada 16) ----
// Uma por vez, na primeira vez que cada coisa acontece; nunca durante o tour, com um diálogo
// aberto ou com a janela escondida (aí a dica espera, ou vai para o dock, se ele pedir antes).
const dicas = criarFila({
  ligadas: () => snap?.config?.dicas_ligadas !== false,
  vistas: () => snap?.config?.dicas_vistas ?? [],
  podeMostrar: () => !tourAtivo() && !dialogoAberto() && !document.hidden && visao !== 'ajustes',
  reivindicar: (id) => chamar('dica_reivindicar', { id }).catch(() => 'esperar'),
  async mostrar(id) {
    const d = DICAS[id];
    const r = await balaoDeDica(d.texto(snap), d.alvo(snap), id);
    if (r === 'desligar') await chamar('dicas_ligar', { ligadas: false }).catch((e) => console.error('worktimer: dicas_ligar', e));
    return r;
  },
  concluir: (id, vista) => chamar('dica_saiu', { id, vista }).catch((e) => console.error('worktimer: dica_saiu', e)),
});
/**
 * As dicas do estado de agora (as do dock ficam com o dock). A cada estado: a vista e a que já está
 * na fila não entram de novo, e "Mostrar as dicas de novo" vale na hora (revisão).
 */
function verDicas(s) {
  for (const id of gatilhos(s)) if (!DICAS[id].soNoDock) dicas.oferecer(id);
}
// A janela principal ocupada (o tour, um diálogo) avisa o núcleo, que segura as dicas das duas
// janelas, e a dica à vista fecha (sem contar como vista: volta depois).
let principalOcupada = false;
/** `forcar`: manda o estado mesmo sem mudança (a página começou, a janela voltou a aparecer). */
function conferirOcupada(forcar = false) {
  const ocupada = Boolean(document.querySelector('body > .dlg-fundo, body > .tour'));
  if (ocupada) fecharDicaAVista();
  if (ocupada === principalOcupada && forcar !== true) return;
  principalOcupada = ocupada;
  chamar('principal_ocupada', { ocupada }).catch((e) => console.error('worktimer: principal_ocupada', e));
}
new MutationObserver(() => conferirOcupada()).observe(document.body, { childList: true });
// A página (re)começou: o núcleo solta o que era dela (uma dica, o ocupado de antes de recarregar)
// e ela informa o estado de agora. Escondida, a dica dela fecha (o núcleo também solta ao
// esconder); de volta, informa de novo (re-revisão da rodada 16).
chamar('dicas_pagina_comecou')
  .catch(() => {})
  .finally(() => conferirOcupada(true));
document.addEventListener('visibilitychange', () => {
  if (document.hidden) fecharDicaAVista();
  else conferirOcupada(true);
});
// o núcleo fechou a dica à vista (a principal ficou ocupada)
window.__TAURI__.event.listen('dica-fechar', ({ payload }) => fecharDicaAVista(payload));
// o que esperava (o tour, um diálogo, a janela escondida) tenta de novo
setInterval(() => dicas.tentar(), 2_000);

// ---- início: primeiro uso e "+ Novo timer…" do dock ----

/** Resolve quando o início foi decidido (o primeiro uso, ou nada a abrir). */
let resolverInicio = () => {};
const inicioResolvido = new Promise((ok) => (resolverInicio = ok));
/** Resolve quando o início já buscou o "+ Novo timer…" que fez a janela abrir. */
let inicioPronto = () => {};
const inicioBuscou = new Promise((ok) => (inicioPronto = ok));

async function pedidoNovoTimer() {
  try {
    return await chamar('novo_timer_pedido');
  } catch (e) {
    console.error('worktimer: novo_timer_pedido', e);
    return null;
  }
}

/** Sessões com um "+ Novo timer…" esperando ou aberto: um pedido repetido não abre outro. */
const pedidosAbertos = new Set();

/**
 * O "+ Novo timer…" da pergunta de sessão nova: o "Novo timer" com o nome pela pasta e o "sempre
 * nesta pasta" marcado. Com outro diálogo aberto (o primeiro uso, um editar), espera
 * ele fechar. Na vez dele, relê o estado (`decidirPedido`): a pergunta que já saiu descarta o
 * pedido (nada é criado), e um timer de mesmo nome vira "Já existe…: ligar a ele?". Criado, o
 * timer novo fica selecionado. `depois()` roda quando o pedido termina (o primeiro uso).
 */
function abrirNovoTimerDaSessao(pedido, depois = null) {
  // repetido: não abre outro (e quem esperava o fim dele não fica esperando)
  if (pedidosAbertos.has(pedido.sessao_id)) return depois?.();
  pedidosAbertos.add(pedido.sessao_id);
  const vez = async () => {
    let s = null;
    try {
      s = await chamar('obter_estado');
    } catch (e) {
      console.error('worktimer: obter_estado', e);
    }
    // outro diálogo abriu enquanto o estado vinha: volta para a fila
    if (dialogoAberto()) return quandoLivre(vez);
    try {
      const d = s ? decidirPedido(s, pedido) : { descartar: true };
      if (d.descartar) {
        avisar(TEXTO_PEDIDO_DESCARTADO);
      } else if (d.ligarA) {
        await ligarAoExistente(d.ligarA, pedido, () => novoTimer(s, null, pedido, { mesmoNome: true }));
      } else {
        const id = await novoTimer(s, null, pedido);
        if (id !== null) selecionar(id);
      }
    } finally {
      pedidosAbertos.delete(pedido.sessao_id);
      depois?.();
    }
  };
  quandoLivre(vez);
}

/**
 * Na primeira leitura do estado: o primeiro uso (sem timers) e o pedido do dock.
 * Com o pedido, o "Novo timer" vem antes do assistente (que vem depois dele, sem o passo de
 * criar timers se o timer foi criado): o assistente nunca é pulado.
 */
async function inicio(s) {
  const pedido = await pedidoNovoTimer();
  inicioPronto();
  let resultado = 'com-timers';
  try {
    if (semTimers(s)) resultado = await inicioSemTimers({ primeiroUso: !pedido });
  } finally {
    resolverInicio();
  }
  if (!pedido) return;
  const assistente = resultado === 'sem-timers';
  // a janela nasceu pelo "+ Novo timer…" do dock: o fim dele avisa o núcleo (a janela volta para
  // a bandeja, se estava lá), e o assistente vem depois
  abrirNovoTimerDaSessao(pedido, () => {
    fimDoDialogoDoDock();
    if (assistente) quandoLivre(() => primeiroUso({ semCriarTimers: !semTimers(snap) }));
  });
}

/** O dock pediu um "+ Novo timer…" com a janela já aberta. */
async function verNovoTimer() {
  await inicioBuscou;
  const pedido = await pedidoNovoTimer();
  // o fim do "Novo timer" (criado, cancelado ou descartado) avisa o núcleo
  if (pedido) abrirNovoTimerDaSessao(pedido, fimDoDialogoDoDock);
}

/**
 * Os diálogos pedidos pelo dock abrem aqui (rodada 16). Cada pedido avisa o núcleo uma vez, quando
 * o diálogo dele acaba; com o último, a janela volta a ficar como estava antes do primeiro
 * (escondida, ou fechada para a bandeja). O núcleo conta os pedidos (revisão: sem prazo).
 */
const fimDoDialogoDoDock = () =>
  chamar('dialogo_do_dock_fechou').catch((e) => console.error('worktimer: dialogo_do_dock_fechou', e));

/** O "Ligar a um timer" pedido pelo dock ("Escolher timer…", "Outro timer…", "Trocar"). */
async function verLigarSessao() {
  await inicioBuscou;
  let p = null;
  try {
    p = await chamar('ligar_sessao_pedido');
  } catch (e) {
    console.error('worktimer: ligar_sessao_pedido', e);
  }
  if (!p) return;
  const reserva = sessaoDeReserva(p.sessao_id, { nome: p.nome, cwd: p.cwd, timer_id: p.timer_id });
  const aoNovo = p.com_novo ? (sessaoId) => agir('pedir_novo_timer', { sessao_id: sessaoId }) : null;
  quandoLivre(async () => {
    await ligarATimer(lerSnap, p.sessao_id, aoNovo, reserva);
    // o "Ligar a um timer" acabou quando não sobra diálogo (o "+ Novo timer…" de dentro dele é
    // outro pedido, contado à parte)
    quandoSemDialogo(fimDoDialogoDoDock);
  });
}

function aoTeclar(ev) {
  if ((ev.key === 'Enter' || ev.key === ' ') && ev.target.matches('.corpo [data-timer][tabindex]')) {
    ev.preventDefault();
    selecionar(Number(ev.target.dataset.timer));
    return;
  }
  // a tecla N abre o "+ Novo" (rodada 20), com o foco fora de campo e sem diálogo nem tour
  if (snap && teclaDoNovo(ev, document.activeElement, dialogoAberto() || tourAtivo())) {
    const b = $('#bt-novo');
    if (!b) return;
    ev.preventDefault();
    alternarMenuNovo(b);
  }
}

// ---- início ----

nomearPagina(document);
document.addEventListener('click', aoClicar);
document.addEventListener('keydown', aoTeclar);
document.addEventListener('visibilitychange', ligarRelogio);

/** A leitura do estado falhou (o banco não abriu depois de trocar): o erro fica à vista. */
function erroDoEstado(e) {
  const el = $('#aviso');
  if (!el) return;
  if (e === null) {
    if (el.dataset.estado) {
      el.hidden = true;
      delete el.dataset.estado;
    }
    return;
  }
  clearTimeout(avisoId);
  el.dataset.estado = '1';
  el.textContent = typeof e === 'string' ? e : `Não deu para ler o estado do ${NOME_APP}.`;
  el.hidden = false;
}

aoMudar((s) => {
  snap = s;
  usarAtividades(s.atividades);
  usarCorte(s.config?.corte_min);
  usarEsperaApp(s.config?.espera_app_min);
  aplicarTema();
  if (visao === null) {
    visao = VISOES.includes(s.config.visao) ? s.config.visao : 'painel';
    mostrarVisao();
    // janela recriada pelo ⚙ do dock (depois de fechada para a bandeja): abre nos Ajustes
    if (querAjustes) abrirAjustes(secaoDoPedido(querAjustes));
    pedirAjustes();
  }
  desenhar();
  // com timers e os hooks desligados, a faixa "Ligar" (os timers podem ter acabado de nascer)
  mostrarFaixaHooks();
  if (!primeiroUsoVisto) {
    primeiroUsoVisto = true;
    // sem timers: o primeiro uso
    inicio(s);
    ligarFaixaTour(s);
  }
  verDicas(s);
}, erroDoEstado);
// a autoria no console do WebView (rodada 18)
console.info(linhaDoConsole());
// o ⚙ do dock abre a janela principal nos Ajustes (o pedido fica no núcleo até a página buscar)
window.__TAURI__.event.listen('abrir-ajustes', pedirAjustes);
// o X da janela: grava o pendente dos Ajustes e fecha, ou pergunta antes
window.__TAURI__.event.listen('pedir-fechar', ({ payload }) => aoPedirFechar(payload));
// o minimizar: grava o pendente e vai para a bandeja, ou pergunta antes
window.__TAURI__.event.listen('pedir-minimizar', ({ payload }) => aoPedirMinimizar(payload));
// o "+ Novo timer…" da pergunta do dock (o pedido fica no núcleo até a página buscar)
window.__TAURI__.event.listen('novo-timer', verNovoTimer);
// o "Ligar a um timer" pedido pelo dock (rodada 16), e o que fez a janela nascer
window.__TAURI__.event.listen('ligar-sessao', verLigarSessao);
verLigarSessao();
// um aviso do dock com o dock escondido (rodada 15): a faixa daqui; o "Desfazer" volta ao dock,
// que faz o que ele faz
window.__TAURI__.event.listen('aviso-do-dock', ({ payload: a }) =>
  faixa(a.texto, a.botao ?? null, a.botao ? () => chamar('aviso_clicado', { id: a.id }) : null, a.ms ?? 4000),
);
ligarFaixaHooks();
ligarRelogio();
