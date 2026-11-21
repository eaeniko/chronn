// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Visão "Foco": a lista por grupo à esquerda e o relógio grande do timer selecionado
// à direita, com os trechos do dia (mockup B).
import { relogio, relogioGrande, curto, totalDeTimers, horaDoDia, textoDoAjuste, minusculas } from './formato.js';
import {
  esc,
  cor,
  msDoTimer,
  cabecalhoGrupo,
  chaveGrupo,
  controles,
  destino,
  textoDoCard,
  marcaAjuste,
  marcaEncerrado,
  semTimerNenhum,
  linhasAtividades,
  atividadePorId,
  tituloDoTempo,
  etiquetaDesde,
  textoLembrete,
  motivoNaTela,
} from './componentes.js';
import { nomeDoGrupo } from './espacos.js';
import { TEXTOS } from './textos.js';

function item(timer, agora, selecionado) {
  const cls = ['fi', timer.rodando && 'run', selecionado && 'sel'].filter(Boolean).join(' ');
  return `<button class="${cls}" style="--c:${cor(timer.cor)}" data-acao="selecionar" data-timer="${timer.id}"${selecionado ? ' aria-current="true"' : ''}>
  <i></i><span class="nm">${esc(timer.nome)}</span><span class="t mono" data-tempo="${timer.id}"${tituloDoTempo(timer, agora)}>${relogio(msDoTimer(timer, agora))}</span>
</button>`;
}

function lista(snap, agora, fechados, selecionado) {
  const total = totalDeTimers(snap.grupos.flatMap((g) => g.timers), agora);
  const grupos = snap.grupos
    .map((g) => {
      const aberto = !fechados.has(chaveGrupo(g));
      const itens = aberto ? g.timers.map((t) => item(t, agora, t.id === selecionado)).join('') : '';
      return `<section class="fg${aberto ? '' : ' fechado'}">${cabecalhoGrupo({ ...g, nome: nomeDoGrupo(snap, g) }, agora, aberto, 'gh fgh')}${itens}</section>`;
    })
    .join('');
  return `<nav class="fs" data-rolagem="foco-lista" aria-label="${TEXTOS.timersRotulo}">
  <h4>${TEXTOS.hoje} · <span class="mono" data-total-dia>${curto(total)}</span></h4>
  ${grupos || semTimerNenhum()}
</nav>`;
}

/** Trechos do dia do timer, do mais cedo ao mais tarde, e depois os ajustes à mão. */
function trechos(timer, meiaNoite, agora) {
  const v = [...timer.trechos].sort((a, b) => a.inicio - b.inicio);
  const ajustes = (timer.ajustes ?? [])
    .map((a) => {
      const at = (a.atividade_id ?? 1) !== (timer.atividade_padrao_id ?? 1) ? atividadePorId(a.atividade_id) : null;
      const nome = at ? ` · ${esc(minusculas(at.nome))}` : '';
      return `<div class="sg sg-mao"><b>${TEXTOS.aMao}</b><span>${esc(a.motivo ?? TEXTOS.ajuste)}${nome}</span><em class="mono">${esc(textoDoAjuste(a.delta_ms))}</em></div>`;
    })
    .join('');
  if (!v.length && !ajustes) return `<p class="vazio">${TEXTOS.nadaContadoHojePonto}</p>`;
  return v
    .map((t) => {
      const fim = t.fim === null ? TEXTOS.agora : horaDoDia(t.fim, meiaNoite);
      const dur = curto((t.fim ?? agora) - t.inicio);
      // a atividade, quando não é a padrão do timer ("Play manual · reunião")
      const at = (t.atividade_id ?? 1) !== (timer.atividade_padrao_id ?? 1) ? atividadePorId(t.atividade_id) : null;
      const nome = at ? ` · ${esc(minusculas(at.nome))}` : '';
      return `<div class="sg${t.fim === null ? ' on' : ''}"><b class="mono">${horaDoDia(t.inicio, meiaNoite)} – ${fim}</b><span>${TEXTOS.origemFoco(t.origem) ?? esc(t.origem)}${nome}</span><em class="mono">${dur}</em></div>`;
    })
    .join('') + ajustes;
}

function principal(timer, grupo, meiaNoite, agora) {
  if (!timer) return `<div class="fm"><p class="vazio">${TEXTOS.crieUmTimer}</p></div>`;
  const { hm, s } = relogioGrande(msDoTimer(timer, agora));
  const motivo = timer.rodando && timer.motivo ? `<i></i>${motivoNaTela(timer, agora)}` : textoLembrete(timer, agora) || marcaEncerrado(timer);
  const caminho = grupo && grupo.id !== null ? `<span class="pt">${esc(grupo.nome)}</span>` : '';
  return `<div class="fm" data-rolagem="foco-relogio" style="--c:${cor(timer.cor)}">
  <div class="ft"><i></i><span class="nm">${esc(timer.nome)}</span>${caminho}<span class="sp"></span><span class="chip">${esc(textoDoCard(timer, grupo).texto)}</span></div>
  <div class="clk mono" data-relogio="${timer.id}">${hm}<span>:${s}</span></div>
  ${etiquetaDesde(timer, agora)}
  <div class="why">${motivo}</div>
  ${linhasAtividades(timer, agora)}
  ${marcaAjuste(timer)}
  ${controles(timer, agora)}
  <div class="segs"><h5>${TEXTOS.hoje}</h5>${trechos(timer, meiaNoite, agora)}</div>
  ${destino(timer, grupo)}
</div>`;
}

export function visaoFoco(snap, agora, fechados, timer, grupo, meiaNoite) {
  return `<div class="fv">${lista(snap, agora, fechados, timer?.id ?? null)}${principal(timer, grupo, meiaNoite, agora)}</div>`;
}
