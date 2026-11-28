// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
/**
 * Spec 004: arquivar com destino (as conversas e as regras de pasta do timer), a lista dos
 * arquivados (Ajustes → Arquivados, com o Desarquivar) e o apagar de verdade: sem horas, direto e
 * com "Desfazer"; com horas, com o nome digitado e um backup antes (o "Arquivar" fica como a
 * recomendação).
 */
import { chamar } from './api.js';
import { abrirDialogo, faixa } from './editar.js';
import { esc, cor } from './componentes.js';
import { curto } from './formato.js';
import { TEXTOS, localeAtual } from './textos.js';

/** Por quanto tempo o arquivar e o apagar sem horas podem ser desfeitos (a faixa). */
export const DESFAZER_ARQUIVO_MS = 10_000;

// o "Ver os arquivados" da faixa: a página diz como abrir os Ajustes nessa seção
let abrirArquivados = () => {};
export function usarAbrirArquivados(fn) {
  abrirArquivados = fn;
}

// o grupo apagado sai do "recolhido" guardado pela página (o id pode voltar num grupo novo; revisão M1)
let esquecerGrupo = () => {};
export function usarEsquecerGrupo(fn) {
  esquecerGrupo = fn;
}

const timersDe = (snap) => (snap?.grupos ?? []).flatMap((g) => g.timers);

/** "03/11" (em inglês, "Nov 3") de um dia "AAAA-MM-DD". */
export function diaCurto(dia) {
  const [a, m, d] = String(dia).split('-').map(Number);
  if (!a || !m || !d) return '';
  const loc = localeAtual();
  if (loc.startsWith('pt')) return `${String(d).padStart(2, '0')}/${String(m).padStart(2, '0')}`;
  return new Intl.DateTimeFormat(loc, { timeZone: 'UTC', month: 'short', day: 'numeric' }).format(new Date(Date.UTC(a, m - 1, d)));
}

/** As horas guardadas para o texto ("12h 40min"), ou vazio sem nenhuma. */
const horasDe = (uso) => (uso?.total_ms >= 60_000 ? curto(uso.total_ms) : '');

/** "3 conversas e a regra da pasta D:\proj" (HTML), ou vazio sem nenhuma das duas. */
export function oQueContaNele(uso) {
  const partes = [];
  if (uso?.conversas) partes.push(`<b>${esc(TEXTOS.nConversas(uso.conversas))}</b>`);
  const regras = uso?.regras ?? [];
  if (regras.length === 1) partes.push(TEXTOS.regraDaPasta(`<code class="aj-cam">${esc(regras[0])}</code>`));
  else if (regras.length > 1) partes.push(TEXTOS.regrasDePastas(regras.length));
  return partes.join(TEXTOS.conjE);
}

/** "As 3 conversas ficam sem timer. A regra da pasta D:\proj sai." (HTML), ou vazio. */
export function oQueAconteceAoApagar(uso) {
  const partes = [];
  if (uso?.conversas) partes.push(esc(TEXTOS.conversasFicamSemTimer(uso.conversas)));
  const regras = uso?.regras ?? [];
  if (regras.length === 1) partes.push(TEXTOS.regraSai(`<code class="aj-cam">${esc(regras[0])}</code>`));
  else if (regras.length > 1) partes.push(esc(TEXTOS.regrasSaem(regras.length)));
  return partes.join(' ');
}

/**
 * A escolha do destino (US 004.2, B1): "Mover para outro timer" (com o seletor) ou "Não contar
 * mais". Vazio quando não há conversa nem regra. `opcoes`: os timers à vista que podem receber.
 * Vem marcado o "Mover" quando há sugestão; senão, o "Não contar mais".
 */
export function htmlDestino(uso, opcoes, doGrupo = false) {
  const oque = oQueContaNele(uso);
  if (!oque) return '';
  const sugestao = opcoes.some((t) => t.id === uso.sugestao) ? uso.sugestao : null;
  const mover = sugestao !== null;
  const lista = opcoes
    .map((t) => `<option value="${t.id}"${t.id === sugestao ? ' selected' : ''}>${esc(t.nome)}</option>`)
    .join('');
  const seletor = opcoes.length
    ? `<select name="destino" aria-label="${esc(TEXTOS.timerDeDestino)}">${sugestao === null ? '<option value="" selected disabled>—</option>' : ''}${lista}</select>`
    : '';
  const varios = (uso.conversas ?? 0) + (uso.regras?.length ?? 0) > 1;
  return `<p class="dlg-p">${(doGrupo ? TEXTOS.contamNoGrupo : TEXTOS.contamNele)(oque, varios)}</p>
<div class="escolhas" role="radiogroup" aria-label="${esc(TEXTOS.timerDeDestino)}">
  <label class="esc"><input type="radio" name="destino_modo" value="mover"${mover ? ' checked' : ''}${opcoes.length ? '' : ' disabled'}><span><b>${esc(TEXTOS.moverParaOutro)}</b><small>${esc(TEXTOS.moverParaOutroDica)}</small>${seletor}</span></label>
  <label class="esc"><input type="radio" name="destino_modo" value="nao"${mover ? '' : ' checked'}><span><b>${esc(TEXTOS.naoContarMais)}</b><small>${esc(TEXTOS.naoContarMaisDica)}</small></span></label>
</div>`;
}

/** O destino escolhido no formulário: o id do timer, ou nulo ("Não contar mais"). */
export function destinoDoForm(form) {
  const modo = form.querySelector('input[name="destino_modo"]:checked')?.value;
  if (modo !== 'mover') return null;
  const v = Number(form.elements.destino?.value);
  if (!v) throw TEXTOS.escolhaODestino;
  return v;
}

/** Escolher um timer no seletor marca o "Mover". */
function ligarDestino(form) {
  form.elements.destino?.addEventListener('change', () => {
    const r = form.querySelector('input[name="destino_modo"][value="mover"]');
    if (r) r.checked = true;
  });
}

/** A faixa depois de arquivar: "Desfazer" (desarquiva) e "Ver os arquivados". */
function faixaArquivado(nome, desfazer) {
  faixa(TEXTOS.faixaArquivado(nome), TEXTOS.desfazer, desfazer, DESFAZER_ARQUIVO_MS, { rotulo: TEXTOS.verArquivados, aoClicar: () => abrirArquivados() });
}

/** "Arquivar…" do ⋯ do cartão (e do editar): o diálogo B1. `depois()`: depois de arquivar. */
export async function arquivarTimer(snap, timerId, depois = () => {}) {
  const t = timersDe(snap).find((x) => x.id === timerId);
  if (!t) return;
  const uso = await chamar('uso_do_timer', { timer_id: timerId });
  const opcoes = timersDe(snap).filter((x) => x.id !== timerId);
  abrirDialogo({
    titulo: esc(TEXTOS.arquivarTimerTitulo(t.nome)),
    corpo: `<p class="dlg-p">${esc(TEXTOS.arquivarTexto(horasDe(uso)))}</p>${htmlDestino(uso, opcoes)}`,
    enviar: TEXTOS.arquivar,
    aoMontar: ligarDestino,
    async aoEnviar(form) {
      await chamar('arquivar_timer', { timer_id: timerId, destino: destinoDoForm(form) });
      faixaArquivado(t.nome, async () => {
        await chamar('desarquivar_timer', { timer_id: timerId });
        depois();
      });
      depois();
    },
  });
}

/** "Arquivar" do ⋯ do grupo e dos Ajustes: o mesmo diálogo, com os timers do grupo. */
export async function arquivarGrupo(snap, grupoId, depois = () => {}) {
  const g = (snap?.grupos ?? []).find((x) => x.id === grupoId);
  if (!g) return;
  const uso = await chamar('uso_do_grupo', { grupo_id: grupoId });
  const opcoes = timersDe(snap).filter((x) => !g.timers.some((t) => t.id === x.id));
  abrirDialogo({
    titulo: esc(TEXTOS.arquivarGrupoTitulo(g.nome)),
    corpo: `<p class="dlg-p">${esc(TEXTOS.arquivarGrupoTexto3(horasDe(uso)))}</p>${htmlDestino(uso, opcoes, true)}`,
    enviar: TEXTOS.arquivar,
    aoMontar: ligarDestino,
    async aoEnviar(form) {
      await chamar('arquivar_grupo', { grupo_id: grupoId, destino: destinoDoForm(form) });
      faixaArquivado(g.nome, async () => {
        await chamar('desarquivar_grupo', { grupo_id: grupoId });
        depois();
      });
      depois();
    },
  });
}

// ---- apagar (US 004.5) ----

/** O aviso do que some: "12h 40min em 6 dias (de 03/11 a 21/11) somem do histórico…". */
export function avisoDoApagar(uso) {
  const periodo = TEXTOS.periodoDosDias(diaCurto(uso.primeiro_dia), diaCurto(uso.ultimo_dia));
  return TEXTOS.apagarAviso(curto(uso.total_ms), uso.dias, periodo);
}

/** Com horas guardadas (ou algo rodando): o apagar pede o nome e faz o backup. */
export const temHoras = (uso) => uso.total_ms > 0 || uso.rodando;

/** O nome digitado confere (sem diferenciar maiúsculas e os espaços das pontas), como no núcleo. */
export const nomeConfere = (digitado, nome) => {
  const n = (s) => String(s ?? '').trim().toLowerCase();
  return n(digitado) !== '' && n(digitado) === n(nome);
};

/** O campo do nome e o botão que só acende com ele certo. */
function ligarNome(form, nome, ativo = () => true) {
  const bt = form.querySelector('[type="submit"]');
  const conferir = () => {
    bt.disabled = ativo() && !nomeConfere(form.elements.nome_confirma?.value, nome);
  };
  form.addEventListener('input', conferir);
  form.addEventListener('change', conferir);
  conferir();
}

const campoDoNome = (nome) =>
  `<label class="campo"><span>${TEXTOS.digiteONome(`<b>${esc(nome)}</b>`)}</span><input type="text" name="nome_confirma" autocomplete="off" spellcheck="false"></label>`;

/**
 * "Apagar…" de um timer (o ⋯ do cartão, o editar e os Arquivados). Sem horas, apaga direto, com
 * "Desfazer"; com horas, o diálogo C2. `arquivado`: ele já está arquivado (sem o "Arquivar").
 */
export async function apagarTimer(snap, timer, { arquivado = false, depois = () => {} } = {}) {
  const uso = await chamar('uso_do_timer', { timer_id: timer.id });
  if (!temHoras(uso)) {
    const a = await chamar('apagar_timer', { timer_id: timer.id, nome: null });
    if (a) faixa(TEXTOS.faixaApagado(timer.nome), TEXTOS.desfazer, async () => { await chamar('desfazer_apagar', { apagado: a }); depois(); }, DESFAZER_ARQUIVO_MS);
    depois();
    return;
  }
  const semTimer = oQueAconteceAoApagar(uso);
  const h = abrirDialogo({
    titulo: esc(TEXTOS.apagarComHorasTitulo(timer.nome)),
    corpo: `<p class="dlg-aviso">${esc(avisoDoApagar(uso))}</p>
${arquivado ? '' : `<p class="dlg-p">${esc(TEXTOS.apagarArquiveAntes)}</p>`}
${semTimer ? `<p class="dlg-p">${semTimer}</p>` : ''}
${campoDoNome(timer.nome)}
<small class="dlg-p">${esc(TEXTOS.backupAntesDeApagar)}</small>`,
    enviar: TEXTOS.apagarComHoras,
    perigo: true,
    // os três botões numa linha só (Cancelar, Arquivar e Apagar)
    classe: 'dlg-apagar',
    aoMontar(form) {
      ligarNome(form, timer.nome);
      if (arquivado || !snap) return;
      // "Arquivar (recomendado)", antes do apagar
      const bt = document.createElement('button');
      bt.type = 'button';
      bt.className = 'bt pri';
      bt.textContent = TEXTOS.arquivarRecomendado;
      bt.addEventListener('click', () => arquivarTimer(snap, timer.id, depois).catch((e) => faixa(typeof e === 'string' ? e : TEXTOS.naoDeuCerto)));
      form.querySelector('.dlg-pe').insertBefore(bt, form.querySelector('[type="submit"]'));
    },
    async aoEnviar(form) {
      await chamar('apagar_timer', { timer_id: timer.id, nome: form.elements.nome_confirma.value });
      faixa(TEXTOS.faixaApagado(timer.nome));
      depois();
    },
  });
  return h;
}

/**
 * "Apagar…" de um grupo (o ⋯ do grupo e os Arquivados): "Só o grupo" (os timers vão para o "Sem
 * grupo", com as horas; com "Desfazer") ou "O grupo e os timers" (com horas, o nome e o backup).
 * Sem timers, apaga direto, com "Desfazer".
 */
export async function apagarGrupo(grupo, { depois = () => {} } = {}) {
  const uso = await chamar('uso_do_grupo', { grupo_id: grupo.id });
  const comDesfazer = async (com_timers) => {
    const a = await chamar('apagar_grupo', { grupo_id: grupo.id, com_timers, nome: null });
    esquecerGrupo(grupo.id);
    if (a) faixa(TEXTOS.faixaApagado(grupo.nome), TEXTOS.desfazer, async () => { await chamar('desfazer_apagar', { apagado: a }); depois(); }, DESFAZER_ARQUIVO_MS);
    depois();
  };
  if (!uso.timers) return comDesfazer(true);
  const horas = temHoras(uso);
  abrirDialogo({
    titulo: esc(TEXTOS.apagarGrupoTitulo(grupo.nome)),
    corpo: `<p class="dlg-p">${esc(TEXTOS.grupoTem(TEXTOS.nTimers(uso.timers)))}</p>
<div class="escolhas" role="radiogroup" aria-label="${esc(TEXTOS.apagarGrupoTitulo(grupo.nome))}">
  <label class="esc"><input type="radio" name="modo" value="so" checked><span><b>${esc(TEXTOS.soOGrupo)}</b><small>${esc(TEXTOS.soOGrupoDica)}</small></span></label>
  <label class="esc"><input type="radio" name="modo" value="tudo"><span><b>${esc(TEXTOS.grupoETimers)}</b><small>${esc(horas ? avisoDoApagar(uso) : TEXTOS.grupoETimersDica(''))}</small></span></label>
</div>
${horas ? `<div data-nome hidden>${campoDoNome(grupo.nome)}<small class="dlg-p">${esc(TEXTOS.backupAntesDeApagar)}</small></div>` : ''}`,
    enviar: TEXTOS.apagar,
    perigo: true,
    aoMontar(form) {
      const tudo = () => form.querySelector('input[name="modo"]:checked')?.value === 'tudo';
      if (horas) {
        form.addEventListener('change', () => {
          form.querySelector('[data-nome]').hidden = !tudo();
        });
        ligarNome(form, grupo.nome, tudo);
      }
    },
    async aoEnviar(form) {
      const tudo = form.querySelector('input[name="modo"]:checked')?.value === 'tudo';
      if (!tudo || !horas) return comDesfazer(tudo);
      await chamar('apagar_grupo', { grupo_id: grupo.id, com_timers: true, nome: form.elements.nome_confirma.value });
      esquecerGrupo(grupo.id);
      faixa(TEXTOS.faixaApagado(grupo.nome));
      depois();
    },
  });
}

// ---- Ajustes → Arquivados (US 004.4) ----

/**
 * A lista dos arquivados (A1): os grupos (com os timers dentro) e os timers soltos, cada um com o
 * tempo guardado, "Desarquivar" e "Apagar…". `dados`: o `arquivados` do núcleo (nulo: lendo).
 * `espacos`: com 2 ou mais à vista, cada item diz o espaço.
 */
export function htmlArquivados(dados, espacos = []) {
  if (!dados) return `<p class="vazio">${esc(TEXTOS.carregando)}</p>`;
  if (dados.erro) return `<p class="aj-erro">${esc(TEXTOS.naoDeuCerto)}</p>`;
  const ativos = espacos.filter((e) => !e.arquivado);
  const doEspaco = (id) => (ativos.length > 1 ? (espacos.find((e) => e.id === id)?.nome ?? '') : '');
  const juntar = (...p) => p.filter(Boolean).map(esc).join(' · ');
  const botoes = (tipo, id, comDialogo) =>
    `<button type="button" class="bt mini" data-aj="desarquivar-${tipo}" data-id="${id}">${esc(TEXTOS.desarquivar)}</button>` +
    `<button type="button" class="bt mini perigo-txt" data-aj="apagar-${tipo}" data-id="${id}">${esc(comDialogo ? TEXTOS.apagarReticencias : TEXTOS.apagar)}</button>`;
  const itens = [];
  for (const g of dados.grupos) {
    itens.push(
      `<li style="--c:${cor(g.cor)}"><i></i><span class="aj-n">${esc(g.nome)}</span><small>${juntar(TEXTOS.nTimers(g.timers.length), doEspaco(g.espaco_id), curto(g.total_ms))}</small>${botoes('grupo', g.id, g.timers.length > 0 || g.total_ms > 0)}</li>`,
    );
    for (const t of g.timers) {
      itens.push(
        `<li class="aj-arq-sub" style="--c:${cor(t.cor)}"><i></i><span class="aj-n">${esc(t.nome)}</span><small>${juntar(t.arquivado ? TEXTOS.arquivadoAParte : '', curto(t.total_ms))}</small></li>`,
      );
    }
  }
  for (const t of dados.timers) {
    itens.push(
      `<li style="--c:${cor(t.cor)}"><i></i><span class="aj-n">${esc(t.nome)}</span><small>${juntar(t.grupo_nome ?? TEXTOS.semGrupo, doEspaco(t.espaco_id), curto(t.total_ms))}</small>${botoes('timer', t.id, t.total_ms > 0)}</li>`,
    );
  }
  if (!itens.length) itens.push(`<li class="aj-vazio">${esc(TEXTOS.nadaArquivado)}</li>`);
  return `<ul class="aj-lista" data-lista="arquivados">${itens.join('')}</ul>`;
}

/**
 * Um clique na lista dos arquivados. `recarregar()` lê a lista de novo e redesenha. Devolve se
 * tratou o clique.
 */
export async function cliqueNosArquivados(acao, id, dados, recarregar) {
  const timer = () => dados?.timers.find((t) => t.id === id) ?? dados?.grupos.flatMap((g) => g.timers).find((t) => t.id === id);
  const grupo = () => dados?.grupos.find((g) => g.id === id);
  switch (acao) {
    case 'desarquivar-timer':
      await chamar('desarquivar_timer', { timer_id: id });
      await recarregar();
      return true;
    case 'desarquivar-grupo':
      await chamar('desarquivar_grupo', { grupo_id: id });
      await recarregar();
      return true;
    case 'apagar-timer': {
      const t = timer();
      if (t) await apagarTimer(null, t, { arquivado: true, depois: recarregar });
      return true;
    }
    case 'apagar-grupo': {
      const g = grupo();
      if (g) await apagarGrupo(g, { depois: recarregar });
      return true;
    }
    default:
      return false;
  }
}
