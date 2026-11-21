// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Linha do tempo: a régua do dia (8h–19h, estendida se houver tempo fora dela).
// Blocos lisos ligaram sozinhos (Claude, leitura); listrados são play manual. O ajuste à mão de
// um timer aparece ao lado do nome ("+15 min"). Com mais de uma atividade no dia (Trabalho,
// Reunião...), cada bloco leva embaixo uma faixa na cor da atividade, com a legenda. Só leitura.
// A linha do tempo é do dia: os tempos aqui são os de hoje (`msDeHoje`), não o número do timer
// (o acumulado desde o zerar).
import { trechosDoDia, reguaDoDia, horaMin, atividadesDosTrechos, msDeHoje, minusculas } from './formato.js';
import { esc, cor, botaoPlay, ajusteDoTimer, atividadesAtuais } from './componentes.js';
import { gruposDaAba, comAbas, espacoPorId } from './espacos.js';
import { TEXTOS } from './textos.js';

/** A chave da linha do tempo do Painel no "abrir e fechar" dos grupos (rodada 21). */
export const CHAVE_LINHA = 'linha-do-tempo';

const H = 3_600_000;

/** "+15 min" ao lado do nome do timer (só "±" na faixa compacta), com os motivos na dica. */
function marca(t, curta = false) {
  const a = ajusteDoTimer(t);
  return a ? `<em class="mao" title="${esc(a.dica)}">${curta ? '±' : esc(a.texto)}</em>` : '';
}

/**
 * Régua comum aos timers dados; `diaInicio` é a meia-noite local do dia de trabalho. `ativs`: as
 * atividades com trecho nesses timers; com mais de uma, os blocos levam a faixa da atividade.
 */
function regua(timers, diaInicio, agora) {
  const { ini, fim } = reguaDoDia(timers.flatMap((t) => t.trechos), diaInicio, agora);
  const vao = fim - ini;
  const agoraNaRegua = agora >= diaInicio + ini * H && agora <= diaInicio + fim * H;
  const todas = atividadesAtuais();
  const ativs = atividadesDosTrechos(timers, todas);
  const pintar = ativs.length > 1;
  const corDe = (id) => cor(todas.find((a) => a.id === id)?.cor);
  return {
    ini,
    fim,
    vao,
    ativs: pintar ? ativs : [],
    pAgora: agoraNaRegua ? ((agora - diaInicio - ini * H) / (vao * H)) * 100 : null,
    blocos: (t) =>
      trechosDoDia(t, diaInicio, agora, ini, fim, todas)
        .map((b) => {
          const cls = [b.manual && 'm', pintar && 'at'].filter(Boolean).join(' ');
          const a = pintar ? `;--a:${corDe(b.atividade)}` : '';
          return `<b${cls ? ` class="${cls}"` : ''} title="${esc(b.titulo)}" style="left:${b.left.toFixed(3)}%;width:${b.width.toFixed(3)}%${a}"></b>`;
        })
        .join(''),
  };
}

/** A legenda das atividades (a faixa embaixo do bloco na cor de cada uma). */
const legendaDasAtividades = (ativs) =>
  ativs.map((a) => `<span><i class="sw at" style="--a:${cor(a.cor)}"></i>${esc(minusculas(a.nome))}</span>`).join('');

function marcas(ini, fim, passo) {
  const v = [];
  for (let h = ini; h < fim; h += passo) v.push(`<span style="left:${((h - ini) / (fim - ini)) * 100}%">${h % 24}h</span>`);
  return v.join('');
}

/** Faixa compacta do Painel: só os timers que tiveram tempo hoje. */
export function linhaCompacta(grupos, diaInicio, agora) {
  const timers = grupos.flatMap((g) => g.timers).filter((t) => msDeHoje(t, agora) >= 1000);
  if (!timers.length) return '';
  const r = regua(timers, diaInicio, agora);
  const linhas = timers
    .map((t) => `<div class="lb"><span>${esc(t.nome)}</span>${marca(t, true)}</div><div class="ln" style="--c:${cor(t.cor)}">${r.blocos(t)}</div>`)
    .join('');
  // a legenda das atividades fica embaixo da régua, fora dela (a linha do "agora" não a cruza)
  const leg = r.ativs.length ? `<div class="tl-leg">${legendaDasAtividades(r.ativs)}</div>` : '';
  return `<div class="tl">
  <div></div><div class="hrs mono">${marcas(r.ini, r.fim, r.vao > 14 ? 3 : 2)}</div>
  ${linhas}
  ${r.pAgora === null ? '' : `<div class="nw" style="--p:${r.pAgora / 100}"></div>`}
</div>${leg}`;
}

/**
 * A linha do tempo no fim do Painel (rodada 21): uma seção que abre e fecha como um grupo, com a
 * mesma seta (o estado fica lembrado, `CHAVE_LINHA`). Desligada em Ajustes → Painel
 * (`mostrar` falso), não aparece. Aberta e sem tempo hoje, diz "Nada contado hoje".
 */
export function secaoLinhaDoPainel(grupos, diaInicio, agora, { mostrar = true, aberta = true } = {}) {
  if (!mostrar) return '';
  // o cabeçalho sempre (aberta ou fechada); aberta e sem tempo hoje, "Nada contado hoje"
  const corpo = aberta ? linhaCompacta(grupos, diaInicio, agora) || `<p class="vazio">${esc(TEXTOS.nadaContadoHoje)}</p>` : '';
  return `<section class="grp lt-painel${aberta ? '' : ' fechado'}">
  <div class="gh"><button class="gd" data-acao="dobrar" data-grupo="${CHAVE_LINHA}" aria-expanded="${aberta}"><span class="fd" aria-hidden="true">${aberta ? '▾' : '▸'}</span><b>${esc(TEXTOS.linhaDoTempo)}</b></button></div>
  ${corpo}
</section>`;
}

/** Visão "Linha do tempo": uma linha por timer, por grupo, com o total e o play. */
export function visaoLinha(snap, diaInicio, agora, aba = null) {
  const grupos = gruposDaAba(snap, aba).filter((g) => g.timers.length);
  if (!grupos.length) return `<div class="lt-vazio"><p class="vazio">${esc(TEXTOS.nenhumTimerAinda)}</p></div>`;
  // em "Todos" com abas, o nome do espaço antes do primeiro grupo dele (rodada 20)
  const comEspacos = aba === null && comAbas(snap);
  const r = regua(grupos.flatMap((g) => g.timers), diaInicio, agora);
  const agoraEm = r.pAgora === null ? '' : `<i class="nw" style="left:${r.pAgora.toFixed(3)}%"></i>`;
  const linhas = grupos
    .map((g, i) => {
      const novoEspaco = comEspacos && (i === 0 || grupos[i - 1].espaco_id !== g.espaco_id);
      const esp = novoEspaco ? espacoPorId(snap, g.espaco_id) : null;
      const cabEspaco = esp ? `<div class="lt-esp" style="--e:${cor(esp.cor)}"><i></i>${esc(esp.nome)}</div>` : '';
      const titulo = `${cabEspaco}<div class="lt-g">${esc(g.nome)}</div>`;
      const timers = g.timers
        .map((t) => {
          const ms = msDeHoje(t, agora);
          return `<div class="lt-lb" style="--c:${cor(t.cor)}"><i></i><span>${esc(t.nome)}</span>${marca(t)}</div>
  <div class="lt-ln${t.rodando ? ' on' : ''}" style="--c:${cor(t.cor)}">${r.blocos(t)}${agoraEm}</div>
  <div class="lt-tt mono${ms < 1000 ? ' zero' : ''}" data-hm-hoje="${t.id}" title="${TEXTOS.hoje}">${horaMin(ms)}</div>
  <div class="lt-pl" style="--c:${cor(t.cor)}">${botaoPlay(t, t.rodando ? 'pl on' : 'pl')}</div>`;
        })
        .join('');
      return titulo + timers;
    })
    .join('');
  return `<div class="lt" data-rolagem="linha" style="--horas:${r.vao}">
  <div class="lt-grade">
    <div></div><div class="lt-hrs mono">${marcas(r.ini, r.fim, r.vao > 16 ? 2 : 1)}</div><div></div><div></div>
    ${linhas}
  </div>
  <div class="lt-leg">
    <span><i class="sw"></i>${TEXTOS.legLigouSozinho}</span>
    <span><i class="sw m"></i>${TEXTOS.legDeuPlay}</span>
    ${grupos.some((g) => g.timers.some(ajusteDoTimer)) ? `<span><em class="mao">±</em>${TEXTOS.legAjustado}</span>` : ''}
    ${legendaDasAtividades(r.ativs)}
    <span class="ag">│ ${TEXTOS.agora}</span>
  </div>
</div>`;
}
