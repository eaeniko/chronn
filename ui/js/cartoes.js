// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Visão "Cartões": a grade por grupo, sem o lado direito (mockup A).
// Cartão maior, com o destino das horas (e "auto" quando ligou sozinho) e ↺ (zerar) / ✎ à mão.
import { relogio, totalDeTimers, tempoPorAtividade, msDeHoje, hojeDesdeZerar } from './formato.js';
import {
  esc,
  cor,
  msDoTimer,
  botaoPlay,
  botaoEncerrar,
  secaoGrupo,
  chaveGrupo,
  semTimerNenhum,
  rotuloDoDestino,
  destinoDoGrupo,
  ajusteDoTimer,
  botaoReuniao,
  divisaoDoTimer,
  atividadesAtuais,
  etiquetaReuniao,
  tituloDoTempo,
  botaoZerar,
  botaoMenuTimer,
  etiquetaSituacao,
} from './componentes.js';
import { gruposDaAba, secoesDaAba } from './espacos.js';
import { TEXTOS } from './textos.js';

/** Linha curta do destino no cartão (o selo do grupo já diz o card padrão por extenso). */
function destinoCurto(timer, grupo) {
  if (timer.link_card) return TEXTOS.destinoCard(rotuloDoDestino(timer.link_card, timer.link_card_nome));
  const g = destinoDoGrupo(grupo);
  if (g) return g.tipo === 'card' ? TEXTOS.destinoCardPadrao(g.rotulo) : TEXTOS.destinoProjeto(g.rotulo);
  return TEXTOS.semCardAinda;
}

function cartaoGrande(timer, grupo, agora, selecionado) {
  const ms = msDoTimer(timer, agora);
  const cls = ['kc', timer.rodando && 'on', selecionado && 'sel', !timer.rodando && ms < 1000 && 'zero']
    .filter(Boolean)
    .join(' ');
  // o "auto": o timer ligou sozinho (a dica pelo motivo, que o núcleo manda como chave)
  const dicaAuto = timer.rodando ? TEXTOS.motivoAuto(timer.motivo) : null;
  const auto = dicaAuto ? `<span class="au" title="${esc(dicaAuto)}">${TEXTOS.auto}</span> · ` : '';
  // os ajustes de hoje que entram no número (os feitos depois do zerar)
  const aj = ajusteDoTimer(hojeDesdeZerar(timer));
  const mao = aj ? `<span class="mao" title="${esc(aj.dica)}">${esc(TEXTOS.comAMao(aj.texto))}</span> · ` : '';
  // a fatia de cada atividade no número do timer, só com mais de uma (com a divisão na dica)
  const at = tempoPorAtividade(timer, atividadesAtuais(), agora);
  const faixa =
    at.length > 1
      ? `<div class="kat" title="${esc(divisaoDoTimer(timer, agora).chave)}" aria-hidden="true">${at
          .map((a) => `<i style="flex-grow:${a.ms};background:${cor(a.cor)}"></i>`)
          .join('')}</div>`
      : '';
  return `<div class="${cls}" style="--c:${cor(timer.cor)}" data-timer="${timer.id}" tabindex="0">
  <div class="h"><i></i><span class="nm">${esc(timer.nome)}</span>${botaoMenuTimer(timer)}${botaoReuniao(timer, 'pe')}</div>
  ${etiquetaReuniao(timer, agora)}
  <div class="ds">${etiquetaSituacao(timer, agora)}${auto}${mao}${esc(destinoCurto(timer, grupo))}</div>
  <div class="r">
    <span class="tm mono" data-tempo="${timer.id}"${tituloDoTempo(timer, agora)}>${relogio(ms)}</span>
    <span class="ac">
      ${botaoZerar(timer, 'mi', agora)}
      <button class="mi" data-acao="editar" data-timer="${timer.id}" title="${TEXTOS.editar}" aria-label="${esc(TEXTOS.doTimer(TEXTOS.editar, timer.nome))}">✎</button>
    </span>
    ${botaoEncerrar(timer, 'pe')}
    ${botaoPlay(timer, 'pl')}
  </div>
  ${faixa}
</div>`;
}

/** Barra com a fatia de cada timer no total do dia (o tempo de hoje, sem olhar o zerar). */
function barraDoDia(timers, agora) {
  const total = totalDeTimers(timers, agora);
  if (total < 1000) return '<div class="kb" aria-hidden="true"></div>';
  const partes = timers
    .map((t) => ({ t, ms: msDeHoje(t, agora) }))
    .filter((x) => x.ms >= 1000)
    .sort((a, b) => b.ms - a.ms)
    .map((x) => `<i style="flex-grow:${x.ms};background:${cor(x.t.cor)}" title="${esc(x.t.nome)}"></i>`)
    .join('');
  return `<div class="kb" aria-hidden="true">${partes}</div>`;
}

/**
 * A visão Cartões da aba `aba` (rodada 20: nulo é "Todos"; com abas, "Todos" põe o nome de cada
 * espaço acima dos grupos dele). A barra do dia é a da aba. As abas ficam no cabeçalho (rodada 21).
 */
export function visaoCartoes(snap, agora, fechados, selecionado, aba = null) {
  const grupos = gruposDaAba(snap, aba);
  const desenhar = (g) =>
    secaoGrupo(g, agora, !fechados.has(chaveGrupo(g)), selecionado, (t) => cartaoGrande(t, g, agora, t.id === selecionado));
  const corpo = grupos.length ? secoesDaAba(snap, aba, desenhar, agora) : semTimerNenhum();
  return `<div class="kv" data-rolagem="cartoes">
  ${barraDoDia(grupos.flatMap((g) => g.timers), agora)}
  ${corpo}
</div>`;
}
