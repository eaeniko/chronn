// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// O tour da janela principal (rodada 16): um destaque em cada parte real da tela e um balão ao
// lado dele. Todo passo tem Voltar, Próximo (no último, "Concluir"), "Pular tour", o X e o Esc;
// as setas ← → e o Enter também andam. Fechado ou concluído, grava `tour_visto` e não volta
// sozinho. Sem a parte na tela (sem timer, sem pergunta, painel fechado), o passo aparece no
// centro, com a mesma explicação: o tour nunca quebra nem pula sozinho.
import { chamar } from './api.js';
import { NOME_APP } from './nome.js';
import { TEXTOS } from './textos.js';

/**
 * Um passo com o título e o texto no idioma de agora (rodada 23): `grupo` e `id` acham o par
 * [título, texto] em `TEXTOS.tour` (o caminho dos mini-tours é "mini.<tela>"). `extra`: o alvo e
 * o resto.
 */
function passo(grupo, id, extra = {}) {
  const par = () => grupo.split('.').reduce((o, k) => o[k], TEXTOS.tour)[id];
  return {
    id,
    get titulo() {
      return par()[0];
    },
    get texto() {
      const t = par()[1];
      return typeof t === 'function' ? t(NOME_APP) : t;
    },
    ...extra,
  };
}

/** Um passo com título e texto vindos de duas chaves do `TEXTOS` (os da rodada 20 e 22). */
const passoDe = (id, titulo, texto, extra = {}) => ({
  id,
  get titulo() {
    return TEXTOS[titulo];
  },
  get texto() {
    return TEXTOS[texto];
  },
  ...extra,
});

/** O passo do dock: com o texto do dock escondido (o modo "Só quando houver uma pergunta"). */
const PASSO_DOCK = Object.defineProperty(passo('completo', 'dock', { alvo: null, dock: true }), 'textoEscondido', {
  enumerable: true,
  get: () => TEXTOS.tour.completo.dock[2],
});

/**
 * O tour completo (rodada 16, 8 passos; 9 com o "+ Novo" da rodada 20; 10 com o menu das telas
 * da rodada 22), na ordem. `alvo`: o seletor da parte da tela; `dock`: o
 * dock pisca neste passo (com o dock escondido, nada pisca; revisão).
 */
export const PASSOS = [
  passo('completo', 'cartao', { alvo: '#grupos .k' }),
  passo('completo', 'etiquetas', { alvo: '#grupos .k .kx' }),
  passo('completo', 'hoje', { alvo: '#total-dia' }),
  // rodada 22: as telas foram para um menu ("Painel ▾"); o passo aponta para ele
  passoDe('telas', 'tourTelasTitulo', 'tourTelasTexto', { alvo: '#bt-telas' }),
  passo('completo', 'perguntas', { alvo: '#perguntas' }),
  passo('completo', 'painel', { alvo: '#foco' }),
  PASSO_DOCK,
  passoDe('novo', 'tourNovoTitulo', 'tourNovoTexto', { alvo: '#bt-novo' }),
  passo('completo', 'zerar', { alvo: '#bt-zerar-todos' }),
  passo('completo', 'ajustes', { alvo: '#bt-ajustes' }),
];

/**
 * O tour essencial (rodada 17, 5 passos): o que o "Fazer o tour", a faixa "Novo: um tour…" e o
 * "Como começar" do ? oferecem. `semClaude`: o texto e o botão do passo quando o Claude Code não
 * está ligado.
 */
export const PASSOS_ESSENCIAL = [
  passo('essencial', 'cartao', { alvo: '#grupos .k' }),
  passo('essencial', 'claude', { alvo: '#grupos .k .kx' }),
  passo('essencial', 'pastas', {
    alvo: '#perguntas',
    semClaude: {
      get texto() {
        return TEXTOS.tour.essencial.pastasSemClaude[0];
      },
      acao: 'ligar-claude',
      get botao() {
        return TEXTOS.tour.essencial.pastasSemClaude[1];
      },
    },
  }),
  PASSO_DOCK,
  passo('essencial', 'ajuda', { alvo: '#bt-tour' }),
];

/** O nome de cada tela, como o topo e os Ajustes mostram (no idioma de agora). */
export const NOMES_DAS_TELAS = {
  get painel() {
    return TEXTOS.telaPainel;
  },
  get foco() {
    return TEXTOS.telaFoco;
  },
  get cartoes() {
    return TEXTOS.telaCartoes;
  },
  get linha() {
    return TEXTOS.linhaDoTempo;
  },
  get ajustes() {
    return TEXTOS.telaAjustes;
  },
};

/** Os mini-tours de cada tela (o "Esta tela" do ?), de 2 a 4 passos. */
export const MINI_TOURS = {
  painel: [
    passo('mini.painel', 'grupos', { alvo: '#grupos' }),
    passoDe('abas', 'tourAbasTitulo', 'tourAbasTexto', { alvo: '#abas .abas', soComAbas: true }),
    passoDe('novo', 'tourNovoTitulo', 'tourNovoTexto', { alvo: '#bt-novo' }),
    passo('mini.painel', 'pausar-tudo', { alvo: '#bt-pausar-tudo' }),
    passo('mini.painel', 'painel', { alvo: '#foco' }),
  ],
  foco: [passo('mini.foco', 'relogio', { alvo: '#v-foco .fm' }), passo('mini.foco', 'lista', { alvo: '#v-foco .fs' })],
  cartoes: [passo('mini.cartoes', 'cartoes', { alvo: '#v-cartoes .k' }), passo('mini.cartoes', 'zoho', { alvo: '#v-cartoes .k .ds' })],
  linha: [
    passo('mini.linha', 'trechos', { alvo: '#v-linha .lt-ln' }),
    passo('mini.linha', 'cores', { alvo: '#v-linha .lt-leg' }),
    passo('mini.linha', 'ajuste', { alvo: '#v-linha .mao' }),
  ],
  ajustes: [
    passo('mini.ajustes', 'dock', { alvo: '#aj-dock' }),
    passo('mini.ajustes', 'claude', { alvo: '#aj-claude' }),
    passo('mini.ajustes', 'grupos', { alvo: '#aj-grupos' }),
    passo('mini.ajustes', 'ajuda', { alvo: '#aj-ajuda' }),
  ],
};

/** Os itens do menu do ? para a tela `visao`. */
export function itensDoMenuAjuda(visao) {
  const nome = NOMES_DAS_TELAS[visao] ?? NOMES_DAS_TELAS.painel;
  return [
    { id: 'essencial', rotulo: TEXTOS.comoComecar },
    { id: 'tela', rotulo: TEXTOS.estaTela(nome) },
    { id: 'completo', rotulo: TEXTOS.tourCompleto },
  ];
}

/** O menu do ? (o HTML). */
export const htmlDoMenuAjuda = (visao) =>
  `<div class="menu-ajuda" role="menu" aria-label="${esc(TEXTOS.ajudaMenu)}">${itensDoMenuAjuda(visao)
    .map((it) => `<button type="button" role="menuitem" data-ajuda="${it.id}">${esc(it.rotulo)}</button>`)
    .join('')}</div>`;

/**
 * Os passos de cada tour: `essencial`, `completo` ou `tela` (o mini-tour da tela `visao`). O passo
 * das abas de espaço (rodada 20) só entra com `abas` (2 ou mais espaços).
 */
export function passosDoTour(tipo, visao = 'painel', { abas = false } = {}) {
  if (tipo === 'completo') return PASSOS;
  if (tipo === 'tela') return (MINI_TOURS[visao] ?? MINI_TOURS.painel).filter((p) => abas || !p.soComAbas);
  return PASSOS_ESSENCIAL;
}

/**
 * Onde fica o balão (`{ w, h }`) junto ao alvo (um retângulo `{ left, top, right, bottom }`, ou
 * nulo), dentro da janela (`{ w, h }`): abaixo, acima, à direita ou à esquerda, o primeiro lado em
 * que ele cabe inteiro; sem alvo, ou sem lado que caiba, no centro. Devolve `{ lado, x, y }`.
 */
export function ladoDoBalao(alvo, balao, janela, margem = 12) {
  const { w, h } = balao;
  const centro = { lado: 'centro', x: Math.max(0, Math.round((janela.w - w) / 2)), y: Math.max(0, Math.round((janela.h - h) / 2)) };
  if (!alvo) return centro;
  const prender = (v, min, max) => Math.round(Math.min(Math.max(v, min), Math.max(min, max)));
  const meioX = (alvo.left + alvo.right) / 2;
  const meioY = (alvo.top + alvo.bottom) / 2;
  const xCentrado = prender(meioX - w / 2, margem, janela.w - w - margem);
  const yCentrado = prender(meioY - h / 2, margem, janela.h - h - margem);
  const cabeLargura = w <= janela.w - 2 * margem;
  const cabeAltura = h <= janela.h - 2 * margem;
  if (cabeLargura && alvo.bottom + margem + h <= janela.h - margem) return { lado: 'abaixo', x: xCentrado, y: Math.round(alvo.bottom + margem) };
  if (cabeLargura && alvo.top - margem - h >= margem) return { lado: 'acima', x: xCentrado, y: Math.round(alvo.top - margem - h) };
  if (cabeAltura && alvo.right + margem + w <= janela.w - margem) return { lado: 'direita', x: Math.round(alvo.right + margem), y: yCentrado };
  if (cabeAltura && alvo.left - margem - w >= margem) return { lado: 'esquerda', x: Math.round(alvo.left - margem - w), y: yCentrado };
  return centro;
}

function esc(s) {
  return String(s ?? '').replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c]);
}

/** O passo mostra o "sem Claude Code"? (o Claude Code não está ligado e o passo tem a troca) */
const semClaudeNoPasso = (p, { claudeLigado = true } = {}) => Boolean(p.semClaude) && claudeLigado === false;

/**
 * O texto do passo (`dockEscondido`: o dock está no modo "Só quando houver uma pergunta";
 * `claudeLigado` falso: o texto do "sem Claude Code").
 */
export const textoDoPasso = (p, contexto = {}) =>
  (semClaudeNoPasso(p, contexto) && p.semClaude.texto) || (contexto.dockEscondido && p.textoEscondido) || p.texto;

/**
 * O balão do passo `i` (`contexto.passos`: o tour; o completo, sem ele): título, texto, "Passo n de
 * N", o X, "Pular tour", Voltar e Próximo, e o botão do "sem Claude Code" quando é o caso.
 */
export function htmlDoBalao(i, contexto = {}) {
  const passos = contexto.passos ?? PASSOS;
  const p = passos[i];
  const ultimo = i === passos.length - 1;
  const acao = semClaudeNoPasso(p, contexto)
    ? `<div class="tour-acao"><button type="button" class="bt" data-tour="acao" data-acao="${esc(p.semClaude.acao)}">${esc(p.semClaude.botao)}</button></div>`
    : '';
  return `<button type="button" class="tour-x" data-tour="fechar" title="${esc(TEXTOS.fecharTour)}" aria-label="${esc(TEXTOS.fecharTour)}">×</button>
<small class="tour-n">${esc(TEXTOS.passoNDe(i + 1, passos.length))}</small>
<h3 id="tour-t">${esc(p.titulo)}</h3>
<p id="tour-d">${esc(textoDoPasso(p, contexto))}</p>
${acao}<div class="tour-pe"><button type="button" class="tour-pular" data-tour="pular">${esc(TEXTOS.pularTour)}</button><span class="sp"></span><button type="button" class="bt" data-tour="voltar"${i === 0 ? ' disabled' : ''}>${esc(TEXTOS.voltarBt)}</button><button type="button" class="bt pri" data-tour="proximo">${esc(ultimo ? TEXTOS.concluir : TEXTOS.proximo)}</button></div>`;
}

/**
 * A lógica do tour, sem a tela: `desenhar(i)` mostra o passo (nulo: some), `gravar()` grava
 * `tour_visto` e `piscarDock(sim)` liga o piscar do dock no passo dele (`semPiscar`: o dock está
 * escondido, nada pisca). `pausar()`/`retomar()`: um diálogo veio por cima (revisão).
 */
export function criarTour({ desenhar, gravar, piscarDock, semPiscar = false, passos = PASSOS }) {
  let i = 0;
  let aberto = true;
  let piscando = false;
  const piscar = (sim) => {
    if (sim !== piscando) {
      piscando = sim;
      piscarDock(sim);
    }
  };
  const t = {
    get passo() {
      return i;
    },
    get aberto() {
      return aberto;
    },
    ir(n) {
      if (!aberto) return;
      i = Math.max(0, Math.min(passos.length - 1, n));
      piscar(passos[i].dock === true && !semPiscar);
      desenhar(i);
    },
    /** Um diálogo veio por cima: o dock para de piscar (o passo fica guardado). */
    pausar() {
      piscar(false);
    },
    /** O diálogo fechou: volta no mesmo passo. */
    retomar() {
      return t.ir(i);
    },
    proximo() {
      if (i === passos.length - 1) return t.fechar();
      return t.ir(i + 1);
    },
    voltar() {
      return t.ir(i - 1);
    },
    /** Concluir, pular, o X e o Esc: some e grava `tour_visto`. */
    fechar() {
      if (!aberto) return null;
      aberto = false;
      piscar(false);
      desenhar(null);
      return gravar();
    },
    /** Uma tecla: Esc fecha, → e Enter andam, ← volta. Devolve se a tecla era do tour. */
    tecla(k) {
      if (k === 'Escape') t.fechar();
      else if (k === 'ArrowRight' || k === 'Enter') t.proximo();
      else if (k === 'ArrowLeft') t.voltar();
      else return false;
      return true;
    },
  };
  return t;
}

/** A parte da tela do passo, se está à vista agora (sem ela, o passo vai para o centro). */
function alvoNaTela(passo) {
  if (!passo.alvo) return null;
  const el = document.querySelector(passo.alvo);
  if (!el || el.closest('[hidden]') || !el.childElementCount && !el.textContent.trim()) return null;
  const r = el.getBoundingClientRect();
  return r.width > 0 && r.height > 0 ? el : null;
}

let ativo = null;
/** Redesenha o passo à vista (o tour aberto e sem diálogo por cima). */
let redesenhar = () => {};

/** O tour está na tela? (as dicas esperam ele acabar) */
export const tourAtivo = () => ativo !== null;

/** O estado mudou (o Claude Code acabou de ser ligado): o passo à vista se redesenha. */
export const redesenharTour = () => redesenhar();

/**
 * Abre o tour no primeiro passo (um de cada vez). Já grava `tour_visto`: o tour começado não
 * volta sozinho, nem se a janela fechar no meio (revisão). `dockEscondido`: o dock está no modo
 * "Só quando houver uma pergunta" (o passo dele diz isso e nada pisca).
 */
export function iniciarTour({ passos = PASSOS, dockEscondido = false, claudeLigado = true, marcarVisto = true, aoAcao = null } = {}) {
  if (ativo) return;
  // o essencial e o completo gravam `tour_visto`; o mini-tour de uma tela, não
  const gravar = marcarVisto
    ? () => chamar('marcar_tour_visto').catch((e) => console.error('worktimer: marcar_tour_visto', e))
    : () => {};
  gravar();
  const antes = document.activeElement;
  const camada = document.createElement('div');
  camada.className = 'tour';
  camada.innerHTML =
    '<div class="tour-luz" hidden></div><div class="tour-balao" role="dialog" aria-modal="true" aria-labelledby="tour-t" aria-describedby="tour-d"></div>';
  const luz = camada.querySelector('.tour-luz');
  const balao = camada.querySelector('.tour-balao');
  let passoAtual = 0;

  const posicionar = () => {
    const el = alvoNaTela(passos[passoAtual]);
    const janela = { w: window.innerWidth, h: window.innerHeight };
    let r = null;
    if (el) {
      const b = el.getBoundingClientRect();
      // o destaque com uma folga em volta, sem sair da janela
      r = {
        left: Math.max(4, b.left - 6),
        top: Math.max(4, b.top - 6),
        right: Math.min(janela.w - 4, b.right + 6),
        bottom: Math.min(janela.h - 4, b.bottom + 6),
      };
      Object.assign(luz.style, { left: `${r.left}px`, top: `${r.top}px`, width: `${r.right - r.left}px`, height: `${r.bottom - r.top}px` });
    }
    luz.hidden = !el;
    camada.classList.toggle('sem-alvo', !el);
    const tam = balao.getBoundingClientRect();
    const p = ladoDoBalao(r, { w: tam.width, h: tam.height }, janela);
    balao.dataset.lado = p.lado;
    Object.assign(balao.style, { left: `${p.x}px`, top: `${p.y}px` });
  };

  const tour = criarTour({
    desenhar(i) {
      if (i === null) return sair();
      passoAtual = i;
      alvoNaTela(passos[i])?.scrollIntoView({ block: 'nearest', inline: 'nearest' });
      // o estado do Claude Code é relido a cada desenho (revisão: ligado pelo botão do passo, o
      // passo das pastas passa a explicar as pastas)
      const ligado = typeof claudeLigado === 'function' ? claudeLigado() : claudeLigado;
      balao.innerHTML = htmlDoBalao(i, { passos, dockEscondido, claudeLigado: ligado });
      posicionar();
      balao.querySelector('[data-tour="proximo"]').focus();
    },
    gravar,
    piscarDock: (sim) => chamar('dock_piscar', { ligado: sim }).catch(() => {}),
    semPiscar: dockEscondido,
    passos,
  });
  // a janela fecha (ou recarrega) no meio: o dock para de piscar
  const aoSairDaPagina = () => chamar('dock_piscar', { ligado: false }).catch(() => {});
  window.addEventListener('pagehide', aoSairDaPagina);

  const teclado = (ev) => {
    // Enter num botão do balão é o clique dele
    if (ev.key === 'Enter' && ev.target.closest?.('.tour-balao button')) return;
    if (ev.key === 'Tab') {
      // o foco fica no balão
      const bts = [...balao.querySelectorAll('button:not([disabled])')];
      const k = bts.indexOf(document.activeElement);
      ev.preventDefault();
      bts[(k + (ev.shiftKey ? bts.length - 1 : 1)) % bts.length]?.focus();
      return;
    }
    if (tour.tecla(ev.key)) {
      ev.preventDefault();
      ev.stopPropagation();
    }
  };
  const aoClicar = (ev) => {
    const b = ev.target.closest('[data-tour]');
    if (!b) return;
    const acao = b.dataset.tour;
    if (acao === 'proximo') tour.proximo();
    else if (acao === 'voltar') tour.voltar();
    // o botão do passo (o "Conectar o Claude Code"): abre o diálogo por cima, e o tour pausa
    else if (acao === 'acao') aoAcao?.(b.dataset.acao);
    else tour.fechar();
  };
  // a janela mudou de tamanho, ou a tela foi redesenhada: o balão e o destaque acompanham
  const aoRedimensionar = () => {
    if (!pausado) posicionar();
  };
  const relogio = setInterval(aoRedimensionar, 1000);
  // Um diálogo (o "Ligar a um timer" pedido pelo dock, por exemplo) veio por cima: o tour some e
  // solta o teclado; quando o diálogo fecha, volta no mesmo passo (revisão).
  let pausado = false;
  const comDialogo = () => Boolean(document.querySelector('body > .dlg-fundo'));
  const conferirDialogo = () => {
    if (comDialogo() && !pausado) {
      pausado = true;
      camada.hidden = true;
      document.removeEventListener('keydown', teclado, true);
      tour.pausar();
    } else if (!comDialogo() && pausado && tour.aberto) {
      pausado = false;
      camada.hidden = false;
      document.addEventListener('keydown', teclado, true);
      tour.retomar();
    }
  };
  redesenhar = () => {
    if (!pausado && tour.aberto) tour.ir(tour.passo);
  };
  const observador = new MutationObserver(conferirDialogo);
  observador.observe(document.body, { childList: true });
  function sair() {
    redesenhar = () => {};
    clearInterval(relogio);
    observador.disconnect();
    window.removeEventListener('pagehide', aoSairDaPagina);
    window.removeEventListener('resize', aoRedimensionar);
    document.removeEventListener('keydown', teclado, true);
    camada.remove();
    ativo = null;
    if (antes instanceof HTMLElement && document.contains(antes)) antes.focus({ preventScroll: true });
  }
  camada.addEventListener('click', aoClicar);
  window.addEventListener('resize', aoRedimensionar);
  document.addEventListener('keydown', teclado, true);
  document.body.append(camada);
  ativo = tour;
  tour.ir(0);
  // um diálogo já aberto (pedido pelo dock enquanto o tour começava): começa pausado
  conferirDialogo();
}

/** A faixa "Novo: um tour…" para quem já usa o app e nunca viu o tour (com timers). */
export const mostrarFaixaTour = (config, temTimers) => Boolean(temTimers) && config?.tour_visto !== true;
