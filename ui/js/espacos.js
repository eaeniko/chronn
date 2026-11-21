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

// ---- os menus ⋯ e o mover (rodada 21) ----

/** Os outros espaços não arquivados, para onde o grupo pode ir. */
export const espacosParaMover = (snap, g) => espacosAtivos(snap).filter((e) => e.id !== espacoDoGrupo(g));

/**
 * O menu ⋯ do cabeçalho de um grupo: Renomear (o primeiro, rodada 24), Editar, "Mover para outro
 * espaço…" (só com 2 ou mais espaços) e Arquivar.
 */
export function itensDoMenuGrupo(snap, g) {
  const v = [
    { id: 'renomear', rotulo: TEXTOS.renomear },
    { id: 'editar', rotulo: TEXTOS.editar },
  ];
  if (comAbas(snap) && espacosParaMover(snap, g).length) v.push({ id: 'mover', rotulo: TEXTOS.moverParaOutroEspaco });
  v.push({ id: 'arquivar', rotulo: TEXTOS.arquivar });
  return v;
}

const botaoDeMenu = (attrs, rotulo, desabilitado = false) =>
  `<button type="button" role="menuitem" ${attrs}${desabilitado ? ' disabled' : ''}>${esc(rotulo)}</button>`;

export const htmlDoMenuGrupo = (snap, g) =>
  `<div class="menu-ajuda menu-grupo" role="menu" aria-label="${esc(g.nome)}">${itensDoMenuGrupo(snap, g)
    .map((it) => botaoDeMenu(`data-mg="${it.id}" data-grupo="${g.id}"`, it.rotulo))
    .join('')}</div>`;

/** O segundo passo do "Mover para outro espaço…": os outros espaços, na ordem. */
export const htmlDoMenuEspacos = (snap, g) =>
  `<div class="menu-ajuda menu-mover" role="menu" aria-label="${esc(TEXTOS.moverParaOutroEspaco)}">${espacosParaMover(snap, g)
    .map((e) => botaoDeMenu(`data-mover-grupo="${g.id}" data-espaco="${e.id}"`, e.nome))
    .join('')}</div>`;

/** O menu ⋯ do cartão de um timer: Editar e "Mover para…" (o rótulo no idioma de agora). */
export const ITENS_DO_MENU_TIMER = [
  { id: 'editar', get rotulo() { return TEXTOS.editar; } },
  { id: 'mover', get rotulo() { return TEXTOS.moverPara; } },
];

export const htmlDoMenuTimer = (t) =>
  `<div class="menu-ajuda menu-timer" role="menu" aria-label="${esc(t.nome)}">${ITENS_DO_MENU_TIMER.map((it) =>
    botaoDeMenu(`data-mt="${it.id}" data-timer="${t.id}"`, it.rotulo),
  ).join('')}</div>`;

/**
 * Para onde o timer pode ir ("Mover para…"): por espaço (com 2 ou mais), os grupos e o "Sem
 * grupo" de cada um; com um espaço só, os grupos e o "Sem grupo", sem o nome do espaço.
 * `[{ espaco, itens: [{ grupoId, espacoId, rotulo, atual }] }]`; `atual` é onde ele está.
 */
export function destinosDoTimer(snap, t) {
  const espacos = comAbas(snap) ? espacosAtivos(snap) : [espacosAtivos(snap)[0] ?? { id: t.espaco_id ?? ESPACO_PRIMEIRO, nome: '' }];
  const doTimer = t.espaco_id ?? ESPACO_PRIMEIRO;
  return espacos.map((e) => ({
    espaco: comAbas(snap) ? e : null,
    itens: [
      ...snap.grupos
        .filter((g) => g.id !== null && g.id !== undefined && (!comAbas(snap) || espacoDoGrupo(g) === e.id))
        .map((g) => ({ grupoId: g.id, espacoId: espacoDoGrupo(g), rotulo: g.nome, atual: t.grupo_id === g.id })),
      { grupoId: null, espacoId: e.id, rotulo: TEXTOS.semGrupo, atual: (t.grupo_id ?? null) === null && doTimer === e.id },
    ],
  }));
}

export const htmlDoMenuDestinos = (snap, t) =>
  `<div class="menu-ajuda menu-mover" role="menu" aria-label="${esc(TEXTOS.moverPara)}">${destinosDoTimer(snap, t)
    .map(
      ({ espaco, itens }) =>
        (espaco ? `<div class="menu-sec" role="presentation"><i style="--e:${cor(espaco.cor)}"></i>${esc(espaco.nome)}</div>` : '') +
        itens
          .map((it) =>
            botaoDeMenu(
              `data-mover-timer="${t.id}" data-grupo="${it.grupoId ?? ''}" data-espaco="${it.espacoId}"`,
              it.atual ? `${it.rotulo} · ${TEXTOS.ondeEsta}` : it.rotulo,
              it.atual,
            ),
          )
          .join(''),
    )
    .join('')}</div>`;

/**
 * O aviso depois de mover: "Obra foi para Pessoal. As horas dele, também as de dias anteriores,
 * agora contam em Pessoal." `destino`: o grupo ou, sem grupo, o espaço; a segunda frase só quando
 * o espaço mudou.
 */
export function textoDoMovido(snap, nome, { grupoId = null, espacoId, espacoAntes }) {
  const espaco = espacoPorId(snap, espacoId);
  const grupo = grupoId === null ? null : snap.grupos.find((g) => g.id === grupoId);
  const destino = grupo ? grupo.nome : espaco?.nome ?? TEXTOS.semGrupo;
  return TEXTOS.movido(nome, destino, espacoId !== espacoAntes ? espaco?.nome ?? null : null);
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
  { id: 'timer', get rotulo() { return TEXTOS.novoTimer; } },
  { id: 'grupo', get rotulo() { return TEXTOS.novoGrupo; } },
  { id: 'espaco', get rotulo() { return TEXTOS.novoEspaco; } },
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

/** As formas do espaço no dock recolhido (Ajustes → Dock, rodada 24); a primeira é o padrão. */
export const DOCK_ESPACO = ['timer', 'texto', 'cor'];

/**
 * O espaço de um timer no dock recolhido (rodada 24): `{ modo, espaco }`. `modo` é a escolha dos
 * Ajustes (`config.dock_espaco`): "timer" (o padrão: um ponto na cor do timer e o espaço na dica
 * dele), "texto" (o nome do espaço na linha da situação) ou "cor" (o ponto na cor do espaço). Com
 * um espaço só, ou sem achar o do timer, as três ficam iguais à "timer", sem espaço.
 */
export function espacoNoRecolhido(snap, timer) {
  const escolha = snap?.config?.dock_espaco;
  const modo = DOCK_ESPACO.includes(escolha) ? escolha : DOCK_ESPACO[0];
  if (!comAbas(snap)) return { modo: DOCK_ESPACO[0], espaco: null };
  const espaco = espacoPorId(snap, timer?.espaco_id ?? ESPACO_PRIMEIRO);
  return espaco ? { modo, espaco } : { modo: DOCK_ESPACO[0], espaco: null };
}

/**
 * O ponto do timer no dock recolhido (rodada 24): um só. Na cor do timer (o `--c` da linha) ou,
 * no modo "cor", na do espaço, com o mesmo halo de rodando. Com o espaço à mão, fora do modo
 * "texto" (lá o nome já está escrito), a dica diz "Relatório · espaço Pessoal".
 */
export function pontoRecolhido(timer, { modo, espaco }) {
  if (!espaco || modo === 'texto') return '<i></i>';
  const dica = esc(TEXTOS.dicaPontoEspaco(timer.nome, espaco.nome));
  const estilo = modo === 'cor' ? ` style="--c:${cor(espaco.cor)}"` : '';
  return `<i${estilo} title="${dica}" aria-label="${dica}"></i>`;
}

/**
 * O nome do espaço na linha da situação (o modo "texto", rodada 24): "Pessoal", na cor do
 * espaço. Vazio fora do modo "texto" ou sem espaço.
 */
export function espacoEmTexto({ modo, espaco }) {
  if (modo !== 'texto' || !espaco) return '';
  return `<span class="sesp" style="--e:${cor(espaco.cor)}">${esc(espaco.nome)}</span>`;
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
