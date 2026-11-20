// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Diálogos: novo/editar grupo e timer (com o nome dos links do Zoho e o "Ajustar hoje", que tem o
// "Descartar o tempo de hoje…"), as faixas do zerar e do encerrar com o "Desfazer" e ligar sessões
// do Claude.
// Sem dependência do Painel: o dock também usa `ligarATimer` (opção "Outro timer…" / "Escolher timer…" da pergunta).
// Cada função recebe o snapshot atual; erros dos comandos aparecem dentro do diálogo.
import { chamar } from './api.js';
import { esc, cor, ESTADO_SESSAO, ajusteDoTimer, atividadesAtivas, atividadePorId } from './componentes.js';
import { codigosDoLink, deltaDoAjuste, textoDoAjuste, TRABALHO } from './formato.js';
import { NOME_APP } from './nome.js';
import { espacosAtivos, comAbas, gruposPorEspaco, espacoPreEscolhido, ESPACO_PRIMEIRO } from './espacos.js';
import { TEXTOS } from './textos.js';

/** Paleta fixa dos timers e grupos (a mesma dos mockups). */
export const PALETA = [
  '#ff6b4a',
  '#7b88ff',
  '#2fd19b',
  '#f2b63c',
  '#c37be6',
  '#e86fa6',
  '#5ec8f0',
  '#9bd35a',
  '#d9a066',
  '#7d808a',
];

const texto = (e) => (typeof e === 'string' ? e : 'Não deu certo. Tente de novo.');
export { texto as textoDoErro };
const vazioParaNulo = (s) => (s.trim() === '' ? null : s.trim());

// ---- diálogo genérico ----

let atual = null;
let restaurarFoco = () => {};
/**
 * O que espera não haver diálogo aberto: os diálogos pedidos com um protegido na tela e os
 * `quandoLivre`. Saem um por vez, na ordem, quando o diálogo da vez fecha.
 */
const esperando = [];

function soltarEsperando() {
  setTimeout(() => {
    while (!atual && esperando.length) esperando.shift()();
  }, 0);
  conferirSemDialogo();
}

/**
 * O que espera não sobrar diálogo nenhum (rodada 16: o "Ligar a um timer" pedido pelo dock avisa
 * o núcleo quando acaba). Um diálogo que dá lugar a outro na mesma hora não conta. O "+ Novo
 * timer…" de dentro dele é outro pedido, que o núcleo conta à parte (revisão: sem prazo).
 */
const semDialogo = [];
let semDialogoId = null;

function conferirSemDialogo() {
  if (!semDialogo.length) return;
  clearTimeout(semDialogoId);
  semDialogoId = setTimeout(() => {
    if (atual || esperando.length) return;
    for (const fn of semDialogo.splice(0)) fn();
  }, 0);
}

/** Roda `fn` uma vez, quando não sobrar diálogo aberto (o que está aberto e os que vierem logo depois). */
export function quandoSemDialogo(fn) {
  semDialogo.push(fn);
  if (!atual) conferirSemDialogo();
}

/** Roda `fn` agora, sem diálogo aberto, ou quando o aberto (e os que esperavam antes) fechar. */
export function quandoLivre(fn) {
  if (!atual && !esperando.length) fn();
  else esperando.push(fn);
}

/** Há um diálogo aberto agora? */
export const dialogoAberto = () => atual !== null;

/** Quem abriu o diálogo pode ter sido redesenhado; `fn(elementoAntigo)` acha o novo e foca. */
export function aoPerderFoco(fn) {
  restaurarFoco = fn;
}

const FOCAVEIS = 'input:not([disabled]), select:not([disabled]), textarea:not([disabled]), button:not([disabled]), summary';

/**
 * Abre um diálogo modal. `titulo` e `corpo` são HTML (já escapado). `aoEnviar(form)` roda no
 * Enter ou no botão principal; se lançar, a mensagem aparece no diálogo e ele fica aberto.
 * `aoMontar(form)` liga os cliques próprios do corpo. `voltar` troca "Cancelar" por "Voltar".
 * `cancelar` troca o rótulo do botão secundário (`null` = sem ele) e `aoCancelar` o que ele faz
 * (o padrão é `voltar`, se houver, ou fechar). `classe` vai no formulário (ex.: "largo").
 * `aoFechar(enviado)` roda sempre que o diálogo sai. Esc fecha.
 * Abrir um diálogo com outro aberto troca o conteúdo (o foco volta para quem abriu o primeiro).
 * `protegido`: nada o tira da tela além dos botões dele (nem Esc, nem clicar fora), e um diálogo
 * pedido enquanto ele está aberto espera (abre quando nenhum estiver aberto). Quem o abriu fecha
 * antes de abrir o passo seguinte.
 * Devolve `{ fechar, mostrarErro, form }` (num diálogo que espera, `form` chega quando ele abre).
 */
export function abrirDialogo(opcoes) {
  const h = { form: null, fechar: () => {}, mostrarErro: () => {} };
  if (atual?.protegido) {
    const vez = () => montarDialogo(opcoes, h);
    esperando.push(vez);
    h.fechar = () => {
      const i = esperando.indexOf(vez);
      if (i >= 0) esperando.splice(i, 1);
    };
    return h;
  }
  montarDialogo(opcoes, h);
  return h;
}

function montarDialogo(
  {
    titulo,
    corpo,
    enviar = 'Salvar',
    perigo = false,
    voltar = null,
    cancelar = voltar ? 'Voltar' : 'Cancelar',
    aoCancelar = voltar,
    classe = '',
    protegido = false,
    aoEnviar,
    aoMontar,
    aoFechar,
  },
  h,
) {
  const anterior = atual ? atual.anterior : document.activeElement;
  if (atual) atual.fechar(false);

  const fundo = document.createElement('div');
  fundo.className = 'dlg-fundo';
  fundo.innerHTML = `<form class="dlg ${classe}" role="dialog" aria-modal="true" aria-labelledby="dlg-t" novalidate>
  <h3 id="dlg-t">${titulo}</h3>
  <div class="dlg-corpo">${corpo}</div>
  <p class="dlg-erro" role="alert" hidden></p>
  <div class="dlg-pe">
    ${cancelar ? `<button type="button" class="bt" data-cancelar>${cancelar}</button>` : ''}
    ${enviar ? `<button type="submit" class="bt ${perigo ? 'perigo' : 'pri'}">${enviar}</button>` : ''}
  </div>
</form>`;
  const form = fundo.querySelector('form');
  const erro = fundo.querySelector('.dlg-erro');

  const estado = { anterior, protegido };
  let enviado = false;
  const fechar = (devolverFoco = true) => {
    if (atual !== estado) return;
    atual = null;
    fundo.remove();
    document.removeEventListener('keydown', teclado, true);
    if (devolverFoco && anterior instanceof HTMLElement) {
      if (document.contains(anterior)) anterior.focus({ preventScroll: true });
      else restaurarFoco(anterior);
    }
    aoFechar?.(enviado);
    soltarEsperando();
  };
  estado.fechar = fechar;

  const mostrarErro = (msg) => {
    erro.textContent = msg;
    erro.hidden = !msg;
  };

  function teclado(ev) {
    if (ev.key === 'Escape') {
      ev.preventDefault();
      ev.stopPropagation();
      if (!protegido && !enviando) fechar();
      return;
    }
    // Enter no "Mais opções" (summary) abre e fecha, como um botão
    if (ev.key === 'Enter' && !ev.target.matches('button, textarea, select, summary')) {
      // Enter envia também com o foco numa caixa ou numa cor
      ev.preventDefault();
      // num diálogo de perigo, só o clique (ou Enter) no próprio botão envia
      if (enviar && !perigo) form.requestSubmit();
      return;
    }
    if (ev.key === 'Tab') {
      const f = [...form.querySelectorAll(FOCAVEIS)].filter((el) => el.offsetParent !== null || el === document.activeElement);
      // rádios do mesmo grupo contam como uma parada só
      const paradas = f.filter((el) => el.type !== 'radio' || el.checked || !form.querySelector(`input[name="${el.name}"]:checked`));
      if (!paradas.length) return;
      const i = paradas.indexOf(document.activeElement);
      const prox = ev.shiftKey ? (i <= 0 ? paradas.length - 1 : i - 1) : i === paradas.length - 1 ? 0 : i + 1;
      ev.preventDefault();
      paradas[prox].focus();
    }
  }

  // enquanto o "enviar" roda (um Restaurar, por exemplo), nada mais tira o diálogo: o botão
  // secundário fica desabilitado, e Esc e o clique fora não fecham (o que roda seguiria sem ele)
  let enviando = false;
  const secundario = form.querySelector('[data-cancelar]');
  secundario?.addEventListener('click', () => !enviando && (aoCancelar ? aoCancelar() : fechar()));
  fundo.addEventListener('mousedown', (ev) => {
    if (ev.target === fundo && !protegido && !enviando) fechar();
  });
  form.addEventListener('submit', async (ev) => {
    ev.preventDefault();
    if (!aoEnviar) return fechar();
    const bt = form.querySelector('[type="submit"]');
    if (bt?.disabled || enviando) return;
    if (bt) bt.disabled = true;
    enviando = true;
    if (secundario) secundario.disabled = true;
    mostrarErro('');
    try {
      await aoEnviar(form);
      enviado = true;
      enviando = false;
      fechar();
    } catch (e) {
      console.error('worktimer: diálogo', e);
      mostrarErro(texto(e));
    } finally {
      enviando = false;
      if (bt) bt.disabled = false;
      if (secundario) secundario.disabled = false;
    }
  });

  document.addEventListener('keydown', teclado, true);
  document.body.append(fundo);
  atual = estado;
  aoMontar?.(form);
  const primeiro = form.querySelector('[autofocus]') ?? form.querySelector('.dlg-corpo ' + FOCAVEIS) ?? form.querySelector(FOCAVEIS);
  primeiro?.focus();
  if (primeiro?.select && primeiro.type === 'text') primeiro.select();
  Object.assign(h, { fechar, mostrarErro, form });
}

// ---- faixas com "Desfazer" ----

let faixaAtual = null;
/** Quem mostra as faixas no lugar do rodapé da página (o dock: a janela dos avisos, rodada 15). */
let faixaDeFora = null;

/** As faixas desta página passam a ir para `fn(msg, botao, aoClicar, ms)`. */
export function usarFaixa(fn) {
  faixaDeFora = fn;
}

/** Faixa no rodapé com um botão; some sozinha depois de `ms`. */
export function faixa(msg, botao = null, aoClicar = null, ms = 4000) {
  if (faixaDeFora) return faixaDeFora(msg, botao, aoClicar, ms);
  faixaAtual?.remove();
  const el = document.createElement('div');
  el.className = 'faixa';
  el.setAttribute('role', 'status');
  el.innerHTML = `<span>${esc(msg)}</span>${botao ? `<button type="button">${esc(botao)}</button>` : ''}`;
  const id = setTimeout(() => el.remove(), ms);
  el.querySelector('button')?.addEventListener('click', async () => {
    clearTimeout(id);
    try {
      await aoClicar();
      el.remove();
    } catch (e) {
      console.error('worktimer: faixa', e);
      el.querySelector('span').textContent = texto(e);
      el.querySelector('button').remove();
      setTimeout(() => el.remove(), 4000);
    }
  });
  document.body.append(el);
  faixaAtual = el;
  return el;
}

// ---- faixa "Encerrado. Desfazer" ----

/** Por quanto tempo o ■ pode ser desfeito na tela (o núcleo aceita até 60 s). */
export const DESFAZER_ENCERRAR_MS = 10_000;

/** Texto da faixa depois do ■. */
export const textoDaFaixaEncerrado = (nome) => (nome ? `${nome} encerrado.` : 'Encerrado.');

/** Faixa "<timer> encerrado. Desfazer" por 10 s, como a do zerar. */
export function faixaEncerrado(timerId, nome) {
  return faixa(textoDaFaixaEncerrado(nome), 'Desfazer', () => chamar('desfazer_encerrar', { timer_id: timerId }), DESFAZER_ENCERRAR_MS);
}

// ---- faixa "Cliente A zerado. Desfazer" ----

/** Por quanto tempo o ↺ e o "Zerar todos" podem ser desfeitos (a faixa). */
export const DESFAZER_ZERAR_MS = 10_000;

/** Texto da faixa depois do ↺. */
export const textoDaFaixaZerado = (nome) => (nome ? `${nome} zerado.` : 'Zerado.');

/** Texto da faixa depois do ↺ "Só <atividade>" (rodada 19). */
export const textoDaFaixaZeradoAtividade = (timer, atividade) =>
  `${timer ? `${timer}: ` : ''}só ${atividade} voltou a 0:00.`;

/** Texto da faixa depois do "Zerar todos". */
export const TEXTO_ZERADOS_TODOS = 'Todos os timers zerados.';

/**
 * Faixa do zerar (sem diálogo de confirmação: nada é apagado) por 10 s; o "Desfazer" volta o
 * zerar de antes de cada timer. `zerado`: o que o `zerar`, o `zerar_atividade` ou o `zerar_todos`
 * devolveu.
 */
export function faixaZerado(zerado, texto) {
  return faixa(texto, 'Desfazer', () => chamar('desfazer_zerar', { zerado }), DESFAZER_ZERAR_MS);
}

// ---- campos ----

const campoTexto = (nome, rotulo, valor = '', dica = '', extra = '') => `<label class="campo">
  <span>${rotulo}</span>
  <input type="text" name="${nome}" value="${esc(valor ?? '')}" autocomplete="off" spellcheck="false" ${extra}>
  ${dica ? `<small>${dica}</small>` : ''}
</label>`;

function campoCor(atualCor, nome = 'cor', legenda = 'Cor') {
  const c = cor(atualCor);
  const cores = PALETA.includes(c) ? PALETA : [...PALETA, c];
  const sw = cores
    .map(
      (x) =>
        `<label class="sw" style="--c:${x}" title="${x}"><input type="radio" name="${nome}" value="${x}"${x === c ? ' checked' : ''} aria-label="${legenda} ${x}"><i></i></label>`,
    )
    .join('');
  return `<fieldset class="campo cores"><legend>${legenda}</legend><div class="sws">${sw}</div></fieldset>`;
}

/**
 * "Mais opções": o que só alguns usam (o card do Zoho, a sessão do Claude e a regra de pasta, o
 * ajuste à mão, arquivar), fechado por padrão. `resumo` diz, ainda fechado, o que já tem dentro.
 */
export const maisOpcoes = (conteudo, aberto = false, resumo = '') =>
  `<details class="mais"${aberto ? ' open' : ''}><summary>Mais opções${resumo ? ` <small>${esc(resumo)}</small>` : ''}</summary><div class="mais-corpo">${conteudo}</div></details>`;

/** Foca o campo, abrindo antes o "Mais opções" em que ele estiver. */
function focar(el) {
  const d = el?.closest?.('details');
  if (d) d.open = true;
  el?.focus?.();
}

const caixa = (nome, rotulo, dica = '', marcada = false) => `<label class="chk">
  <input type="checkbox" name="${nome}"${marcada ? ' checked' : ''}><span>${rotulo}${dica ? `<small>${dica}</small>` : ''}</span>
</label>`;

/** Valor da opção "+ Novo grupo…" no seletor de grupo do timer. */
export const NOVO_GRUPO = 'novo';

/**
 * Seletor de grupo do timer, com "+ Novo grupo…" no fim: escolhido, aparecem ali mesmo o nome e a
 * cor do grupo novo, que é criado junto com o timer ao salvar.
 */
export function campoGrupo(snap, grupoId, espacoId = espacoPreEscolhido(snap, null), grupoArquivado = null) {
  const grupos = snap.grupos.filter((g) => g.id !== null);
  const op = (g) => `<option value="${g.id}"${g.id === grupoId ? ' selected' : ''}>${esc(g.nome)}</option>`;
  // o grupo atual fora da lista (arquivado): fica como opção marcada, para salvar sem mexer não
  // trocar em silêncio (re-revisão da rodada 20)
  const fora =
    grupoId !== null && grupoId !== undefined && !grupos.some((g) => g.id === grupoId)
      ? `<option value="${grupoId}" selected>${esc(grupoArquivado ?? TEXTOS.grupo)} (${esc(TEXTOS.arquivado)})</option>`
      : '';
  // com 2 ou mais espaços, os grupos vêm separados por espaço (rodada 20)
  const ops = comAbas(snap)
    ? gruposPorEspaco(snap)
        .map(({ espaco, grupos: gs }) => {
          const dele = gs.filter((g) => g.id !== null);
          return dele.length ? `<optgroup label="${esc(espaco.nome)}">${dele.map(op).join('')}</optgroup>` : '';
        })
        .join('')
    : grupos.map(op).join('');
  return `<label class="campo"><span>Grupo</span><select name="grupo">${fora}${ops}<option value=""${grupoId === null ? ' selected' : ''}>${esc(TEXTOS.semGrupo)}</option><option value="${NOVO_GRUPO}">+ Novo grupo…</option></select></label>
<div class="timer-espaco"${grupoId === null ? '' : ' hidden'}>${campoEspaco(snap, espacoId, 'espaco', TEXTOS.espacoDoTimerDica)}</div>
<div class="grupo-novo" hidden>
  ${campoTexto('grupo_nome', 'Nome do grupo novo', '', '', 'placeholder="ex.: Clientes"')}
  ${campoCor(corLivre(grupos.map((g) => g.cor)), 'grupo_cor', 'Cor do grupo')}
</div>`;
}

/**
 * O seletor de espaço (rodada 20): os não arquivados, na ordem (e o atual, se arquivado, para não
 * trocar sem querer). `dica`: o texto pequeno embaixo. Com um espaço só (revisão da rodada 20),
 * não aparece: a tela fica como antes.
 */
export function campoEspaco(snap, espacoId, nome = 'espaco', dica = '') {
  const lista = espacosAtivos(snap);
  const atual = (snap.espacos ?? []).find((e) => e.id === espacoId);
  if (lista.length < 2 && !atual?.arquivado) return '';
  if (atual && atual.arquivado) lista.push(atual);
  const ops = lista
    .map((e) => `<option value="${e.id}"${e.id === espacoId ? ' selected' : ''}>${esc(e.nome)}${e.arquivado ? ` (${esc(TEXTOS.arquivado)})` : ''}</option>`)
    .join('');
  return `<label class="campo"><span>${esc(TEXTOS.espaco)}</span><select name="${nome}">${ops}</select>${dica ? `<small>${esc(dica)}</small>` : ''}</label>`;
}

/**
 * O espaço escolhido no formulário (ou `padrao`, sem o campo; nulo deixa o núcleo usar o espaço
 * padrão, o primeiro não arquivado).
 */
const espacoDoForm = (form, padrao = null) => {
  const v = Number(ler(form, 'espaco'));
  return Number.isInteger(v) && v > 0 ? v : padrao;
};

/** Mostra o nome e a cor do grupo novo quando "+ Novo grupo…" é escolhido. */
function ligarNovoGrupo(form) {
  const sel = form.elements.grupo;
  const bloco = form.querySelector('.grupo-novo');
  if (!sel || !bloco) return;
  // o espaço vale para o "Sem grupo" e para o grupo novo (rodada 20)
  const espaco = form.querySelector('.timer-espaco');
  sel.addEventListener('change', () => {
    bloco.hidden = sel.value !== NOVO_GRUPO;
    if (espaco) espaco.hidden = sel.value !== '' && sel.value !== NOVO_GRUPO;
    if (!bloco.hidden) form.elements.grupo_nome.focus();
  });
}

/**
 * O grupo escolhido no diálogo do timer: `{ id }` de um grupo que existe (`null` = sem grupo) ou
 * `{ novo: { nome, cor } }`. Lança se o grupo novo estiver sem nome.
 */
export function grupoDoForm(valor, nomeNovo, corNova) {
  if (valor === NOVO_GRUPO) {
    const nome = String(nomeNovo ?? '').trim();
    if (!nome) throw 'Dê um nome ao grupo novo';
    return { novo: { nome, cor: corNova } };
  }
  return { id: valor === '' || valor === undefined ? null : Number(valor) };
}

/** Grupo com esse nome (sem diferenciar maiúsculas nem espaços nas pontas), ou nulo. */
export function grupoComNome(grupos, nome) {
  const chave = String(nome ?? '').trim().toLowerCase();
  return grupos.find((g) => g.id !== null && String(g.nome).trim().toLowerCase() === chave) ?? null;
}

/**
 * O `grupo_id` do timer no envio. Com "+ Novo grupo…", cria o grupo aqui; se já há um grupo com
 * esse nome, usa ele. `feito.grupo` guarda o grupo criado: um "Salvar" de novo depois de um erro
 * (ao criar o timer, por exemplo) reaproveita esse grupo, renomeado se o nome mudou, em vez de
 * criar outro. O que sobrar sem uso sai em `arquivarSobra`.
 */
async function grupoDoTimer(form, snap, feito) {
  let g;
  try {
    g = grupoDoForm(ler(form, 'grupo'), ler(form, 'grupo_nome'), ler(form, 'grupo_cor'));
  } catch (e) {
    form.elements.grupo_nome?.focus();
    throw e;
  }
  if (!g.novo) return g.id;
  const ja = grupoComNome(snap.grupos, g.novo.nome);
  if (ja) return ja.id;
  if (feito.grupo) {
    const { nome, cor } = g.novo;
    if (feito.grupo.nome !== nome || feito.grupo.cor !== cor) {
      await chamar('editar_grupo', { grupo: grupoParaSalvar({ ...feito.grupo, nome, cor }) });
      feito.grupo = { ...feito.grupo, nome, cor };
    }
    return feito.grupo.id;
  }
  const s = await chamar('criar_grupo', { nome: g.novo.nome, cor: g.novo.cor, espaco_id: espacoDoForm(form) });
  const novo = s.grupos.find((x) => x.id === maiorId(s.grupos));
  if (!novo) throw 'Não achei o grupo novo. Tente de novo.';
  feito.grupo = novo;
  return novo.id;
}

/**
 * O grupo criado por este diálogo que ficou sem uso (o diálogo foi cancelado depois de um erro,
 * ou o timer acabou noutro grupo) é arquivado: não fica um grupo vazio nas listas.
 */
async function arquivarSobra(feito, usado) {
  const g = feito.grupo;
  // já tem o timer (mesmo que salvar o card depois falhe): nunca é arquivado
  if (g && g.id === usado) feito.usado = true;
  if (!g || feito.usado) return;
  feito.grupo = null;
  try {
    await chamar('editar_grupo', { grupo: grupoParaSalvar({ ...g, arquivado: true }) });
  } catch (e) {
    console.error('worktimer: arquivar o grupo que sobrou', e);
  }
}

/**
 * Link do Zoho com o nome que a pessoa dá a ele: o link só traz códigos (ex.: `#P146/itemdetails/I7`),
 * então a tela mostra o código e, se houver, o nome ("INV - Inovação · I7").
 */
export function campoLink(nome, rotulo, valor, dica, nomeDoNome, rotuloDoNome, valorDoNome, card = false) {
  return `${campoTexto(nome, rotulo, valor, dica, `data-link="${nomeDoNome}"${card ? ' data-card="1"' : ''}`)}<label class="campo">
  <span>${rotuloDoNome}</span>
  <input type="text" name="${nomeDoNome}" value="${esc(valorDoNome ?? '')}" autocomplete="off" spellcheck="false" placeholder="ex.: INV - Inovação">
  <small data-codigo="${nomeDoNome}">${esc(dicaDoCodigo(valor, card))}</small>
</label>`;
}

/**
 * Dica embaixo do nome: o código achado no link. Num campo de card (`card`), avisa quando o link
 * não aponta para um item do Zoho Sprints. O nome só vale com um link.
 */
export function dicaDoCodigo(link, card = false) {
  const { projeto, item } = codigosDoLink(link);
  const codigo = item ? `item ${item}${projeto ? ` do projeto ${projeto}` : ''}` : projeto ? `projeto ${projeto}` : null;
  const aviso = card && String(link ?? '').trim() && !item ? 'Atenção: este link não aponta para um item (card) do Zoho Sprints. ' : '';
  if (codigo) return `${aviso}O link traz só o código (${codigo}). O nome é opcional e aparece na tela junto com ele.`;
  return `${aviso}Opcional, e só vale com o link. Aparece na tela no lugar do link.`;
}

/** O que identifica o destino de um link: o código do Zoho (item, senão projeto) ou o próprio link. */
const chaveDoLink = (link) => {
  const { projeto, item } = codigosDoLink(link);
  return item ?? projeto ?? String(link ?? '').trim();
};

/**
 * O link mudou de destino: o nome antigo sai (seria o nome de outro card), a menos que a pessoa
 * já tenha mexido no nome. Voltar ao link de antes devolve o nome que saiu sozinho.
 * Devolve { nome, aviso, limpo } (`limpo`: o nome saiu sozinho e pode voltar).
 */
export function nomeAoTrocarLink({ linkAntes, nomeAntes, linkAgora, nomeAgora, limpo = false }) {
  const mudou = chaveDoLink(linkAgora) !== chaveDoLink(linkAntes);
  const antes = String(nomeAntes ?? '').trim();
  const parado = { nome: nomeAgora, aviso: null, limpo: false };
  if (!antes) return parado;
  if (mudou) {
    // o nome antigo (ou já tirado e ainda vazio) sai; um nome escrito pela pessoa fica
    if (nomeAgora === nomeAntes || (limpo && nomeAgora === '')) {
      return { nome: '', aviso: `O link mudou: o nome "${antes}" saiu. Escreva de novo se ainda vale.`, limpo: true };
    }
    return parado;
  }
  // de volta ao destino de antes: o nome que saiu sozinho volta
  return limpo && nomeAgora === '' ? { nome: nomeAntes, aviso: null, limpo: false } : parado;
}

/** A dica do código acompanha o link enquanto ele é colado ou editado; o nome antigo sai se o destino mudar. */
function ligarCodigos(form) {
  for (const el of form.querySelectorAll('input[data-link]')) {
    const campoNome = form.elements[el.dataset.link];
    const linkAntes = el.value;
    const nomeAntes = campoNome?.value ?? '';
    let limpo = false;
    el.addEventListener('input', () => {
      const dica = form.querySelector(`[data-codigo="${el.dataset.link}"]`);
      let texto = dicaDoCodigo(el.value, el.dataset.card === '1');
      if (campoNome) {
        const r = nomeAoTrocarLink({ linkAntes, nomeAntes, linkAgora: el.value, nomeAgora: campoNome.value, limpo });
        campoNome.value = r.nome;
        limpo = r.limpo;
        if (r.aviso) texto = `${r.aviso} ${texto}`;
      }
      if (dica) dica.textContent = texto;
    });
  }
}

/**
 * As opções de um seletor de atividade: as ativas, na ordem da lista, com `escolhida` marcada (e
 * ela no fim, se estiver arquivada, para não sumir do seletor).
 */
export function opcoesDeAtividade(ativas, escolhida) {
  const v = [...ativas];
  const atual = atividadePorId(escolhida);
  if (atual && !v.some((a) => a.id === atual.id)) v.push(atual);
  return v
    .map((a) => `<option value="${a.id}"${a.id === escolhida ? ' selected' : ''}>${esc(a.nome)}${a.arquivada ? ' (arquivada)' : ''}</option>`)
    .join('');
}

/**
 * "Ajustar hoje" do editar timer: a mais ou a menos, minutos, a atividade (a padrão do timer, que
 * é Trabalho se ninguém mudou; o seletor só aparece com mais de uma atividade) e um motivo curto.
 * `escolhida`: a atividade já escolhida (o ✎ da linha dela no painel do timer); sem o seletor,
 * ela vai num campo escondido. No fim, o "Descartar o tempo de hoje…" (o que o ↺ fazia antes da
 * rodada 11), que abre a confirmação (`ligarDescartar`).
 */
export function campoAjuste(timer, ativas = atividadesAtivas(), escolhida = null) {
  const a = ajusteDoTimer(timer);
  const hoje = a
    ? `Hoje já tem ${esc(a.texto)} à mão. Para desfazer, ajuste ao contrário.`
    : `Para o que o ${NOME_APP} não viu (uma reunião longe do PC) ou contou a mais.`;
  const padrao = escolhida ?? timer.atividade_padrao_id ?? TRABALHO;
  const seletor =
    ativas.length > 1
      ? `<span class="am-de">de</span><select name="aj_atividade" aria-label="Atividade do ajuste">${opcoesDeAtividade(ativas, ativas.some((x) => x.id === padrao) ? padrao : ativas[0].id)}</select>`
      : escolhida !== null
        ? `<input type="hidden" name="aj_atividade" value="${Number(escolhida)}">`
        : '';
  return `<fieldset class="campo ajuste-mao"><legend>Ajustar hoje</legend>
<div class="am-linha">
  <select name="aj_sinal" aria-label="A mais ou a menos"><option value="+">a mais</option><option value="-">a menos</option></select>
  <input type="text" name="aj_min" inputmode="numeric" autocomplete="off" placeholder="0" aria-label="Minutos do ajuste"><em>min</em>${seletor}
</div>
<input type="text" name="aj_motivo" maxlength="80" autocomplete="off" placeholder="Motivo (opcional)" aria-label="Motivo do ajuste">
<small>${hoje}</small>
<button type="button" class="am-desc" data-descartar>Descartar o tempo de hoje…</button>
</fieldset>`;
}

/** O grupo como o núcleo grava (`editar_grupo`), com os nomes dos links (sem eles, sumiriam). */
export const grupoParaSalvar = (g) => ({
  id: g.id,
  nome: g.nome,
  cor: g.cor,
  link_projeto: g.link_projeto ?? null,
  card_padrao: g.card_padrao ?? null,
  link_projeto_nome: g.link_projeto_nome ?? null,
  card_padrao_nome: g.card_padrao_nome ?? null,
  ordem: g.ordem,
  arquivado: Boolean(g.arquivado),
  espaco_id: g.espaco_id ?? ESPACO_PRIMEIRO,
});

/** O timer como o núcleo grava (`editar_timer`), com o nome do card. */
export const timerParaSalvar = (t) => ({
  id: t.id,
  grupo_id: t.grupo_id ?? null,
  nome: t.nome,
  cor: t.cor,
  link_card: t.link_card ?? null,
  link_card_nome: t.link_card_nome ?? null,
  ordem: t.ordem,
  arquivado: Boolean(t.arquivado),
  atividade_padrao_id: t.atividade_padrao_id ?? TRABALHO,
  espaco_id: t.espaco_id ?? ESPACO_PRIMEIRO,
});

const ler = (form, nome) => form.elements[nome]?.value ?? '';
const marcado = (form, nome) => Boolean(form.elements[nome]?.checked);

function nomeObrigatorio(form) {
  const nome = ler(form, 'nome').trim();
  if (!nome) {
    form.elements.nome.focus();
    throw 'O nome não pode ficar vazio';
  }
  return nome;
}

/** Primeira cor da paleta que ninguém usa (ou a seguinte, em rodízio). */
function corLivre(usadas) {
  return PALETA.find((c) => !usadas.includes(c)) ?? PALETA[usadas.length % PALETA.length];
}

const todosTimers = (snap) => snap.grupos.flatMap((g) => g.timers);
const maiorId = (v) => v.reduce((m, x) => (x.id !== null && x.id > m ? x.id : m), -Infinity);

// ---- grupos ----

/** "Novo grupo", com o campo Espaço (rodada 20; `espacoId`: o pré-escolhido, o da aba). */
export function novoGrupo(snap, espacoId = espacoPreEscolhido(snap, null)) {
  return new Promise((ok) => {
    abrirDialogo({
      titulo: 'Novo grupo',
      enviar: 'Criar',
      corpo:
        campoTexto('nome', 'Nome', '', '', 'autofocus') +
        campoEspaco(snap, espacoId) +
        campoCor(corLivre(snap.grupos.map((g) => g.cor))) +
        camposDoGrupo({}),
      aoMontar: ligarCodigos,
      async aoEnviar(form) {
        const nome = nomeObrigatorio(form);
        const c = ler(form, 'cor');
        const s = await chamar('criar_grupo', { nome, cor: c, espaco_id: espacoDoForm(form, espacoId) });
        const g = s.grupos.find((x) => x.id === maiorId(s.grupos));
        const links = linksDoGrupo(form);
        if (g && Object.values(links).some(Boolean)) {
          await chamar('editar_grupo', { grupo: grupoParaSalvar({ ...g, ...links }) });
        }
        ok(g?.id ?? null);
      },
      aoFechar: () => ok(null),
    });
  });
}

/** Links do grupo e os nomes deles. */
const camposDoGrupo = (g) =>
  campoLink('link_projeto', 'Link do projeto', g.link_projeto, '', 'link_projeto_nome', 'Nome do projeto', g.link_projeto_nome) +
  campoLink(
    'card_padrao',
    'Card padrão',
    g.card_padrao,
    'os timers do grupo sem card próprio lançam aqui',
    'card_padrao_nome',
    'Nome do card padrão',
    g.card_padrao_nome,
    true,
  );

/** Link e nome do formulário; nome sem link não é guardado (nunca apareceria). */
export function linkComNome(form, nomeLink, nomeNome) {
  const link = vazioParaNulo(ler(form, nomeLink));
  return [link, link ? vazioParaNulo(ler(form, nomeNome)) : null];
}

const linksDoGrupo = (form) => {
  const [link_projeto, link_projeto_nome] = linkComNome(form, 'link_projeto', 'link_projeto_nome');
  const [card_padrao, card_padrao_nome] = linkComNome(form, 'card_padrao', 'card_padrao_nome');
  return { link_projeto, link_projeto_nome, card_padrao, card_padrao_nome };
};

export function editarGrupo(snap, grupoId) {
  const g = snap.grupos.find((x) => x.id === grupoId);
  if (!g) return;
  abrirDialogo({
    titulo: 'Editar grupo',
    corpo:
      campoTexto('nome', 'Nome', g.nome, '', 'autofocus') +
      campoEspaco(snap, g.espaco_id ?? ESPACO_PRIMEIRO) +
      campoCor(g.cor) +
      camposDoGrupo(g) +
      caixa('arquivado', 'Arquivar o grupo', 'O grupo e os timers dele saem das listas.'),
    aoMontar: ligarCodigos,
    async aoEnviar(form) {
      const nome = nomeObrigatorio(form);
      await chamar('editar_grupo', {
        grupo: grupoParaSalvar({
          ...g,
          nome,
          cor: ler(form, 'cor'),
          ...linksDoGrupo(form),
          arquivado: marcado(form, 'arquivado'),
          espaco_id: espacoDoForm(form, g.espaco_id ?? ESPACO_PRIMEIRO),
        }),
      });
    },
  });
}

// ---- espaços (rodada 20) ----

/** "+ Novo → Espaço": nome e cor. Devolve se criou. */
export function novoEspaco(snap) {
  return new Promise((ok) => {
    abrirDialogo({
      titulo: TEXTOS.tituloNovoEspaco,
      enviar: 'Criar',
      corpo:
        campoTexto('nome', 'Nome', '', '', `autofocus maxlength="24" placeholder="${esc(TEXTOS.espacoPlaceholder)}"`) +
        campoCor(corLivre((snap.espacos ?? []).map((e) => e.cor))),
      async aoEnviar(form) {
        const nome = nomeObrigatorio(form);
        await chamar('criar_espaco', { nome, cor: ler(form, 'cor') });
        ok(true);
      },
      aoFechar: () => ok(false),
    });
  });
}

// ---- timers ----

/** Aviso da pasta-mãe (rodada 15): uma regra nela pegaria vários projetos. */
export const avisoPastaMae = (timerNome) =>
  `Esta pasta tem vários projetos dentro. Toda conversa aberta nela vai para ${timerNome}.`;

/**
 * "Sempre que for nesta pasta" (`conf`: a conferência da regra, `{ pasta_mae, outro_timer_nome }`).
 * Vem marcado, a não ser numa pasta-mãe (o aviso diz por quê) ou quando a regra da pasta já é de
 * outro timer (marcar passa a regra para este). Marcar continua possível nos dois casos.
 * `timerNome` já vem escapado (pode ter HTML).
 */
export function campoRegra(pasta, conf, timerNome) {
  const avisos = [];
  if (conf?.pasta_mae) avisos.push(avisoPastaMae(timerNome));
  if (conf?.outro_timer_nome) avisos.push(`A regra desta pasta é de ${esc(conf.outro_timer_nome)}: marcar passa a regra para ${timerNome}.`);
  return (
    caixa('regra', 'Sempre que for nesta pasta', esc(pasta), avisos.length === 0) +
    avisos.map((a) => `<p class="dlg-aviso" role="note">${a}</p>`).join('')
  );
}

/**
 * O começo do "Novo timer" aberto pelo "+ Novo timer…" de uma sessão nova (`pedido`: sessão,
 * nome sugerido pela pasta do projeto, a pasta e a conferência da regra): de qual sessão é e o
 * "sempre nesta pasta" já marcado (a regra de pasta, como no "Ligar"), menos numa pasta-mãe.
 */
export const camposDaSessao = (pedido) =>
  `<p class="dlg-p">Sessão nova do Claude na pasta <code class="aj-cam">${esc(pedido.pasta)}</code>: ela passa a contar no timer novo, desde o início.</p>` +
  campoRegra(pedido.pasta, pedido, 'o timer novo');

const nomeComparavel = (s) => String(s ?? '').trim().toLowerCase();

/** Perguntas que são de uma sessão: a de sessão nova e a de conversa nova numa pasta com regra. */
export const PERGUNTAS_DE_SESSAO = ['sessao_nova', 'conversa_nova'];

/**
 * A pergunta da sessão ("Sessão nova… Em qual timer contar?" ou "Conversa nova: … contando em X
 * pela pasta"), se ainda está no estado.
 */
export const perguntaDaSessao = (snap, sessaoId) =>
  snap?.perguntas?.find((p) => PERGUNTAS_DE_SESSAO.includes(p.tipo) && p.sessao_id === sessaoId) ?? null;

/** O timer com esse nome (sem diferenciar maiúsculas nem espaços nas pontas), ou nulo. */
export function timerComNome(snap, nome) {
  const k = nomeComparavel(nome);
  return k ? (todosTimers(snap).find((t) => nomeComparavel(t.nome) === k) ?? null) : null;
}

/**
 * O que fazer com um "+ Novo timer…" na vez dele, pelo estado de agora: `{ descartar }` se a
 * pergunta da sessão já saiu (ela foi ligada por outro caminho: uma regra trazida, o "Escolher
 * timer…"), `{ ligarA: timer }` se já há um timer com o nome sugerido, ou `{ criar }`.
 */
export function decidirPedido(snap, pedido) {
  if (!perguntaDaSessao(snap, pedido.sessao_id)) return { descartar: true };
  const t = timerComNome(snap, pedido.nome);
  return t ? { ligarA: t } : { criar: true };
}

/** Aviso quando o pedido é descartado (a pergunta da sessão já saiu). */
export const TEXTO_PEDIDO_DESCARTADO = 'A sessão já foi ligada a um timer: nada foi criado.';

/**
 * "Já existe <timer>": ligar a sessão a ele (desde o início, com a regra se marcada) em vez de
 * criar outro com o mesmo nome. "Criar outro" chama `criarOutro()`. Confere a pergunta de novo
 * ao ligar. Resolve quando o diálogo sai.
 */
export function ligarAoExistente(timer, pedido, criarOutro) {
  return new Promise((ok) => {
    let trocou = false;
    abrirDialogo({
      titulo: `Já existe <span class="dlg-cor" style="--c:${cor(timer.cor)}">${esc(timer.nome)}</span>`,
      corpo: `<p class="dlg-p">Ligar a sessão a ele, em vez de criar outro timer com o mesmo nome?</p>${camposDaSessao(pedido)}`,
      enviar: `Ligar a ${esc(timer.nome)}`,
      cancelar: 'Criar outro',
      aoCancelar() {
        trocou = true;
        ok(criarOutro());
      },
      async aoEnviar(form) {
        if (!perguntaDaSessao(await chamar('obter_estado'), pedido.sessao_id)) {
          faixa(TEXTO_PEDIDO_DESCARTADO);
          return;
        }
        await ligar(pedido.sessao_id, timer.id, true, marcado(form, 'regra'));
      },
      aoFechar: () => {
        if (!trocou) ok(null);
      },
    });
  });
}

/**
 * "Novo timer". Com `pedido` (o "+ Novo timer…" da pergunta de sessão nova), o nome vem
 * sugerido pela pasta e, ao criar, a sessão é ligada ao timer novo (desde o início, com a regra
 * de pasta se a caixa ficar marcada), o que resolve a pergunta. Cancelar não muda a pergunta.
 * Ao criar, o estado é conferido de novo: com a pergunta já resolvida, nada é criado; com um
 * timer de mesmo nome (e sem `mesmoNome`), oferece ligar a ele (`ligarAoExistente`).
 */
export function novoTimer(snap, grupoId = null, pedido = null, { mesmoNome = false, espacoId = null } = {}) {
  const grupo = snap.grupos.find((g) => g.id === grupoId && g.id !== null) ?? null;
  const usadas = (grupo?.timers ?? todosTimers(snap)).map((t) => t.cor);
  const corInicial = grupo && !grupo.timers.length && PALETA.includes(grupo.cor) ? grupo.cor : corLivre(usadas);
  // `timer`: o timer já criado (um "Criar" de novo, depois de um erro ao ligar, não cria outro)
  const feito = { grupo: null, timer: null };
  return new Promise((ok) => {
    abrirDialogo({
      titulo: 'Novo timer',
      enviar: 'Criar',
      // só nome, grupo e cor à vista; o resto em "Mais opções" (aberto quando veio de uma sessão,
      // com o "sempre nesta pasta" à vista)
      corpo:
        campoTexto('nome', 'Nome', pedido?.nome ?? '', '', 'autofocus') +
        campoGrupo(snap, grupo?.id ?? null, espacoId ?? espacoPreEscolhido(snap, null)) +
        campoCor(corInicial) +
        maisOpcoes((pedido ? camposDaSessao(pedido) : '') + campoCard({}), Boolean(pedido)),
      aoMontar(form) {
        ligarCodigos(form);
        ligarNovoGrupo(form);
      },
      async aoEnviar(form) {
        const nome = nomeObrigatorio(form);
        let id = feito.timer;
        if (pedido && id === null) {
          const agora = await chamar('obter_estado');
          if (!perguntaDaSessao(agora, pedido.sessao_id)) {
            faixa(TEXTO_PEDIDO_DESCARTADO);
            return;
          }
          const igual = !mesmoNome && timerComNome(agora, nome);
          if (igual) {
            // troca este diálogo pelo "Já existe"; o "Criar outro" volta para cá
            ligarAoExistente(igual, { ...pedido, nome }, () => novoTimer(agora, null, { ...pedido, nome }, { mesmoNome: true }));
            return;
          }
        }
        if (id === null) {
          const grupo_id = await grupoDoTimer(form, snap, feito);
          const s = await chamar('criar_timer', { grupo_id, nome, cor: ler(form, 'cor'), espaco_id: espacoDoForm(form, espacoId ?? espacoPreEscolhido(snap, null)) });
          await arquivarSobra(feito, grupo_id);
          const t = todosTimers(s).find((x) => x.id === maiorId(todosTimers(s)));
          id = t?.id ?? null;
          feito.timer = id;
          const card = cardDoTimer(form);
          if (t && (card.link_card || card.link_card_nome)) {
            await chamar('editar_timer', { timer: timerParaSalvar({ ...t, ...card }) });
          }
        }
        if (pedido && id !== null) await ligar(pedido.sessao_id, id, true, marcado(form, 'regra'));
        ok(id);
      },
      aoFechar: (enviado) => {
        if (!enviado) arquivarSobra(feito, null);
        ok(null);
      },
    });
  });
}

const campoCard = (t) =>
  campoLink('link_card', 'Link do card', t.link_card, 'vazio = usa o card padrão do grupo', 'link_card_nome', 'Nome do card', t.link_card_nome, true);

const cardDoTimer = (form) => {
  const [link_card, link_card_nome] = linkComNome(form, 'link_card', 'link_card_nome');
  return { link_card, link_card_nome };
};

/**
 * O ajuste do formulário: { delta_ms, motivo, atividade_id } ou nulo (sem minutos). Lança se
 * estiver errado. Sem o seletor de atividade, `atividade_id` vem nulo (o núcleo usa a padrão do
 * timer).
 */
export function ajusteDoForm(form) {
  const min = ler(form, 'aj_min').trim();
  const motivo = vazioParaNulo(ler(form, 'aj_motivo'));
  if (!min) {
    if (!motivo) return null;
    focar(form.elements.aj_min);
    throw 'Ajuste: diga quantos minutos.';
  }
  const delta = deltaDoAjuste(ler(form, 'aj_sinal'), min);
  if (delta === null) {
    focar(form.elements.aj_min);
    throw 'Ajuste: use um número inteiro de minutos, de 1 a 1440.';
  }
  const at = ler(form, 'aj_atividade');
  return { delta_ms: delta, motivo, atividade_id: at ? Number(at) : null };
}

/**
 * "Atividade padrão" do editar timer: a do Claude trabalhando, da leitura e do ▶ (um timer de
 * curso pode ter Estudo). Só aparece com mais de uma atividade.
 */
export function campoAtividadePadrao(timer, ativas = atividadesAtivas()) {
  if (ativas.length < 2 && atividadePorId(timer.atividade_padrao_id ?? TRABALHO)?.arquivada !== true) return '';
  return `<label class="campo"><span>Atividade padrão</span><select name="atividade_padrao">${opcoesDeAtividade(ativas, timer.atividade_padrao_id ?? TRABALHO)}</select>
<small>A do Claude trabalhando, da leitura e do ▶. O botão de reunião, ao lado do ▶, conta como Reunião.</small></label>`;
}

/**
 * "Atividades no painel" do editar timer (rodada 20): uma caixa por atividade não arquivada,
 * marcadas as que aparecem (o padrão é todas). A padrão do timer vem marcada e desabilitada, com a
 * dica. Ocultar só esconde a linha no painel: com tempo desde o zerar ou rodando, ela aparece.
 */
export function campoAtividadesNoPainel(timer, ativas = atividadesAtivas()) {
  if (ativas.length < 2) return '';
  const ocultas = new Set(timer.atividades_ocultas ?? []);
  const padrao = timer.atividade_padrao_id ?? TRABALHO;
  const caixas = ativas
    .map((a) => {
      const ehPadrao = a.id === padrao;
      const marcada = ehPadrao || !ocultas.has(a.id);
      const dica = ehPadrao ? ` title="${esc(TEXTOS.atividadePadraoSempre)}"` : '';
      return `<label class="chk at-painel"${dica}><input type="checkbox" name="at_painel" value="${a.id}"${marcada ? ' checked' : ''}${ehPadrao ? ' disabled' : ''}><span><i style="--a:${cor(a.cor)}"></i>${esc(a.nome)}</span></label>`;
    })
    .join('');
  return `<fieldset class="campo at-painel-g"><legend>${esc(TEXTOS.atividadesNoPainel)}</legend>${caixas}<small>${esc(TEXTOS.atividadesNoPainelDica)}</small></fieldset>`;
}

/** As atividades desmarcadas em "Atividades no painel" (nulo sem a seção: ficam as de antes). */
export function atividadesOcultasDoForm(form) {
  const caixas = [...(form.querySelectorAll?.('input[name="at_painel"]') ?? [])];
  if (!caixas.length) return null;
  return caixas.filter((c) => !c.checked && !c.disabled).map((c) => Number(c.value));
}

/** Trocar a atividade padrão no editar timer troca a caixa desabilitada (a padrão aparece sempre). */
function ligarAtividadesNoPainel(form) {
  const sel = form.elements.atividade_padrao;
  if (!sel) return;
  sel.addEventListener('change', () => {
    for (const c of form.querySelectorAll('input[name="at_painel"]')) {
      const ehPadrao = c.value === sel.value;
      if (ehPadrao) c.checked = true;
      c.disabled = ehPadrao;
      c.closest('label').title = ehPadrao ? TEXTOS.atividadePadraoSempre : '';
    }
  });
}

/** O que o "Mais opções" do editar timer já tem dentro, para ver sem abrir. */
export function resumoDoTimer(t) {
  const partes = [];
  if (t.link_card) partes.push('card do Zoho');
  const padrao = t.atividade_padrao_id ?? TRABALHO;
  if (padrao !== TRABALHO) partes.push(atividadePorId(padrao)?.nome.toLowerCase() ?? 'atividade');
  if (ajusteDoTimer(t)) partes.push('ajuste de hoje');
  return partes.join(' · ');
}

// ---- pastas deste timer (rodada 15) ----

/**
 * O que perguntar antes de gravar uma regra (`conf`: a conferência do núcleo), ou nulo: a pasta-mãe
 * e a regra que já é de outro timer pedem um clique a mais. `timerNome` já vem escapado.
 */
export function confirmacaoDaRegra(conf, timerNome) {
  const textos = [];
  if (conf?.pasta_mae) textos.push(avisoPastaMae(timerNome));
  if (conf?.outro_timer_nome) textos.push(`A regra desta pasta é de ${esc(conf.outro_timer_nome)}. Passar para ${timerNome}?`);
  if (!textos.length) return null;
  return { textos, botao: conf.outro_timer_nome ? `Passar para ${timerNome}` : 'Pôr a regra assim mesmo' };
}

/**
 * "Pastas deste timer" no editar timer: as regras de pasta dele, cada uma com "Trocar pasta…" e
 * "Tirar", e "+ Acrescentar pasta…". `pendente`: a confirmação de uma regra escolhida
 * (`confirmacaoDaRegra` com a pasta); `erro`: por que a última pasta não serviu.
 */
export function corpoPastasDoTimer(regras, timerId, pendente = null, erro = '', estadoClaude = null) {
  const deste = (regras ?? []).filter((r) => r.timer_id === timerId);
  const lista = deste.length
    ? `<ul class="pt-lista">${deste
        .map(
          (r) => `<li><code class="aj-cam" title="${esc(r.pasta)}">${esc(r.pasta)}</code><button type="button" class="bt mini" data-pasta="trocar" data-id="${r.id}">Trocar pasta…</button><button type="button" class="bt mini" data-pasta="tirar" data-id="${r.id}">Tirar</button></li>`,
        )
        .join('')}</ul>`
    : '<p class="pt-vazio">Nenhuma pasta. Uma conversa aberta numa pasta daqui entra sozinha neste timer.</p>';
  const conf = pendente
    ? `<div class="dlg-aviso pt-conf" role="alert"><code class="aj-cam">${esc(pendente.pasta)}</code>${pendente.textos.map((x) => `<p>${x}</p>`).join('')}<div class="pt-bts"><button type="button" class="bt mini pri" data-pasta="confirmar">${pendente.botao}</button><button type="button" class="bt mini" data-pasta="cancelar">Cancelar</button></div></div>`
    : '';
  return `<span>Pastas deste timer</span>${linhaClaudeDesconectado(estadoClaude)}${lista}${conf}${erro ? `<p class="pt-erro" role="alert">${esc(erro)}</p>` : ''}
<div class="pt-bts"><button type="button" class="bt mini" data-pasta="acrescentar">+ Acrescentar pasta…</button></div>
<small>Vale para as conversas novas: as que já estão ligadas ficam onde estão.</small>`;
}

/** Avisa a tela de Ajustes (Grupos e regras de pasta) que as regras mudaram. */
function regrasMudaram(regras) {
  document.dispatchEvent(new CustomEvent('regras-mudaram', { detail: regras }));
}

/** Liga os botões das "Pastas deste timer" (tudo grava na hora, fora do "Salvar" do timer). */
function ligarPastasDoTimer(form, timer) {
  const bloco = form.querySelector('[data-bloco="pastas-timer"]');
  if (!bloco) return;
  let regras = [];
  let pendente = null;
  let erro = '';
  let estadoClaude = null;
  const desenhar = () => {
    bloco.innerHTML = corpoPastasDoTimer(regras, timer.id, pendente, erro, estadoClaude);
  };
  ligarLinhaClaude(bloco);
  estadoDoClaude().then((e) => {
    estadoClaude = e;
    desenhar();
  });
  const gravar = async (pasta, trocar) => {
    regras = await chamar('salvar_regra', { pasta, timer_id: timer.id, trocar: trocar ?? null });
    pendente = null;
    erro = '';
    desenhar();
    regrasMudaram(regras);
  };
  const escolher = async (trocar) => {
    // não chamar de `atual`: esse nome é o diálogo aberto (o estado do módulo)
    const trocada = regras.find((r) => r.id === trocar);
    const pasta = await chamar('escolher_pasta', {
      inicial: trocada?.pasta ?? null,
      titulo: trocada ? `Trocar a pasta de ${timer.nome}` : `Pasta para ${timer.nome}`,
    });
    if (!pasta) return;
    const conf = await chamar('conferir_regra', { pasta, timer_id: timer.id });
    if (conf.erro) {
      pendente = null;
      erro = conf.erro;
      return desenhar();
    }
    const c = confirmacaoDaRegra(conf, esc(timer.nome));
    if (c) {
      pendente = { ...c, pasta: conf.pasta, trocar };
      erro = '';
      return desenhar();
    }
    await gravar(conf.pasta, trocar);
  };
  const tirar = async (id) => {
    const r = regras.find((x) => x.id === id);
    if (!r) return;
    regras = await chamar('remover_regra_pasta', { id });
    pendente = null;
    erro = '';
    desenhar();
    regrasMudaram(regras);
    faixa('Regra de pasta tirada.', 'Desfazer', async () => {
      // a pasta que virou regra de outro timer no meio-tempo não é tomada: o erro diz de quem
      regras = await chamar('restaurar_regra', { pasta: r.pasta, timer_id: r.timer_id });
      if (bloco.isConnected) desenhar();
      regrasMudaram(regras);
    }, 10_000);
  };
  bloco.addEventListener('click', async (ev) => {
    const b = ev.target.closest('[data-pasta]');
    if (!b || b.disabled) return;
    const id = b.dataset.id ? Number(b.dataset.id) : null;
    b.disabled = true;
    try {
      switch (b.dataset.pasta) {
        case 'acrescentar':
          return await escolher(null);
        case 'trocar':
          return await escolher(id);
        case 'tirar':
          return await tirar(id);
        case 'confirmar':
          return pendente && (await gravar(pendente.pasta, pendente.trocar));
        case 'cancelar':
          pendente = null;
          return desenhar();
      }
    } catch (e) {
      console.error('worktimer: pastas do timer', e);
      erro = texto(e);
      desenhar();
    } finally {
      if (b.isConnected) b.disabled = false;
    }
  });
  chamar('regras_pasta')
    .then((v) => {
      regras = v;
      desenhar();
    })
    .catch((e) => {
      console.error('worktimer: regras_pasta', e);
      erro = texto(e);
      desenhar();
    });
}

export function editarTimer(snap, timerId) {
  const t = todosTimers(snap).find((x) => x.id === timerId);
  if (!t) return;
  const feito = { grupo: null };
  abrirDialogo({
    titulo: 'Editar timer',
    corpo:
      campoTexto('nome', 'Nome', t.nome, '', 'autofocus') +
      campoGrupo(snap, t.grupo_id, t.espaco_id ?? ESPACO_PRIMEIRO, t.grupo_arquivado ?? null) +
      campoCor(t.cor) +
      `<div class="campo pastas-timer" data-bloco="pastas-timer">${corpoPastasDoTimer([], t.id)}</div>` +
      maisOpcoes(
        campoCard(t) +
          campoAtividadePadrao(t) +
          campoAtividadesNoPainel(t) +
          campoAjuste(t) +
          caixa('arquivado', 'Arquivar o timer', 'Ele sai das listas e o tempo dele sai do total do dia.'),
        false,
        resumoDoTimer(t),
      ),
    aoMontar(form) {
      ligarCodigos(form);
      ligarNovoGrupo(form);
      ligarDescartar(form, snap, t, () => editarTimer(snap, t.id));
      ligarPastasDoTimer(form, t);
      ligarAtividadesNoPainel(form);
    },
    async aoEnviar(form) {
      const nome = nomeObrigatorio(form);
      const ajuste = ajusteDoForm(form);
      // o nome do grupo novo é conferido antes do ajuste (que já grava)
      if (ler(form, 'grupo') === NOVO_GRUPO && !ler(form, 'grupo_nome').trim()) {
        form.elements.grupo_nome.focus();
        throw 'Dê um nome ao grupo novo';
      }
      // o ajuste antes (o timer arquivado não aceita mais ajuste); feito, os campos esvaziam para
      // um "Salvar" de novo, depois de um erro, não ajustar duas vezes
      if (ajuste) await gravarAjuste(form, t.id);
      const grupo_id = await grupoDoTimer(form, snap, feito);
      await chamar('editar_timer', {
        timer: timerParaSalvar({
          ...t,
          grupo_id,
          nome,
          cor: ler(form, 'cor'),
          ...cardDoTimer(form),
          arquivado: marcado(form, 'arquivado'),
          atividade_padrao_id: form.elements.atividade_padrao ? Number(ler(form, 'atividade_padrao')) : t.atividade_padrao_id,
          espaco_id: espacoDoForm(form, t.espaco_id ?? ESPACO_PRIMEIRO),
        }),
        ocultas: atividadesOcultasDoForm(form),
      });
      await arquivarSobra(feito, grupo_id);
    },
    aoFechar: (enviado) => {
      if (!enviado) arquivarSobra(feito, null);
    },
  });
}

/** Grava o ajuste do formulário e mostra a faixa com o "Desfazer". Devolve se havia ajuste. */
async function gravarAjuste(form, timerId) {
  const ajuste = ajusteDoForm(form);
  if (!ajuste) return false;
  const id = await chamar('ajustar_hoje', {
    timer_id: timerId,
    atividade_id: ajuste.atividade_id,
    delta_ms: ajuste.delta_ms,
    motivo: ajuste.motivo,
  });
  form.elements.aj_min.value = '';
  form.elements.aj_motivo.value = '';
  // a faixa sai já, para o "Desfazer" existir mesmo se salvar o resto falhar
  const de = ajuste.atividade_id ? atividadePorId(ajuste.atividade_id) : null;
  faixa(
    `Ajustado à mão: ${textoDoAjuste(ajuste.delta_ms)}${de ? ` de ${de.nome.toLowerCase()}` : ''} hoje.`,
    'Desfazer',
    () => chamar('desfazer_ajuste', { id }),
    10_000,
  );
  return true;
}

/**
 * "Ajustar hoje" sozinho, já numa atividade: o ✎ da linha dela no painel do timer. O mesmo campo
 * do editar timer (dá para trocar a atividade ali).
 */
export function ajustarHoje(snap, timerId, atividadeId) {
  const t = todosTimers(snap).find((x) => x.id === timerId);
  if (!t) return;
  abrirDialogo({
    titulo: `Ajustar hoje: ${esc(t.nome)}`,
    corpo: campoAjuste(t, atividadesAtivas(), atividadeId),
    enviar: 'Ajustar',
    aoMontar(form) {
      // o foco vai nos minutos (a atividade já veio escolhida)
      form.elements.aj_min?.setAttribute('autofocus', '');
      ligarDescartar(form, snap, t, () => ajustarHoje(snap, timerId, atividadeId));
    },
    async aoEnviar(form) {
      if (!(await gravarAjuste(form, t.id))) {
        focar(form.elements.aj_min);
        throw 'Diga quantos minutos.';
      }
    },
  });
}

// ---- descartar o tempo de hoje ----

/** O botão "Descartar o tempo de hoje…" do "Ajustar hoje" abre a confirmação; `voltar` reabre o diálogo de antes. */
function ligarDescartar(form, snap, timer, voltar) {
  form.querySelector('[data-descartar]')?.addEventListener('click', () => confirmarDescartar(snap, timer.id, voltar));
}

/**
 * "Descartar o tempo de hoje de Cliente A?": o timer pausa e o tempo de hoje dele (trechos e ajustes à
 * mão) sai das contas e do total do dia. Era o ↺ "Zerar hoje" até a rodada 11; agora o ↺ só zera
 * o número. Dá para desfazer pela faixa, por 10 s. "Voltar" reabre o diálogo de onde veio.
 */
export function confirmarDescartar(snap, timerId, voltar = null) {
  const t = todosTimers(snap).find((x) => x.id === timerId);
  if (!t) return;
  abrirDialogo({
    titulo: `Descartar o tempo de hoje de ${esc(t.nome)}?`,
    corpo: '<p class="dlg-p">O timer pausa e o tempo de hoje dele sai das contas e do total do dia, inclusive os ajustes à mão. Para só recomeçar o número de 0:00, sem apagar nada, use o ↺ Zerar. Dá para desfazer logo em seguida.</p>',
    enviar: 'Descartar',
    perigo: true,
    voltar,
    async aoEnviar() {
      const d = await chamar('descartar_hoje', { timer_id: t.id });
      if (d.trechos.length || d.ajustes.length) {
        faixa(`Tempo de hoje de ${t.nome} descartado.`, 'Desfazer', () => chamar('desfazer_descartar', { trechos: d.trechos, ajustes: d.ajustes }), 10_000);
      } else {
        faixa('Nada para descartar hoje.');
      }
    },
  });
}

// ---- fechar a janela principal ----

/** Corpo do "Fechar o app?". */
export const corpoFechar = () =>
  `<p class="dlg-p">Na bandeja (os ícones ao lado do relógio) o ${NOME_APP} continua contando. Para voltar, é só clicar no ícone dele.</p>
${caixa('lembrar', 'Não perguntar de novo', 'Dá para mudar em Ajustes.')}`;

/** Um "Fechar o app?" já está esperando outro diálogo fechar. */
let fecharDepois = false;

/**
 * O X da janela principal com `ao_fechar = "perguntar"`. "Manter na bandeja" fecha a janela
 * (a bandeja e o dock continuam); "Sair" encerra o app. Esc ou clicar fora: nada fecha.
 * `antes()` roda antes de responder (as gravações pendentes dos Ajustes).
 * Com outro diálogo aberto (editar um timer, a prévia dos hooks…), ele não é trocado: a
 * pergunta vem quando ele fechar.
 */
export function perguntarAoFechar({ antes = async () => {} } = {}) {
  const aberto = document.querySelector('.dlg-fundo');
  if (aberto) {
    if (!aberto.querySelector('.dlg-fechar') && !fecharDepois) {
      fecharDepois = true;
      faixa(`Termine este diálogo; depois o ${NOME_APP} pergunta se fecha.`);
      const obs = new MutationObserver(() => {
        if (document.querySelector('.dlg-fundo')) return;
        obs.disconnect();
        fecharDepois = false;
        perguntarAoFechar({ antes });
      });
      obs.observe(document.body, { childList: true });
    }
    aberto.querySelector('.dlg [type="submit"], .dlg button')?.focus();
    return;
  }
  const responder = async (form, acao) => {
    const lembrar = marcado(form, 'lembrar');
    await antes();
    await chamar('fechar_principal', { acao, lembrar });
  };
  const d = abrirDialogo({
    titulo: `Fechar o ${NOME_APP}?`,
    corpo: corpoFechar(),
    enviar: 'Manter na bandeja',
    cancelar: 'Sair',
    classe: 'dlg-fechar',
    async aoCancelar() {
      try {
        await responder(d.form, 'sair');
      } catch (e) {
        console.error('worktimer: fechar_principal', e);
        d.mostrarErro(texto(e));
      }
    },
    aoMontar(form) {
      // o foco começa no botão principal, não na caixa
      form.querySelector('[type="submit"]').autofocus = true;
    },
    aoEnviar: (form) => responder(form, 'bandeja'),
  });
}

// ---- minimizar a janela principal ----

/** Corpo do "Minimizar para a bandeja?". */
export const corpoMinimizar = () =>
  `<p class="dlg-p">Na bandeja a janela fecha e sai da memória, e o ${NOME_APP} continua contando; para voltar, é só clicar no ícone dele. Na barra de tarefas ela só fica minimizada.</p>
${caixa('lembrar', 'Não perguntar de novo', 'Dá para mudar em Ajustes.')}`;

/**
 * O minimizar da janela principal com `ao_minimizar = "perguntar"` (o núcleo já trouxe a janela
 * de volta). "Bandeja" fecha a janela como o X; "Barra de tarefas" minimiza. Esc ou clicar fora:
 * a janela fica aberta. Com outro diálogo aberto, não pergunta: minimiza na barra.
 * `antes()` roda antes de responder (as gravações pendentes dos Ajustes).
 */
export function perguntarAoMinimizar({ antes = async () => {} } = {}) {
  const responder = async (form, acao) => {
    const lembrar = form ? marcado(form, 'lembrar') : false;
    await antes();
    await chamar('minimizar_principal', { acao, lembrar });
  };
  if (document.querySelector('.dlg-fundo')) {
    responder(null, 'barra').catch((e) => console.error('worktimer: minimizar_principal', e));
    return;
  }
  const d = abrirDialogo({
    titulo: 'Minimizar para a bandeja?',
    corpo: corpoMinimizar(),
    enviar: 'Bandeja',
    cancelar: 'Barra de tarefas',
    classe: 'dlg-minimizar',
    async aoCancelar() {
      try {
        await responder(d.form, 'barra');
        d.fechar();
      } catch (e) {
        console.error('worktimer: minimizar_principal', e);
        d.mostrarErro(texto(e));
      }
    },
    aoMontar(form) {
      form.querySelector('[type="submit"]').autofocus = true;
    },
    aoEnviar: (form) => responder(form, 'bandeja'),
  });
}

// ---- sessões do Claude ----

/**
 * Liga a sessão ao timer. Se a sessão tem a pergunta dela pendente (sessão nova ou conversa nova),
 * responde a pergunta (assim ela some do dock). A resposta "ligar" sempre liga desde o início;
 * para "daqui para frente", liga pelo comando e depois dispensa a pergunta de sessão nova (a da
 * conversa nova sai sozinha ao ligar).
 * Lê o snapshot na hora, para não responder uma pergunta que já saiu.
 */
export async function ligar(sessaoId, timerId, desdeInicio, regra, soHoje = false) {
  const p = perguntaDaSessao(await chamar('obter_estado'), sessaoId);
  if (p && desdeInicio) {
    // a resposta da conversa nova já leva só os trechos de hoje (o núcleo)
    return chamar('responder', { pergunta_id: p.id, valor: `ligar:${timerId}${regra ? ':regra' : ''}` });
  }
  const s = await chamar('ligar_sessao', {
    sessao_id: sessaoId,
    timer_id: timerId,
    desde_inicio: desdeInicio,
    criar_regra: regra,
    so_hoje: soHoje,
  });
  return p?.tipo === 'sessao_nova' ? chamar('responder', { pergunta_id: p.id, valor: 'nao' }) : s;
}

// ---- o ✎ da sessão ligada (rodada 15) ----

/** Como a sessão entrou no timer, para o ✎ dela. */
export function comoEntrou(sessao) {
  switch (sessao?.ligacao) {
    case 'regra':
      return sessao.regra_pasta ? `pela pasta ${sessao.regra_pasta}` : 'pela pasta';
    case 'mao':
      return 'ligada à mão';
    case 'herdada':
      return `continuação de “${sessao.nome}”`;
    case 'leitura':
      return 'pela leitura';
    default:
      // de antes da versão que grava a origem (`deduzida`), ou sem origem gravada
      return 'ligada antes desta versão do app';
  }
}

/**
 * O corpo do ✎ da sessão: nome, pasta (só leitura), como entrou e o que dá para fazer. Ligada pela
 * regra, também "Trocar por uma subpasta…" e "Apagar esta regra".
 */
export function corpoSessao(sessao) {
  const regra = sessao.ligacao === 'regra' && sessao.regra_id != null;
  return `<label class="campo"><span>Nome</span><input type="text" value="${esc(sessao.nome)}" readonly></label>
<label class="campo"><span>Pasta</span><input type="text" class="mono" value="${esc(sessao.cwd)}" readonly title="${esc(sessao.cwd)}"></label>
<div class="campo"><span>Como entrou</span><p class="ses-como">${esc(comoEntrou(sessao))}</p></div>
<div class="ses-acoes">
  <button type="button" class="bt" data-ses="mover" autofocus>Mover para outro timer…</button>
  <button type="button" class="bt" data-ses="desligar">Desligar deste timer</button>
  ${regra ? '<button type="button" class="bt" data-ses="subpasta">Trocar por uma subpasta…</button><button type="button" class="bt perigo-txt" data-ses="apagar-regra">Apagar esta regra</button>' : ''}
</div>
<small class="ses-dica">Desligar: a conversa fica sem timer e volta a perguntar na próxima mensagem; o tempo já contado fica onde está.${regra ? ' A regra vale para as conversas novas: mexer nela não muda as já ligadas.' : ''}</small>`;
}

/** A sessão `sessaoId` ligada ao timer `timerId`, no estado. */
const sessaoNoTimer = (snap, timerId, sessaoId) =>
  todosTimers(snap)
    .find((t) => t.id === timerId)
    ?.sessoes?.find((s) => s.id === sessaoId) ?? null;

/** O ✎ de uma sessão ligada, no painel do timer. */
export function editarSessao(lerSnap, timerId, sessaoId) {
  const snap = lerSnap();
  const timer = todosTimers(snap).find((t) => t.id === timerId);
  const sessao = sessaoNoTimer(snap, timerId, sessaoId);
  if (!timer || !sessao) return;
  const reabrir = () => editarSessao(lerSnap, timerId, sessaoId);
  const d = abrirDialogo({
    titulo: `Sessão em <span class="dlg-cor" style="--c:${cor(timer.cor)}">${esc(timer.nome)}</span>`,
    corpo: corpoSessao(sessao),
    enviar: null,
    cancelar: 'Fechar',
    classe: 'dlg-sessao',
    aoMontar(form) {
      form.querySelector('.ses-acoes').addEventListener('click', async (ev) => {
        const b = ev.target.closest('[data-ses]');
        if (!b || b.disabled) return;
        b.disabled = true;
        try {
          switch (b.dataset.ses) {
            case 'mover':
              d.fechar();
              return await ligarATimer(lerSnap, sessao.id, null, sessao, { soHoje: true });
            case 'desligar':
              await chamar('desligar_sessao', { sessao_id: sessao.id });
              return d.fechar();
            case 'subpasta':
              return await trocarPorSubpasta(d, sessao);
            case 'apagar-regra':
              return apagarRegraDaSessao(sessao, timer, reabrir);
          }
        } catch (e) {
          console.error('worktimer: sessão', e);
          d.mostrarErro(texto(e));
        } finally {
          if (b.isConnected) b.disabled = false;
        }
      });
    },
  });
}

/** "Trocar por uma subpasta…": o seletor abre na pasta da regra; o núcleo confere a escolha. */
async function trocarPorSubpasta(d, sessao) {
  const pasta = await chamar('escolher_pasta', { inicial: sessao.regra_pasta, titulo: 'Subpasta para a regra' });
  if (!pasta) return;
  const regras = await chamar('trocar_regra_por_subpasta', { regra_id: sessao.regra_id, pasta, sessao_id: sessao.id });
  d.fechar();
  regrasMudaram(regras);
  faixa(`A regra agora vale em ${pasta}.`);
}

/** "Apagar esta regra": confirma (Voltar volta ao ✎) e apaga, com "Desfazer" por 10 s. */
function apagarRegraDaSessao(sessao, timer, voltar) {
  abrirDialogo({
    titulo: 'Apagar esta regra?',
    corpo: `<p class="dlg-p">Conversas novas em <code class="aj-cam">${esc(sessao.regra_pasta)}</code> deixam de entrar sozinhas em ${esc(timer.nome)} e voltam a perguntar. As já ligadas ficam onde estão.</p>`,
    enviar: 'Apagar',
    perigo: true,
    voltar,
    async aoEnviar() {
      const regras = await chamar('remover_regra_pasta', { id: sessao.regra_id });
      regrasMudaram(regras);
      faixa('Regra de pasta apagada.', 'Desfazer', async () => {
        // a pasta que virou regra de outro timer no meio-tempo não é tomada: o erro diz de quem
        regrasMudaram(await chamar('restaurar_regra', { pasta: sessao.regra_pasta, timer_id: timer.id }));
      }, 10_000);
    },
  });
}

/** Mover uma sessão que começou há menos que isto: o padrão é "desde o início". */
export const MOVER_DESDE_INICIO_MS = 15 * 60_000;

/**
 * O "desde o início" vem marcado? Sim para a sessão sem timer, para a que começou há menos de
 * 15 min (a regra de pasta acabou de ligá-la errado: o pouco que contou vai junto) e para a
 * conversa nova que ainda pergunta "Certo?" (rodada 15: ela foi para a regra sem você escolher).
 * Mover uma sessão mais antiga, de outro timer, começa em "daqui para frente".
 */
export function desdeInicioPadrao(sessao, agora) {
  if (sessao?.timer_id === null || sessao?.timer_id === undefined) return true;
  if (sessao.conversa_nova) return true;
  return typeof sessao.criada_em === 'number' && agora - sessao.criada_em < MOVER_DESDE_INICIO_MS;
}

/** Passo 2, comum aos dois caminhos: desde quando e se vira regra da pasta. */
/** A conferência da regra para a pasta (sem núcleo, como se nada houvesse). */
async function conferirRegra(pasta, timerId) {
  try {
    return await chamar('conferir_regra', { pasta, timer_id: timerId });
  } catch (e) {
    console.error('worktimer: conferir_regra', e);
    return { pasta, pasta_mae: false, outro_timer_nome: null, erro: null };
  }
}

/** O texto do "Desde o início" no passo de ligar (`soHoje`: só os trechos de hoje vêm). */
export const textoDesdeInicio = (timerNome, soHoje) =>
  soHoje
    ? `O tempo que a sessão contou hoje vem para ${timerNome}. Os dias anteriores ficam onde estão.`
    : `O tempo que a sessão já contou vem para ${timerNome}.`;

async function passoOpcoes(sessao, timer, voltar) {
  const inicio = desdeInicioPadrao(sessao, Date.now());
  const conferencia = await conferirRegra(sessao.cwd, timer.id);
  // não chamar de `marcado`: esse nome é o leitor do formulário (linha ~552), usado no aoEnviar
  const atributoMarcado = (sim) => (sim ? ' checked autofocus' : '');
  abrirDialogo({
    titulo: `Ligar <b>${esc(sessao.nome)}</b> a <span class="dlg-cor" style="--c:${cor(timer.cor)}">${esc(timer.nome)}</span>`,
    corpo: `<p class="dlg-p">Desde o início ou daqui para frente?</p>
<div class="escolhas" role="radiogroup" aria-label="Desde quando">
  <label class="esc"><input type="radio" name="desde" value="inicio"${atributoMarcado(inicio)}><span><b>Desde o início</b><small>${textoDesdeInicio(esc(timer.nome), Boolean(sessao.so_hoje))}</small></span></label>
  <label class="esc"><input type="radio" name="desde" value="agora"${atributoMarcado(!inicio)}><span><b>Daqui para frente</b><small>Só o tempo a partir de agora.</small></span></label>
</div>
${conferencia.erro ? '' : campoRegra(conferencia.pasta, conferencia, esc(timer.nome))}`,
    enviar: 'Ligar',
    voltar,
    async aoEnviar(form) {
      await ligar(sessao.id, timer.id, ler(form, 'desde') === 'inicio', marcado(form, 'regra'), Boolean(sessao.so_hoje));
    },
  });
}

async function buscarSessoes() {
  try {
    return { sessoes: await chamar('sessoes_recentes') };
  } catch (e) {
    console.error('worktimer: sessoes_recentes', e);
    return { erro: texto(e) };
  }
}

/** "+ Ligar uma sessão" de um timer: escolhe a sessão, depois desde quando. */
export async function ligarSessao(lerSnap, timerId) {
  const snap = lerSnap();
  const timer = todosTimers(snap).find((x) => x.id === timerId);
  if (!timer) return;
  const { sessoes, erro } = await buscarSessoes();
  const estadoClaude = await estadoDoClaude();
  const nomeTimer = new Map(todosTimers(snap).map((t) => [t.id, t.nome]));
  const passo1 = () => {
    const itens = (sessoes ?? [])
      .map((s, i) => {
        const mesma = s.timer_id === timer.id;
        const onde = mesma ? 'já ligada' : s.timer_id !== null ? `em ${esc(nomeTimer.get(s.timer_id) ?? 'outro timer')}` : '';
        return `<button type="button" class="it" data-i="${i}"${mesma ? ' disabled' : ''}>
  <span class="it-n">${esc(s.nome)}</span><span class="it-e">${esc(ESTADO_SESSAO[s.estado] ?? s.estado)}</span>
  <small class="it-p">${esc(s.cwd)}</small>${onde ? `<small class="it-t">${onde}</small>` : ''}
</button>`;
      })
      .join('');
    const vazio = erro
      ? `<p class="dlg-p">${esc(erro)}</p>`
      : '<p class="dlg-p">Nenhuma sessão do Claude nos últimos 7 dias.</p>';
    abrirDialogo({
      titulo: `Ligar uma sessão a <span class="dlg-cor" style="--c:${cor(timer.cor)}">${esc(timer.nome)}</span>`,
      corpo: linhaClaudeDesconectado(estadoClaude) + (itens ? `<div class="lista" data-rolagem="sessoes">${itens}</div>` : vazio),
      enviar: null,
      aoMontar(form) {
        ligarLinhaClaude(form);
        form.querySelector('.lista')?.addEventListener('click', (ev) => {
          const b = ev.target.closest('.it');
          if (b && !b.disabled) passoOpcoes(sessoes[Number(b.dataset.i)], timer, passo1);
        });
      },
    });
  };
  passo1();
}

/** A sessão quando ela não vem em `sessoes_recentes` (o comando falhou, ou ela saiu da lista). */
export const sessaoDeReserva = (id, r = {}) => ({
  id,
  nome: r.nome || 'Sessão do Claude',
  cwd: r.cwd ?? '',
  timer_id: r.timer_id ?? null,
  ...(typeof r.criada_em === 'number' ? { criada_em: r.criada_em } : {}),
});

/** Corpo do "Ligar a um timer": os timers por grupo e, com `comNovo`, o "+ Novo timer…" no fim. */
/**
 * "O Claude Code não está conectado · Conectar" (rodada 18), nos diálogos de ligar sessões e
 * pastas a um timer, quando os hooks estão desligados. O "Conectar" pede o diálogo à janela
 * principal (`conectar-claude`).
 */
export const linhaClaudeDesconectado = (estado) =>
  estado === 'desligado'
    ? '<p class="cx-aviso" role="note">O Claude Code não está conectado · <button type="button" class="cx-link" data-conectar-claude>Conectar</button></p>'
    : '';

/**
 * O estado dos hooks agora (sem leitura: nulo, e a linha não aparece). Sem o Claude Code neste
 * PC, "ausente": a linha não aparece (o "Conectar" não teria o que conectar).
 */
async function estadoDoClaude() {
  try {
    const c = await chamar('conexao_claude');
    return c.encontrado === false ? 'ausente' : c.estado;
  } catch {
    return null;
  }
}

/** O clique no "Conectar" da linha: o diálogo de conectar abre por cima. */
function ligarLinhaClaude(raizEl) {
  raizEl?.addEventListener('click', (ev) => {
    if (ev.target.closest('[data-conectar-claude]')) document.dispatchEvent(new CustomEvent('conectar-claude'));
  });
}

export function corpoLigarATimer(snap, sessao, comNovo = false, estadoClaude = null) {
  // por espaço e grupo (rodada 20; com um espaço só, por grupo, como antes)
  const htmlDosGrupos = (gs) =>
    gs
      .filter((g) => g.timers.length)
      .map(
        (g) =>
          `<div class="lista-g">${esc(g.nome)}</div>` +
          g.timers
            .map(
              (t) =>
                `<button type="button" class="it it-t1" data-timer="${t.id}" style="--c:${cor(t.cor)}"${t.id === sessao.timer_id ? ' disabled' : ''}><i></i><span class="it-n">${esc(t.nome)}</span>${t.id === sessao.timer_id ? '<small class="it-t">já ligada</small>' : ''}</button>`,
            )
            .join(''),
      )
      .join('');
  const doEspaco = ({ espaco, grupos: gs }) => {
    const html = htmlDosGrupos(gs);
    return html && espaco ? `<div class="lista-e" style="--e:${cor(espaco.cor)}"><i></i>${esc(espaco.nome)}</div>${html}` : html;
  };
  const grupos = gruposPorEspaco(snap).map(doEspaco).join('');
  const novo = comNovo ? '<button type="button" class="it it-novo" data-novo="1"><span class="it-n">+ Novo timer…</span></button>' : '';
  const lista = grupos
    ? `<div class="lista">${grupos}${novo}</div>`
    : `<p class="dlg-p">Nenhum timer ainda.</p>${novo ? `<div class="lista">${novo}</div>` : ''}`;
  return `<p class="dlg-p"><b>${esc(sessao.nome)}</b>${sessao.cwd ? `<br><small class="it-p">${esc(sessao.cwd)}</small>` : ''}</p>
${linhaClaudeDesconectado(estadoClaude)}${lista}`;
}

/**
 * "Ligar a um timer": escolhe o timer para uma sessão (a opção "Outro timer…" / "Escolher timer…"
 * da pergunta de sessão nova, no dock). `lerSnap()` devolve o snapshot atual de quem chama.
 * Com `aoNovoTimer(sessaoId)`, a lista termina em "+ Novo timer…" (também sem nenhum timer):
 * o diálogo fecha e quem chamou abre o "Novo timer".
 * `reserva`: o que se sabe da sessão se ela não vier na lista de sessões recentes (o "Trocar"
 * passa o timer dela: sem saber quando ela começou, mover começa em "daqui para frente").
 */
export async function ligarATimer(lerSnap, sessaoId, aoNovoTimer = null, reserva = {}, opcoes = {}) {
  const { sessoes } = await buscarSessoes();
  const achada = sessoes?.find((s) => s.id === sessaoId) ?? sessaoDeReserva(sessaoId, reserva);
  // a conversa nova que pergunta "Certo?" vai desde o início, e o "desde o início" dela e o do
  // "Mover" do ✎ (`opcoes.soHoje`) levam só os trechos de hoje (rodada 15)
  const conversaNova = perguntaDaSessao(lerSnap(), sessaoId)?.tipo === 'conversa_nova';
  const sessao = { ...achada, conversa_nova: conversaNova, so_hoje: conversaNova || Boolean(opcoes.soHoje) };
  const estadoClaude = await estadoDoClaude();
  const passo1 = () => {
    const d = abrirDialogo({
      titulo: 'Ligar a um timer',
      corpo: corpoLigarATimer(lerSnap(), sessao, Boolean(aoNovoTimer), estadoClaude),
      enviar: null,
      aoMontar(form) {
        ligarLinhaClaude(form);
        form.querySelector('.lista')?.addEventListener('click', (ev) => {
          const b = ev.target.closest('.it');
          if (!b || b.disabled) return;
          if (b.dataset.novo) {
            d.fechar();
            aoNovoTimer(sessao.id);
            return;
          }
          const t = todosTimers(lerSnap()).find((x) => x.id === Number(b.dataset.timer));
          if (t) passoOpcoes(sessao, t, passo1);
        });
      },
    });
  };
  passo1();
}
