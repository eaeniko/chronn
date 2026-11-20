// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Espaços (rodada 20): um nível acima dos grupos ("Trabalho", "Pessoal"). Com 2 ou mais espaços
// não arquivados, o Painel, os Cartões e a Linha do tempo ganham as abas "Todos · <espaço> · …";
// com um só, a tela fica como antes. Também o "+ Novo" global (Timer, Grupo, Espaço) e o que vem
// pré-escolhido nele. Sem DOM: testado com node --test.
import { esc, cor } from './componentes.js';
import { curto, totalDeTimers, dicaDoTotalDoDia, totalDoGrupo } from './formato.js';
import { TEXTOS } from './textos.js';

/** O primeiro espaço (o padrão das colunas `espaco_id`). */
export const ESPACO_PRIMEIRO = 1;

/** Os espaços não arquivados, na ordem. */
export const espacosAtivos = (snap) =>
  (snap?.espacos ?? []).filter((e) => !e.arquivado).sort((a, b) => a.ordem - b.ordem || a.id - b.id);

/** As abas aparecem? Só com 2 ou mais espaços não arquivados. */
export const comAbas = (snap) => espacosAtivos(snap).length >= 2;

export const espacoPorId = (snap, id) => (snap?.espacos ?? []).find((e) => e.id === id) ?? null;

/**
 * A aba escolhida: o id do espaço (`config.espaco_aba`), ou nulo para "Todos". Sem abas, ou com o
 * espaço guardado fora da tela (apagado, arquivado), "Todos".
 */
export function abaAtual(snap) {
  if (!comAbas(snap)) return null;
  const id = snap.config?.espaco_aba ?? null;
  return espacosAtivos(snap).some((e) => e.id === id) ? id : null;
}

/** O espaço de um grupo do snapshot (os de antes da rodada 20 não têm: o primeiro). */
const espacoDoGrupo = (g) => g.espaco_id ?? ESPACO_PRIMEIRO;

/** Os grupos da aba (nulo: todos), na ordem do snapshot (por espaço). */
export const gruposDaAba = (snap, aba) => (aba === null ? snap.grupos : snap.grupos.filter((g) => espacoDoGrupo(g) === aba));

/** Os timers da aba. */
export const timersDaAba = (snap, aba) => gruposDaAba(snap, aba).flatMap((g) => g.timers);

/** As abas "Todos · <espaço> · …" (vazio sem abas). */
export function htmlAbas(snap, aba) {
  if (!comAbas(snap)) return '';
  const botao = (id, nome, c = null) => {
    const on = id === aba;
    const ponto = c ? `<i style="--e:${cor(c)}" aria-hidden="true"></i>` : '';
    return `<button type="button" role="tab" class="aba${on ? ' on' : ''}" data-acao="aba" data-espaco="${id ?? ''}" aria-selected="${on}">${ponto}${esc(nome)}</button>`;
  };
  return `<div class="abas" role="tablist" aria-label="${esc(TEXTOS.abasRotulo)}">${botao(null, TEXTOS.todos)}${espacosAtivos(snap)
    .map((e) => botao(e.id, e.nome, e.cor))
    .join('')}</div>`;
}

/**
 * Cabeçalho de um espaço em "Todos": o nome e a soma dos timers dele (a soma dos grupos e dos
 * timers sem grupo), com o tempo real do mesmo período na dica.
 */
export function cabecalhoEspaco(espaco, timers, agora) {
  const total = totalDoGrupo(timers, agora, espaco.real_antes_ms ?? 0);
  return `<div class="esp-h" style="--e:${cor(espaco.cor)}"><i aria-hidden="true"></i><b>${esc(espaco.nome)}</b><span class="tt mono" data-total-espaco="${espaco.id}" title="${esc(total.dica)}">${esc(total.texto)}</span></div>`;
}

/**
 * Os grupos da aba, cada um desenhado por `desenharGrupo(g)`. Em "Todos" (com abas), o nome de
 * cada espaço vem como cabeçalho acima dos grupos dele.
 */
export function secoesDaAba(snap, aba, desenharGrupo, agora) {
  if (aba !== null || !comAbas(snap)) return gruposDaAba(snap, aba).map(desenharGrupo).join('');
  // os espaços na ordem e, no fim, um arquivado que tenha um timer rodando (revisão da rodada 20:
  // rodando nunca fica invisível)
  const ativos = espacosAtivos(snap);
  const outros = (snap.espacos ?? []).filter((e) => !ativos.includes(e) && snap.grupos.some((g) => espacoDoGrupo(g) === e.id));
  return [...ativos, ...outros]
    .map((e) => {
      const grupos = gruposDaAba(snap, e.id);
      if (!grupos.length) return '';
      return `<section class="esp" data-espaco="${e.id}">${cabecalhoEspaco(e, grupos.flatMap((g) => g.timers), agora)}${grupos
        .map(desenharGrupo)
        .join('')}</section>`;
    })
    .join('');
}

/** O total do dia da aba: a união de hoje dos timers dela (em "Todos", de todos). */
export const totalDaAba = (snap, aba, agora) => totalDeTimers(timersDaAba(snap, aba), agora);

/**
 * A dica do total do dia da aba: de onde vem o número (a da rodada 12) e, com abas, o total geral
 * e o de cada espaço ("Total geral 5h 10min · Trabalho 3h · Pessoal 2h 10min").
 */
export function dicaDoTotalDaAba(snap, aba, agora) {
  const base = dicaDoTotalDoDia(timersDaAba(snap, aba), agora);
  if (!comAbas(snap)) return base;
  const partes = [`${TEXTOS.totalGeral} ${curto(totalDaAba(snap, null, agora))}`].concat(
    espacosAtivos(snap).map((e) => `${e.nome} ${curto(totalDaAba(snap, e.id, agora))}`),
  );
  return `${base}\n${partes.join(' · ')}`;
}

/**
 * A aba que vale para o topo (o total do dia, "Zerar todos", "Pausar tudo"): a lembrada, nas telas
 * com abas e também nos Ajustes (revisão da rodada 20); no Foco, que não tem abas, "Todos".
 */
export const abaDoTopo = (snap, visao) => (visao === 'foco' ? null : abaAtual(snap));

/**
 * O topo de uma tela: o escopo (o espaço, ou nulo) e o texto e a dica do "Zerar todos", da mesma
 * fonte. A página põe o escopo no botão (`data-espaco`), e o clique lê dali: o texto e o clique
 * nunca discordam.
 */
export function topoDaTela(snap, visao) {
  const espaco = abaDoTopo(snap, visao);
  return { espaco, texto: textoZerarTodos(snap, espaco), dica: dicaZerarTodos(snap, espaco), pausar: textoPausarTudo(snap, espaco) };
}

/** O escopo gravado num botão (`data-espaco`): o espaço, ou nulo ("Todos"). */
export const espacoDoBotao = (el) => {
  const v = el?.dataset?.espaco;
  return v === undefined || v === '' ? null : Number(v);
};

/**
 * O nome de um grupo nas listas sem cabeçalho de espaço (o Foco, o dock expandido): com dois ou
 * mais "Sem grupo" à vista (de espaços diferentes, mesmo sem abas: o de um espaço arquivado com um
 * timer rodando), "Sem grupo · Pessoal" (revisões da rodada 20).
 */
export function nomeDoGrupo(snap, g) {
  if (g.id !== null && g.id !== undefined) return g.nome;
  if (snap.grupos.filter((x) => x.id === null || x.id === undefined).length < 2) return g.nome;
  const e = espacoPorId(snap, espacoDoGrupo(g));
  return e ? `${g.nome} · ${e.nome}` : g.nome;
}

/** Para onde vão os arquivados de um espaço apagado: o primeiro outro espaço não arquivado. */
export const destinoAoApagar = (snap, id) => espacosAtivos(snap).find((e) => e.id !== id) ?? null;

/**
 * Dá para apagar o espaço? (`{ pode, dica }`; o mesmo que o núcleo decide.) Não com grupos ou
 * timers sem grupo à vista nele (`ativos`, do núcleo) nem sendo o último não arquivado.
 */
export function podeApagarEspaco(snap, e) {
  if (!destinoAoApagar(snap, e.id)) return { pode: false, dica: TEXTOS.ultimoEspaco };
  if ((e.ativos ?? 0) > 0) return { pode: false, dica: TEXTOS.apagarSoVazio };
  return { pode: true, dica: '' };
}

/** O texto da confirmação de apagar: para onde vão os arquivados, se houver. */
export function textoAoApagar(snap, e) {
  const n = e.arquivados ?? 0;
  const destino = destinoAoApagar(snap, e.id);
  return n > 0 && destino ? TEXTOS.arquivadosVaoPara(n, destino.nome) : TEXTOS.apagarEspacoVazio;
}

/** O nome da aba (nulo em "Todos"). */
const nomeDaAba = (snap, aba) => (aba === null ? null : espacoPorId(snap, aba)?.nome ?? null);

/** "↺ Zerar todos", ou "↺ Zerar todos (Pessoal)" na aba de um espaço. */
export const textoZerarTodos = (snap, aba) => TEXTOS.daAba(TEXTOS.zerarTodos, nomeDaAba(snap, aba));

/** "Pausar tudo", ou "Pausar tudo (Pessoal)" na aba de um espaço. */
export const textoPausarTudo = (snap, aba) => TEXTOS.daAba(TEXTOS.pausarTudo, nomeDaAba(snap, aba));

/** A dica do "Zerar todos" da aba (nula em "Todos": fica a do HTML). */
export const dicaZerarTodos = (snap, aba) => {
  const nome = nomeDaAba(snap, aba);
  return nome ? TEXTOS.dicaZerarTodosDoEspaco(nome) : null;
};

/** O texto da faixa do "Zerar todos" da aba (nulo em "Todos": fica o de antes). */
export const textoZeradosDaAba = (snap, aba) => {
  const nome = nomeDaAba(snap, aba);
  return nome ? TEXTOS.zeradosTodosDoEspaco(nome) : null;
};

/** O espaço pré-escolhido no "Novo grupo" e no "Sem grupo" do timer novo: o da aba, ou o primeiro. */
export const espacoPreEscolhido = (snap, aba) => aba ?? espacosAtivos(snap)[0]?.id ?? ESPACO_PRIMEIRO;

/**
 * O grupo pré-escolhido no "Novo timer" (`{ grupoId, espacoId }`; `grupoId` nulo é o "Sem grupo"
 * do espaço `espacoId`): o grupo do timer aberto no painel, se houver; senão, o "Sem grupo" do
 * espaço da aba (em "Todos", do primeiro espaço).
 */
export function grupoPreEscolhido(snap, aba, timerAberto = null) {
  if (timerAberto) return { grupoId: timerAberto.grupo_id ?? null, espacoId: timerAberto.espaco_id ?? ESPACO_PRIMEIRO };
  return { grupoId: null, espacoId: espacoPreEscolhido(snap, aba) };
}

/** Os itens do menu do "+ Novo". */
export const ITENS_DO_MENU_NOVO = [
  { id: 'timer', rotulo: TEXTOS.novoTimer },
  { id: 'grupo', rotulo: TEXTOS.novoGrupo },
  { id: 'espaco', rotulo: TEXTOS.novoEspaco },
];

/** O menu do "+ Novo" (no estilo do menu do ?). */
export const htmlDoMenuNovo = () =>
  `<div class="menu-ajuda menu-novo" role="menu" aria-label="${esc(TEXTOS.novoMenu)}">${ITENS_DO_MENU_NOVO.map(
    (it) => `<button type="button" role="menuitem" data-novo="${it.id}">${esc(it.rotulo)}</button>`,
  ).join('')}</div>`;

/** Um elemento que recebe texto (onde a tecla N é uma letra, não o atalho). */
const editavel = (el) =>
  Boolean(el) && (['INPUT', 'TEXTAREA', 'SELECT'].includes(el.tagName) || el.isContentEditable === true);

/**
 * A tecla N abre o "+ Novo"? Só o N (maiúsculo ou não) sem Ctrl, Alt nem ⌘, sem repetição, com o
 * foco fora de campo e sem diálogo aberto.
 */
export function teclaDoNovo(ev, focado, comDialogo = false) {
  if (ev.key !== 'n' && ev.key !== 'N') return false;
  if (ev.ctrlKey || ev.altKey || ev.metaKey || ev.repeat) return false;
  return !comDialogo && !editavel(focado);
}

/**
 * O ponto pequeno na cor do espaço do timer, com o nome do espaço na dica (o dock, com 2 ou mais
 * espaços; vazio com um só).
 */
export function pontoDoEspaco(snap, timer) {
  if (!comAbas(snap)) return '';
  const e = espacoPorId(snap, timer.espaco_id ?? ESPACO_PRIMEIRO);
  if (!e) return '';
  return `<i class="desp" style="--e:${cor(e.cor)}" title="${esc(`${TEXTOS.espaco}: ${e.nome}`)}" aria-label="${esc(`${TEXTOS.espaco}: ${e.nome}`)}"></i>`;
}

/**
 * Os grupos agrupados por espaço, para as listas de timers ("Outro timer…"): `[{ espaco, grupos }]`
 * na ordem dos espaços; sem abas, um só item com `espaco` nulo.
 */
export function gruposPorEspaco(snap) {
  if (!comAbas(snap)) return [{ espaco: null, grupos: snap.grupos }];
  return espacosAtivos(snap)
    .map((e) => ({ espaco: e, grupos: gruposDaAba(snap, e.id) }))
    .filter((x) => x.grupos.length);
}
