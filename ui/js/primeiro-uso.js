// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Primeiro uso (quando não há nenhum timer): seis passos em diálogos, numerados no título
// ("1 de 6 · Boas-vindas"; rodada 18).
// 1 boas-vindas (o que o app é e o que ele não faz) · 2 grupo e timers · 3 conectar apps (o
// cartão do Claude Code) · 4 pastas dos projetos (os projetos recentes deste PC; só com timers) ·
// 5 iniciar com o Windows · 6 pronto, com o resumo. Esc fecha o assistente a qualquer momento.
import { chamar, iniciarComWindows } from './api.js';
import { esc } from './componentes.js';
import { abrirDialogo, PALETA, avisoPastaMae } from './editar.js';
import { linhas, faltamCriar } from './formato.js';
import { corpoPrevia, corpoInstalado, lerPrevia, gravarPastas, htmlConexaoClaude } from './ajustes.js';
import { NOME_APP, PERFIL_TESTE } from './nome.js';

/** Os passos do assistente, na ordem (o das pastas só com timers criados). */
export const PASSOS_DO_PRIMEIRO_USO = ['boas-vindas', 'timers', 'claude', 'pastas', 'windows', 'pronto'];

/** O nome de cada passo, no título. */
export const NOMES_DOS_PASSOS = {
  'boas-vindas': 'Boas-vindas',
  timers: 'Grupo e timers',
  claude: 'Conectar apps',
  pastas: 'Pastas dos projetos',
  windows: 'Iniciar com o Windows',
  pronto: 'Pronto',
};

const TOTAL = PASSOS_DO_PRIMEIRO_USO.length;
const numero = (id) => PASSOS_DO_PRIMEIRO_USO.indexOf(id) + 1;

/** O título do passo, com o número: "1 de 6 · Boas-vindas". */
export const tituloDoPasso = (id) => `${numero(id)} de ${TOTAL} · ${NOMES_DOS_PASSOS[id]}`;

/** O que o assistente ligou (para o resumo do "Pronto"). */
let feito = { timers: 0, claude: false, pastas: 0 };

/** O passo "Pastas dos projetos" aparece? Com algum timer criado (com ou sem app conectado). */
export const temPassoDasPastas = ({ timers }) => (timers?.length ?? 0) > 0;

const plural = (n, um, varios, nenhum) => (n === 0 ? nenhum : n === 1 ? `1 ${um}` : `${n} ${varios}`);

/** O resumo do "Pronto": "2 timers · Claude Code conectado · 1 pasta". */
export function resumoDoPronto({ timers = 0, claude = false, pastas = 0 } = {}) {
  return [
    plural(timers, 'timer', 'timers', 'nenhum timer'),
    claude ? 'Claude Code conectado' : 'nenhum app conectado',
    plural(pastas, 'pasta', 'pastas', 'nenhuma pasta'),
  ].join(' · ');
}

/** As boas-vindas: o que o app é e o que ele não faz (rodada 18). */
export const TEXTO_BOAS_VINDAS = `O ${NOME_APP} conta o seu tempo por timer: no play, ou sozinho quando um app conectado trabalha numa pasta ligada ao timer.`;
export const TEXTO_LOCAL = `Tudo roda neste computador. O ${NOME_APP} não usa IA, não coleta dados e não envia nada para lugar nenhum. Você decide o que conectar e quais pastas contam. É para o seu controle pessoal.`;

/** O que é grupo e o que é timer. */
export const TEXTO_GRUPO = 'Um grupo junta timers, como um cliente ou uma área. Cada timer é um projeto ou atividade.';

/** O passo "Conectar apps". */
export const TEXTO_CONECTAR = `Conecte os apps que você usa e o ${NOME_APP} conta sozinho enquanto eles trabalham. Por enquanto: Claude Code.`;

/** A explicação do passo das pastas (genérica: vale para qualquer app). */
export const TEXTO_PASTAS =
  'Cada app conectado trabalha numa pasta (por exemplo, a do projeto aberto no Claude Code). Ligue a pasta a um timer e o tempo dela conta lá sozinho.';

/** A frase do fim do passo das pastas, sempre. */
export const TEXTO_NAO_ACHOU = `Não achou? Sem problema: na primeira conversa nova, o ${NOME_APP} pergunta em qual timer contar e pode lembrar a pasta.`;

/** O aviso do perfil de teste no passo de conectar (rodada 17). */
export const AVISO_PERFIL_TESTE = 'Perfil de teste: o Claude Code real não é alterado';

const DIA = 24 * 60 * 60 * 1000;

/** "usado hoje", "usado ontem", "usado há 2 dias" (sem data: "escolhida agora"). */
export function textoUsadoHa(usadoEm, agora = Date.now()) {
  if (typeof usadoEm !== 'number') return 'escolhida agora';
  const dias = Math.max(0, Math.floor((agora - usadoEm) / DIA));
  return dias === 0 ? 'usado hoje' : dias === 1 ? 'usado ontem' : `usado há ${dias} dias`;
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
  if (linha.conf?.outro_timer_nome && timer) avisos.push(`A regra desta pasta é de ${esc(linha.conf.outro_timer_nome)}: ela passa para ${esc(timer.nome)}.`);
  const opcoes = [`<option value=""${linha.timer_id ? '' : ' selected'}>—</option>`]
    .concat(timers.map((t) => `<option value="${t.id}"${t.id === linha.timer_id ? ' selected' : ''}>${esc(t.nome)}</option>`))
    .join('');
  return `<li class="pu-recente" data-i="${i}">
  <span class="pu-recente-nome" title="${esc(linha.pasta)}">${esc(linha.nome)}</span>
  <small class="pu-recente-uso">${esc(textoUsadoHa(linha.usado_em, agora))}</small>
  <select data-recente="${i}" aria-label="Timer de ${esc(linha.nome)}">${opcoes}</select>
  ${avisos.map((a) => `<p class="dlg-aviso">${a}</p>`).join('')}${linha.erro ? `<p class="pu-pasta-erro" role="alert">${esc(linha.erro)}</p>` : ''}
</li>`;
}

/** O corpo do passo das pastas: o texto, a lista (se houver), "Escolher outra pasta…" e o "Não achou?". */
export function corpoDasPastas(linhasRecentes, timers, erro = '', agora = Date.now()) {
  const lista = linhasRecentes.length
    ? `<p class="pu-sub">Projetos recentes neste PC</p><ul class="pu-recentes">${linhasRecentes.map((l, i) => linhaDoRecente(l, i, timers, agora)).join('')}</ul>`
    : '';
  return `<p class="dlg-p">${TEXTO_PASTAS}</p>
<div class="pu-pastas-corpo">${lista}</div>
<div class="aj-bts"><button type="button" class="bt mini" data-pu="outra">Escolher outra pasta…</button></div>
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
    enviar: 'Começar',
    cancelar: 'Agora não',
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
<label class="campo"><span>Nome do grupo</span><input type="text" name="grupo" placeholder="Ex.: Cliente A" autocomplete="off" spellcheck="false" autofocus></label>
<label class="campo"><span>Timers</span><textarea name="timers" rows="4" spellcheck="false" placeholder="Um por linha. Ex.:&#10;Projeto X&#10;Reuniões&#10;Estudos"></textarea><small>Um por linha. Depois dá para criar mais, editar e arquivar.</small></label>`,
    enviar: 'Criar',
    cancelar: 'Pular',
    aoCancelar: () => ligarClaude(),
    async aoEnviar(form) {
      const nome = form.elements.grupo.value.trim();
      const timers = linhas(form.elements.timers.value);
      if (!nome) {
        form.elements.grupo.focus();
        throw 'Dê um nome ao grupo.';
      }
      if (!timers.length) {
        form.elements.timers.focus();
        throw 'Escreva pelo menos um timer.';
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
    return depoisDoClaude(`<p class="dlg-p">Não deu para conectar o Claude Code agora (${esc(p.erro)}). Dá para tentar depois em Ajustes → Conexões.</p>`);
  }
  abrirDialogo({
    titulo: tituloDoPasso('claude'),
    classe: 'pu largo',
    corpo: corpoConectarApps({ estado, encontrado, settings: p.settings, previa: p.previa, pastas: p.pastas }),
    enviar: conectar ? (estado === 'antigo' ? 'Atualizar' : 'Conectar') : 'Continuar',
    cancelar: conectar ? 'Pular' : null,
    aoCancelar: () => depoisDoClaude(''),
    async aoEnviar(form) {
      if (!conectar) return depoisDoClaude('');
      // as pastas que nunca contam são gravadas antes de os hooks existirem
      await gravarPastas(form);
      const copia = await chamar('hooks_instalar');
      feito.claude = true;
      await depoisDoClaude(`<div class="pu-ok"><p class="dlg-p"><b>Claude Code conectado.</b></p>${corpoInstalado(copia)}</div>`);
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
    enviar: 'Continuar',
    cancelar: 'Pular',
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
            linha.erro = typeof e === 'string' ? e : 'Não deu certo. Tente de novo.';
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
          const pasta = await chamar('escolher_pasta', { inicial: null, titulo: 'Pasta de um projeto' });
          if (!pasta) return;
          const conf = await chamar('conferir_regra', { pasta, timer_id: null });
          if (conf.erro) {
            erro = conf.erro;
          } else {
            erro = '';
            acrescentarPasta(linhasRecentes, conf);
          }
        } catch (e) {
          erro = typeof e === 'string' ? e : 'Não deu certo. Tente de novo.';
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
<p class="dlg-p">Aberto desde o começo do dia, o ${NOME_APP} não perde nenhuma sessão do Claude.</p>
<div class="escolhas" role="radiogroup" aria-label="Iniciar com o Windows">
  <label class="esc"><input type="radio" name="auto" value="sim" checked autofocus><span><b>Sim</b><small>Abre na bandeja quando você entra no Windows.</small></span></label>
  <label class="esc"><input type="radio" name="auto" value="nao"><span><b>Não</b><small>Você abre quando quiser.${atual ? ' (Hoje ele já inicia com o Windows.)' : ''}</small></span></label>
</div>`,
    enviar: 'Continuar',
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
<p class="dlg-p">O ${NOME_APP} fica na bandeja do Windows, e o dock mostra o que está rodando. Quer ver o essencial? O tour leva 1 minuto.</p>
<p class="dlg-p">Tudo isso muda depois em Ajustes (⚙).</p>`,
    enviar: 'Fazer o tour (1 min)',
    cancelar: 'Agora não',
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
