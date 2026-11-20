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
  novoGrupoBotao,
  novoTimerBotao,
  rotuloDoDestino,
  destinoDoGrupo,
  ajusteDoTimer,
  botaoReuniao,
  divisaoDoTimer,
  atividadesAtuais,
  etiquetaReuniao,
  tituloDoTempo,
  botaoZerar,
  etiquetaSituacao,
} from './componentes.js';

/** Motivo → dica do "auto" (o timer ligou sozinho). */
const AUTO = {
  'Claude trabalhando': 'ligou sozinho (Claude trabalhando)',
  'reunião · Claude trabalhando': 'o Claude ligou sozinho durante a reunião',
  'você lendo': 'ligou sozinho (você lendo a resposta)',
  'você escrevendo': 'ligou sozinho (você lendo a resposta e escrevendo)',
};

/** Linha curta do destino no cartão (o selo do grupo já diz o card padrão por extenso). */
function destinoCurto(timer, grupo) {
  if (timer.link_card) return `card: ${rotuloDoDestino(timer.link_card, timer.link_card_nome)}`;
  const g = destinoDoGrupo(grupo);
  if (g) return `${g.tipo === 'card' ? 'card padrão' : 'projeto'}: ${g.rotulo}`;
  return 'sem card ainda';
}

function cartaoGrande(timer, grupo, agora, selecionado) {
  const ms = msDoTimer(timer, agora);
  const cls = ['kc', timer.rodando && 'on', selecionado && 'sel', !timer.rodando && ms < 1000 && 'zero']
    .filter(Boolean)
    .join(' ');
  const auto = timer.rodando && AUTO[timer.motivo] ? `<span class="au" title="${AUTO[timer.motivo]}">auto</span> · ` : '';
  // os ajustes de hoje que entram no número (os feitos depois do zerar)
  const aj = ajusteDoTimer(hojeDesdeZerar(timer));
  const mao = aj ? `<span class="mao" title="${esc(aj.dica)}">${esc(aj.texto)} à mão</span> · ` : '';
  // a fatia de cada atividade no número do timer, só com mais de uma (com a divisão na dica)
  const at = tempoPorAtividade(timer, atividadesAtuais(), agora);
  const faixa =
    at.length > 1
      ? `<div class="kat" title="${esc(divisaoDoTimer(timer, agora).chave)}" aria-hidden="true">${at
          .map((a) => `<i style="flex-grow:${a.ms};background:${cor(a.cor)}"></i>`)
          .join('')}</div>`
      : '';
  return `<div class="${cls}" style="--c:${cor(timer.cor)}" data-timer="${timer.id}" tabindex="0">
  <div class="h"><i></i><span class="nm">${esc(timer.nome)}</span>${botaoReuniao(timer, 'pe')}</div>
  ${etiquetaReuniao(timer, agora)}
  <div class="ds">${etiquetaSituacao(timer, agora)}${auto}${mao}${esc(destinoCurto(timer, grupo))}</div>
  <div class="r">
    <span class="tm mono" data-tempo="${timer.id}"${tituloDoTempo(timer, agora)}>${relogio(ms)}</span>
    <span class="ac">
      ${botaoZerar(timer, 'mi', agora)}
      <button class="mi" data-acao="editar" data-timer="${timer.id}" title="Editar" aria-label="Editar ${esc(timer.nome)}">✎</button>
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

export function visaoCartoes(snap, agora, fechados, selecionado) {
  const grupos = snap.grupos;
  const corpo = grupos.length
    ? grupos
        .map((g) =>
          secaoGrupo(g, agora, !fechados.has(chaveGrupo(g)), selecionado, (t) =>
            cartaoGrande(t, g, agora, t.id === selecionado),
          ),
        )
        .join('')
    : `<p class="vazio">Nenhum timer ainda.</p><div class="grid">${novoTimerBotao(null)}</div>`;
  return `<div class="kv" data-rolagem="cartoes">
  ${barraDoDia(grupos.flatMap((g) => g.timers), agora)}
  ${corpo}
  ${novoGrupoBotao()}
</div>`;
}
