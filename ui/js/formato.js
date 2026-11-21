// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Formatação de tempo e posições da linha do tempo (sem DOM; testado com node --test). Os textos
// vêm do `textos.js`, no idioma de agora; as datas e os dias da semana, pelo `Intl` (rodada 23).
import { TEXTOS, localeAtual } from './textos.js';

const S = 1000;
const M = 60 * S;
const H = 60 * M;

const dois = (n) => String(n).padStart(2, '0');
const partes = (ms) => {
  const s = Math.floor(Math.max(0, ms) / S);
  return { h: Math.floor(s / 3600), m: Math.floor((s % 3600) / 60), s: s % 60 };
};

/** "2:23:16" (horas sem zero à esquerda). */
export function relogio(ms) {
  const p = partes(ms);
  return `${p.h}:${dois(p.m)}:${dois(p.s)}`;
}

/** { hm: "02:23", s: "16" } para o relógio grande. */
export function relogioGrande(ms) {
  const p = partes(ms);
  return { hm: `${dois(p.h)}:${dois(p.m)}`, s: dois(p.s) };
}

/** "1:32" (horas e minutos, para totais de grupo). */
export function horaMin(ms) {
  const p = partes(ms);
  return `${p.h}:${dois(p.m)}`;
}

/** "6h 12min", "2h 01min"; "45min" abaixo de 1 h (em inglês, "2h 01m" e "45m"). Total do dia e totais dos grupos. */
export function curto(ms) {
  const min = Math.floor(Math.max(0, ms) / M);
  const h = Math.floor(min / 60);
  return h > 0 ? TEXTOS.durHMin(h, dois(min % 60)) : TEXTOS.durMin(min);
}

/** O nome em minúsculas pelo idioma (a atividade no meio da frase: "1h reunião"). */
export const minusculas = (s) => String(s ?? '').toLocaleLowerCase(localeAtual());

/** Duração da união dos trechos; trecho aberto (fim nulo) vai até `agora`. */
export function uniaoMs(trechos, agora) {
  const iv = trechos
    .map((t) => [t.inicio, t.fim ?? agora])
    .filter(([a, b]) => b > a)
    .sort((x, y) => x[0] - y[0]);
  let total = 0;
  let ini = null;
  let fim = null;
  for (const [a, b] of iv) {
    if (fim === null || a > fim) {
      if (fim !== null) total += fim - ini;
      ini = a;
      fim = b;
    } else if (b > fim) {
      fim = b;
    }
  }
  if (fim !== null) total += fim - ini;
  return total;
}

/** Trabalho: a atividade dos trechos e ajustes sem atividade (de antes da versão 5 do banco). */
export const TRABALHO = 1;
/** Reunião: a do 👥 (pelo id; renomear não muda o botão). */
export const REUNIAO = 2;

/** O que um ajuste à mão muda numa atividade com `uniao` de tempo: nunca tira mais do que ela tem. */
const ajusteEfetivo = (uniao, soma) => Math.max(soma, -uniao);

/**
 * As contas de hoje do timer. `porAtividade`: Map atividade → tempo, a união dos trechos de cada
 * atividade (o Claude, a leitura e o ▶ sobrepostos contam uma vez) mais os ajustes nela, nunca
 * abaixo de zero. `total`: a soma das atividades (cada uma é um lançamento: o Claude das 9:00 às
 * 11:00 durante a reunião das 10:00 às 11:10 dá 3h 10min). `uniao`: a união dos trechos, sem
 * dobrar entre atividades; `real`: o tempo real com os ajustes (`realAjustado`), o que o timer
 * põe no dia. Sem a lista de ajustes (só `ajuste_ms`), tudo em Trabalho.
 */
export function contasDoTimer(timer, agora) {
  const trechos = new Map();
  for (const t of timer.trechos) {
    const a = t.atividade_id ?? TRABALHO;
    if (!trechos.has(a)) trechos.set(a, []);
    trechos.get(a).push(t);
  }
  const somas = new Map();
  if (timer.ajustes) {
    for (const a of timer.ajustes) {
      const k = a.atividade_id ?? TRABALHO;
      somas.set(k, (somas.get(k) ?? 0) + a.delta_ms);
    }
  } else if (timer.ajuste_ms) {
    somas.set(TRABALHO, timer.ajuste_ms);
  }
  const uniao = uniaoMs(timer.trechos, agora);
  const porAtividade = new Map();
  let mudou = 0;
  let total = 0;
  for (const a of new Set([...trechos.keys(), ...somas.keys()])) {
    const base = uniaoMs(trechos.get(a) ?? [], agora);
    const ef = ajusteEfetivo(base, somas.get(a) ?? 0);
    porAtividade.set(a, base + ef);
    mudou += ef;
    total += base + ef;
  }
  const maior = Math.max(0, ...porAtividade.values());
  return { uniao, real: realAjustado(uniao, mudou, maior), total, porAtividade };
}

/**
 * O tempo real do timer com os ajustes (o que ele põe no dia): a união mais o que os ajustes
 * mudaram nas atividades (`mudou`), nunca abaixo da maior atividade já ajustada nem de zero. Com
 * atividades sobrepostas (o Claude durante a reunião), tirar o tempo de uma não tira do dia a
 * parte que a outra cobre. A mesma conta do núcleo (`relatorio::real_ajustado`).
 */
export const realAjustado = (uniao, mudou, maiorAtividade) => Math.max(uniao + mudou, maiorAtividade, 0);

/**
 * Desde quando cada atividade do timer conta (rodada 19): o mais recente entre o zerar do timer
 * (`zerado_em`) e o zerar só dela (`zerados_atividade`). Devolve `atividade → instante`, ou nulo
 * sem zerar nenhum.
 */
export function zeradoDaAtividade(timer) {
  const proprios = new Map((timer.zerados_atividade ?? []).map((z) => [z.atividade_id, z.zerado_em]));
  const z = timer.zerado_em;
  if ((z === undefined || z === null) && !proprios.size) return null;
  return (atividade) => Math.max(z ?? -Infinity, proprios.get(atividade ?? TRABALHO) ?? -Infinity);
}

/**
 * O timer só com o que entra no número dele hoje (rodada 11): os trechos de hoje a partir do último
 * zerar (`zerado_em`; o que atravessa o instante conta a partir dele, inclusive o aberto) e os
 * ajustes de hoje feitos depois dele. Desde a rodada 19, cada atividade desde o seu zerar
 * (`zeradoDaAtividade`): o do timer ou o só dela, o mais recente. Sem zerar, o timer como veio.
 */
export function hojeDesdeZerar(timer) {
  const zerado = zeradoDaAtividade(timer);
  if (!zerado) return timer;
  const trechos = timer.trechos
    .map((t) => [t, zerado(t.atividade_id)])
    .filter(([t, z]) => (t.fim ?? Infinity) > z)
    .map(([t, z]) => (t.inicio >= z ? t : { ...t, inicio: z }));
  if (!timer.ajustes) return { ...timer, trechos };
  const ajustes = timer.ajustes.filter((a) => (a.criado_em ?? Infinity) >= zerado(a.atividade_id));
  return { ...timer, trechos, ajustes, ajuste_ms: ajustes.reduce((s, a) => s + a.delta_ms, 0) };
}

/**
 * As contas do número do timer: o acumulado desde o último zerar. Hoje (`hojeDesdeZerar`) com as
 * contas do dia mais os dias fechados que o núcleo manda (`acumulado_antes`, por atividade).
 * `porAtividade`: Map atividade → tempo; `total`: a soma delas.
 */
export function contasAcumuladas(timer, agora) {
  const { porAtividade: hoje } = contasDoTimer(hojeDesdeZerar(timer), agora);
  const porAtividade = new Map(hoje);
  for (const a of timer.acumulado_antes ?? []) porAtividade.set(a.atividade_id, (porAtividade.get(a.atividade_id) ?? 0) + a.ms);
  let total = 0;
  for (const ms of porAtividade.values()) total += ms;
  return { porAtividade, total };
}

/**
 * O número do timer: o acumulado desde o último zerar (a soma das atividades, cada uma já com os
 * ajustes e nunca negativa). Não volta a zero na virada do dia (rodada 11).
 */
export function msDoTimer(timer, agora) {
  return contasAcumuladas(timer, agora).total;
}

/** O tempo de hoje do timer (a linha do tempo, que segue por dia), sem olhar o zerar. */
export function msDeHoje(timer, agora) {
  return contasDoTimer(timer, agora).total;
}

/**
 * "desde hoje 09:00", "desde ontem 14:20" ou "desde 15/11 10:05" (de outro ano, "15/11/2025
 * 10:05"): desde quando o número do timer conta (o último zerar). "hoje" e "ontem" são do dia de
 * trabalho, pelo corte (`corteMin`, minutos depois da meia-noite local): com o corte às 03:00, um
 * zerar à 01:30 de 19/11 é do dia de trabalho 18, e às 10:00 do 19 diz "desde ontem 01:30" (revisão
 * da rodada 11). A hora e a data mostradas são as do relógio. Vazio sem zerar (zerado "desde
 * sempre").
 */
export function desdeTexto(zeradoEm, agora, corteMin = 0) {
  if (!zeradoEm) return '';
  const z = new Date(zeradoEm);
  const hora = `${dois(z.getHours())}:${dois(z.getMinutes())}`;
  // o dia de trabalho de um instante: a data local dele recuada do corte
  const diaDeTrabalho = (t) => {
    const d = new Date(t);
    d.setMinutes(d.getMinutes() - corteMin);
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  };
  const dz = diaDeTrabalho(zeradoEm);
  const hoje = new Date(diaDeTrabalho(agora));
  if (dz === hoje.getTime()) return TEXTOS.desdeHoje(hora);
  if (dz === new Date(hoje.getFullYear(), hoje.getMonth(), hoje.getDate() - 1).getTime()) return TEXTOS.desdeOntem(hora);
  return TEXTOS.desdeData(dataCurta(z, z.getFullYear() !== new Date(agora).getFullYear()), hora);
}

/**
 * Tempo de cada atividade do timer desde o zerar, as que tiveram tempo (≥ 1 s), na ordem da lista
 * de atividades: [{ id, nome, cor, ms }]. Atividade fora da lista (não deveria) fica no fim.
 */
export function tempoPorAtividade(timer, atividades, agora) {
  const { porAtividade } = contasAcumuladas(timer, agora);
  const ordem = (id) => {
    const i = (atividades ?? []).findIndex((a) => a.id === id);
    return i < 0 ? Infinity : i;
  };
  return [...porAtividade.entries()]
    .filter(([, ms]) => ms >= 1000)
    .sort((x, y) => ordem(x[0]) - ordem(y[0]) || x[0] - y[0])
    .map(([id, ms]) => {
      const a = (atividades ?? []).find((x) => x.id === id);
      return { id, nome: a?.nome ?? TEXTOS.atividadeGenerica, cor: a?.cor ?? null, ms };
    });
}

/**
 * O menu do ↺ do timer (rodada 19): "O timer todo" e um "Só <atividade>" para cada atividade com
 * tempo desde o último zerar dela (na ordem da lista). Com uma atividade só com tempo (ou
 * nenhuma), vazio: o ↺ zera direto, como antes. [{ zerar: 'timer' | 'atividade', id?, rotulo }].
 */
export function itensDoMenuZerar(timer, atividades, agora) {
  const v = tempoPorAtividade(timer, atividades, agora);
  if (v.length < 2) return [];
  return [{ zerar: 'timer', rotulo: TEXTOS.oTimerTodo }, ...v.map((a) => ({ zerar: 'atividade', id: a.id, rotulo: TEXTOS.soAtividade(a.nome) }))];
}

/**
 * As linhas por atividade do painel do timer, na ordem da lista de atividades: todas as que não
 * estão arquivadas (rodada 19: o Estudo aparece mesmo sem tempo), mais uma arquivada que teve tempo
 * desde o zerar ou está rodando. [{ id, nome, cor, ms, rodando, manual, arquivada }]: `ms`, o
 * acumulado desde o zerar; `rodando`, algum trecho aberto dela no timer (o ▶, o 👥, o Claude ou a
 * leitura); `manual`, o play manual aberto é dela.
 * Rodada 20: as atividades ocultas do timer (`atividades_ocultas`, do editar timer) não aparecem,
 * a não ser com tempo desde o zerar ou rodando (nenhum tempo contado fica invisível); a padrão do
 * timer nunca é ocultada.
 */
export function linhasDeAtividade(timer, atividades, agora) {
  const lista = atividades ?? [];
  const { porAtividade } = contasAcumuladas(timer, agora);
  const abertas = new Set(timer.trechos.filter((t) => t.fim === null || t.fim === undefined).map((t) => t.atividade_id ?? TRABALHO));
  const ocultas = new Set(timer.atividades_ocultas ?? []);
  ocultas.delete(timer.atividade_padrao_id ?? TRABALHO);
  const ids = new Set();
  for (const a of lista) if (!a.arquivada && !ocultas.has(a.id)) ids.add(a.id);
  // sem a lista (não deveria), ao menos a padrão do timer
  if (!lista.length) ids.add(timer.atividade_padrao_id ?? TRABALHO);
  for (const [id, ms] of porAtividade) if (ms >= 1000) ids.add(id);
  for (const id of abertas) ids.add(id);
  const ordem = (id) => {
    const i = lista.findIndex((a) => a.id === id);
    return i < 0 ? Infinity : i;
  };
  return [...ids]
    .sort((x, y) => ordem(x) - ordem(y) || x - y)
    .map((id) => {
      const a = lista.find((x) => x.id === id);
      return {
        id,
        nome: a?.nome ?? TEXTOS.atividadeGenerica,
        cor: a?.cor ?? null,
        ms: porAtividade.get(id) ?? 0,
        rodando: abertas.has(id),
        manual: timer.manual_atividade_id === id,
        arquivada: a?.arquivada === true,
      };
    });
}

/**
 * "2h trabalho · 1h 10min reunião": a divisão do timer por atividade desde o zerar, só quando
 * houve mais de uma (quem não usa atividades não vê nada). Vazio com uma só.
 */
export function divisaoPorAtividade(timer, atividades, agora) {
  const v = tempoPorAtividade(timer, atividades, agora);
  if (v.length < 2) return '';
  return v.map((a) => `${curto(a.ms)} ${minusculas(a.nome)}`).join(' · ');
}

/**
 * Total de vários timers (o dia, um grupo): o tempo real, a união de todos os trechos mais o que
 * os ajustes mudaram no tempo real de cada timer (`real` menos a união dele), nunca abaixo do
 * maior tempo real de timer nem de zero. Pode ficar menor que a soma dos timers (atividades ou
 * timers sobrepostos).
 */
export function totalDeTimers(timers, agora) {
  let total = uniaoMs(timers.flatMap((t) => t.trechos), agora);
  let maior = 0;
  for (const t of timers) {
    const c = contasDoTimer(t, agora);
    total += c.real - c.uniao;
    maior = Math.max(maior, c.real);
  }
  // com timers sobrepostos, um ajuste negativo deixaria o total abaixo do tempo real de um deles
  return Math.max(0, total, maior);
}

/**
 * A dica do total do dia (rodada 12): de onde vem o número. "tempo contado 1min · ajustes +30min
 * (Cliente C)": o tempo contado é a união dos trechos de hoje; os ajustes, o que os ajustes à mão de hoje
 * mudaram no total (o total menos o contado, então os dois somam o número), com os timers que têm
 * ajuste hoje, separados por vírgula, até 3 nomes e depois "+N". Sem ajuste, só o tempo contado.
 */
export function dicaDoTotalDoDia(timers, agora) {
  const contado = uniaoMs(timers.flatMap((t) => t.trechos), agora);
  const texto = TEXTOS.tempoContado(curto(contado));
  const com = timers.filter((t) => (t.ajustes ?? []).length > 0 || (t.ajuste_ms ?? 0) !== 0);
  if (!com.length) return texto;
  const efeito = totalDeTimers(timers, agora) - contado;
  const nomes = com.slice(0, 3).map((t) => t.nome).join(', ') + (com.length > 3 ? ` +${com.length - 3}` : '');
  return `${texto} · ${TEXTOS.ajustes(`${efeito < 0 ? '−' : '+'}${curto(Math.abs(efeito))}`, nomes)}`;
}

/**
 * Total de um grupo: a soma dos números dos timers dele (cada um desde o seu zerar; o Claude
 * durante a reunião conta nas duas atividades). O tempo real do grupo (a união) vai só na dica do
 * cabeçalho; o total do dia, no topo, continua a união de hoje.
 */
export function somaDosTimers(timers, agora) {
  return timers.reduce((s, t) => s + msDoTimer(t, agora), 0);
}

/**
 * O cabeçalho do grupo: { texto: a soma dos timers, dica: "tempo real: …" }. O tempo real é o do
 * mesmo período: a união de hoje de cada timer desde o zerar dele mais a dos dias fechados, que o
 * núcleo manda no grupo (`realAntesMs`, o `real_antes_ms`).
 */
export function totalDoGrupo(timers, agora, realAntesMs = 0) {
  const real = realAntesMs + totalDeTimers(timers.map(hojeDesdeZerar), agora);
  return { texto: curto(somaDosTimers(timers, agora)), dica: TEXTOS.tempoReal(curto(real)) };
}

/** "+15 min", "−1h 05min": o ajuste à mão (arredonda para o minuto). */
export function textoDoAjuste(ms) {
  const min = Math.round(Math.abs(ms) / M);
  const sinal = ms < 0 ? '−' : '+';
  const h = Math.floor(min / 60);
  return h > 0 ? TEXTOS.ajusteHMin(sinal, h, dois(min % 60)) : TEXTOS.ajusteMin(sinal, min);
}

/** O "Ajustar hoje": `sinal` "+" ou "-" e os minutos (1 a 1440, inteiros) → ms com sinal; nulo se inválido. */
export function deltaDoAjuste(sinal, texto) {
  const t = String(texto ?? '').trim();
  if (!/^\d+$/.test(t)) return null;
  const min = Number(t);
  if (min < 1 || min > 1440) return null;
  return (sinal === '-' ? -1 : 1) * min * M;
}

/** O link é do Zoho Sprints (`https://sprints.zoho.com/...`, `sprints.zoho.eu`, ...)? */
export function linkDoZohoSprints(link) {
  try {
    const u = new URL(String(link ?? '').trim());
    return /^https?:$/.test(u.protocol) && /^sprints\.zoho\.[a-z.]+$/i.test(u.hostname);
  } catch {
    return false;
  }
}

/**
 * Códigos de um link do Zoho Sprints: `.../workspace/exemplo#P146/itemdetails/I7` →
 * { projeto: 'P146', item: 'I7' }. O link não traz nomes (o "exemplo" é o espaço de trabalho).
 * Link de outro site não tem código.
 */
export function codigosDoLink(link) {
  if (!linkDoZohoSprints(link)) return { projeto: null, item: null };
  const t = String(link).trim();
  const achar = (re) => t.match(re)?.[1].toUpperCase() ?? null;
  // um pedaço do caminho ou do # que é só P<número> ou I<número>
  return { projeto: achar(/[/#](P\d+)(?=$|[/?#&])/i), item: achar(/[/#](I\d+)(?=$|[/?#&])/i) };
}

/**
 * Como o destino aparece na tela: o nome dado pela pessoa e o código do link ("INV - Inovação · I7");
 * sem nome, só o código ("I7", ou "P146" para o link do projeto); link que não é do Zoho, o último
 * pedaço do caminho; texto que não é link, como está.
 */
export function rotuloDoDestino(link, nome = null) {
  const n = String(nome ?? '').trim();
  const { projeto, item } = codigosDoLink(link);
  const codigo = item ?? projeto;
  if (n) return codigo ? `${n} · ${codigo}` : n;
  return codigo ?? rotuloDeLink(link);
}

/** Link vira o último pedaço do caminho; texto comum fica como está. */
export function rotuloDeLink(s) {
  const t = String(s ?? '').trim();
  if (!/^https?:\/\//i.test(t)) return t;
  try {
    const u = new URL(t);
    const ult = u.pathname.split('/').filter(Boolean).pop();
    return ult ? decodeURIComponent(ult).replace(/[-_]+/g, ' ') : u.hostname;
  } catch {
    return t;
  }
}

/**
 * Etiqueta do que o timer está fazendo: curta (dock recolhido) e por extenso (expandido), no
 * idioma de agora (`TEXTOS.situacao`). As chaves: trabalhando, lendo, escrevendo (o foco na caixa
 * de mensagem da conversa lida: conta como leitura), esperando, reuniao (a reunião manual 👥: vem
 * na frente das outras), reuniao_claude (com o Claude trabalhando junto), manual (o ▶ manual,
 * rodada 13) e segundo_plano (o turno terminou com uma tarefa em segundo plano: não conta tempo).
 */
export const SITUACAO = new Proxy(
  {},
  {
    get: (_, k) => TEXTOS.situacao[k],
    has: (_, k) => k in TEXTOS.situacao,
  },
);

/** A situação é um lembrete (o timer parado com uma sessão que espera você, ou em segundo plano)? */
export const ehLembrete = (s) => s === 'esperando' || s === 'segundo_plano';

/**
 * O lembrete ainda fica à vista neste lugar? `limiteMin`: o tempo dos Ajustes "Esperando você"
 * (0 = até a virada do dia, que o núcleo já corta), contado do fim de turno mais recente
 * (`espera_ultima`; rodada 13).
 */
export function lembreteVisivel(t, agora, limiteMin) {
  if (!limiteMin) return true;
  const ultima = t?.espera_ultima;
  return ultima !== null && ultima !== undefined && agora - ultima < limiteMin * 60_000;
}

/** O estado do Claude como segunda parte da etiqueta do manual ("manual + esperando você"). */
const SEGUNDA = new Proxy(
  {},
  {
    get: (_, k) => TEXTOS.situacaoSegunda[k],
    has: (_, k) => k in TEXTOS.situacaoSegunda,
  },
);

/**
 * A etiqueta do timer (rodada 14). Com o ▶ manual (ou a reunião manual) rodando, as duas partes:
 * o manual e o que o Claude faz no timer ("manual + Claude", "manual + esperando você", "reunião +
 * segundo plano"...), porque o fim do Claude não pausa o manual e o aviso não pode se perder. Sem
 * o manual, a situação de sempre, numa parte só (`forma` "curta" ou "longa"). O lembrete
 * ("esperando você", "segundo plano") some depois de `limiteMin` (o tempo de cada lugar).
 * { p1: { k, texto }, p2: { k, texto } | null, texto } ou nulo sem nada para mostrar.
 */
export function etiquetaComposta(t, agora, limiteMin, forma = 'curta') {
  const manual = t?.manual_atividade_id !== null && t?.manual_atividade_id !== undefined;
  // o pausado do dock recolhido: "pausado + esperando você" (revisão da rodada 14)
  const pausado = Boolean(t?.pausado);
  let claude = t?.situacao_claude ?? null;
  if (claude === null && t?.situacao_claude === undefined) {
    // um snapshot de antes da rodada 14: a situação dele, sem a parte do manual
    const s = situacaoDoTimer(t);
    claude = s === 'reuniao_claude' ? 'trabalhando' : s in SEGUNDA ? s : null;
  }
  if (ehLembrete(claude) && !lembreteVisivel(t, agora, limiteMin)) claude = null;
  if (pausado) {
    // pausado, só o lembrete do Claude entra na segunda parte
    const p2 = ehLembrete(claude) ? { k: claude, texto: SEGUNDA[claude] } : null;
    const pausado = TEXTOS.pausado;
    return { p1: { k: 'pausado', texto: pausado }, p2, texto: p2 ? `${pausado} + ${p2.texto}` : pausado };
  }
  if (manual) {
    const p1 = t.manual_atividade_id === REUNIAO ? { k: 'reuniao', texto: TEXTOS.reuniaoMin } : { k: 'manual', texto: TEXTOS.manual };
    const p2 = claude ? { k: claude, texto: SEGUNDA[claude] } : null;
    return { p1, p2, texto: p2 ? `${p1.texto} + ${p2.texto}` : p1.texto };
  }
  if (!claude) return null;
  const texto = SITUACAO[claude][forma];
  return { p1: { k: claude, texto }, p2: null, texto };
}

/** A situação para mostrar num lugar com o tempo `limiteMin` (o lembrete vencido some). */
export function situacaoNaTela(t, agora, limiteMin) {
  const s = situacaoDoTimer(t);
  return ehLembrete(s) && !lembreteVisivel(t, agora, limiteMin) ? null : s;
}

/**
 * A situação do timer para a etiqueta: a do núcleo, e "reuniao_claude" quando a reunião roda com o
 * Claude trabalhando junto (um trecho do Claude aberto no timer).
 */
export function situacaoDoTimer(t) {
  const s = t?.situacao ?? null;
  if (s === 'reuniao' && (t.trechos ?? []).some((x) => x.origem === 'claude' && (x.fim === null || x.fim === undefined))) return 'reuniao_claude';
  return s;
}

/**
 * Há o que encerrar (■)? O timer rodando, ou parado com o lembrete "esperando você". Encerrado
 * e parado, não há mais nada: o ■ some (ou fica apagado, onde a fileira de botões é fixa).
 */
export const podeEncerrar = (t) => Boolean(t?.rodando || ehLembrete(t?.situacao));

/** Posição em % na régua das `horaIni` às `horaFim`, recortada em 0–100. */
export function posicaoNaLinha(inicio, fim, diaInicio, horaIni = 8, horaFim = 19) {
  const zero = diaInicio + horaIni * H;
  const vao = (horaFim - horaIni) * H;
  const pct = (t) => Math.min(100, Math.max(0, ((t - zero) / vao) * 100));
  const left = pct(inicio);
  return { left, width: Math.max(0, pct(fim) - left) };
}

/**
 * Régua do dia em horas desde a meia-noite: 8h–19h, estendida para caber os trechos
 * (nunca antes de 0h nem depois de 36h). Trecho aberto vai até `agora`.
 */
export function reguaDoDia(trechos, diaInicio, agora) {
  let ini = 8;
  let fim = 19;
  for (const t of trechos) {
    ini = Math.min(ini, Math.floor((t.inicio - diaInicio) / H));
    fim = Math.max(fim, Math.ceil(((t.fim ?? agora) - diaInicio) / H));
  }
  return { ini: Math.max(0, ini), fim: Math.min(36, fim) };
}

/**
 * Blocos de um timer na régua (`horaIni`–`horaFim`), por ordem de início:
 * [{ left, width, manual, atividade, titulo }] em %. `manual` = play do usuário (bloco
 * listrado); `atividade` = o id da atividade do trecho; `titulo` = a dica do bloco ("09:30–11:00 ·
 * Claude trabalhando · 1h 30min", com " · reunião" quando `atividades` é dada e a atividade não é
 * a padrão do timer).
 */
export function trechosDoDia(timer, diaInicio, agora, horaIni = 8, horaFim = 19, atividades = null) {
  return [...timer.trechos]
    .sort((a, b) => a.inicio - b.inicio)
    .map((t) => {
      const { left, width } = posicaoNaLinha(t.inicio, t.fim ?? agora, diaInicio, horaIni, horaFim);
      const atividade = t.atividade_id ?? TRABALHO;
      const outra = atividades && atividade !== (timer.atividade_padrao_id ?? TRABALHO);
      const nome = outra ? atividades.find((a) => a.id === atividade)?.nome : null;
      const titulo = tituloDoTrecho(t, diaInicio, agora) + (nome ? ` · ${minusculas(nome)}` : '');
      return { left, width, manual: t.origem === 'manual', atividade, titulo };
    });
}

/**
 * As atividades que tiveram trecho entre os timers dados, na ordem da lista: a linha do tempo
 * pinta as atividades (e mostra a legenda) só quando há mais de uma.
 */
export function atividadesDosTrechos(timers, atividades) {
  const ids = new Set(timers.flatMap((t) => t.trechos.map((x) => x.atividade_id ?? TRABALHO)));
  return (atividades ?? []).filter((a) => ids.has(a.id));
}

/** "09:12": hora local do instante, contada da meia-noite `diaInicio`. */
export function horaDoDia(ms, diaInicio) {
  // dá a volta no relógio para os dois lados (instante antes da meia-noite ou depois das 24h)
  const min = ((Math.floor((ms - diaInicio) / M) % 1440) + 1440) % 1440;
  return `${dois(Math.floor(min / 60))}:${dois(min % 60)}`;
}

/** "09:30–11:00 · Claude trabalhando · 1h 30min"; trecho aberto termina em "agora". */
export function tituloDoTrecho(trecho, diaInicio, agora) {
  const aberto = trecho.fim === null || trecho.fim === undefined;
  const fim = aberto ? agora : trecho.fim;
  const ate = aberto ? TEXTOS.agora : horaDoDia(fim, diaInicio);
  return `${horaDoDia(trecho.inicio, diaInicio)}–${ate} · ${TEXTOS.origem(trecho.origem)} · ${curto(fim - trecho.inicio)}`;
}

/** A primeira letra maiúscula ("terça" → "Terça"). */
const capitular = (s) => (s ? s[0].toLocaleUpperCase(localeAtual()) + s.slice(1) : s);

/**
 * "2026-11-17" → "Terça, 17 nov" (em inglês, "Tuesday, Nov 17"): o dia da semana e o mês pelo
 * `Intl` no idioma de agora (rodada 23). Em português, sem o "-feira" e sem o ponto do mês.
 */
export function dataDoDia(dia) {
  const [a, m, d] = dia.split('-').map(Number);
  const data = new Date(Date.UTC(a, m - 1, d));
  const loc = localeAtual();
  const fmt = (o) => new Intl.DateTimeFormat(loc, { timeZone: 'UTC', ...o }).format(data);
  if (loc.startsWith('pt')) {
    const sem = capitular(fmt({ weekday: 'long' }).replace(/-feira$/, ''));
    return `${sem}, ${d} ${fmt({ month: 'short' }).replace('.', '')}`;
  }
  return `${fmt({ weekday: 'long' })}, ${fmt({ month: 'short', day: 'numeric' })}`;
}

/**
 * A data curta de um instante local, para o "desde …": "15/11" (de outro ano, "15/11/2025"); em
 * inglês, "Nov 15" ("Nov 15, 2025").
 */
function dataCurta(z, comAno) {
  const loc = localeAtual();
  if (loc.startsWith('pt')) return `${dois(z.getDate())}/${dois(z.getMonth() + 1)}${comAno ? `/${z.getFullYear()}` : ''}`;
  return new Intl.DateTimeFormat(loc, { month: 'short', day: 'numeric', ...(comAno ? { year: 'numeric' } : {}) }).format(z);
}

/**
 * A data e a hora de um backup, do ISO local que o núcleo manda ("2026-11-05T09:30"): "05/11/2026
 * 09:30"; em inglês, "Nov 5, 2026 09:30", com o mês por extenso para não confundir dia e mês
 * (rodada 23, revisão I1). Um texto que não é ISO passa como veio.
 */
export function dataDoBackup(iso) {
  const m = /^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})/.exec(String(iso ?? ''));
  if (!m) return String(iso ?? '');
  const [, a, mes, d, h, min] = m;
  const loc = localeAtual();
  if (loc.startsWith('pt')) return `${d}/${mes}/${a} ${h}:${min}`;
  const data = new Date(Date.UTC(Number(a), Number(mes) - 1, Number(d)));
  const dia = new Intl.DateTimeFormat(loc, { timeZone: 'UTC', month: 'short', day: 'numeric', year: 'numeric' }).format(data);
  return `${dia} ${h}:${min}`;
}

/**
 * Timers do dock recolhido: [{ id, nome, cor, ms, desde, rodando, situacao }] (`ms`, o número do
 * timer; `desde`, "desde ontem 14:20", a dica dele). Primeiro os rodando, do
 * que começou primeiro (o trecho aberto mais antigo) ao mais recente; com `comSituacao`, depois
 * os parados com uma sessão esperando você (o lembrete), da que terminou antes (`espera_desde`).
 * No máximo 4. Sem `comSituacao`, `situacao` vem nula.
 */
const DOCK_MAX = 4;

/**
 * Os timers pausados (o ⏸ ou a inatividade; o ■ não) há menos de `limiteMin` (`pausado_dock_min`,
 * 10 min por padrão; 0 = sai na hora; rodada 14), do ⏸ mais recente ao mais antigo.
 */
function pausados(timers, agora, limiteMin) {
  if (!limiteMin) return [];
  return timers
    .filter((t) => !t.rodando && !t.encerrado && t.pausado_em !== null && t.pausado_em !== undefined && agora - t.pausado_em < limiteMin * 60_000)
    .sort((a, b) => b.pausado_em - a.pausado_em || a.id - b.id);
}

/**
 * Os timers parados com uma sessão esperando você (ou em segundo plano), da que terminou antes à
 * mais recente; no dock recolhido, só os que ainda ficam à vista (`espera_dock_min`, 30 min por
 * padrão; rodada 13).
 */
function lembretes(timers, agora, limiteMin) {
  return timers
    .filter((t) => !t.rodando && ehLembrete(t.situacao) && lembreteVisivel(t, agora, limiteMin))
    .sort((a, b) => (a.espera_desde ?? Infinity) - (b.espera_desde ?? Infinity) || a.id - b.id);
}

/** Quantos lembretes "esperando você" não couberam no dock recolhido (o "+N esperando"). */
export function lembretesDeFora(snap, comSituacao = false, agora = snap.agora) {
  return lembretesQueFicaramDeFora(snap, comSituacao, agora).length;
}

/** Os pausados e os lembretes que não couberam no dock recolhido, na ordem. */
function lembretesQueFicaramDeFora(snap, comSituacao, agora) {
  const todos = snap.grupos.flatMap((g) => g.timers);
  const rodando = todos.filter((t) => t.rodando).length;
  return parados(snap, todos, agora, comSituacao).slice(Math.max(0, DOCK_MAX - rodando));
}

/** O tempo do pausado no dock recolhido (Ajustes → "Lembretes no dock"; 10 min sem config). */
const pausadoDoDock = (snap) => snap.config?.pausado_dock_min ?? 10;

/**
 * Os parados do dock recolhido, depois dos rodando: primeiro os pausados (com o ▶ para retomar;
 * rodada 14), depois os lembretes "esperando você" (só com `comSituacao`), sem repetir.
 */
function parados(snap, todos, agora, comSituacao) {
  const p = pausados(todos, agora, pausadoDoDock(snap)).map((t) => ({ ...t, pausado: true }));
  const l = comSituacao ? lembretes(todos, agora, esperaDoDock(snap)).filter((t) => !p.some((x) => x.id === t.id)) : [];
  return [...p, ...l];
}

/**
 * O "+N" do dock recolhido, separando "esperando você" de "segundo plano" (revisão da rodada 13):
 * { texto: "+2 esperando · +1 segundo plano", dica } ou nulo sem nada de fora.
 */
export function textoDosDeFora(snap, comSituacao = false, agora = snap.agora) {
  const fora = lembretesQueFicaramDeFora(snap, comSituacao, agora);
  if (!fora.length) return null;
  const pa = fora.filter((t) => t.pausado).length;
  const sp = fora.filter((t) => !t.pausado && t.situacao === 'segundo_plano').length;
  const esp = fora.length - sp - pa;
  const f = TEXTOS.fora;
  const partes = [pa && f.pausado(pa), esp && f.esperando(esp), sp && f.segundoPlano(sp)].filter(Boolean);
  const dicas = [pa && f.dicaPausado(pa), esp && f.dicaEsperando(esp), sp && f.dicaSegundoPlano(sp)].filter(Boolean);
  return { texto: partes.join(' · '), dica: f.dica(dicas) };
}

/** A dica do lembrete no dock recolhido: o que ele quer dizer e o que fazer. */
export function dicaDoLembrete(t) {
  if (t.pausado) return TEXTOS.dicaLembretePausado(t.nome);
  return t.situacao === 'segundo_plano' ? TEXTOS.dicaLembreteSegundoPlano(t.nome) : TEXTOS.dicaLembreteEsperando(t.nome);
}

/** O tempo do lembrete no dock recolhido (Ajustes → "Esperando você"; 30 min sem config). */
const esperaDoDock = (snap) => snap.config?.espera_dock_min ?? 30;

export function timersParaDock(snap, agora = snap.agora, comSituacao = false) {
  const inicio = (t) => Math.min(...t.trechos.filter((x) => x.fim === null).map((x) => x.inicio), Infinity);
  const todos = snap.grupos.flatMap((g) => g.timers);
  const rodando = todos
    .filter((t) => t.rodando)
    .map((t) => ({ t, ini: inicio(t) }))
    .sort((a, b) => a.ini - b.ini || a.t.id - b.t.id)
    .map(({ t }) => t);
  return [...rodando, ...parados(snap, todos, agora, comSituacao)].slice(0, DOCK_MAX).map((t) => ({
    id: t.id,
    nome: t.nome,
    cor: t.cor,
    ms: msDoTimer(t, agora),
    desde: desdeTexto(t.zerado_em, agora, snap.config?.corte_min ?? 0),
    rodando: t.rodando,
    situacao: comSituacao ? situacaoDoTimer(t) : null,
    // o pausado (rodada 14): fica com o ▶ e a etiqueta "pausado"
    pausado: Boolean(t.pausado),
    // a etiqueta de duas partes ("manual + esperando você"), com o tempo do recolhido
    etiqueta: comSituacao ? etiquetaComposta(t, agora, esperaDoDock(snap)) : null,
  }));
}

/**
 * Troca o texto do elemento só se mudou: o relógio anda a cada segundo, mas "1:32" e "6h 12min"
 * mudam uma vez por minuto, e escrever o mesmo texto ainda faz o WebView redesenhar.
 */
/** O tema escolhido segue o sistema? "sistema" (rodada 21) e "windows" (o nome de antes). */
export const segueOSistema = (tema) => tema !== 'claro' && tema !== 'escuro';

/**
 * O tema na tela: "claro" ou "escuro". "Sistema" (o padrão, rodada 21) segue o
 * `prefers-color-scheme` (`escuroDoSistema`).
 */
export const temaResolvido = (tema, escuroDoSistema) =>
  segueOSistema(tema) ? (escuroDoSistema ? 'escuro' : 'claro') : tema;

export function trocarTexto(el, texto) {
  if (el.textContent !== texto) el.textContent = texto;
}

/**
 * A dica (o `title` nativo) em linhas de no máximo `max` caracteres (rodada 19): o Chromium não
 * quebra a dica sozinho, e uma linha longa sai cortada na borda da tela. Quebra só entre palavras;
 * as quebras que já existem ficam, e uma palavra maior que o limite fica inteira numa linha. Passar
 * o texto já quebrado de novo dá o mesmo texto (o observador das páginas depende disso).
 */
export function quebrarDica(texto, max = 60) {
  return String(texto)
    .split('\n')
    .map((linha) => {
      if (linha.length <= max) return linha;
      const linhas = [];
      let atual = '';
      for (const p of linha.split(' ').filter(Boolean)) {
        if (!atual) atual = p;
        else if (atual.length + 1 + p.length <= max) atual += ` ${p}`;
        else {
          linhas.push(atual);
          atual = p;
        }
      }
      if (atual) linhas.push(atual);
      return linhas.join('\n');
    })
    .join('\n');
}

/**
 * O dock expandido recolhe quando a janela dele perde o foco (clicar fora)? Não com um diálogo
 * aberto nele, nem durante o redimensionamento, nem quando ele expandiu só para mostrar um
 * diálogo (esse recolhe quando o diálogo fecha).
 */
export function recolherAoPerderFoco({ expandido, dialogo, redimensionando, soPeloDialogo }) {
  return expandido === true && !dialogo && !redimensionando && !soPeloDialogo;
}

/** Texto de uma caixa "um por linha" → lista sem espaços nas pontas, sem vazias nem repetidas (sem diferenciar maiúsculas). */
export function linhas(texto) {
  const vistos = new Set();
  const v = [];
  for (const l of String(texto ?? '').split(/\r?\n/)) {
    const t = l.trim();
    const k = t.toLowerCase();
    if (t && !vistos.has(k)) {
      vistos.add(k);
      v.push(t);
    }
  }
  return v;
}

/** 180 → "3" (campos de tempo em minutos; a config guarda segundos). */
export function minutosDeSegundos(s) {
  return String(Math.round((s / 60) * 100) / 100);
}

/** "10" → 600; aceita vírgula. Nulo se o resultado não ficar entre 1 min (60 s) e `maxMin` (24 h). */
export function segundosDeMinutos(texto, maxMin = 1440) {
  const t = String(texto ?? '').trim().replace(',', '.');
  if (!/^\d+(\.\d+)?$/.test(t)) return null;
  const s = Math.round(Number(t) * 60);
  return Number.isFinite(s) && s >= 60 && s <= maxMin * 60 ? s : null;
}

/** Timers da lista que ainda não foram criados (sem diferenciar maiúsculas): o "Criar" de novo depois de uma falha. */
export function faltamCriar(nomes, jaCriados) {
  const feitos = new Set(jaCriados.map((n) => n.toLowerCase()));
  return nomes.filter((n) => !feitos.has(n.toLowerCase()));
}

/**
 * Um caminho do usuário abreviado para o texto corrido (rodada 18): `C:\\Users\\nome\\.claude\\settings.json`
 * → `~\\.claude\\settings.json`. A prévia do JSON continua exata (ela mostra o que será gravado).
 */
export const abreviarCaminho = (p) => String(p ?? '').replace(/^[A-Za-z]:[\\/]Users[\\/][^\\/]+(?=[\\/]|$)/i, '~');

/** Tamanho de arquivo para a tela: "820 B", "48 KB", "1,2 MB" ("1.2 MB" em inglês). */
export function tamanho(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} KB`;
  const mb = bytes / (1024 * 1024);
  return `${(mb < 10 ? mb.toFixed(1) : Math.round(mb).toString()).replace('.', TEXTOS.decimal)} MB`;
}

const escAviso = (s) =>
  String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);

/**
 * A faixa de um aviso do dock na janela dos avisos (rodada 15): o texto e, se houver, o botão
 * (o "Desfazer").
 */
export const htmlDoAviso = (aviso) =>
  `<div class="faixa${aviso.botao2 ? ' dupla' : ''}" role="status"><span>${escAviso(aviso.texto)}</span>${
    aviso.botao2 ? `<button type="button" class="b2" data-qual="2">${escAviso(aviso.botao2)}</button>` : ''
  }${aviso.botao ? `<button type="button" data-qual="1">${escAviso(aviso.botao)}</button>` : ''}</div>`;
