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
 * O tour completo (rodada 16, 8 passos; 9 com o "+ Novo" da rodada 20; 10 com o menu das telas
 * da rodada 22), na ordem. `alvo`: o seletor da parte da tela; `dock`: o
 * dock pisca neste passo.
 */
export const PASSOS = [
  {
    id: 'cartao',
    titulo: 'O cartão do timer',
    texto: '▶ conta à mão, ⏸ pausa e ■ encerra. Clique no cartão para abrir o painel dele.',
    alvo: '#grupos .k',
  },
  {
    id: 'etiquetas',
    titulo: 'As etiquetas',
    texto: '“Claude” aparece quando o Claude Code trabalha numa pasta ligada ao timer. Há também “esperando você”, “segundo plano” e “manual + …”.',
    alvo: '#grupos .k .kx',
  },
  {
    id: 'hoje',
    titulo: 'O total de hoje',
    texto: 'Quanto você trabalhou hoje. Passe o mouse em cima para ver de onde vem o número.',
    alvo: '#total-dia',
  },
  {
    // rodada 22: as telas foram para um menu ("Painel ▾"); o passo aponta para ele
    id: 'telas',
    titulo: TEXTOS.tourTelasTitulo,
    texto: TEXTOS.tourTelasTexto,
    alvo: '#bt-telas',
  },
  {
    id: 'perguntas',
    titulo: 'As perguntas',
    texto: 'Numa conversa nova do Claude, o app pergunta em qual timer contar. A pergunta aparece aqui e no dock.',
    alvo: '#perguntas',
  },
  {
    id: 'painel',
    titulo: 'O painel do timer',
    texto: 'Mostra as sessões do Claude ligadas ao timer, cada uma com o ✎. O “Ajustar hoje” fica no editar do timer.',
    alvo: '#foco',
  },
  {
    id: 'dock',
    titulo: 'O dock',
    texto: 'A janela pequena sempre à vista, com o que está rodando (ela está piscando agora). Dá para arrastar e expandir; em Ajustes → Dock, ela pode aparecer só para as perguntas.',
    // com o dock no modo "Só quando houver uma pergunta" (escondido): nada pisca (revisão)
    textoEscondido: 'A janela pequena que mostra o que está rodando. Agora ela aparece só quando há uma pergunta; em Ajustes → Dock, dá para deixá-la sempre à vista.',
    alvo: null,
    dock: true,
  },
  {
    id: 'novo',
    titulo: TEXTOS.tourNovoTitulo,
    texto: TEXTOS.tourNovoTexto,
    alvo: '#bt-novo',
  },
  {
    id: 'zerar',
    titulo: 'Zerar',
    texto: '↺ zera um timer e “Zerar todos” zera todos. Nada sai do histórico.',
    alvo: '#bt-zerar-todos',
  },
  {
    id: 'ajustes',
    titulo: 'Ajustes ⚙',
    texto: 'Inatividade, dock, lembretes e Conexões (o Claude Code). Em Ajuda, dá para ver este tour de novo.',
    alvo: '#bt-ajustes',
  },
];

/** O passo do dock (o mesmo nos dois tours). */
const PASSO_DOCK = PASSOS.find((p) => p.id === 'dock');

/**
 * O tour essencial (rodada 17, 5 passos): o que o "Fazer o tour", a faixa "Novo: um tour…" e o
 * "Como começar" do ? oferecem. `semClaude`: o texto e o botão do passo quando o Claude Code não
 * está ligado.
 */
export const PASSOS_ESSENCIAL = [
  {
    id: 'cartao',
    titulo: 'O cartão do timer',
    texto: '▶ conta à mão e ⏸ pausa. Clique no cartão para abrir o painel dele.',
    alvo: '#grupos .k',
  },
  {
    id: 'claude',
    titulo: 'Apps conectados contam sozinhos',
    texto: 'Um app conectado conta sozinho quando trabalha numa pasta do timer. Com o Claude Code, aparece a etiqueta “Claude”, e “esperando você” quando ele termina.',
    alvo: '#grupos .k .kx',
  },
  {
    id: 'pastas',
    titulo: 'As pastas',
    texto: 'Uma conversa nova do Claude pergunta aqui em qual timer contar. A pasta de cada timer fica no editar timer, em “Pastas deste timer”.',
    alvo: '#perguntas',
    semClaude: {
      texto: 'O Claude Code ainda não está conectado: sem ele, os timers só contam à mão. Conectado, cada pasta conta no timer dela.',
      acao: 'ligar-claude',
      botao: 'Conectar o Claude Code',
    },
  },
  PASSO_DOCK,
  {
    id: 'ajuda',
    titulo: 'Mais ajuda',
    texto: `Em cada tela, o ? explica o que ela mostra. O tour completo fica em Ajustes → Ajuda. Tudo roda neste computador: o ${NOME_APP} não envia nada para lugar nenhum.`,
    alvo: '#bt-tour',
  },
];

/** O nome de cada tela, como o topo e os Ajustes mostram. */
export const NOMES_DAS_TELAS = { painel: 'Painel', foco: 'Foco', cartoes: 'Cartões', linha: 'Linha do tempo', ajustes: 'Ajustes' };

/** Os mini-tours de cada tela (o "Esta tela" do ?), de 2 a 4 passos. */
export const MINI_TOURS = {
  painel: [
    { id: 'grupos', titulo: 'Grupos e cartões', texto: 'Cada grupo junta os timers de um projeto. O cartão mostra o tempo de hoje e os botões ▶ e ⏸.', alvo: '#grupos' },
    { id: 'abas', titulo: TEXTOS.tourAbasTitulo, texto: TEXTOS.tourAbasTexto, alvo: '#abas .abas', soComAbas: true },
    { id: 'novo', titulo: TEXTOS.tourNovoTitulo, texto: TEXTOS.tourNovoTexto, alvo: '#bt-novo' },
    { id: 'pausar-tudo', titulo: 'Pausar tudo', texto: 'Para de uma vez todos os timers que estão contando (com abas, os da aba).', alvo: '#bt-pausar-tudo' },
    { id: 'painel', titulo: 'O painel do timer', texto: 'O timer escolhido, à direita: as atividades, o ajuste de hoje e as sessões do Claude ligadas a ele.', alvo: '#foco' },
  ],
  foco: [
    { id: 'relogio', titulo: 'O timer grande', texto: 'O tempo de hoje do timer escolhido, com os trechos do dia logo abaixo.', alvo: '#v-foco .fm' },
    { id: 'lista', titulo: 'A lista', texto: 'Escolha outro timer aqui; o total de hoje fica no topo da lista.', alvo: '#v-foco .fs' },
  ],
  cartoes: [
    { id: 'cartoes', titulo: 'Os cartões', texto: 'Um cartão por timer, com o tempo de hoje, ↺ zerar e ✎ editar.', alvo: '#v-cartoes .k' },
    { id: 'zoho', titulo: 'O Zoho', texto: 'Embaixo do nome, o card do Zoho para onde as horas vão. O link do card fica no editar timer (✎).', alvo: '#v-cartoes .k .ds' },
  ],
  linha: [
    { id: 'trechos', titulo: 'Os trechos', texto: 'Cada barra é um trecho de hoje, na hora em que aconteceu, por atividade.', alvo: '#v-linha .lt-ln' },
    { id: 'cores', titulo: 'As cores', texto: 'A legenda diz o que cada cor e cada risco querem dizer: ligou sozinho, você deu play.', alvo: '#v-linha .lt-leg' },
    { id: 'ajuste', titulo: 'O ± do ajuste', texto: 'O ± marca o timer com tempo ajustado à mão hoje.', alvo: '#v-linha .mao' },
  ],
  ajustes: [
    { id: 'dock', titulo: 'Dock e lembretes', texto: 'Dock: quando ele aparece. Lembretes no dock: quanto tempo o “esperando você” e o pausado ficam à vista.', alvo: '#aj-dock' },
    { id: 'claude', titulo: 'Conexões e inatividade', texto: 'Conexões: o Claude Code conectado conta sozinho. Inatividade: depois de quanto tempo parado os timers pausam (em Tempos, no modo desenvolvedor).', alvo: '#aj-claude' },
    { id: 'grupos', titulo: 'Grupos, regras e backups', texto: 'Grupos e regras de pasta: o que entra sozinho em cada timer. Backups: as cópias do banco, para voltar atrás.', alvo: '#aj-grupos' },
    { id: 'ajuda', titulo: 'Ajuda', texto: 'As dicas na tela e os tours, a qualquer hora.', alvo: '#aj-ajuda' },
  ],
};

/** Os itens do menu do ? para a tela `visao`. */
export function itensDoMenuAjuda(visao) {
  const nome = NOMES_DAS_TELAS[visao] ?? NOMES_DAS_TELAS.painel;
  return [
    { id: 'essencial', rotulo: 'Como começar' },
    { id: 'tela', rotulo: `Esta tela: ${nome}` },
    { id: 'completo', rotulo: 'Tour completo' },
  ];
}

/** O menu do ? (o HTML). */
export const htmlDoMenuAjuda = (visao) =>
  `<div class="menu-ajuda" role="menu" aria-label="Ajuda">${itensDoMenuAjuda(visao)
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
  return `<button type="button" class="tour-x" data-tour="fechar" title="Fechar o tour" aria-label="Fechar o tour">×</button>
<small class="tour-n">Passo ${i + 1} de ${passos.length}</small>
<h3 id="tour-t">${esc(p.titulo)}</h3>
<p id="tour-d">${esc(textoDoPasso(p, contexto))}</p>
${acao}<div class="tour-pe"><button type="button" class="tour-pular" data-tour="pular">Pular tour</button><span class="sp"></span><button type="button" class="bt" data-tour="voltar"${i === 0 ? ' disabled' : ''}>Voltar</button><button type="button" class="bt pri" data-tour="proximo">${ultimo ? 'Concluir' : 'Próximo'}</button></div>`;
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
