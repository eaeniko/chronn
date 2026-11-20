// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Dicas soltas (rodada 16): cada uma aparece uma vez só, na primeira vez que a pessoa encontra
// a coisa no uso. Uma por vez, nunca durante o tour nem com um diálogo aberto; a que coincidir
// espera a vez. Na janela principal, um balão pequeno junto à parte da tela (o mesmo
// posicionamento do tour); o que acontece no dock (com o dock à vista) vai na janela de aviso.
// Das duas janelas, mostra a que pedir primeiro ao núcleo (`dica_reivindicar`).
import { ladoDoBalao } from './tour.js';

const minutos = (s) => Math.max(1, Math.round((Number(s) || 600) / 60));

/** As dicas. `alvo(snap)`: a parte da tela na janela principal; `texto(snap)`: o que diz. */
export const DICAS = {
  claude_contando: {
    texto: () => 'Este timer anda sozinho quando o Claude Code trabalha numa pasta ligada a ele.',
    alvo: (snap) => cartaoCom(snap, (t) => t.situacao === 'trabalhando'),
  },
  pergunta: {
    texto: () => 'Uma conversa nova do Claude começou. Escolha aqui em qual timer ela conta.',
    alvo: () => '#perguntas',
  },
  esperando: {
    texto: () => 'O Claude terminou e o timer parou de contar. A etiqueta “esperando você” lembra que ele espera sua resposta.',
    alvo: (snap) => cartaoCom(snap, (t) => t.situacao === 'esperando'),
  },
  inatividade: {
    texto: (snap) =>
      `Depois de ${minutos(snap?.config?.inatividade_s)} min sem mexer no PC, os timers pausam e o app pergunta o que fazer com o tempo fora. Esse tempo muda em Ajustes → Tempos (no modo desenvolvedor).`,
    alvo: () => '#perguntas',
  },
  dock_expande: {
    texto: () => 'Arraste o dock para onde quiser. O − recolhe, e o ícone na bandeja esconde ou mostra o dock.',
    alvo: () => null,
    soNoDock: true,
  },
  zerar: {
    texto: () => 'Zerar não apaga nada: o histórico do dia continua.',
    alvo: () => '#bt-zerar-todos',
  },
  manual_mais: {
    texto: () => 'O ▶ manual e o Claude contam juntos neste timer. O tempo não conta em dobro.',
    alvo: (snap) => cartaoCom(snap, (t) => t.situacao === 'manual' && Boolean(t.situacao_claude)),
  },
};

const todosTimers = (snap) => (snap?.grupos ?? []).flatMap((g) => g.timers ?? []);

function cartaoCom(snap, f) {
  const t = todosTimers(snap).find(f);
  return t ? `#grupos .k[data-timer="${t.id}"]` : null;
}

/**
 * As dicas que o estado de agora dispara (as coisas que estão acontecendo): o Claude contando
 * num timer, a pergunta de sessão nova ou de conversa nova, o "esperando você", a pausa por
 * inatividade e o "manual + …". O ↺ e o dock expandindo vêm das ações (`oferecer`).
 */
export function gatilhos(snap) {
  const ids = [];
  const timers = todosTimers(snap);
  const perguntas = snap?.perguntas ?? [];
  if (timers.some((t) => t.situacao === 'trabalhando')) ids.push('claude_contando');
  if (perguntas.some((p) => p.tipo === 'sessao_nova' || p.tipo === 'conversa_nova')) ids.push('pergunta');
  if (timers.some((t) => t.situacao === 'esperando')) ids.push('esperando');
  if (perguntas.some((p) => p.tipo === 'volta_inatividade')) ids.push('inatividade');
  if (timers.some((t) => t.situacao === 'manual' && t.situacao_claude)) ids.push('manual_mais');
  return ids;
}

/**
 * A fila das dicas de uma janela: uma por vez, na ordem em que chegaram. Quem decide a vez é o
 * núcleo, para o app inteiro (revisão da rodada 16).
 * - `ligadas()` e `vistas()`: a config de agora (desligadas, nada entra; vista, não volta);
 * - `podeMostrar()`: esta janela pode mostrar agora (sem isso, a dica espera; `tentar()` depois);
 * - `reivindicar(id)`: o núcleo responde "mostrar", "esperar" (outra dica à vista, o tour, um
 *   diálogo) ou "nao" (desligadas, vista);
 * - `mostrar(id)`: mostra e resolve com "entendi", "desligar", "venceu" (o prazo) ou
 *   "interrompida" (fechada no meio: o tour, um diálogo, outra tela);
 * - `concluir(id, vista)`: avisa o núcleo; a interrompida não fica vista e volta para a fila.
 */
export function criarFila({ ligadas, vistas, podeMostrar, reivindicar, mostrar, concluir }) {
  const fila = [];
  let mostrando = null;
  const f = {
    get mostrando() {
      return mostrando;
    },
    get fila() {
      return [...fila];
    },
    oferecer(id) {
      if (!DICAS[id] || !ligadas() || vistas().includes(id) || fila.includes(id) || mostrando === id) return;
      fila.push(id);
      return f.tentar();
    },
    async tentar() {
      while (!mostrando && fila.length && ligadas() && podeMostrar()) {
        const id = fila[0];
        if (vistas().includes(id)) {
          fila.shift();
          continue;
        }
        mostrando = id;
        let vez = 'esperar';
        try {
          vez = await reivindicar(id);
        } catch {
          vez = 'esperar';
        }
        if (vez === 'esperar') {
          // a vez é de outra dica (ou da principal ocupada): tenta de novo depois
          mostrando = null;
          return;
        }
        fila.shift();
        if (vez !== 'mostrar') {
          mostrando = null;
          continue;
        }
        let r = 'interrompida';
        try {
          r = await mostrar(id);
        } finally {
          const vista = r !== 'interrompida';
          mostrando = null;
          if (!vista && !fila.includes(id)) fila.unshift(id);
          await concluir(id, vista);
        }
      }
      // desligadas no meio do caminho: as que esperavam saem
      if (!ligadas()) fila.length = 0;
    },
  };
  return f;
}

/** O balão de uma dica (o texto e os dois botões). */
export const htmlDaDica = (texto) =>
  `<p id="dica-t">${String(texto).replace(/[&<>"]/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;' })[c])}</p>
<div class="dica-pe"><button type="button" class="dica-nao" data-dica="desligar">Não mostrar mais dicas</button><button type="button" class="bt pri" data-dica="entendi">Entendi</button></div>`;

/** A dica à vista na janela principal (para fechar no meio). */
let balaoAtual = null;

/**
 * Fecha a dica à vista sem contar como vista (o tour começou, um diálogo abriu, a tela mudou):
 * ela volta depois. `id`: só se for esta.
 */
export function fecharDicaAVista(id = null) {
  if (balaoAtual && (id === null || balaoAtual.id === id)) balaoAtual.fechar('interrompida');
}

/**
 * Mostra a dica na janela principal, junto à parte da tela (`seletor`; sem ela à vista, no centro),
 * e resolve com "entendi" ou "desligar" quando a pessoa clica, ou "interrompida" (`fecharDicaAVista`).
 */
export function balaoDeDica(texto, seletor, id = null) {
  return new Promise((ok) => {
    const el = document.createElement('div');
    el.className = 'dica';
    el.setAttribute('role', 'dialog');
    el.setAttribute('aria-labelledby', 'dica-t');
    el.innerHTML = htmlDaDica(texto);
    document.body.append(el);
    const posicionar = () => {
      const alvo = seletor ? document.querySelector(seletor) : null;
      const b = alvo && !alvo.closest('[hidden]') ? alvo.getBoundingClientRect() : null;
      const r = b && b.width > 0 && b.height > 0 ? b : null;
      const t = el.getBoundingClientRect();
      const p = ladoDoBalao(r, { w: t.width, h: t.height }, { w: window.innerWidth, h: window.innerHeight }, 10);
      el.dataset.lado = p.lado;
      Object.assign(el.style, { left: `${p.x}px`, top: `${p.y}px` });
    };
    posicionar();
    const relogio = setInterval(posicionar, 1000);
    window.addEventListener('resize', posicionar);
    const fechar = (r) => {
      clearInterval(relogio);
      window.removeEventListener('resize', posicionar);
      el.remove();
      if (balaoAtual?.el === el) balaoAtual = null;
      ok(r);
    };
    balaoAtual = { id, el, fechar };
    el.addEventListener('click', (ev) => {
      const b = ev.target.closest('[data-dica]');
      if (b) fechar(b.dataset.dica);
    });
  });
}
