// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Peças da interface. Cada função devolve HTML (texto); os cliques são tratados
// por delegação em quem monta a tela, pelos atributos data-acao / data-timer / data-grupo.
import { relogio, relogioGrande, curto, horaMin, msDoTimer, contasAcumuladas, hojeDesdeZerar, desdeTexto, totalDoGrupo, rotuloDeLink, rotuloDoDestino, textoDoAjuste, podeEncerrar, tempoPorAtividade, linhasDeAtividade, itensDoMenuZerar, trocarTexto, situacaoNaTela, etiquetaComposta, ehLembrete, SITUACAO, TRABALHO, REUNIAO, minusculas } from './formato.js';
import { TEXTOS } from './textos.js';

export { msDoTimer, rotuloDeLink, rotuloDoDestino, podeEncerrar };

/**
 * As atividades do último snapshot (`snap.atividades`): quem desenha chama `usarAtividades` a
 * cada estado novo, e as peças daqui (o 👥, a divisão por atividade) leem delas.
 */
let atividades = [];
export function usarAtividades(lista) {
  atividades = Array.isArray(lista) ? lista : [];
}
export const atividadesAtuais = () => atividades;
/** As que não estão arquivadas (as que dá para escolher). */
export const atividadesAtivas = () => atividades.filter((a) => !a.arquivada);
export const atividadePorId = (id) => atividades.find((a) => a.id === id) ?? null;

/**
 * O corte do dia do último snapshot (`snap.config.corte_min`), para o "desde hoje/ontem" seguir o
 * dia de trabalho: quem desenha chama `usarCorte` a cada estado novo, como `usarAtividades`.
 */
let corteMin = 0;
export function usarCorte(min) {
  corteMin = Number.isFinite(min) ? min : 0;
}
export const corteAtual = () => corteMin;

/**
 * Quanto tempo o "esperando você" fica à vista no app e no dock expandido (`snap.config.espera_app_min`;
 * 0 = até a virada do dia; rodada 13): `usarEsperaApp` a cada estado novo.
 */
let esperaAppMin = 0;
export function usarEsperaApp(min) {
  esperaAppMin = Number.isFinite(min) ? min : 0;
}
export const esperaAppAtual = () => esperaAppMin;

/**
 * O HTML de uma etiqueta (`etiquetaComposta`), com a cor de cada parte (classes `s-<estado>`).
 * Com mais de `max` letras, a segunda parte vira um ponto na cor dela, com a etiqueta inteira na
 * dica (rodada 14). `sem1`: sem a primeira parte (o cartão com a reunião já tem a etiqueta dela).
 */
export function htmlEtiqueta(e, max = Infinity, sem1 = false) {
  if (!e) return '';
  const p1 = sem1 ? '' : `<span class="s-${e.p1.k}">${esc(e.p1.texto)}</span>`;
  if (!e.p2) return p1;
  // sem a primeira parte, só a segunda ("+ esperando você") conta no espaço
  const tamanho = sem1 ? e.p2.texto.length + 2 : e.texto.length;
  if (tamanho > max) return `${p1}<span class="pt2 s-${e.p2.k}" title="${esc(e.texto)}"></span>`;
  return `${p1}<span class="mais">${sem1 ? '+ ' : ' + '}</span><span class="s-${e.p2.k}">${esc(e.p2.texto)}</span>`;
}

/**
 * A etiqueta da situação no cartão (rodadas 13 e 14): "manual", "esperando você", "segundo
 * plano", "trabalhando", "lendo"..., e as duas partes com o manual ("manual + esperando você").
 * Com a reunião, a etiqueta dela (`etiquetaReuniao`) mais a segunda parte. O lembrete some depois
 * do tempo dos Ajustes. `max`: o cartão estreito do Painel põe a segunda parte num ponto.
 */
export function etiquetaSituacao(timer, agora, max = Infinity) {
  const e = etiquetaComposta(timer, agora, esperaAppMin);
  if (!e) return '';
  const reuniao = e.p1.k === 'reuniao';
  if (reuniao && !e.p2) return '';
  const k = e.p2 ? `${e.p1.k}_${e.p2.k}` : e.p1.k;
  return `<small class="ksit ${k}" title="${esc(e.texto)}">${htmlEtiqueta(e, max, reuniao)}</small>`;
}

/** O lembrete por extenso no lugar do motivo (painel do timer e Foco), com o timer parado. */
export function textoLembrete(timer, agora) {
  const k = situacaoNaTela(timer, agora, esperaAppMin);
  return !timer.rodando && ehLembrete(k) ? `<span class="lembrete ${k}">${esc(SITUACAO[k].longa)}</span>` : '';
}

/**
 * O motivo do painel do timer (e do Foco) rodando: com o manual e um estado do Claude junto, as
 * duas partes ("manual + esperando você"; rodada 14); senão, o motivo do núcleo.
 */
export function motivoNaTela(timer, agora) {
  const e = etiquetaComposta(timer, agora, esperaAppMin, 'longa');
  if (e?.p2) return `<span class="etq">${htmlEtiqueta(e)}</span>`;
  return esc(TEXTOS.motivo(timer.motivo));
}

const ESC = { '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' };

export const esc = (s) => String(s ?? '').replace(/[&<>"']/g, (c) => ESC[c]);

/** Só cores hex entram no atributo style. */
export const cor = (c) => (/^#[0-9a-f]{3,8}$/i.test(c ?? '') ? c : '#8b8e98');

/**
 * Ajuste à mão de hoje do timer: { texto: "+15 min", dica: "ajustado à mão: +15 min · reunião" },
 * ou nulo sem ajuste (ou se os ajustes se anulam).
 */
export function ajusteDoTimer(timer) {
  const ms = timer.ajuste_ms ?? 0;
  if (!ms) return null;
  const motivos = (timer.ajustes ?? []).map((a) => a.motivo).filter(Boolean);
  const texto = textoDoAjuste(ms);
  return { texto, dica: `${TEXTOS.ajustadoAMao(texto)}${motivos.length ? ` · ${motivos.join('; ')}` : ''}` };
}

/**
 * "ajustado à mão: +15 min" (painel do timer), com os motivos na dica: os ajustes de hoje que
 * entram no número do timer (os feitos depois do zerar).
 */
export function marcaAjuste(timer) {
  const a = ajusteDoTimer(hojeDesdeZerar(timer));
  return a ? `<div class="mao" title="${esc(a.dica)}">${esc(TEXTOS.ajustadoAMao(a.texto))}</div>` : '';
}

const ICONE = {
  play: '<svg viewBox="0 0 10 10" aria-hidden="true"><path d="M2.6 1.4v7.2c0 .4.4.6.7.4l5.6-3.6a.5.5 0 0 0 0-.8L3.3 1c-.3-.2-.7 0-.7.4z"/></svg>',
  pausar: '<svg viewBox="0 0 10 10" aria-hidden="true"><rect x="2" y="1.4" width="2.2" height="7.2" rx=".6"/><rect x="5.8" y="1.4" width="2.2" height="7.2" rx=".6"/></svg>',
  encerrar: '<svg viewBox="0 0 10 10" aria-hidden="true"><rect x="1.9" y="1.9" width="6.2" height="6.2" rx="1"/></svg>',
  // 👥: duas pessoas, na cor do texto (discreto, como os outros ícones)
  reuniao:
    '<svg viewBox="0 0 12 10" aria-hidden="true"><circle cx="4.2" cy="2.7" r="1.9"/><path d="M.6 9.2c0-2 1.6-3.5 3.6-3.5s3.6 1.5 3.6 3.5z"/><circle cx="8.6" cy="3.1" r="1.6" opacity=".75"/><path d="M8.3 5.9c1.8 0 3.1 1.3 3.1 3.3H8.7c0-1.3-.4-2.4-1.2-3.1.3-.1.5-.2.8-.2z" opacity=".75"/></svg>',
  // Trabalho: um notebook
  trabalho:
    '<svg viewBox="0 0 12 10" aria-hidden="true"><path d="M2.2 1h7.6c.4 0 .7.3.7.7V7H1.5V1.7c0-.4.3-.7.7-.7zm.5 1.2v3.6h6.6V2.2z"/><path d="M0 7.7h12c0 .7-.5 1.3-1.2 1.3H1.2C.5 9 0 8.4 0 7.7z"/></svg>',
  // Estudo: um livro aberto
  estudo:
    '<svg viewBox="0 0 12 10" aria-hidden="true"><path d="M5.4 2.1C4.3 1.3 2.7 1 .8 1.1c-.3 0-.5.2-.5.5v6.2c0 .3.2.5.5.5 1.8-.1 3.3.2 4.6 1zM6.6 2.1c1.1-.8 2.7-1.1 4.6-1 .3 0 .5.2.5.5v6.2c0 .3-.2.5-.5.5-1.8-.1-3.3.2-4.6 1z"/></svg>',
  // as outras atividades: uma etiqueta
  atividade:
    '<svg viewBox="0 0 12 10" aria-hidden="true"><path d="M1 1.6C1 1.3 1.3 1 1.6 1h4.2c.2 0 .3.1.4.2l4.5 3.4c.3.2.3.6 0 .8L6.2 8.8c-.1.1-.3.2-.4.2H1.6c-.3 0-.6-.3-.6-.6zM3.2 4a.9.9 0 1 0 0 1.8.9.9 0 0 0 0-1.8z"/></svg>',
};

/** O ícone da atividade: Trabalho, Reunião e Estudo têm o seu; as outras, uma etiqueta. */
const iconeDaAtividade = (id) =>
  id === TRABALHO ? ICONE.trabalho : id === REUNIAO ? ICONE.reuniao : id === 3 ? ICONE.estudo : ICONE.atividade;

export function botaoPlay(timer, classe) {
  const acao = timer.rodando ? 'pausar' : 'play';
  const dica = timer.rodando ? TEXTOS.pausar : TEXTOS.comecar;
  return `<button class="${classe}" data-acao="${acao}" data-timer="${timer.id}" title="${dica}" aria-label="${esc(TEXTOS.doTimer(dica, timer.nome))}">${ICONE[acao]}</button>`;
}

/**
 * 👥 Reunião, ao lado do ▶: liga um play manual com a atividade Reunião (com o ▶ ligado, troca
 * o trabalho pela reunião no mesmo instante). Com a reunião rodando, fica aceso e apertá-lo de
 * novo encerra a reunião (o ▶ que ela substituiu volta). Não aparece com a atividade Reunião
 * arquivada, nem no timer em que ela já é a padrão (o ▶ faz o mesmo).
 */
export function botaoReuniao(timer, classe) {
  const r = atividadePorId(REUNIAO);
  if (!r || r.arquivada || timer.atividade_padrao_id === REUNIAO) return '';
  const nome = esc(r.nome);
  const min = minusculas(nome);
  const on = timer.manual_atividade_id === REUNIAO;
  const dica = on
    ? TEXTOS.reuniaoDicaOn(min)
    : timer.manual_atividade_id
      ? TEXTOS.reuniaoDicaTroca(nome, min)
      : TEXTOS.reuniaoDica(nome, min);
  const cor = /^#[0-9a-f]{3,8}$/i.test(r.cor ?? '') ? ` style="--a:${r.cor}"` : '';
  return `<button class="${classe} reu${on ? ' on' : ''}" data-acao="${on ? 'parar-reuniao' : 'reuniao'}" data-timer="${timer.id}" title="${dica}" aria-label="${TEXTOS.comTimer(on ? TEXTOS.encerrarAtividade(min) : nome, esc(timer.nome))}" aria-pressed="${on}"${cor}>${ICONE.reuniao}</button>`;
}

/**
 * A divisão do timer por atividade desde o zerar ("2h trabalho · 1h 10min reunião"), com a cor de
 * cada uma; só quando o timer teve mais de uma atividade. `chave` é o texto, para o relógio só
 * redesenhar quando ele muda.
 */
export function divisaoDoTimer(timer, agora) {
  const v = tempoPorAtividade(timer, atividades, agora);
  if (v.length < 2) return { chave: '', html: '' };
  const chave = v.map((a) => `${curto(a.ms)} ${minusculas(a.nome)}`).join(' · ');
  const html = v
    .map((a) => `<span><i style="--a:${cor(a.cor)}"></i>${esc(curto(a.ms))} ${esc(minusculas(a.nome))}</span>`)
    .join('<em>·</em>');
  return { chave, html };
}

/**
 * A sublinha da divisão por atividade (vazia com uma atividade só): na linha do timer do dock
 * expandido (`classe` "dx-at").
 */
export function linhaDivisao(timer, agora, classe = 'divat') {
  const d = divisaoDoTimer(timer, agora);
  return `<div class="${classe}" data-divisao="${timer.id}" data-chave="${esc(d.chave)}" title="${esc(dicaDoTempo(timer, agora, TEXTOS.porAtividade))}">${d.html}</div>`;
}

/**
 * A dica do número do timer: "desde ontem 14:20" (o último zerar), com `antes` na frente se
 * dado ("Por atividade, desde ontem 14:20"). Nos cartões e no dock, a etiqueta "desde …" fica só
 * aqui; no painel do timer ela aparece embaixo do relógio grande.
 */
export function dicaDoTempo(timer, agora, antes = '') {
  const d = desdeTexto(timer.zerado_em, agora, corteMin);
  return [antes, d].filter(Boolean).join(', ');
}

/** O atributo ` title="desde …"` do número do timer (nada sem zerar). */
export function tituloDoTempo(timer, agora) {
  const d = dicaDoTempo(timer, agora);
  return d ? ` title="${esc(d)}"` : '';
}

/**
 * As linhas por atividade do painel do timer, embaixo do relógio grande (que mostra o total da
 * tarefa, a soma delas): cor e ícone, o tempo desde o zerar, a bolinha acesa com "rodando", o ▶/⏸
 * dela e um ✎ que abre o "Ajustar hoje" já nela (o ajuste vale para o dia e, feito depois do
 * zerar, entra no número). A reunião rodando fica destacada na cor dela. A
 * atividade arquivada (com tempo hoje) aparece sem os botões.
 */
export function linhasAtividades(timer, agora) {
  const v = linhasDeAtividade(timer, atividades, agora);
  if (!v.length) return '';
  const nomeT = esc(timer.nome);
  const linhas = v.map((a) => {
    const nome = esc(a.nome);
    const min = minusculas(nome);
    const cls = ['lat', a.rodando && 'on', a.rodando && a.id === REUNIAO && 'reu', a.arquivada && 'arq'].filter(Boolean).join(' ');
    const acao = a.rodando ? 'pausar-atividade' : 'play-atividade';
    const dica = a.rodando ? TEXTOS.pausarAtividade(min) : TEXTOS.comecarAtividade(min);
    const botoes = a.arquivada
      ? ''
      : `<button class="lp" data-acao="${acao}" data-timer="${timer.id}" data-atividade="${a.id}" title="${dica}" aria-label="${TEXTOS.comTimer(dica, nomeT)}">${ICONE[a.rodando ? 'pausar' : 'play']}</button>` +
        `<button class="lz" data-acao="zerar-atividade" data-timer="${timer.id}" data-atividade="${a.id}" title="${TEXTOS.zerarSo(nome)}" aria-label="${TEXTOS.comTimer(TEXTOS.zerarSo(nome), nomeT)}"${a.ms >= 1000 ? '' : ' hidden'}>↺</button>` +
        `<button class="le" data-acao="ajustar-atividade" data-timer="${timer.id}" data-atividade="${a.id}" title="${TEXTOS.ajustarHojeAtividade(min)}" aria-label="${TEXTOS.comTimer(TEXTOS.ajustarHojeAtividade(min), nomeT)}">✎</button>`;
    return `<div class="${cls}" style="--a:${cor(a.cor)}"><i class="pt" aria-hidden="true"></i><span class="lic">${iconeDaAtividade(a.id)}</span><span class="nm">${nome}</span>${a.rodando ? `<em class="st">${TEXTOS.rodandoEtq}</em>` : ''}<span class="t mono" data-at="${timer.id}:${a.id}">${relogio(a.ms)}</span>${botoes}</div>`;
  });
  return `<div class="lats" role="group" aria-label="${TEXTOS.porAtividadeDesdeZerar}">${linhas.join('')}</div>`;
}

/**
 * Cartões (Painel e Cartões) com a reunião rodando: a etiqueta "reunião" na cor dela e o tempo
 * dela desde o zerar, menor ("reunião 0:42"). O tempo principal do cartão é o total da tarefa.
 */
export function etiquetaReuniao(timer, agora) {
  if (timer.manual_atividade_id !== REUNIAO) return '';
  const r = atividadePorId(REUNIAO);
  const ms = contasAcumuladas(timer, agora).porAtividade.get(REUNIAO) ?? 0;
  const nome = esc(minusculas(r?.nome ?? TEXTOS.reuniaoMin));
  return `<div class="kreu" style="--a:${cor(r?.cor)}" title="${esc(dicaDoTempo(timer, agora, TEXTOS.tempoDela(nome)))}"><b>${nome}</b><span class="mono" data-at-hm="${timer.id}:${REUNIAO}">${horaMin(ms)}</span></div>`;
}

/**
 * A cada segundo: os tempos por atividade (`data-at`, "1:02:03", e `data-at-hm`, "1:02") e as
 * sublinhas da divisão por atividade (só quando o texto muda) dentro de `raiz`.
 */
export function andarAtividades(raiz, timers, agora) {
  for (const t of timers) {
    const marcas = raiz.querySelectorAll(`[data-at^="${t.id}:"], [data-at-hm^="${t.id}:"], [data-divisao="${t.id}"]`);
    if (!marcas.length) continue;
    const { porAtividade } = contasAcumuladas(t, agora);
    for (const el of marcas) {
      if (el.dataset.divisao !== undefined) {
        const d = divisaoDoTimer(t, agora);
        if (el.dataset.chave !== d.chave) {
          el.dataset.chave = d.chave;
          el.innerHTML = d.html;
        }
        continue;
      }
      const hm = el.dataset.atHm !== undefined;
      const ms = porAtividade.get(Number((hm ? el.dataset.atHm : el.dataset.at).split(':')[1])) ?? 0;
      trocarTexto(el, hm ? horaMin(ms) : relogio(ms));
      // o ↺ da linha só com tempo desde o zerar (rodada 19)
      const z = hm ? null : el.parentElement?.querySelector?.('.lz');
      if (z && z.hidden !== ms < 1000) z.hidden = ms < 1000;
    }
  }
}

/**
 * ■ Encerrar, ao lado do play/pause. Sem nada para encerrar (parado, sem lembrete), não aparece;
 * com `fixo`, fica apagado no lugar (onde a fileira de botões não deve pular).
 */
export function botaoEncerrar(timer, classe, fixo = false) {
  const pode = podeEncerrar(timer);
  if (!pode && !fixo) return '';
  // a dica do ■: o que ele faz e o que desfaz
  const dica = pode ? TEXTOS.dicaEncerrar : timer.encerrado ? TEXTOS.dicaEncerrado : TEXTOS.nadaParaEncerrar;
  return `<button class="${classe} enc" data-acao="encerrar" data-timer="${timer.id}" title="${esc(dica)}" aria-label="${esc(TEXTOS.doTimer(TEXTOS.encerrar, timer.nome))}"${pode ? '' : ' disabled'}>${ICONE.encerrar}</button>`;
}

/** Texto embaixo do relógio do timer encerrado (parado). */
export const textoEncerrado = (timer) =>
  !timer.rodando && timer.encerrado ? TEXTOS.textoEncerrado : '';

/** O texto de encerrado, apagado (no lugar do motivo, que vem na cor do timer). */
export function marcaEncerrado(timer) {
  const t = textoEncerrado(timer);
  return t ? `<span class="encerrado">${esc(t)}</span>` : '';
}

/**
 * A marca de ajuste no cartão do Painel ("+30 min à mão", com os motivos na dica), como na visão
 * Cartões: os ajustes de hoje que entram no número (rodada 12). `curta` ("+30 min"): ao lado da
 * etiqueta da reunião, onde não cabe o resto.
 */
export function marcaAjusteCartao(timer, curta = false) {
  const a = ajusteDoTimer(hojeDesdeZerar(timer));
  return a ? `<em class="mao" title="${esc(a.dica)}">${esc(curta ? a.texto : TEXTOS.comAMao(a.texto))}</em>` : '';
}

/**
 * A linha embaixo do nome no cartão do Painel: a reunião rodando, a situação (rodada 13) e o
 * ajuste de hoje (vazia sem nenhum).
 */
function linhaExtra(timer, agora) {
  const reu = etiquetaReuniao(timer, agora);
  // o cartão estreito: com mais de 18 letras, a segunda parte vira um ponto
  const sit = etiquetaSituacao(timer, agora, 18);
  const aj = marcaAjusteCartao(timer, Boolean(reu || sit));
  return reu || sit || aj ? `<div class="kx">${reu}${sit}${aj}</div>` : '';
}

export function cartaoTimer(timer, agora, selecionado) {
  const ms = msDoTimer(timer, agora);
  const cls = ['k', timer.rodando && 'on', selecionado && 'sel', !timer.rodando && ms < 1000 && 'zero']
    .filter(Boolean)
    .join(' ');
  return `<div class="${cls}" style="--c:${cor(timer.cor)}" data-timer="${timer.id}" tabindex="0">
  <div class="h"><i></i><span class="nm">${esc(timer.nome)}</span>${botaoMenuTimer(timer)}${botaoReuniao(timer, 'pe')}</div>
  ${linhaExtra(timer, agora)}
  <div class="r"><span class="tm mono" data-tempo="${timer.id}"${tituloDoTempo(timer, agora)}>${relogio(ms)}</span><span class="bts">${botaoEncerrar(timer, 'pe')}${botaoPlay(timer, 'pl')}</span></div>
</div>`;
}

/**
 * O valor atual de um seletor sempre vem como opção marcada (revisão da rodada 21: nunca trocar
 * em silêncio). `ops`: o HTML das opções; `valor`: o valor de agora. Se ele está entre as
 * opções, fica marcado (só ele); se não está (arquivado, fora da lista), entra primeiro, marcado,
 * com `rotuloFora`. Sem valor (nulo, vazio), as opções como vieram.
 */
export function comOAtual(ops, valor, rotuloFora) {
  if (valor === null || valor === undefined || valor === '') return ops;
  const v = esc(String(valor));
  // só o atributo, dentro da tag <option ...>: o texto das opções (escapado, sem "<" nem ">")
  // nunca muda, nem um nome com "selected" (re-revisão da rodada 21)
  const desmarcadas = ops.replace(/(<option\b[^>]*?) selected(?=[\s>])/g, '$1');
  const alvo = `<option value="${v}"`;
  if (desmarcadas.includes(alvo)) return desmarcadas.replace(alvo, `${alvo} selected`);
  return `<option value="${v}" selected>${esc(rotuloFora)}</option>${desmarcadas}`;
}

/** O ⋯ do cartão de um timer (rodada 21): Editar e "Mover para…". */
export const botaoMenuTimer = (timer) =>
  `<button class="km" data-acao="menu-timer" data-timer="${timer.id}" title="${esc(TEXTOS.mais)}" aria-label="${esc(TEXTOS.mais)}: ${esc(timer.nome)}" aria-haspopup="menu" aria-expanded="false">⋯</button>`;

/**
 * A chave de um grupo na tela (abrir/fechar, o total do cabeçalho). Cada espaço tem o seu "Sem
 * grupo" (rodada 20): o do primeiro espaço segue "sem", como antes; os outros, "sem-<espaço>".
 */
export const chaveGrupo = (grupo) =>
  grupo.id !== null && grupo.id !== undefined
    ? String(grupo.id)
    : (grupo.espaco_id ?? 1) === 1
      ? 'sem'
      : `sem-${grupo.espaco_id}`;

/**
 * O grupo vazio (rodada 20; no lugar do cartão tracejado "+ Novo timer"): uma linha discreta,
 * "Grupo vazio · + Timer", que abre o "Novo timer" já nesse grupo.
 */
export const linhaGrupoVazio = (grupo) =>
  `<div class="g-vazio">${esc(TEXTOS.grupoVazio)} · <button type="button" data-acao="novo-timer" data-grupo="${grupo.id ?? ''}" data-espaco="${grupo.espaco_id ?? 1}">${esc(TEXTOS.grupoVazioTimer)}</button></div>`;

/** Sem timer nenhum (na tela ou na aba): o texto que aponta para o "+ Novo". */
export const semTimerNenhum = () => `<p class="vazio">${esc(TEXTOS.nenhumTimer)}</p>`;

/**
 * "2 rodando · Pausar tudo" no topo da coluna dos grupos; nada quando tudo está parado. `texto`:
 * o do botão (rodada 20: "Pausar tudo (Pessoal)" na aba de um espaço).
 */
export const barraRodando = (rodando, texto = TEXTOS.pausarTudo, espaco = null) =>
  rodando > 0
    ? `<div class="rd"><span>${esc(TEXTOS.rodando(rodando))}</span><button class="pa" data-acao="pausar-tudo" data-espaco="${espaco ?? ''}">${esc(texto)}</button></div>`
    : '';

/**
 * Cabeçalho do grupo: abre/fecha, ✎ (menos no "Sem grupo") e o total do grupo: a soma dos números
 * dos timers dele (cada um desde o seu zerar), com o tempo real (a união) do mesmo período na dica.
 */
export function cabecalhoGrupo(grupo, agora, aberto, classe = 'gh') {
  const chave = chaveGrupo(grupo);
  const total = totalDoGrupo(grupo.timers, agora, grupo.real_antes_ms ?? 0);
  const n = grupo.timers.length;
  const links = linksDoGrupo(grupo);
  const selo = links ? esc(links) : aberto ? '' : esc(TEXTOS.nTimers(n));
  // rodada 21: o ⋯ do grupo (Editar, Mover para outro espaço…, Arquivar), menos no "Sem grupo"
  const editar =
    grupo.id === null
      ? ''
      : `<button class="ge" data-acao="menu-grupo" data-grupo="${grupo.id}" title="${esc(TEXTOS.mais)}" aria-label="${esc(TEXTOS.mais)}: ${esc(grupo.nome)}" aria-haspopup="menu" aria-expanded="false">⋯</button>`;
  return `<div class="${classe}">
    <button class="gd" data-acao="dobrar" data-grupo="${chave}" aria-expanded="${aberto}">
      <span class="fd" aria-hidden="true">${aberto ? '▾' : '▸'}</span><b>${esc(grupo.nome)}</b>${selo ? `<span class="lk">${selo}</span>` : ''}
    </button>${editar}
    <span class="tt mono" data-total-grupo="${chave}" title="${esc(total.dica)}">${esc(total.texto)}</span>
  </div>`;
}

/** Seção do grupo; `cartao(timer)` desenha cada timer (padrão: o cartão do Painel). */
export function secaoGrupo(grupo, agora, aberto, selecionado = null, cartao = null) {
  const desenhar = cartao ?? ((t) => cartaoTimer(t, agora, t.id === selecionado));
  const vazio = !grupo.timers.length && grupo.id !== null;
  const grade = !aberto ? '' : vazio ? linhaGrupoVazio(grupo) : `<div class="grid">${grupo.timers.map(desenhar).join('')}</div>`;
  return `<section class="grp${aberto ? '' : ' fechado'}">
  ${cabecalhoGrupo(grupo, agora, aberto)}
  ${grade}
</section>`;
}

/**
 * Lado direito: nome, grupo, relógio grande (o número do timer: o acumulado desde o zerar), logo
 * abaixo dele "desde ontem 14:20", o motivo e os controles.
 */
export function relogioFoco(timer, agora, grupo = null) {
  const { hm, s } = relogioGrande(msDoTimer(timer, agora));
  const caminho = grupo && grupo.id !== null ? `<div class="path">${esc(grupo.nome)}</div>` : '';
  // parado: o lembrete ("esperando você", "segundo plano") ou o encerrado
  const motivo = timer.rodando && timer.motivo ? motivoNaTela(timer, agora) : textoLembrete(timer, agora) || marcaEncerrado(timer);
  return `<div class="foco-topo" style="--c:${cor(timer.cor)}">
  <div class="n"><i></i><span>${esc(timer.nome)}</span></div>
  ${caminho}
</div>
<div class="clk mono" data-relogio="${timer.id}">${hm}<span>:${s}</span></div>
${etiquetaDesde(timer, agora)}
<div class="why">${motivo}</div>
${linhasAtividades(timer, agora)}
${marcaAjuste(timer)}
${controles(timer, agora)}`;
}

/** "desde hoje 09:00" embaixo do relógio grande (vazio sem zerar); anda com o relógio (`data-desde`). */
export function etiquetaDesde(timer, agora) {
  return `<div class="desde" data-desde="${timer.id}">${esc(desdeTexto(timer.zerado_em, agora, corteMin))}</div>`;
}

/**
 * O ↺ do timer (painel do timer, Foco e cartões). Com duas ou mais atividades com tempo desde o
 * zerar, ele abre o menu (`itensDoMenuZerar`): a dica diz que abre a escolha, e o botão leva
 * `aria-haspopup="menu"` e `aria-expanded` (a página marca `true` com o menu aberto).
 */
export function botaoZerar(timer, classe = '', agora = Date.now()) {
  const menu = itensDoMenuZerar(timer, atividades, agora).length > 0;
  const cls = classe ? `class="${classe}" ` : '';
  const extra = menu ? ' aria-haspopup="menu" aria-expanded="false"' : '';
  return `<button ${cls}data-acao="zerar" data-timer="${timer.id}" title="${esc(menu ? TEXTOS.dicaZerarMenu : TEXTOS.dicaZerar)}" aria-label="${esc(TEXTOS.zerarTimer(timer.nome))}"${extra}>↺</button>`;
}

/**
 * O menu do ↺ (rodada 19): os itens de `itensDoMenuZerar`, no estilo do menu do ?. `data-zerar`
 * ("timer" ou "atividade") e `data-atividade` dizem o que zerar.
 */
export const htmlDoMenuZerar = (timer, itens) =>
  `<div class="menu-ajuda menu-zerar" role="menu" aria-label="${esc(TEXTOS.zerarTimer(timer.nome))}">${itens
    .map(
      (it) =>
        `<button type="button" role="menuitem" data-zerar="${it.zerar}" data-timer="${timer.id}"${it.id === undefined ? '' : ` data-atividade="${it.id}"`}>${esc(it.rotulo)}</button>`,
    )
    .join('')}</div>`;

/**
 * ↺ zerar, 👥 reunião, play/pausa grande, ■ encerrar (apagado quando não há o que encerrar) e ✎
 * editar.
 */
export function controles(timer, agora = Date.now()) {
  return `<div class="ctl">
  ${botaoZerar(timer, '', agora)}
  ${botaoReuniao(timer, 'rc')}
  ${botaoPlay(timer, 'big')}
  ${botaoEncerrar(timer, 'ec', true)}
  <button data-acao="editar" data-timer="${timer.id}" title="${TEXTOS.editar}" aria-label="${TEXTOS.editarOTimer}">✎</button>
</div>`;
}

/**
 * Botões de uma pergunta do núcleo. O primeiro é o destacado. "outro" ("Outro timer…" /
 * "Escolher timer…") abre a lista de timers e "novo" ("+ Novo timer…") abre o "Novo timer" na
 * janela principal: os dois ficam na página, nunca vão ao núcleo como resposta. Na de sessão
 * nova (a faixa da janela principal, opção A), os dois levam o "Sempre que for na pasta"
 * desmarcado (`data-sempre="0"`), como o dock.
 * Os botões ficam numa grade de duas colunas iguais. Com dois, um ao lado do outro; com mais, o
 * destacado ocupa a linha dele, os outros vão aos pares e o que sobrar sozinho ocupa a linha toda
 * (`largo`). O rótulo que não couber corta com reticências e fica inteiro na dica.
 */
export function opcoesDaPergunta(p, classe = '') {
  const n = p.opcoes.length;
  return `<div class="o${classe}">${p.opcoes
    .map(([valor, rotulo], i) => {
      // depois do destacado (o 0), os de índice ímpar começam uma linha: o último deles, sozinho, é largo
      const c = i === 0 ? (n === 2 ? 'y' : 'y largo') : n > 2 && i === n - 1 && i % 2 === 1 ? 'largo' : '';
      const attrs = `${c ? ` class="${c}"` : ''} title="${esc(rotulo)}"`;
      if (valor === 'outro' || valor === 'novo') {
        const acao = valor === 'outro' ? 'outro' : 'novo-timer';
        const sempre = p.tipo === 'sessao_nova' ? ' data-sempre="0"' : '';
        return `<button${attrs} data-acao="${acao}" data-sessao="${esc(p.sessao_id ?? '')}" data-pergunta="${p.id}"${sempre}>${esc(rotulo)}</button>`;
      }
      return `<button${attrs} data-acao="responder" data-pergunta="${p.id}" data-valor="${esc(valor)}">${esc(rotulo)}</button>`;
    })
    .join('')}</div>`;
}

/**
 * O pedido do "Ligar a um timer" que o dock manda à janela principal (rodada 16: nenhum diálogo
 * abre dentro do dock). `d`: o `dataset` do botão (sessão e, se houver, a pergunta); `aviso`: o
 * aviso de ligada sozinha da sessão (o "Trocar"), para ela aparecer mesmo fora das recentes.
 * Com a pergunta, a lista oferece o "+ Novo timer…"; pelo "Trocar" (a sessão já tem timer), não.
 */
export function pedidoDeLigar(d, aviso = null) {
  const pergunta = d.pergunta !== undefined && d.pergunta !== '' ? Number(d.pergunta) : null;
  return {
    sessao_id: d.sessao,
    pergunta_id: pergunta,
    nome: aviso?.sessao_nome ?? null,
    cwd: aviso?.cwd ?? null,
    timer_id: aviso?.timer_id ?? null,
    com_novo: pergunta !== null,
    // o "Sempre que for na pasta" da pergunta de sessão nova (rodada 27)
    ...sempreDoBotao(d),
  };
}

/**
 * O "Sempre que for na pasta" que um botão da pergunta de sessão nova leva (`data-sempre` e
 * `data-pasta`): `{ sempre, pasta }`; fora dela, os dois nulos (como antes).
 */
export function sempreDoBotao(d) {
  if (d?.sempre !== '1' && d?.sempre !== '0') return { sempre: null, pasta: null };
  const sempre = d.sempre === '1';
  return { sempre, pasta: sempre && d.pasta ? d.pasta : null };
}

/**
 * Texto de uma pergunta do núcleo (a faixa da janela principal). A de sessão nova mostra a pasta
 * numa linha dela, cortada com reticências se não couber, e o caminho inteiro na dica (o Chat não
 * tem pasta).
 */
export function textoDaPergunta(p) {
  if (p.tipo === 'sessao_nova') {
    const pasta = p.pasta ? `<div class="ask-pasta" title="${esc(p.pasta)}">${esc(p.pasta_curta || p.pasta)}</div>` : '';
    return `<div class="ask-t">${esc(TEXTOS.sessaoNovaPergunta)}</div>${pasta}`;
  }
  return `<div class="ask-t">${esc(p.texto)}</div>`;
}

// ---- a pergunta de sessão nova no dock (rodada 27, opção A) ----

/**
 * O "Sempre que for na pasta" de cada pergunta (pelo id): `{ sempre, pasta }`. Fica na página,
 * porque o dock se redesenha a cada estado; a caixa começa desmarcada e a pasta, na de agora.
 */
const escolhas = new Map();

/**
 * A escolha de agora da pergunta: `{ sempre, pasta }` (`pasta`: o item da lista escolhido, ou
 * nulo sem lista). Uma pasta que saiu da lista volta para a de agora.
 */
export function escolhaDaPergunta(p) {
  const e = escolhas.get(p.id) ?? { sempre: false, pasta: null };
  const pastas = p.pastas ?? [];
  const pasta = pastas.find((x) => x.pasta === e.pasta) ?? pastas.find((x) => x.agora) ?? pastas[pastas.length - 1] ?? null;
  return { sempre: Boolean(e.sempre && pasta), pasta };
}

/**
 * Trata a mudança da caixa (`data-pg-sempre`) ou da pasta (`data-pg-pasta`) de uma pergunta:
 * guarda a escolha e devolve se era dela (quem chama redesenha). Ao marcar a caixa, a pasta que
 * estava escolhida (`data-pasta` da caixa) fica fixa: ela não pula com a pasta de agora, que
 * muda a cada evento do hook.
 */
export function mudarEscolha(alvo) {
  const id = Number(alvo?.dataset?.pergunta);
  if (!Number.isFinite(id) || (alvo.dataset.pgSempre === undefined && alvo.dataset.pgPasta === undefined)) return false;
  const e = { sempre: false, pasta: null, ...escolhas.get(id) };
  if (alvo.dataset.pgSempre !== undefined) {
    e.sempre = Boolean(alvo.checked);
    if (e.sempre && !e.pasta && alvo.dataset.pasta) e.pasta = alvo.dataset.pasta;
  } else if (alvo.checked) e.pasta = alvo.value;
  escolhas.set(id, e);
  return true;
}

/** Esquece as escolhas das perguntas que já saíram (`ids`: as de agora). */
export function esquecerEscolhas(ids) {
  for (const id of escolhas.keys()) if (!ids.includes(id)) escolhas.delete(id);
}

/** "onde começou · onde está agora": as marcas de uma pasta da lista. */
const marcasDaPasta = (x) =>
  [x.comecou ? TEXTOS.ondeComecou : null, x.agora ? TEXTOS.ondeEstaAgora : null].filter(Boolean).join(' · ');

/**
 * A pergunta "Em qual timer contar?" de uma sessão nova (opção A, aprovada em 21/11): em cima,
 * pequeno, "Sessão nova do Claude"; embaixo, maior, a pergunta; um bloco com as linhas Sessão (o
 * nome em destaque), Vem de (a origem) e Pasta (a de agora, curta, com o caminho inteiro na
 * dica; o Chat não tem). Os botões: o principal ("Contar em X" com "só esta sessão", ou
 * "Escolher o timer…" sem sugestão), "Outro timer…" (só com sugestão), "+ Novo timer…" e o "Não
 * contar" discreto. Embaixo, separado por uma linha (o Chat não tem): a caixa "Sempre que for na
 * pasta", desmarcada, e, marcada, a lista das pastas da sessão (da onde começou até a de agora).
 * Com a caixa marcada, o principal responde `ligar:<t>:pasta:<caminho>` e os outros levam a pasta.
 */
export function perguntaSessaoNova(p) {
  const { sempre, pasta } = escolhaDaPergunta(p);
  const pastas = p.pastas ?? [];
  const nome = p.sessao_nome ? esc(p.sessao_nome) : `<i>${esc(TEXTOS.semNomeAinda)}</i>`;
  const origem = TEXTOS.origemDaSessao(p.origem);
  const linhas = [
    `<div class="sn-l"><dt>${esc(TEXTOS.rotuloSessao)}</dt><dd class="sn-nome">${nome}</dd></div>`,
    origem ? `<div class="sn-l"><dt>${esc(TEXTOS.rotuloVemDe)}</dt><dd>${esc(origem)}</dd></div>` : '',
    p.pasta
      ? `<div class="sn-l"><dt>${esc(TEXTOS.rotuloPasta)}</dt><dd class="sn-pasta mono" title="${esc(p.pasta)}">${esc(p.pasta_curta || p.pasta)}</dd></div>`
      : '',
  ].join('');
  // o que os botões levam do "Sempre que for na pasta" (só com a lista)
  const leva = pastas.length ? ` data-sempre="${sempre ? 1 : 0}"${sempre ? ` data-pasta="${esc(pasta.pasta)}"` : ''}` : '';
  const sessao = esc(p.sessao_id ?? '');
  const botao = (valor, rotulo, classe = '', sub = '') => {
    const c = classe ? ` class="${classe}"` : '';
    const corpo = sub ? `<span class="sn-bt">${esc(rotulo)}</span><small>${esc(sub)}</small>` : esc(rotulo);
    const dica = ` title="${esc(sub ? `${rotulo} · ${sub}` : rotulo)}"`;
    if (valor === 'outro' || valor === 'novo') {
      const acao = valor === 'outro' ? 'outro' : 'novo-timer';
      return `<button${c}${dica} data-acao="${acao}" data-sessao="${sessao}" data-pergunta="${p.id}"${leva}>${corpo}</button>`;
    }
    const v = sempre && valor.startsWith('ligar:') ? `${valor}:pasta:${pasta.pasta}` : valor;
    return `<button${c}${dica} data-acao="responder" data-pergunta="${p.id}" data-valor="${esc(v)}">${corpo}</button>`;
  };
  const [principal, ...resto] = p.opcoes;
  const ligar = principal?.[0]?.startsWith('ligar:');
  const meio = resto.filter(([v]) => v !== 'nao');
  const nao = resto.find(([v]) => v === 'nao');
  const botoes =
    (principal ? botao(principal[0], principal[1], 'y largo sn-principal', ligar ? (sempre ? TEXTOS.estaEProximas : TEXTOS.soEstaSessao) : '') : '') +
    meio.map(([v, r], i) => botao(v, r, meio.length === 1 || (meio.length % 2 === 1 && i === meio.length - 1) ? 'largo' : '')).join('') +
    (nao ? botao(nao[0], nao[1], 'largo sn-nao') : '');
  const lista = sempre
    ? `<div class="sn-pastas" role="radiogroup" aria-label="${esc(TEXTOS.pastasDaRegra)}">${pastas
        .map(
          (x) =>
            `<label class="sn-pi"><input type="radio" name="sn-pasta-${p.id}" value="${esc(x.pasta)}" data-pg-pasta data-pergunta="${p.id}"${x.pasta === pasta.pasta ? ' checked' : ''}><span class="mono" title="${esc(x.pasta)}">${esc(x.curta || x.pasta)}</span>${marcasDaPasta(x) ? `<small>${esc(marcasDaPasta(x))}</small>` : ''}</label>`,
        )
        .join('')}</div>`
    : '';
  const deCima = sempre && pastas.length > 1 && pasta.pasta === pastas[0].pasta;
  const rodape = pastas.length
    ? `<div class="sn-sempre" data-tauri-drag-region="false">
<label class="sn-caixa"><input type="checkbox" data-pg-sempre data-pergunta="${p.id}" data-pasta="${esc(pasta.pasta)}"${sempre ? ' checked' : ''}><span>${esc(TEXTOS.sempreQueForNaPasta)}</span></label>${lista}
<small class="sn-ajuda">${esc(TEXTOS.ajudaSempreNaPasta)}${deCima ? ` <b>${esc(TEXTOS.ajudaPastaDeCima)}</b>` : ''}</small></div>`
    : '';
  return `<div class="sn-sobre">${esc(TEXTOS.sessaoNovaSobre)}</div><div class="sn-titulo">${esc(TEXTOS.emQualTimer)}</div><dl class="sn-info">${linhas}</dl><div class="o sn-o">${botoes}</div>${rodape}`;
}

/**
 * Uma pergunta inteira no dock: a de sessão nova pelo desenho novo (rodada 27); as outras, o
 * texto e os botões de sempre.
 */
export const htmlDaPergunta = (p) => (p.tipo === 'sessao_nova' ? perguntaSessaoNova(p) : `${textoDaPergunta(p)}${opcoesDaPergunta(p)}`);

/** O estado de uma sessão na tela ("trabalhando", "esperando você", "parada"). */
export const estadoDaSessao = (estado) => TEXTOS.estadoSessao(estado);

/**
 * O que o cabeçalho do grupo mostra dos links dele: "projeto: INV - Inovação · P146 · card padrão: I7"
 * (cada link com o nome dado a ele, ou só o código); vazio sem links.
 */
export function linksDoGrupo(grupo) {
  const partes = [];
  if (grupo?.link_projeto) partes.push(TEXTOS.linkProjeto(rotuloDoDestino(grupo.link_projeto, grupo.link_projeto_nome)));
  if (grupo?.card_padrao) partes.push(TEXTOS.linkCardPadrao(rotuloDoDestino(grupo.card_padrao, grupo.card_padrao_nome)));
  return partes.join(' · ');
}

/**
 * Para onde vão as horas de um timer sem card próprio: o card padrão do grupo ou, sem ele, o
 * projeto do grupo (hora geral do projeto, seção 9). O nome vem do grupo: o do card padrão, o do
 * projeto ou, sem nenhum, o nome do grupo. `{ tipo: 'card' | 'projeto', rotulo }` ou nulo.
 */
export function destinoDoGrupo(grupo) {
  if (!grupo) return null;
  const nome = (n) => String(n ?? '').trim();
  if (grupo.card_padrao) {
    const n = nome(grupo.card_padrao_nome) || nome(grupo.link_projeto_nome) || nome(grupo.nome);
    return { tipo: 'card', rotulo: rotuloDoDestino(grupo.card_padrao, n) };
  }
  if (grupo.link_projeto) {
    const n = nome(grupo.link_projeto_nome) || nome(grupo.nome);
    return { tipo: 'projeto', rotulo: rotuloDoDestino(grupo.link_projeto, n) };
  }
  return null;
}

/** Texto do destino: o card do timer, o card padrão ou o projeto do grupo, ou nenhum. */
export function textoDoCard(timer, grupo) {
  if (timer.link_card) return { texto: TEXTOS.cardDoTimer(rotuloDoDestino(timer.link_card, timer.link_card_nome)), herdado: false };
  const g = destinoDoGrupo(grupo);
  if (g) return { texto: g.tipo === 'card' ? TEXTOS.cardPadraoDoGrupo(g.rotulo) : TEXTOS.projetoDoGrupo(g.rotulo), herdado: true };
  return { texto: TEXTOS.semCard, herdado: true };
}

/** "Onde estas horas vão": o card do timer (ou o padrão do grupo) e as sessões ligadas. */
export function destino(timer, grupo) {
  const c = textoDoCard(timer, grupo);
  const acao = timer.link_card ? TEXTOS.trocarCard : destinoDoGrupo(grupo) ? TEXTOS.usarOutroCard : TEXTOS.definirCard;
  const card = `<div class="l${c.herdado ? ' inh' : ''}"><span class="sn">${esc(c.texto)}</span>
  <button class="em" data-acao="editar" data-timer="${timer.id}">${acao}</button></div>`;
  const sessoes = timer.sessoes
    .map(
      (s) => `<div class="l s"><span class="sn" title="${esc(s.cwd)}">${esc(s.nome)}</span><em>${esc(estadoDaSessao(s.estado))}</em>
  <button class="x" data-acao="editar-sessao" data-sessao="${esc(s.id)}" data-timer="${timer.id}" title="${TEXTOS.verMudarSessao}" aria-label="${esc(TEXTOS.verMudar(s.nome))}">✎</button>
  <button class="x" data-acao="desligar" data-sessao="${esc(s.id)}" title="${TEXTOS.desligarSessao}" aria-label="${esc(TEXTOS.desligar(s.nome))}">✕</button></div>`,
    )
    .join('');
  return `<div class="dest">
  <h5>${TEXTOS.ondeAsHorasVao}</h5>
  ${card}
  ${sessoes ? `<h5 class="h5-ses">${TEXTOS.sessoesLigadas}</h5>${sessoes}` : ''}
  <button class="l add" data-acao="ligar-sessao" data-timer="${timer.id}">${TEXTOS.ligarSessaoMais}</button>
</div>`;
}
