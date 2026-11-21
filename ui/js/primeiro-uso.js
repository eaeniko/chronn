// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Primeiro uso (quando não há nenhum timer): seis passos em diálogos, numerados no título
// ("1 de 6 · Boas-vindas"; rodada 18).
// 1 boas-vindas (o que o app é e o que ele não faz) · 2 grupo e timers · 3 conectar apps (o
// cartão do Claude Code) · 4 pastas dos projetos (os projetos recentes deste PC; só com timers) ·
// 5 iniciar com o Windows · 6 pronto, com o resumo. Esc fecha o assistente a qualquer momento.
import { chamar, iniciarComWindows } from './api.js';
import { esc, comOAtual } from './componentes.js';
import { abrirDialogo, PALETA, avisoPastaMae } from './editar.js';
import { linhas, faltamCriar } from './formato.js';
import { corpoPrevia, corpoInstalado, lerPrevia, gravarPastas, htmlConexaoClaude } from './ajustes.js';
import { NOME_APP, PERFIL_TESTE } from './nome.js';
import { TEXTOS, aoTrocarIdioma } from './textos.js';

/** Os passos do assistente, na ordem (o das pastas só com timers criados). */
export const PASSOS_DO_PRIMEIRO_USO = ['boas-vindas', 'timers', 'claude', 'pastas', 'windows', 'pronto'];

/** O nome de cada passo, no título (no idioma de agora). */
export const NOMES_DOS_PASSOS = Object.fromEntries(
  PASSOS_DO_PRIMEIRO_USO.map((id) => [id, null]),
);
for (const id of PASSOS_DO_PRIMEIRO_USO) Object.defineProperty(NOMES_DOS_PASSOS, id, { enumerable: true, get: () => TEXTOS.puPassos[id] });

const TOTAL = PASSOS_DO_PRIMEIRO_USO.length;
const numero = (id) => PASSOS_DO_PRIMEIRO_USO.indexOf(id) + 1;

/** O título do passo, com o número: "1 de 6 · Boas-vindas". */
export const tituloDoPasso = (id) => TEXTOS.puTitulo(numero(id), TOTAL, NOMES_DOS_PASSOS[id]);

/** O que o assistente ligou (para o resumo do "Pronto"). */
let feito = { timers: 0, claude: false, pastas: 0 };

/** O passo "Pastas dos projetos" aparece? Com algum timer criado (com ou sem app conectado). */
export const temPassoDasPastas = ({ timers }) => (timers?.length ?? 0) > 0;

/** O resumo do "Pronto": "2 timers · Claude Code conectado · 1 pasta". */
export function resumoDoPronto({ timers = 0, claude = false, pastas = 0 } = {}) {
  return [TEXTOS.puNTimers(timers), claude ? TEXTOS.claudeConectado : TEXTOS.puNenhumApp, TEXTOS.puNPastas(pastas)].join(' · ');
}

// Os textos do assistente que outros módulos (e os testes) leem daqui, no idioma de agora: um
// `export let` refeito a cada troca de idioma (rodada 23).
/** As boas-vindas: o que o app é e o que ele não faz (rodada 18). */
export let TEXTO_BOAS_VINDAS;
export let TEXTO_LOCAL;
/** O que é grupo e o que é timer. */
export let TEXTO_GRUPO;
/** O passo "Conectar apps". */
export let TEXTO_CONECTAR;
/** A explicação do passo das pastas (genérica: vale para qualquer app). */
export let TEXTO_PASTAS;
/** A frase do fim do passo das pastas, sempre. */
export let TEXTO_NAO_ACHOU;
/** O aviso do perfil de teste no passo de conectar (rodada 17). */
export let AVISO_PERFIL_TESTE;
function textosDoModulo() {
  TEXTO_BOAS_VINDAS = TEXTOS.puBoasVindas(NOME_APP);
  TEXTO_LOCAL = TEXTOS.puLocal(NOME_APP);
  TEXTO_GRUPO = TEXTOS.puGrupo;
  TEXTO_CONECTAR = TEXTOS.puConectar(NOME_APP);
  TEXTO_PASTAS = TEXTOS.puPastas;
  TEXTO_NAO_ACHOU = TEXTOS.puNaoAchou(NOME_APP);
  AVISO_PERFIL_TESTE = TEXTOS.puAvisoTeste;
}
textosDoModulo();
aoTrocarIdioma(textosDoModulo);

const DIA = 24 * 60 * 60 * 1000;

/** "usado hoje", "usado ontem", "usado há 2 dias" (sem data: "escolhida agora"). */
export function textoUsadoHa(usadoEm, agora = Date.now()) {
  if (typeof usadoEm !== 'number') return TEXTOS.puEscolhidaAgora;
  const dias = Math.max(0, Math.floor((agora - usadoEm) / DIA));
  return dias === 0 ? TEXTOS.puUsadoHoje : dias === 1 ? TEXTOS.puUsadoOntem : TEXTOS.puUsadoHa(dias);
}

/** A pasta comparável (sem diferenciar maiúsculas, `/` = `\\`, sem barra no fim), como o núcleo. */
const chaveDaPasta = (p) => String(p ?? '').replace(/\//g, '\\').replace(/\\+$/, '').toLowerCase();

/**
 * Uma linha de "Projetos recentes neste PC": o nome curto da pasta (o caminho na dica), quando
 * foi usada e o seletor de timer (— ou os timers criados), com o aviso da escolha (pasta-mãe,
 * regra de outro timer) ou o erro.
 */
export function linhaDoRecente(linha, i, timers, agora = Date.now()) {
  const avisos = [];
  const timer = timers.find((t) => t.id === linha.timer_id);
  if (linha.conf?.pasta_mae && timer) avisos.push(avisoPastaMae(esc(timer.nome)));
  if (linha.conf?.outro_timer_nome && timer) avisos.push(TEXTOS.puRegraDeOutro(esc(linha.conf.outro_timer_nome), esc(timer.nome)));
  // o timer escolhido sempre marcado, mesmo fora da lista (revisão da rodada 21)
  const lista = ['<option value="">—</option>'].concat(timers.map((t) => `<option value="${t.id}">${esc(t.nome)}</option>`)).join('');
  const opcoes = linha.timer_id ? comOAtual(lista, linha.timer_id, TEXTOS.novoTimer) : lista.replace('<option value="">', '<option value="" selected>');
  return `<li class="pu-recente" data-i="${i}">
  <span class="pu-recente-nome" title="${esc(linha.pasta)}">${esc(linha.nome)}</span>
  <small class="pu-recente-uso">${esc(textoUsadoHa(linha.usado_em, agora))}</small>
  <select data-recente="${i}" aria-label="${esc(TEXTOS.puTimerDe(linha.nome))}">${opcoes}</select>
  ${avisos.map((a) => `<p class="dlg-aviso">${a}</p>`).join('')}${linha.erro ? `<p class="pu-pasta-erro" role="alert">${esc(linha.erro)}</p>` : ''}
</li>`;
}

/** O corpo do passo das pastas: o texto, a lista (se houver), "Escolher outra pasta…" e o "Não achou?". */
export function corpoDasPastas(linhasRecentes, timers, erro = '', agora = Date.now()) {
  const lista = linhasRecentes.length
    ? `<p class="pu-sub">${TEXTOS.puRecentes}</p><ul class="pu-recentes">${linhasRecentes.map((l, i) => linhaDoRecente(l, i, timers, agora)).join('')}</ul>`
    : '';
  return `<p class="dlg-p">${TEXTO_PASTAS}</p>
<div class="pu-pastas-corpo">${lista}</div>
<div class="aj-bts"><button type="button" class="bt mini" data-pu="outra">${TEXTOS.puOutraPasta}</button></div>
${erro ? `<p class="pu-pasta-erro" role="alert">${esc(erro)}</p>` : ''}
<p class="pu-nao-achou">${TEXTO_NAO_ACHOU}</p>`;
}

/** As regras a criar ao continuar: uma por pasta com timer escolhido (pastas iguais, uma vez). */
export function regrasDasPastas(linhasRecentes) {
  const vistas = new Set();
  const regras = [];
  for (const l of linhasRecentes ?? []) {
    if (!l?.pasta || !l.timer_id || l.erro || vistas.has(chaveDaPasta(l.pasta))) continue;
    vistas.add(chaveDaPasta(l.pasta));
    regras.push({ timer_id: l.timer_id, pasta: l.pasta });
  }
  return regras;
}

/**
 * Cria as regras escolhidas (`chamarFn`: o `chamar`, ou um de mentira nos testes). Devolve
 * quantas pastas distintas ficaram com regra (o "Pronto" conta essas).
 */
export async function criarRegrasDasPastas(linhasRecentes, chamarFn = chamar) {
  const regras = regrasDasPastas(linhasRecentes);
  for (const r of regras) await chamarFn('salvar_regra', { pasta: r.pasta, timer_id: r.timer_id, trocar: null });
  return regras.length;
}

/** Acrescenta uma pasta escolhida à lista (a que já está lá não repete). Devolve a posição. */
export function acrescentarPasta(linhasRecentes, conf) {
  const k = chaveDaPasta(conf.pasta);
  const i = linhasRecentes.findIndex((l) => chaveDaPasta(l.pasta) === k);
  if (i >= 0) return i;
  const nome = String(conf.pasta).replace(/[\\/]+$/, '').split(/[\\/]/).pop() || conf.pasta;
  linhasRecentes.push({ pasta: conf.pasta, nome, usado_em: null, timer_id: null });
  return linhasRecentes.length - 1;
}

/** Sem nenhum timer no snapshot: é a hora do primeiro uso. */
export const semTimers = (snap) => snap.grupos.every((g) => !g.timers.length);

/**
 * O assistente. `semCriarTimers`: já há timer (criado pelo "+ Novo timer…" do dock antes do
 * assistente), então as boas-vindas seguem direto para ligar ao Claude Code.
 */
export function primeiroUso({ semCriarTimers = false } = {}) {
  feito = { timers: 0, claude: false, pastas: 0 };
  boasVindas(semCriarTimers);
}

/**
 * Sem timers: o primeiro uso (com `primeiroUso: false`, não: quem chamou já tem o que abrir, como o
 * "+ Novo timer…" do dock). Devolve "sem-timers", que o "+ Novo timer…" do dock usa para abrir o
 * assistente depois dele.
 */
export async function inicioSemTimers({ primeiroUso: comPrimeiroUso = true } = {}) {
  if (comPrimeiroUso) primeiroUso();
  return 'sem-timers';
}

function boasVindas(semCriarTimers = false) {
  abrirDialogo({
    titulo: tituloDoPasso('boas-vindas'),
    classe: 'pu',
    corpo: `<p class="dlg-p">${TEXTO_BOAS_VINDAS}</p>
<p class="dlg-p pu-local">${TEXTO_LOCAL}</p>`,
    enviar: TEXTOS.comecar,
    cancelar: TEXTOS.agoraNao,
    aoEnviar: () => (semCriarTimers ? ligarClaude() : criarTimers()),
  });
}

function criarTimers() {
  // Se um comando falhar no meio, o diálogo fica aberto: o "Criar" de novo reaproveita o grupo
  // já criado e só cria os timers que faltam.
  let grupoId = null;
  const criados = [];
  abrirDialogo({
    titulo: tituloDoPasso('timers'),
    classe: 'pu',
    corpo: `<p class="dlg-p">${TEXTO_GRUPO}</p>
<label class="campo"><span>${TEXTOS.puNomeDoGrupo}</span><input type="text" name="grupo" placeholder="${esc(TEXTOS.puGrupoPlaceholder)}" autocomplete="off" spellcheck="false" autofocus></label>
<label class="campo"><span>${TEXTOS.timersRotulo}</span><textarea name="timers" rows="4" spellcheck="false" placeholder="${esc(TEXTOS.puTimersPlaceholder).replace(/\n/g, '&#10;')}"></textarea><small>${TEXTOS.puTimersDica}</small></label>`,
    enviar: TEXTOS.criar,
    cancelar: TEXTOS.pular,
    aoCancelar: () => ligarClaude(),
    async aoEnviar(form) {
      const nome = form.elements.grupo.value.trim();
      const timers = linhas(form.elements.timers.value);
      if (!nome) {
        form.elements.grupo.focus();
        throw TEXTOS.puDeUmNome;
      }
      if (!timers.length) {
        form.elements.timers.focus();
        throw TEXTOS.puUmTimer;
      }
      if (grupoId === null) {
        const s = await chamar('criar_grupo', { nome, cor: PALETA[0] });
        const g = s.grupos.filter((x) => x.id !== null).reduce((m, x) => (m === null || x.id > m.id ? x : m), null);
        grupoId = g?.id ?? null;
      }
      for (const t of faltamCriar(timers, criados)) {
        const i = timers.indexOf(t);
        await chamar('criar_timer', { grupo_id: grupoId, nome: t, cor: PALETA[i % PALETA.length] });
        criados.push(t);
      }
      await ligarClaude();
    },
  });
}

/**
 * O corpo do passo "Conectar apps" (rodada 18): o texto e o cartão do Claude Code (o mesmo dos
 * Ajustes, sem o botão: o do passo é o "Conectar"); para conectar, a frase sobre o arquivo e a
 * prévia exata recolhida em "Ver o que muda". No perfil de teste, o aviso de que o Claude Code
 * real não muda.
 */
export function corpoConectarApps({ estado, encontrado = true, settings = '', previa = '', pastas = [] }, teste = PERFIL_TESTE) {
  const conectar = encontrado && estado !== 'ligado';
  return `<p class="dlg-p">${TEXTO_CONECTAR}</p>
${htmlConexaoClaude(estado, encontrado, settings, false)}
${teste ? `<p class="dlg-aviso" role="note">${AVISO_PERFIL_TESTE}.</p>` : ''}
${conectar ? corpoPrevia(previa, settings, pastas, estado === 'antigo') : ''}`;
}

/** Os timers que existem agora (os criados no passo 2, ou antes do assistente). */
async function timersDeAgora() {
  try {
    return (await chamar('obter_estado')).grupos.flatMap((g) => g.timers);
  } catch (e) {
    console.error('worktimer: obter_estado', e);
    return [];
  }
}

/** Depois de conectar (ou pular): o passo das pastas, se cabe, ou direto o do Windows. */
async function depoisDoClaude(aviso) {
  const timers = await timersDeAgora();
  feito.timers = timers.length;
  if (temPassoDasPastas({ timers })) return pastasDosProjetos(timers, aviso);
  return iniciarComWin(aviso);
}

async function ligarClaude() {
  let conexao = { estado: 'desligado', encontrado: true };
  try {
    conexao = await chamar('conexao_claude');
  } catch (e) {
    console.error('worktimer: conexao_claude', e);
  }
  const { estado, encontrado } = conexao;
  feito.claude = estado === 'ligado';
  // conectado, ou sem o Claude Code neste PC: o cartão diz e o passo só continua
  const conectar = encontrado && estado !== 'ligado';
  const p = conectar ? await lerPrevia() : { settings: '' };
  if (p.erro) {
    return depoisDoClaude(`<p class="dlg-p">${TEXTOS.puNaoConectou(esc(p.erro))}</p>`);
  }
  abrirDialogo({
    titulo: tituloDoPasso('claude'),
    classe: 'pu largo',
    corpo: corpoConectarApps({ estado, encontrado, settings: p.settings, previa: p.previa, pastas: p.pastas }),
    enviar: conectar ? (estado === 'antigo' ? TEXTOS.atualizar : TEXTOS.conectar) : TEXTOS.continuar,
    cancelar: conectar ? TEXTOS.pular : null,
    aoCancelar: () => depoisDoClaude(''),
    async aoEnviar(form) {
      if (!conectar) return depoisDoClaude('');
      // as pastas que nunca contam são gravadas antes de os hooks existirem
      await gravarPastas(form);
      const copia = await chamar('hooks_instalar');
      feito.claude = true;
      await depoisDoClaude(`<div class="pu-ok"><p class="dlg-p"><b>${TEXTOS.claudeConectado}.</b></p>${corpoInstalado(copia)}</div>`);
    },
  });
}

/** Os projetos recentes deste PC (vazio sem o Claude Code conectado, ou sem leitura). */
async function recentesDoPc() {
  if (!feito.claude) return [];
  try {
    return await chamar('projetos_recentes');
  } catch (e) {
    console.error('worktimer: projetos_recentes', e);
    return [];
  }
}

/**
 * "Pastas dos projetos" (rodada 18): a pessoa não precisa saber onde fica a pasta da conversa.
 * "Projetos recentes neste PC" (lidos pelo núcleo) com um seletor de timer em cada um, e "Escolher
 * outra pasta…" para o que não estiver na lista; as conferências da rodada 15 em cada escolha (a
 * recusa da raiz do drive, da pasta do usuário e da que nunca conta; o aviso de pasta-mãe e o de
 * regra de outro timer). Cada pasta fica com um timer só (uma linha por pasta). As regras são
 * criadas ao continuar; "Pular" e "Continuar" sem nada valem. Sempre, o "Não achou?".
 */
export async function pastasDosProjetos(timers, aviso = '', recentes = null) {
  const linhasRecentes = (recentes ?? (await recentesDoPc())).map((r) => ({ ...r, timer_id: null }));
  let erro = '';
  const corpo = () => corpoDasPastas(linhasRecentes, timers, erro);
  abrirDialogo({
    titulo: tituloDoPasso('pastas'),
    classe: 'pu largo',
    corpo: `${aviso}<div class="pu-pastas-passo">${corpo()}</div>`,
    enviar: TEXTOS.continuar,
    cancelar: TEXTOS.pular,
    aoCancelar: () => iniciarComWin(''),
    aoMontar(form) {
      const caixa = form.querySelector('.pu-pastas-passo');
      const desenhar = () => (caixa.innerHTML = corpo());
      caixa.addEventListener('change', async (ev) => {
        const sel = ev.target.closest('select[data-recente]');
        if (!sel) return;
        const linha = linhasRecentes[Number(sel.dataset.recente)];
        linha.timer_id = sel.value ? Number(sel.value) : null;
        linha.conf = null;
        linha.erro = '';
        if (linha.timer_id) {
          try {
            const conf = await chamar('conferir_regra', { pasta: linha.pasta, timer_id: linha.timer_id });
            if (conf.erro) linha.erro = conf.erro;
            else linha.conf = conf;
          } catch (e) {
            linha.erro = typeof e === 'string' ? e : TEXTOS.naoDeuCerto;
          }
        }
        desenhar();
        // o foco volta ao seletor (redesenhado)
        caixa.querySelector(`select[data-recente="${sel.dataset.recente}"]`)?.focus();
      });
      caixa.addEventListener('click', async (ev) => {
        const b = ev.target.closest('[data-pu="outra"]');
        if (!b || b.disabled) return;
        b.disabled = true;
        try {
          const pasta = await chamar('escolher_pasta', { inicial: null, titulo: TEXTOS.puPastaDeUmProjeto });
          if (!pasta) return;
          const conf = await chamar('conferir_regra', { pasta, timer_id: null });
          if (conf.erro) {
            erro = conf.erro;
          } else {
            erro = '';
            acrescentarPasta(linhasRecentes, conf);
          }
        } catch (e) {
          erro = typeof e === 'string' ? e : TEXTOS.naoDeuCerto;
        } finally {
          desenhar();
        }
      });
    },
    async aoEnviar() {
      feito.pastas = await criarRegrasDasPastas(linhasRecentes);
      await iniciarComWin('');
    },
  });
}

async function iniciarComWin(aviso) {
  let atual = false;
  try {
    atual = await iniciarComWindows.ligado();
  } catch (e) {
    console.error('worktimer: autostart', e);
  }
  abrirDialogo({
    titulo: tituloDoPasso('windows'),
    classe: 'pu',
    corpo: `${aviso}
<p class="dlg-p">${TEXTOS.puWindowsTexto(NOME_APP)}</p>
<div class="escolhas" role="radiogroup" aria-label="${TEXTOS.iniciarComWindows}">
  <label class="esc"><input type="radio" name="auto" value="sim" checked autofocus><span><b>${TEXTOS.sim}</b><small>${TEXTOS.puSimDica}</small></span></label>
  <label class="esc"><input type="radio" name="auto" value="nao"><span><b>${TEXTOS.nao}</b><small>${TEXTOS.puNaoDica}${atual ? ` ${TEXTOS.puJaInicia}` : ''}</small></span></label>
</div>`,
    enviar: TEXTOS.continuar,
    cancelar: null,
    async aoEnviar(form) {
      const sim = form.elements.auto.value === 'sim';
      if (sim !== atual) await (sim ? iniciarComWindows.ligar() : iniciarComWindows.desligar());
      const cfg = { ...(await chamar('obter_estado')).config, iniciar_com_windows: sim };
      await chamar('salvar_config', { config: cfg });
      pronto();
    },
  });
}

/**
 * O último passo: "Fazer o tour (1 min)" (o principal) ou "Agora não" (rodada 16). Recusado, o
 * tour não volta sozinho (fica no ? do topo e em Ajustes → Ajuda).
 */
export function pronto(resumo = feito) {
  abrirDialogo({
    titulo: tituloDoPasso('pronto'),
    classe: 'pu',
    corpo: `
<p class="pu-resumo">${esc(resumoDoPronto(resumo))}</p>
<p class="dlg-p">${TEXTOS.puProntoTexto(NOME_APP)}</p>
<p class="dlg-p">${TEXTOS.puProntoMuda}</p>`,
    enviar: TEXTOS.puFazerTour,
    cancelar: TEXTOS.agoraNao,
    // o foco começa no "Fazer o tour (1 min)" (revisão)
    aoMontar(form) {
      form.querySelector('[type="submit"]')?.setAttribute('autofocus', '');
    },
    aoFechar(enviado) {
      if (enviado) document.dispatchEvent(new CustomEvent('pedir-tour'));
      else chamar('marcar_tour_visto').catch((e) => console.error('worktimer: marcar_tour_visto', e));
    },
  });
}
