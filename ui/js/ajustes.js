// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Tela de Ajustes (⚙), na janela principal. Cada campo grava sozinho ao mudar: `salvar_config`
// leva a config inteira, lida do núcleo na hora (assim não desfaz o tema trocado no topo, a
// visão ou o dock). As gravações vão em fila, uma de cada vez.
// Também mora aqui o "Conectar o Claude Code" (prévia exata → Conectar), usado no primeiro uso.
import { chamar, iniciarComWindows } from './api.js';
import { esc, cor } from './componentes.js';
import { abrirDialogo, faixa, textoDoErro, grupoParaSalvar, PALETA, novoEspaco } from './editar.js';
import { linhas, minutosDeSegundos, segundosDeMinutos, tamanho, abreviarCaminho, dataDoBackup } from './formato.js';
import { NOME_APP, PERFIL_TESTE } from './nome.js';
import { TEXTOS, aoTrocarIdioma, IDIOMAS, IDIOMA_SISTEMA, resolverIdioma } from './textos.js';
import { podeApagarEspaco, textoAoApagar } from './espacos.js';
import { htmlAtualizacao, textoDaVersao, linhaDaVerificacao, textoDaUltima, abrirAtualizar } from './atualizacao.js';

// Os textos fixos que outros módulos (e os testes) leem daqui, no idioma de agora: refeitos a cada
// troca de idioma (rodada 23; os `export let` acompanham, pelo vínculo vivo dos módulos).
export let CREDITOS;
export let TEMPOS;
export let TITULO_MODO_DEV;
export let AVISO_TEMPOS;
export let HOOKS_ANTIGOS;
export let HOOKS_DESLIGADOS;
export let AO_FECHAR;
export let AO_MINIMIZAR;
export let ESPERA_OPCOES;
export let PAUSADO_OPCOES;
export let CONVERSA_NOVA;
export let DOCK_MODO;
export let FRASE_CLAUDE;
let SECOES;
let DICA_PASTAS;

function textosDoModulo() {
  CREDITOS = TEXTOS.creditos;
  // campos de tempo (só no modo desenvolvedor; sem ele valem os padrões): config em segundos, tela
  // em minutos. O último número é o máximo em minutos.
  TEMPOS = TEXTOS.tempos;
  TITULO_MODO_DEV = TEXTOS.tituloModoDev;
  // a faixa fixa da seção Tempos
  AVISO_TEMPOS = TEXTOS.avisoTempos;
  // o estado "antigo" dos hooks (Ajustes e a faixa da janela principal)
  HOOKS_ANTIGOS = TEXTOS.hooksAntigos;
  // a faixa dos hooks desligados com timers (revisão da rodada 16; "conectado" desde a rodada 18)
  HOOKS_DESLIGADOS = TEXTOS.hooksDesligados;
  // "Ao fechar a janela": o que o X da janela principal faz
  AO_FECHAR = TEXTOS.aoFecharOps;
  // "Ao minimizar a janela": o que o minimizar da janela principal faz
  AO_MINIMIZAR = TEXTOS.aoMinimizarOps;
  // os tempos do "esperando você" (rodada 13), em minutos; 0 = até a virada do dia
  ESPERA_OPCOES = TEXTOS.esperaOps;
  // os tempos do pausado no dock recolhido (rodada 14), em minutos; 0 = sai na hora
  PAUSADO_OPCOES = TEXTOS.pausadoOps;
  // "Conversa nova numa pasta com regra" (rodada 15)
  CONVERSA_NOVA = TEXTOS.conversaNovaOps;
  // "Dock": sempre à vista (o padrão) ou só quando houver uma pergunta (rodada 12)
  DOCK_MODO = TEXTOS.dockModoOps;
  // a frase do cartão do Claude Code (rodada 18)
  FRASE_CLAUDE = TEXTOS.fraseClaude;
  DICA_PASTAS = TEXTOS.dicaPastas;
  SECOES = ['idioma', 'aparencia', 'painel', 'dock', 'espera', 'sistema', 'claude', 'pastas', 'dia', 'tempos', 'arredondamento', 'atividades', 'grupos', 'backups', 'ajuda', 'sobre'].map(
    (id) => [id, TEXTOS.secoes[id]],
  );
}
textosDoModulo();
aoTrocarIdioma(textosDoModulo);

/**
 * Os créditos do Sobre (rodada 18): o `@eaeniko` abre o site do autor e "Código no GitHub" o
 * código, no navegador padrão (o núcleo abre; a página só diz qual dos dois). Embaixo, a
 * versão e a marca de autoria.
 */
export function htmlCreditos(assinatura = '') {
  const autor = '<button type="button" class="aj-link" data-aj="link" data-link="autor" title="eaeniko.okin.club">@eaeniko</button>';
  const codigo = `<button type="button" class="aj-link" data-aj="link" data-link="codigo" title="github.com/eaeniko/chronn">${TEXTOS.codigoNoGithub}</button>`;
  return `<p class="aj-cred">${TEXTOS.creditosHtml(autor, codigo)}</p>
${assinatura ? `<p class="aj-assinatura mono">${esc(assinatura)}</p>` : ''}`;
}

/** "Apagar o perfil de teste e recomeçar" (rodada 17): só existe no perfil de teste. */
export const htmlApagarPerfil = (teste = PERFIL_TESTE) =>
  teste
    ? `<div class="aj-bts"><button type="button" class="bt perigo" data-aj="apagar-perfil-teste">${TEXTOS.apagarPerfilTeste}</button></div>
<small class="aj-dica">${TEXTOS.apagarPerfilTesteDica}</small>`
    : '';

/** Ajustes → Ajuda (rodada 16): as dicas na tela, mostrá-las de novo e o tour. */
export const htmlAjuda = (c) =>
  `${caixa('dicas_ligadas', TEXTOS.dicasNaTela, TEXTOS.dicasNaTelaDica, c?.dicas_ligadas !== false)}
<div class="aj-bts">
  <button type="button" class="bt" data-aj="dicas-de-novo">${TEXTOS.mostrarDicasDeNovo}</button>
  <button type="button" class="bt pri" data-aj="ver-tour">${TEXTOS.comoComecar}</button>
  <button type="button" class="bt" data-aj="tour-completo">${TEXTOS.tourCompleto}</button>
</div>
<small class="aj-dica">${TEXTOS.ajudaDica}</small>`;

/** O interruptor discreto do Sobre. */
export const campoModoDev = (ligado) => `<label class="chk aj-dev">
  <input type="checkbox" name="modo_dev"${ligado ? ' checked' : ''}><span>${TEXTOS.modoDev}<small>${TEXTOS.modoDevDica}</small></span>
</label>`;

/** O corpo da confirmação ao ligar o modo desenvolvedor (o título é `TITULO_MODO_DEV`). */
export const corpoModoDev = () => TEXTOS.corpoModoDev.map((p) => `<p class="dlg-p">${p}</p>`).join('\n');

/** A seção Tempos (e o link dela) só aparece no modo desenvolvedor. */
function mostrarTempos(ligado) {
  for (const el of raiz?.querySelectorAll('#aj-tempos, [data-aj="ir"][data-id="tempos"]') ?? []) el.hidden = !ligado;
}

let raiz = null;
let lerSnap = () => null;
let dados = { info: null, hooks: null, regras: [], autostart: null, backups: [] };
/** Último HTML da lista de grupos e regras (para não redesenhar à toa). */
let gruposDesenhados = '';
let atividadesDesenhadas = '';

// ---- gravar ----

let fila = Promise.resolve();
/** Mudanças ainda tratando (algumas consultam o núcleo antes de entrar na fila). */
const tratando = new Set();

/**
 * Espera o que está para ser gravado: tira o foco do campo (assim o `change` de um texto
 * editado dispara), espera as mudanças em andamento e a fila. Usado antes de fechar a janela.
 */
export async function esperarGravacoes() {
  const ativo = document.activeElement;
  if (raiz && ativo && raiz.contains(ativo)) ativo.blur();
  await Promise.allSettled([...tratando]);
  await fila;
}
/** Gravações na fila: enquanto houver, o snapshot não mexe nos campos (estaria velho). */
let pendentes = 0;
let salvoId = null;

function mostrarSalvo() {
  const el = raiz?.querySelector('.aj-salvo');
  if (!el) return;
  el.classList.add('on');
  clearTimeout(salvoId);
  salvoId = setTimeout(() => el.classList.remove('on'), 1600);
}

/**
 * `mudar(cfg)` altera a config lida agora do núcleo; depois grava. O erro vira uma faixa; com
 * `lancar`, quem chamou recebe o erro (e mostra onde quiser).
 */
function salvar(mudar, { lancar = false } = {}) {
  pendentes += 1;
  const gravar = fila.then(async () => {
    const cfg = { ...(await chamar('obter_estado')).config };
    mudar(cfg);
    await chamar('salvar_config', { config: cfg });
    mostrarSalvo();
  });
  fila = gravar
    .catch((e) => {
      console.error('worktimer: salvar_config', e);
      if (!lancar) faixa(textoDoErro(e));
    })
    .finally(() => (pendentes -= 1));
  return lancar ? gravar : fila;
}

// ---- Claude Code: prévia e instalação ----

/** Campo "Pastas que nunca contam" (Ajustes e o diálogo de ligar ao Claude Code). */
const campoPastas = (pastas, rotulo) => `<label class="campo">${rotulo ? `<span>${rotulo}</span>` : ''}
  <textarea name="pastas_ignoradas" rows="2" spellcheck="false" aria-label="${esc(TEXTOS.pastasNuncaContam)}" placeholder="${esc(TEXTOS.pastasPlaceholder)}">${esc(pastas.join('\n'))}</textarea>
  <small>${DICA_PASTAS}</small>
</label>`;

/**
 * A faixa dos hooks na janela principal, ou nula: com os hooks da versão antiga, "Atualizar" (e o
 * ×); desligados com timers (uma reinstalação, alguém tirou à mão), "Ligar" e "Agora não". Sem
 * timers, quem liga é o primeiro uso. As duas somem até a próxima abertura pelo × ou "Agora não".
 */
export function faixaDosHooks(estado, temTimers, encontrado = true) {
  if (estado === 'antigo') return { texto: HOOKS_ANTIGOS, botao: TEXTOS.atualizar, atualizar: true, esconder: '×' };
  // sem o Claude Code neste PC, a faixa não oferece conectar (rodada 18)
  if (estado === 'desligado' && temTimers && encontrado) return { texto: HOOKS_DESLIGADOS, botao: TEXTOS.conectar, atualizar: false, esconder: TEXTOS.agoraNao };
  return null;
}

/**
 * A faixa do WSL sem os hooks (0.6.1): `{ texto, distro, chave }` com a primeira distro da lista
 * (`chave`: todas, para o "×" esconder só até a lista mudar); sem nenhuma, nulo.
 */
export function faixaDoWsl(distros) {
  if (!distros?.length) return null;
  return { texto: TEXTOS.faixaWslSemHooks(distros.join(', ')), distro: distros[0], chave: distros.join('|') };
}

/**
 * Corpo com a prévia exata do settings.json (bloco monoespaçado com rolagem) e, antes de ligar,
 * as pastas que nunca contam: elas valem desde o primeiro evento. `atualizar`: os hooks da
 * versão antiga saem e entram os de agora.
 */
export function corpoPrevia(previa, settings, pastas = [], atualizar = false) {
  const o = atualizar ? TEXTOS.previaAtualizar : TEXTOS.previaAcrescentar;
  // o caminho abreviado no texto (rodada 18); a prévia, exata e recolhida em "Ver o que muda"
  const cam = `<code class="aj-cam">${esc(abreviarCaminho(settings || 'settings.json'))}</code>`;
  return `<p class="dlg-p">${TEXTOS.previaTexto(NOME_APP, o, cam)}</p>
<details class="mais aj-ver-muda"><summary>${TEXTOS.verOQueMuda}</summary><div class="mais-corpo"><p class="aj-dica">${TEXTOS.caminhosDoUsuario}</p>
<pre class="aj-previa" tabindex="0" aria-label="${TEXTOS.previaRotulo}">${esc(previa)}</pre></div></details>
${campoPastas(pastas, TEXTOS.pastasOpcional)}`;
}

/** Grava as pastas que nunca contam do diálogo (e o hook.json que o hook lê). Chamar antes de `hooks_instalar`. */
export async function gravarPastas(form) {
  const pastas = linhas(form.elements.pastas_ignoradas?.value);
  const cfg = (await chamar('obter_estado')).config;
  await chamar('salvar_config', { config: { ...cfg, pastas_ignoradas: pastas } });
  return pastas;
}

/** Texto depois de instalar: onde ficou a cópia de segurança (ou que o arquivo é novo). */
export function corpoInstalado(copia) {
  return copia
    ? `<p class="dlg-p">${TEXTOS.copiaAnterior}</p><code class="aj-cam aj-bloco">${esc(abreviarCaminho(copia))}</code>`
    : `<p class="dlg-p">${TEXTOS.semConfigAntes}</p>`;
}

/** Lê a prévia, o caminho e as pastas ignoradas de agora; `{ previa, settings, pastas }` ou `{ erro }`. */
export async function lerPrevia() {
  try {
    const [previa, info, snap] = await Promise.all([chamar('hooks_previa'), chamar('ajustes_info'), chamar('obter_estado')]);
    return { previa, settings: info.settings, pastas: snap.config.pastas_ignoradas };
  } catch (e) {
    console.error('worktimer: hooks_previa', e);
    return { erro: textoDoErro(e) };
  }
}

/**
 * "Conectar o Claude Code" (ou "Atualizar a conexão", com `atualizar`): mostra a prévia com
 * Conectar e Cancelar; depois de confirmar, mostra onde ficou a cópia de segurança. Nada é
 * gravado sem o clique da pessoa. Resolve `true` se instalou.
 */
export async function ligarAoClaude({ atualizar = false } = {}) {
  const titulo = atualizar ? TEXTOS.atualizarConexao : TEXTOS.conectarClaude;
  const { previa, settings, pastas, erro } = await lerPrevia();
  return new Promise((ok) => {
    if (erro) {
      abrirDialogo({
        titulo,
        corpo: `<p class="dlg-p">${TEXTOS.naoDeuPreparar(esc(erro))}</p>`,
        enviar: null,
        cancelar: TEXTOS.fechar,
        aoFechar: () => ok(false),
      });
      return;
    }
    let copia = null;
    const d = abrirDialogo({
      titulo,
      classe: 'largo',
      // rodada 28: embaixo, o Claude Code no WSL (só com alguma distro no registro), desenhado
      // quando a leitura chega: o diálogo nunca espera o WSL (revisão I2)
      corpo: `${corpoPrevia(previa, settings, pastas, atualizar)}<div data-wsl-dialogo></div>`,
      enviar: atualizar ? TEXTOS.atualizar : TEXTOS.conectar,
      aoMontar(form) {
        lerWsl().then((wsl) => {
          const lugar = form.querySelector('[data-wsl-dialogo]');
          if (lugar) lugar.innerHTML = htmlWsl(wsl);
        });
        // o "Ligar" ou o "Desligar" de uma distro fecha este diálogo e abre o da distro
        form.addEventListener('click', (ev) => {
          const b = ev.target.closest('[data-aj="wsl-ligar"], [data-aj="wsl-desligar"]');
          if (!b) return;
          ev.preventDefault();
          d.fechar();
          if (b.dataset.aj === 'wsl-ligar') ligarDistro(b.dataset.distro, b.dataset.subir === '1');
          else desligarDistro(b.dataset.distro);
        });
      },
      async aoEnviar(form) {
        // as pastas primeiro: o hook já nasce sabendo o que não registrar
        await gravarPastas(form);
        copia = await chamar('hooks_instalar');
      },
      aoFechar(enviado) {
        if (!enviado) return ok(false);
        abrirDialogo({
          titulo: atualizar ? TEXTOS.conexaoAtualizada : TEXTOS.claudeConectado,
          corpo: corpoInstalado(copia),
          enviar: TEXTOS.ok,
          cancelar: null,
          aoFechar: () => ok(true),
        });
      },
    });
  });
}

function removerHooks() {
  return new Promise((ok) => {
    abrirDialogo({
      titulo: TEXTOS.desconectarClaude,
      corpo: `<p class="dlg-p">${TEXTOS.desconectarTexto(NOME_APP, `<code class="aj-cam">${esc(abreviarCaminho(dados.info?.settings ?? 'settings.json'))}</code>`)}</p>`,
      enviar: TEXTOS.desconectar,
      perigo: true,
      async aoEnviar() {
        await chamar('hooks_remover');
      },
      aoFechar: ok,
    });
  });
}

// ---- desenho ----

const radio = (nome, valor, rotulo, atual) =>
  `<label class="seg-op"><input type="radio" name="${nome}" value="${esc(valor)}"${String(atual) === String(valor) ? ' checked' : ''}><span>${rotulo}</span></label>`;

const caixa = (nome, rotulo, dica, marcada, extra = '') => `<label class="chk">
  <input type="checkbox" name="${nome}"${marcada ? ' checked' : ''} ${extra}><span>${rotulo}${dica ? `<small>${dica}</small>` : ''}</span>
</label>`;

/** "Ao fechar a janela" (`AO_FECHAR`): o que o X da janela principal faz. */
export const campoAoFechar = (atual) => `<div class="campo"><span id="aj-ao-fechar">${TEXTOS.aoFecharRotulo}</span>
<div class="seg" role="radiogroup" aria-labelledby="aj-ao-fechar">${AO_FECHAR.map(([v, r]) => radio('ao_fechar', v, r, atual)).join('')}</div>
<small>${TEXTOS.aoFecharDica(NOME_APP)}</small></div>`;

/**
 * `modoDock`: com o dock "Só quando houver uma pergunta", a opção do dock não promete o dock
 * (rodada 12): o minimizar só minimiza.
 */
export const campoAoMinimizar = (atual, modoDock = 'sempre') => {
  const escondido = modoDock === 'perguntas';
  const ops = AO_MINIMIZAR.map(([v, r]) => [v, v === 'dock' && escondido ? TEXTOS.dockEscondido : r]);
  const dica = escondido ? TEXTOS.aoMinimizarDicaEscondido : TEXTOS.aoMinimizarDica;
  return `<div class="campo" data-campo="ao_minimizar"><span id="aj-ao-minimizar">${TEXTOS.aoMinimizarRotulo}</span>
<div class="seg" role="radiogroup" aria-labelledby="aj-ao-minimizar">${ops.map(([v, r]) => radio('ao_minimizar', v, r, atual)).join('')}</div>
<small>${dica}</small></div>`;
};

/**
 * "Lembretes no dock" (era "Esperando você"): o "esperando você" no dock recolhido (30 min) e no
 * dock expandido e no app (até a virada), e o pausado no dock recolhido (10 min; rodada 14).
 */
export const campoEspera = (c) =>
  [
    ['espera_dock_min', TEXTOS.esperaDock, c.espera_dock_min ?? 30, ESPERA_OPCOES],
    ['espera_app_min', TEXTOS.esperaApp, c.espera_app_min ?? 0, ESPERA_OPCOES],
    ['pausado_dock_min', TEXTOS.pausadoDock, c.pausado_dock_min ?? 10, PAUSADO_OPCOES],
  ]
    .map(
      ([nome, rotulo, atual, opcoes]) => `<div class="campo"><span id="aj-${nome}">${rotulo}</span>
<div class="seg" role="radiogroup" aria-labelledby="aj-${nome}">${opcoes.map(([v, r]) => radio(nome, v, r, atual)).join('')}</div></div>`,
    )
    .join('');

/**
 * "Conversa nova numa pasta com regra" (rodada 15; `CONVERSA_NOVA`): a conversa conta no timer da
 * regra e o dock pergunta "Certo?" (o padrão), ou só a regra, com o aviso "Ligada a X pela pasta".
 */
export const campoConversaNova = (atual) => `<div class="campo"><span id="aj-conversa-nova">${TEXTOS.conversaNovaRotulo}</span>
<div class="seg" role="radiogroup" aria-labelledby="aj-conversa-nova">${CONVERSA_NOVA.map(([v, r]) => radio('conversa_nova', v, r, atual ?? 'confirmar')).join('')}</div>
<small>${TEXTOS.conversaNovaDica}</small></div>`;

/**
 * "Espaço no dock recolhido" (rodada 24): as três formas, cada uma com uma prévia pequena da linha
 * do dock (um ponto, a linha da situação e o nome). As cores da prévia são fixas: o timer azul e o
 * espaço rosa, para a diferença aparecer.
 */
export function campoDockEspaco(atual) {
  const { timer, espaco } = TEXTOS.dockEspacoPrevia;
  const sit = TEXTOS.situacao.escrevendo.curta;
  const previa = {
    timer: `<i></i><b><strong>${esc(timer)}</strong></b>`,
    texto: `<i></i><b><small><u>${esc(espaco)}</u> · ${esc(sit)}</small><strong>${esc(timer)}</strong></b>`,
    cor: `<i class="e"></i><b><strong>${esc(timer)}</strong></b>`,
  };
  const valor = TEXTOS.dockEspacoOps.some(([v]) => v === atual) ? atual : 'timer';
  const ops = TEXTOS.dockEspacoOps.map(
    ([v, r]) =>
      `<label class="seg-op"><input type="radio" name="dock_espaco" value="${v}"${v === valor ? ' checked' : ''}><span><em class="dkp" aria-hidden="true">${previa[v]}</em>${r}</span></label>`,
  );
  return `<div class="campo"><span id="aj-dock-espaco">${TEXTOS.dockEspacoRotulo}</span>
<div class="seg seg-dkesp" role="radiogroup" aria-labelledby="aj-dock-espaco">${ops.join('')}</div>
<small>${TEXTOS.dockEspacoDica}</small></div>`;
}

/** "Dock" (`DOCK_MODO`): sempre à vista (o padrão) ou só quando houver uma pergunta (rodada 12). */
export const campoDockModo = (atual) => `<div class="campo"><span id="aj-dock-modo">${TEXTOS.dockModoRotulo}</span>
<div class="seg" role="radiogroup" aria-labelledby="aj-dock-modo">${DOCK_MODO.map(([v, r]) => radio('dock_modo', v, r, atual ?? 'sempre')).join('')}</div>
<small>${TEXTOS.dockModoDica}</small></div>`;

// ---- backups ----

/** Lista dos backups (o mais recente primeiro), com o "Restaurar" de cada um. */
export function htmlBackups(lista) {
  const itens = lista?.length
    ? lista
        .map(
          (b) => `<li><span class="aj-n mono">${esc(dataDoBackup(b.quando))}</span><small>${tamanho(b.bytes)}</small><button type="button" class="bt mini" data-aj="backup-restaurar" data-nome="${esc(b.nome)}">${TEXTOS.restaurar}</button></li>`,
        )
        .join('')
    : `<li class="aj-vazio">${TEXTOS.nenhumBackup}</li>`;
  return `<ul class="aj-lista" data-lista="backups">${itens}</ul>
<div class="aj-bts">
  <button type="button" class="bt pri" data-aj="backup-agora">${TEXTOS.backupAgora}</button>
  <button type="button" class="bt" data-aj="backup-pasta">${TEXTOS.abrirPasta}</button>
</div>`;
}

/** Corpo da confirmação do "Restaurar". */
export const corpoRestaurar = (b) =>
  `<p class="dlg-p">${TEXTOS.restaurarTexto1(`<b>${esc(dataDoBackup(b.quando))}</b>`)}</p>
<p class="dlg-p">${TEXTOS.restaurarTexto2}</p>`;

const secao = (id, titulo, corpo, dica = '') =>
  `<section class="aj-sec" id="aj-${id}" aria-labelledby="aj-${id}-t"><h3 id="aj-${id}-t">${titulo}</h3>${dica ? `<p class="aj-dica">${dica}</p>` : ''}${corpo}</section>`;

/**
 * O estado do cartão do Claude Code (Ajustes → Conexões e "Conectar apps"): `h` = "desligado" |
 * "antigo" | "ligado" (ou nulo, sem leitura); `encontrado`: a pasta `~\\.claude` existe neste PC.
 */
export function estadoDaConexao(h, encontrado = true) {
  if (!encontrado) {
    return {
      id: 'ausente',
      rotulo: TEXTOS.claudeNaoEncontrado,
      classe: 'ausente',
      botao: { aj: 'ligar-claude', texto: TEXTOS.conectar, desabilitado: true, dica: TEXTOS.instaleClaude },
    };
  }
  if (h === 'ligado') return { id: 'ligado', rotulo: TEXTOS.conectado, classe: 'on', botao: { aj: 'remover-hooks', texto: TEXTOS.desconectar } };
  if (h === 'antigo') return { id: 'antigo', rotulo: TEXTOS.precisaAtualizar, classe: 'antigo', botao: { aj: 'atualizar-hooks', texto: TEXTOS.atualizar, pri: true } };
  if (h === 'desligado') return { id: 'desligado', rotulo: TEXTOS.naoConectado, classe: '', botao: { aj: 'ligar-claude', texto: TEXTOS.conectar, pri: true } };
  return { id: 'sem-leitura', rotulo: TEXTOS.semLeitura, classe: '', botao: { aj: 'ligar-claude', texto: TEXTOS.conectar, pri: true } };
}

/**
 * O cartão do Claude Code: o estado, a frase, o arquivo (abreviado) e o botão do estado
 * (`comBotao` falso: no assistente, o botão é o do passo).
 */
export function htmlConexaoClaude(h, encontrado = true, settings = '', comBotao = true) {
  const e = estadoDaConexao(h, encontrado);
  const b = e.botao;
  const botao = `<button type="button" class="bt${b.pri ? ' pri' : ''}" data-aj="${b.aj}"${b.desabilitado ? ` disabled title="${esc(b.dica)}"` : ''}>${b.texto}</button>`;
  return `<div class="conexao" data-conexao="${e.id}">
  <div class="aj-linha"><b>Claude Code</b><span class="aj-estado ${e.classe}"><i></i>${e.rotulo}</span></div>
  <p class="conexao-frase">${FRASE_CLAUDE}</p>
  ${h === 'antigo' && encontrado ? `<p class="aj-antigo">${HOOKS_ANTIGOS}</p>` : ''}
  ${settings && encontrado ? `<small class="aj-cam">${esc(abreviarCaminho(settings))}</small>` : ''}
  ${comBotao ? `<div class="aj-bts">${botao}</div>` : ''}
  ${b.desabilitado ? `<small class="aj-dica">${esc(b.dica)}.</small>` : ''}
</div>`;
}

/** "21/11 14:05" (ou "Nov 21 14:05"): quando a conta foi vista por último. */
const quandoVista = (ms) => dataDoBackup(new Date(ms - new Date(ms).getTimezoneOffset() * 60_000).toISOString());

/**
 * Ajustes → "Contas do Claude" (rodada 28): só com pelo menos uma conta. Uma linha por conta:
 * "Conta ·1a2b" (os 4 primeiros do resumo), a origem e quando foi vista por último, o seletor de
 * espaço (com "Sem espaço"; só os não arquivados, mais o da conta, se estiver arquivado) e o
 * "Esquecer". Embaixo, a ajuda: o Chronn guarda só um resumo do id.
 */
export function htmlContas(contas, espacos = []) {
  if (!contas?.length) return '';
  const itens = contas
    .map((c) => {
      const visiveis = espacos.filter((e) => !e.arquivado || e.id === c.espaco_id);
      const ops = [`<option value=""${c.espaco_id == null ? ' selected' : ''}>${esc(TEXTOS.semEspaco)}</option>`]
        .concat(visiveis.map((e) => `<option value="${e.id}"${e.id === c.espaco_id ? ' selected' : ''}>${esc(e.nome)}</option>`))
        .join('');
      // 0.6.1: o rótulo do `~/.claude.json` ("Nome · ni•••@dominio · Pro"); sem ele, a curta
      const nome = c.rotulo || TEXTOS.contaCurta(c.curta);
      const espaco = espacos.find((e) => e.id === c.espaco_id);
      return `<li${espaco ? ` style="--c:${cor(espaco.cor)}"` : ''}><i></i><span class="aj-n">${esc(nome)}</span><small>${esc(TEXTOS.contaVista(TEXTOS.origemDaSessao(c.origem), quandoVista(c.visto_em)))}</small>
<select class="aj-conta-esp" data-conta="${esc(c.id)}" aria-label="${esc(TEXTOS.espacoDaConta(nome))}">${ops}</select>
<button type="button" class="bt mini" data-aj="conta-esquecer" data-conta="${esc(c.id)}" aria-label="${esc(TEXTOS.esquecerConta(nome))}">${esc(TEXTOS.esquecer)}</button></li>`;
    })
    .join('');
  return `<div class="aj-contas"><h4 class="aj-sub">${esc(TEXTOS.contasDoClaude)}</h4><ul class="aj-lista">${itens}</ul><small class="aj-dica">${esc(TEXTOS.contasAjuda)}</small></div>`;
}

/**
 * "Claude Code no WSL" (rodada 28), nos Ajustes e no diálogo de conectar: só com pelo menos uma
 * distro no registro (`distros` vazio ou nulo: nada). Uma linha por distro, com o nome e o estado
 * ("ligada", "Claude Code não encontrado", "parada", "hook desatualizado") e o "Ligar" (abre a
 * prévia) ou o "Desligar". Na distro parada (0.6.1), o "Ligar" liga a distro só no clique
 * (`data-subir`); o Chronn nunca liga a distro sozinho.
 */
export function htmlWsl(distros) {
  // a leitura falhou (o WSL não respondeu): a seção diz isso (revisão M10); ainda sem leitura, ou
  // sem distros, nada
  if (distros === null) {
    return `<div class="aj-wsl"><h4 class="aj-sub">${esc(TEXTOS.wslTitulo)}</h4><small class="aj-dica" role="note">${esc(TEXTOS.wslNaoDeuLer)}</small></div>`;
  }
  if (!distros?.length) return '';
  const classe = { ligada: 'on', desatualizado: 'antigo', sem_hooks: 'antigo', sem_claude: 'ausente', parada: 'ausente', desligada: '' };
  const itens = distros
    .map((d) => {
      const ligada = d.estado === 'ligada' || d.estado === 'desatualizado' || d.ligada;
      // 0.6.1: ligada, mas outro programa tirou os hooks do settings.json: o "Ligar de novo" (a
      // prévia, como no primeiro Ligar) vem antes do "Desligar"
      const deNovo =
        d.estado === 'sem_hooks'
          ? `<button type="button" class="bt mini pri" data-aj="wsl-ligar" data-distro="${esc(d.nome)}" title="${esc(TEXTOS.wslSemHooksDica(d.nome))}">${esc(TEXTOS.wslLigarDeNovo)}</button>`
          : '';
      const botao = ligada
        ? `<button type="button" class="bt mini" data-aj="wsl-desligar" data-distro="${esc(d.nome)}">${esc(TEXTOS.wslDesligar)}</button>`
        : d.estado === 'desligada'
          ? `<button type="button" class="bt mini pri" data-aj="wsl-ligar" data-distro="${esc(d.nome)}">${esc(TEXTOS.wslLigar)}</button>`
          : d.estado === 'parada'
            ? `<button type="button" class="bt mini" data-aj="wsl-ligar" data-subir="1" data-distro="${esc(d.nome)}" title="${esc(TEXTOS.wslLigarParada(d.nome))}">${esc(TEXTOS.wslLigar)}</button>`
            : '';
      return `<li data-wsl="${esc(d.nome)}"><span class="aj-n">${esc(d.nome)}</span><span class="aj-estado ${classe[d.estado] ?? ''}"><i></i>${esc(TEXTOS.wslEstado(d.estado))}</span><small></small>${deNovo}${botao}</li>`;
    })
    .join('');
  return `<div class="aj-wsl"><h4 class="aj-sub">${esc(TEXTOS.wslTitulo)}</h4><ul class="aj-lista">${itens}</ul><small class="aj-dica">${esc(TEXTOS.wslAjuda)}</small></div>`;
}

/** As distros do WSL agora (`[]` sem nenhuma; nulo se não deu para ler). Nunca prende a tela: quem chama desenha a seção quando ela chega (revisão I2). */
export async function lerWsl() {
  try {
    return await chamar('wsl_estado');
  } catch (e) {
    console.error('worktimer: wsl_estado', e);
    return null;
  }
}

/**
 * "Ligar" o Claude Code de uma distro: a prévia exata do `settings.json` de lá, como no Windows;
 * nada é gravado sem o clique. `subir` (0.6.1): a distro está parada, e o clique no "Ligar" a liga
 * para mostrar a prévia (e de novo no "Ligar" da prévia, se ela parou enquanto você lia). Resolve
 * `true` se ligou.
 */
export function ligarDistro(nome, subir = false) {
  // revisão M4: a subida pode levar uns segundos; um segundo clique não sobe a distro de novo
  if (subindo.has(nome)) return Promise.resolve(false);
  subindo.add(nome);
  return new Promise((ok) => {
    chamar('wsl_previa', { distro: nome, subir })
      .finally(() => subindo.delete(nome))
      .then(({ previa, settings }) => {
        let copia = null;
        const cam = `<code class="aj-cam">${esc(settings)}</code>`;
        abrirDialogo({
          titulo: esc(TEXTOS.wslLigarTitulo(nome)),
          classe: 'largo',
          corpo: `<p class="dlg-p">${TEXTOS.wslPreviaTexto(NOME_APP, cam)}</p>
<details class="mais aj-ver-muda" open><summary>${TEXTOS.verOQueMuda}</summary><div class="mais-corpo"><pre class="aj-previa" tabindex="0" aria-label="${TEXTOS.previaRotulo}">${esc(previa)}</pre></div></details>`,
          enviar: TEXTOS.wslLigar,
          async aoEnviar() {
            copia = await chamar('wsl_ligar', { distro: nome, subir });
          },
          aoFechar(enviado) {
            if (!enviado) return ok(false);
            abrirDialogo({ titulo: esc(TEXTOS.wslLigada(nome)), corpo: corpoInstalado(copia), enviar: TEXTOS.ok, cancelar: null, aoFechar: () => ok(true) });
          },
        });
      })
      .catch((e) => {
        abrirDialogo({
          titulo: esc(TEXTOS.wslLigarTitulo(nome)),
          corpo: `<p class="dlg-p">${TEXTOS.naoDeuPreparar(esc(textoDoErro(e)))}</p>`,
          enviar: null,
          cancelar: TEXTOS.fechar,
          aoFechar: () => ok(false),
        });
      });
  });
}

/** As distros com o "Ligar" em andamento (a prévia ainda não chegou). */
const subindo = new Set();

/** "Desligar" o Claude Code de uma distro: tira só os nossos hooks de lá. Resolve `true` se desligou. */
export function desligarDistro(nome) {
  return new Promise((ok) => {
    abrirDialogo({
      titulo: esc(TEXTOS.wslDesligarTitulo(nome)),
      corpo: `<p class="dlg-p">${TEXTOS.wslDesligarTexto(NOME_APP, '<code class="aj-cam">~/.claude/settings.json</code>')}</p>`,
      enviar: TEXTOS.wslDesligar,
      perigo: true,
      async aoEnviar() {
        await chamar('wsl_desligar', { distro: nome });
      },
      aoFechar: ok,
    });
  });
}

/**
 * Ajustes → Aparência (rodada 21; o tema saiu do cabeçalho): Claro, Escuro e Sistema (o padrão,
 * que segue o claro/escuro do Windows). O "windows" de antes vale como Sistema.
 */
export const htmlAparencia = (tema) => {
  const t = tema === 'claro' || tema === 'escuro' ? tema : 'sistema';
  return `<div class="seg" role="radiogroup" aria-label="${esc(TEXTOS.temaRotulo)}">${radio('tema', 'claro', TEXTOS.temaClaro, t)}${radio('tema', 'escuro', TEXTOS.temaEscuro, t)}${radio('tema', 'sistema', TEXTOS.temaSistema, t)}</div>`;
};

/**
 * Ajustes → Idioma (rodada 23): Sistema (o padrão: o idioma do Windows), Português (Brasil) e
 * English. O nome de cada idioma vem sempre no próprio idioma; embaixo, qual é o idioma do Windows
 * (com o Sistema escolhido, `efetivo`, o idioma do estado; senão, o `sistema` da página).
 */
export const htmlIdioma = (escolha, efetivo = null, sistema = globalThis.navigator?.language) => {
  const atual = escolha === 'pt-BR' || escolha === 'en' ? escolha : IDIOMA_SISTEMA;
  // o do Windows: com o Sistema escolhido, o que o núcleo resolveu; com outro, o da página
  const doWindows = atual === IDIOMA_SISTEMA && efetivo ? efetivo : resolverIdioma(IDIOMA_SISTEMA, sistema);
  const agora = IDIOMAS.find((i) => i.id === doWindows);
  // o nome de cada idioma com o `lang` dele (o leitor de tela pronuncia certo)
  const ops = [
    radio('idioma', IDIOMA_SISTEMA, TEXTOS.idiomaSistema, atual),
    ...IDIOMAS.map((i) => radio('idioma', i.id, `<bdi lang="${i.lang}">${esc(i.nome)}</bdi>`, atual)),
  ];
  return `<div class="seg" role="radiogroup" aria-label="${esc(TEXTOS.secoes.idioma)}">${ops.join('')}</div>
<small class="aj-dica">${esc(TEXTOS.idiomaSistemaDica(agora?.nome ?? ''))}</small>`;
};

/** Ajustes → Painel (rodada 21): mostrar a linha do tempo no Painel (ligado por padrão). */
export const htmlPainel = (c) => caixa('painel_linha', TEXTOS.mostrarLinhaNoPainel, TEXTOS.mostrarLinhaNoPainelDica, c?.painel_linha !== false);

/** O nome de um espaço vai até isto de letras (o núcleo confere também). */
export const ESPACO_NOME_MAX = 24;

/**
 * Ajustes → Grupos e regras: a lista dos espaços (rodada 20), com a cor, o nome (muda ao sair do
 * campo), subir e descer, Arquivar e Apagar. O último que sobra não arquiva nem apaga; um espaço
 * com grupos ou timers não apaga (mova os grupos antes). Os arquivados ficam no fim, apagados.
 */
export function htmlEspacos(snap) {
  const todos = [...(snap.espacos ?? [])].sort((a, b) => Number(a.arquivado) - Number(b.arquivado) || a.ordem - b.ordem || a.id - b.id);
  const ativos = todos.filter((e) => !e.arquivado);
  const item = (e, i) => {
    const grupos = snap.grupos.filter((g) => (g.espaco_id ?? 1) === e.id);
    const nGrupos = grupos.filter((g) => g.id !== null).length;
    const nTimers = grupos.reduce((s, g) => s + g.timers.length, 0);
    const uso = `<small>${TEXTOS.nGrupos(nGrupos)} · ${TEXTOS.nTimers(nTimers)}</small>`;
    const ultimo = !e.arquivado && ativos.length === 1;
    const desab = (cond, dica) => (cond ? ` disabled title="${esc(dica)}"` : '');
    const ordem = e.arquivado
      ? ''
      : `<button type="button" class="bt mini ic-ord" data-aj="espaco-subir" data-id="${e.id}" aria-label="${esc(TEXTOS.subir)}: ${esc(e.nome)}"${desab(i === 0, TEXTOS.subir)}>↑</button>` +
        `<button type="button" class="bt mini ic-ord" data-aj="espaco-descer" data-id="${e.id}" aria-label="${esc(TEXTOS.descer)}: ${esc(e.nome)}"${desab(i === ativos.length - 1, TEXTOS.descer)}>↓</button>`;
    const arquivar = e.arquivado
      ? `<button type="button" class="bt mini" data-aj="desarquivar-espaco" data-id="${e.id}">${esc(TEXTOS.desarquivar)}</button>`
      : `<button type="button" class="bt mini" data-aj="arquivar-espaco" data-id="${e.id}"${desab(ultimo, TEXTOS.ultimoEspaco)}>${esc(TEXTOS.arquivar)}</button>`;
    // o mesmo que o núcleo decide (revisão da rodada 20): sem nada à vista e nunca o último
    const ap = podeApagarEspaco(snap, e);
    const apagar = `<button type="button" class="bt mini" data-aj="apagar-espaco" data-id="${e.id}"${desab(!ap.pode, ap.dica)}>${esc(TEXTOS.apagar)}</button>`;
    return `<li class="aj-at aj-esp${e.arquivado ? ' arq' : ''}"><input type="color" name="esp_cor" data-id="${e.id}" value="${esc(cor(e.cor))}" aria-label="${esc(TEXTOS.corDe(e.nome))}"><input type="text" class="aj-at-nome" name="esp_nome" data-id="${e.id}" value="${esc(e.nome)}" maxlength="${ESPACO_NOME_MAX}" autocomplete="off" aria-label="${TEXTOS.nomeDoEspaco}"${e.arquivado ? ' disabled' : ''}>${uso}${ordem}${arquivar}${apagar}</li>`;
  };
  return `<h4>${esc(TEXTOS.espacosTitulo)}</h4><p class="aj-dica">${esc(TEXTOS.espacosDica)}</p><ul class="aj-lista" data-lista="espacos">${todos
    .map((e) => item(e, ativos.indexOf(e)))
    .join('')}</ul>
<button type="button" class="bt" data-aj="novo-espaco">${esc(TEXTOS.acrescentarEspaco)}</button>`;
}

function htmlGrupos(snap) {
  const grupos = snap.grupos.filter((g) => g.id !== null);
  const lg = grupos.length
    ? grupos
        .map(
          (g) => `<li style="--c:${cor(g.cor)}"><i></i><span class="aj-n">${esc(g.nome)}</span><small>${TEXTOS.nTimers(g.timers.length)}</small><button type="button" class="bt mini" data-aj="arquivar-grupo" data-id="${g.id}">${TEXTOS.arquivar}</button></li>`,
        )
        .join('')
    : `<li class="aj-vazio">${TEXTOS.nenhumGrupo}</li>`;
  const lr = dados.regras.length
    ? dados.regras
        .map(
          (r) => `<li><span class="aj-n mono" title="${esc(r.pasta)}">${esc(r.pasta)}</span><small>→ ${esc(r.timer_nome || TEXTOS.timerApagado)}</small><button type="button" class="bt mini" data-aj="remover-regra" data-id="${r.id}">${TEXTOS.remover}</button></li>`,
        )
        .join('')
    : `<li class="aj-vazio">${TEXTOS.nenhumaRegra}</li>`;
  return `${htmlEspacos(snap)}<h4>${TEXTOS.grupos}</h4><ul class="aj-lista" data-lista="grupos">${lg}</ul>
<h4>${TEXTOS.regrasDePasta}</h4><p class="aj-dica">${TEXTOS.regrasDica}</p><ul class="aj-lista" data-lista="regras">${lr}</ul>`;
}

/** O nome de uma atividade vai até isto de letras (o núcleo confere também). */
export const ATIVIDADE_NOME_MAX = 40;

/**
 * Ajustes → Atividades: cada atividade com a cor, o nome (muda ao sair do campo) e Arquivar. A
 * última que sobra não arquiva (o botão fica apagado). As arquivadas ficam no fim, apagadas, com
 * Desarquivar: os trechos e ajustes delas seguem contando nelas.
 */
export function htmlAtividades(snap) {
  const todas = snap.atividades ?? [];
  const ativas = todas.filter((a) => !a.arquivada);
  const timers = snap.grupos.flatMap((g) => g.timers);
  const item = (a) => {
    const n = timers.filter((t) => (t.atividade_padrao_id ?? 1) === a.id).length;
    const uso = n ? `<small>${TEXTOS.padraoDe(n)}</small>` : '<small></small>';
    const ultima = !a.arquivada && ativas.length === 1;
    const bt = a.arquivada
      ? `<button type="button" class="bt mini" data-aj="desarquivar-atividade" data-id="${a.id}">${TEXTOS.desarquivar}</button>`
      : `<button type="button" class="bt mini" data-aj="arquivar-atividade" data-id="${a.id}"${ultima ? ` disabled title="${TEXTOS.ultimaAtividade}"` : ''}>${TEXTOS.arquivar}</button>`;
    return `<li class="aj-at${a.arquivada ? ' arq' : ''}"><input type="color" name="at_cor" data-id="${a.id}" value="${esc(cor(a.cor))}" aria-label="${esc(TEXTOS.corDe(a.nome))}"><input type="text" class="aj-at-nome" name="at_nome" data-id="${a.id}" value="${esc(a.nome)}" maxlength="${ATIVIDADE_NOME_MAX}" autocomplete="off" aria-label="${TEXTOS.nomeDaAtividade}"${a.arquivada ? ' disabled' : ''}>${uso}${bt}</li>`;
  };
  return `<ul class="aj-lista" data-lista="atividades">${[...ativas, ...todas.filter((a) => a.arquivada)].map(item).join('')}</ul>
<button type="button" class="bt" data-aj="nova-atividade">${TEXTOS.acrescentarAtividade}</button>`;
}

function html(snap) {
  const c = snap.config;
  const tempos = TEMPOS.map(
    ([k, rotulo, dica]) => `<label class="campo aj-tempo">
  <span>${rotulo}</span>
  <div class="aj-un"><input type="text" inputmode="decimal" name="${k}" value="${minutosDeSegundos(c[k])}" autocomplete="off"><em>${TEXTOS.min}</em></div>
  <small>${dica}</small><small class="aj-erro" data-erro="${k}" hidden></small>
</label>`,
  ).join('');
  const auto = dados.autostart;
  return `<div class="aj">
<nav class="aj-nav" aria-label="${TEXTOS.secoesRotulo}">
  <button type="button" class="aj-voltar" data-aj="voltar">${TEXTOS.voltar}</button>
  ${SECOES.map(([id, t]) => `<a href="#aj-${id}" data-aj="ir" data-id="${id}">${t}</a>`).join('')}
</nav>
<div class="aj-corpo" data-rolagem="ajustes">
  <header class="aj-topo"><h2>${TEXTOS.ajustesTitulo}</h2><span class="aj-salvo" role="status">${TEXTOS.salvo}</span></header>
  ${secao('idioma', TEXTOS.secoes.idioma, htmlIdioma(c.idioma, snap.idioma), TEXTOS.idiomaDica)}
  ${secao('aparencia', TEXTOS.secoes.aparencia, htmlAparencia(c.tema))}
  ${secao('painel', TEXTOS.painelTitulo, htmlPainel(c))}
  ${secao(
    'dock',
    TEXTOS.secoes.dock,
    `${campoDockModo(c.dock_modo)}
<div class="seg" role="radiogroup" aria-label="${TEXTOS.botoesDock}">${radio('dock_botoes', 'um', TEXTOS.umBotao, c.dock_botoes)}${radio('dock_botoes', 'dois', TEXTOS.doisBotoes, c.dock_botoes)}</div>
${caixa('dock_situacao', TEXTOS.dockSituacao, TEXTOS.dockSituacaoDica, c.dock_situacao)}
${campoDockEspaco(c.dock_espaco)}`,
  )}
  ${secao('espera', TEXTOS.secoes.espera, campoEspera(c), TEXTOS.esperaSecaoDica)}
  ${secao(
    'sistema',
    TEXTOS.secoes.sistema,
    caixa(
      'iniciar_com_windows',
      TEXTOS.iniciarComWindows,
      auto === null ? TEXTOS.naoDeuLerEstadoAtual : TEXTOS.abreNaBandeja(NOME_APP),
      Boolean(auto),
      auto === null ? 'disabled' : '',
    ) +
      campoAoFechar(c.ao_fechar) +
      campoAoMinimizar(c.ao_minimizar, c.dock_modo),
  )}
  ${secao(
    'claude',
    TEXTOS.secoes.claude,
    `<div data-bloco="hooks">${htmlConexaoClaude(dados.hooks, dados.encontrado !== false, dados.info?.settings)}</div>
<div data-bloco="wsl">${htmlWsl(dados.wsl)}</div>
<div data-bloco="contas">${htmlContas(snap.contas, snap.espacos)}</div>
<label class="campo"><span>${TEXTOS.programasClaude}</span><textarea name="programas_claude" rows="4" spellcheck="false">${esc(c.programas_claude.join('\n'))}</textarea><small>${TEXTOS.programasClaudeDica}</small></label>`,
  )}
  ${secao('pastas', TEXTOS.secoes.pastas, campoPastas(c.pastas_ignoradas, ''), TEXTOS.pastasSecaoDica)}
  ${secao(
    'dia',
    TEXTOS.secoes.dia,
    `<div class="aj-grade">
<label class="campo aj-tempo"><span>${TEXTOS.corteDoDia}</span><div class="aj-un"><input type="text" name="corte" value="${esc(dados.info?.corte ?? '00:00')}" autocomplete="off" placeholder="HH:MM"><em>HH:MM</em></div><small>${TEXTOS.corteDica}</small><small class="aj-erro" data-erro="corte" hidden></small></label>
</div>
${caixa('zerar_na_virada', TEXTOS.zerarNaVirada, TEXTOS.zerarNaViradaDica, c.zerar_na_virada)}`,
  )}
  ${secao('tempos', TEXTOS.secoes.tempos, `<p class="aj-risco" role="note">${AVISO_TEMPOS}</p><div class="aj-grade">${tempos}</div>`, TEXTOS.temposSecaoDica)}
  ${secao(
    'arredondamento',
    TEXTOS.secoes.arredondamento,
    `<div class="seg" role="radiogroup" aria-label="${TEXTOS.arredondamento}">${[1, 5, 15].map((m) => radio('arredondamento_min', m, `${m} ${TEXTOS.min}`, c.arredondamento_min)).join('')}</div>`,
    TEXTOS.arredondamentoDica,
  )}
  ${secao(
    'atividades',
    TEXTOS.secoes.atividades,
    `<div data-bloco="atividades">${htmlAtividades(snap)}</div>`,
    TEXTOS.atividadesSecaoDica,
  )}
  ${secao('grupos', TEXTOS.gruposERegras, `${campoConversaNova(c.conversa_nova)}<div data-bloco="grupos">${htmlGrupos(snap)}</div>`)}
  ${secao(
    'backups',
    TEXTOS.secoes.backups,
    `<div data-bloco="backups">${htmlBackups(dados.backups)}</div>`,
    TEXTOS.backupsSecaoDica,
  )}
  ${secao('ajuda', TEXTOS.secoes.ajuda, `<div data-bloco="ajuda">${htmlAjuda(c)}${htmlApagarPerfil()}</div>`)}
  ${secao(
    'sobre',
    TEXTOS.secoes.sobre,
    `<div class="aj-sobre"><div class="logo"><b aria-hidden="true"></b>${esc(NOME_APP)}</div>
<p>${TEXTOS.versao} <span class="mono">${esc(textoDaVersao(dados.info?.versao ?? '?', dados.info?.canal))}</span></p>
${htmlAtualizacao({ ligada: c.atualizar_auto !== false, ultima: c.atualizacao_ultima ?? null, estado: verificacao.estado, novidade: verificacao.novidade, app: NOME_APP })}
${htmlCreditos(dados.info?.assinatura)}
${campoModoDev(c.modo_dev)}</div>`,
  )}
</div>
</div>`;
}

// ---- carregar e redesenhar partes ----

/** O "Verificar atualizações" (rodada 25): o estado da linha e a versão achada. */
const verificacao = { estado: null, novidade: null };

function desenharVerificacao() {
  const el = raiz?.querySelector('[data-atu="resultado"]');
  if (el) el.innerHTML = linhaDaVerificacao(verificacao.estado, verificacao.novidade);
  const bt = raiz?.querySelector('[data-aj="atu-verificar"]');
  if (bt) bt.disabled = verificacao.estado === 'procurando';
}

/** Procura agora: "Procurando…", depois o resultado na linha (o ícone do topo vem pelo evento). */
async function verificarAtualizacoes() {
  Object.assign(verificacao, { estado: 'procurando', novidade: null });
  desenharVerificacao();
  try {
    const n = await chamar('atualizacao_procurar');
    Object.assign(verificacao, { estado: n ? 'disponivel' : 'recente', novidade: n ?? null });
  } catch (e) {
    console.error('worktimer: atualizacao_procurar', e);
    Object.assign(verificacao, { estado: 'erro', novidade: null });
  }
  desenharVerificacao();
}

async function carregar() {
  const [info, conexao, regras, autostart, backups] = await Promise.allSettled([
    chamar('ajustes_info'),
    chamar('conexao_claude'),
    chamar('regras_pasta'),
    iniciarComWindows.ligado(),
    chamar('backups_listar'),
  ]);
  const val = (r, padrao) => {
    if (r.status === 'fulfilled') return r.value;
    console.error('worktimer: ajustes', r.reason);
    return padrao;
  };
  dados = {
    info: val(info, null),
    hooks: val(conexao, null)?.estado ?? null,
    encontrado: val(conexao, null)?.encontrado ?? true,
    regras: val(regras, []),
    autostart: val(autostart, null),
    backups: val(backups, []),
    // o WSL chega depois (`atualizarWsl`), sem prender os Ajustes (revisão I2)
    wsl: undefined,
  };
}

/** Lê e redesenha a seção "Claude Code no WSL" (rodada 28), só se mudou. */
async function atualizarWsl() {
  const wsl = await lerWsl();
  if (!dados) return;
  dados.wsl = wsl;
  const b = raiz?.querySelector('[data-bloco="wsl"]');
  const novo = htmlWsl(wsl);
  if (b && b.innerHTML !== novo) b.innerHTML = novo;
}

/** A cada quanto a seção do WSL é lida de novo com os Ajustes à vista (0.6.1). */
export const WSL_A_CADA_MS = 10_000;

/**
 * Os Ajustes seguem à vista em `el` (montados ali, não escondidos, com a janela visível)? Só
 * assim a seção do WSL é lida de novo (0.6.1): fechados, nada chama o `wsl.exe`.
 */
export const ajustesAVista = (el, atual, doc = globalThis.document) =>
  Boolean(el) && atual === el && !el.hidden && el.isConnected !== false && doc?.visibilityState !== 'hidden';

let relerWsl = null;

/** Lê a seção do WSL a cada `WSL_A_CADA_MS` enquanto os Ajustes estão à vista (0.6.1). */
function vigiarWsl(el) {
  clearInterval(relerWsl);
  relerWsl = setInterval(() => {
    // saiu dos Ajustes: para (a próxima montagem começa de novo)
    if (raiz !== el || el.hidden || el.isConnected === false) {
      clearInterval(relerWsl);
      return;
    }
    // a janela escondida, a seção ausente (sem distros) ou um diálogo aberto (a prévia do
    // "Ligar"): espera
    if (!ajustesAVista(el, raiz) || !el.querySelector('[data-bloco="wsl"]') || document.querySelector('.dlg-fundo') || subindo.size) return;
    atualizarWsl();
  }, WSL_A_CADA_MS);
}

/** Redesenha "Contas do Claude" se mudou (com o seletor em uso, espera). */
function desenharContas() {
  const b = raiz?.querySelector('[data-bloco="contas"]');
  const snap = lerSnap();
  if (!b || !snap) return;
  if (b.contains(document.activeElement) && document.activeElement.matches('select')) return;
  const novo = htmlContas(snap.contas, snap.espacos);
  if (b.innerHTML !== novo) b.innerHTML = novo;
}

function desenharBackups() {
  const b = raiz?.querySelector('[data-bloco="backups"]');
  if (b) b.innerHTML = htmlBackups(dados.backups);
}

async function atualizarHooks() {
  try {
    const c = await chamar('conexao_claude');
    dados.hooks = c.estado;
    dados.encontrado = c.encontrado;
  } catch (e) {
    console.error('worktimer: conexao_claude', e);
    dados.hooks = null;
  }
  const b = raiz?.querySelector('[data-bloco="hooks"]');
  if (b) b.innerHTML = htmlConexaoClaude(dados.hooks, dados.encontrado !== false, dados.info?.settings);
  // a faixa da janela principal acompanha
  document.dispatchEvent(new CustomEvent('hooks-mudaram', { detail: dados.hooks }));
}

function desenharGrupos() {
  const b = raiz?.querySelector('[data-bloco="grupos"]');
  const snap = lerSnap();
  if (!b || !snap) return;
  // só redesenha se mudou (o foco do teclado fica onde está)
  const novo = htmlGrupos(snap);
  if (novo !== gruposDesenhados) {
    b.innerHTML = novo;
    gruposDesenhados = novo;
  }
}

/** Redesenha a lista das atividades se mudou, menos com um campo dela sendo editado. */
function desenharAtividades(forcar = false) {
  const b = raiz?.querySelector('[data-bloco="atividades"]');
  const snap = lerSnap();
  if (!b || !snap) return;
  if (!forcar && b.contains(document.activeElement) && document.activeElement.matches('input')) return;
  const novo = htmlAtividades(snap);
  if (novo !== atividadesDesenhadas || forcar) {
    b.innerHTML = novo;
    atividadesDesenhadas = novo;
  }
}

/** A atividade como o núcleo grava (`editar_atividade`). */
const atividadeParaSalvar = (a) => ({ id: a.id, nome: a.nome, cor: a.cor, ordem: a.ordem, arquivada: Boolean(a.arquivada) });

/**
 * Grava a atividade mudada. Arquivar a padrão de algum timer: o núcleo recusa sem `confirmar`, e a
 * recusa vira a pergunta (Reunião é a atividade padrão de Cliente A. Arquivar mesmo assim?), com a
 * atividade para a qual eles passam.
 */
async function salvarAtividade(a, mudar) {
  const nova = { ...atividadeParaSalvar(a), ...mudar };
  try {
    await chamar('editar_atividade', { atividade: nova, confirmar: false });
    mostrarSalvo();
  } catch (e) {
    const msg = textoDoErro(e);
    if (nova.arquivada && !a.arquivada && TEXTOS.reAtividadePadraoDe.test(msg)) {
      const sobra = (lerSnap()?.atividades ?? []).find((x) => x.id !== a.id && !x.arquivada);
      abrirDialogo({
        titulo: TEXTOS.arquivarAtividade(esc(a.nome)),
        corpo: `<p class="dlg-p">${TEXTOS.arquivarAtividadeTexto(esc(msg), esc(sobra?.nome ?? TEXTOS.outraAtividade), esc(a.nome))}</p>`,
        enviar: TEXTOS.arquivar,
        perigo: true,
        async aoEnviar() {
          await chamar('editar_atividade', { atividade: nova, confirmar: true });
        },
      });
    } else {
      faixa(msg);
    }
  } finally {
    desenharAtividades(true);
  }
}

/** O espaço como o núcleo grava (`editar_espaco`). */
const espacoParaSalvar = (e) => ({ id: e.id, nome: e.nome, cor: e.cor, ordem: e.ordem, arquivado: Boolean(e.arquivado) });

/** Grava o espaço mudado; a recusa do núcleo vai para a faixa. */
async function salvarEspaco(e, mudar) {
  try {
    await chamar('editar_espaco', { espaco: { ...espacoParaSalvar(e), ...mudar } });
    mostrarSalvo();
  } catch (erro) {
    faixa(textoDoErro(erro));
  } finally {
    gruposDesenhados = '';
    desenharGrupos();
  }
}

/** Sobe (`passo` −1) ou desce (+1) um espaço na lista: regrava a ordem de cada um que mudou. */
async function moverEspaco(id, passo) {
  const ativos = [...(lerSnap()?.espacos ?? [])].filter((e) => !e.arquivado).sort((a, b) => a.ordem - b.ordem || a.id - b.id);
  const i = ativos.findIndex((e) => e.id === id);
  const j = i + passo;
  if (i < 0 || j < 0 || j >= ativos.length) return;
  [ativos[i], ativos[j]] = [ativos[j], ativos[i]];
  try {
    for (const [k, e] of ativos.entries()) {
      if (e.ordem !== k) await chamar('editar_espaco', { espaco: { ...espacoParaSalvar(e), ordem: k } });
    }
    mostrarSalvo();
  } catch (erro) {
    faixa(textoDoErro(erro));
  }
}

/** Apagar um espaço (vazio): confirma antes; a recusa do núcleo (grupos arquivados nele) vai para a faixa. */
function confirmarApagarEspaco(id) {
  const e = (lerSnap()?.espacos ?? []).find((x) => x.id === id);
  if (!e) return;
  abrirDialogo({
    titulo: esc(TEXTOS.confirmarApagarEspaco(e.nome)),
    corpo: `<p class="dlg-p">${esc(textoAoApagar(lerSnap(), e))}</p>`,
    enviar: TEXTOS.apagar,
    perigo: true,
    async aoEnviar() {
      await chamar('apagar_espaco', { id });
    },
  });
}

/** "+ Acrescentar atividade": nome e cor. */
function novaAtividade() {
  const usadas = (lerSnap()?.atividades ?? []).map((a) => a.cor);
  const livre = PALETA.find((c) => !usadas.includes(c)) ?? PALETA[0];
  abrirDialogo({
    titulo: TEXTOS.novaAtividade,
    corpo: `<label class="campo"><span>${TEXTOS.nome}</span><input type="text" name="nome" maxlength="${ATIVIDADE_NOME_MAX}" autocomplete="off" autofocus placeholder="${esc(TEXTOS.atividadePlaceholder)}"></label>
<label class="campo"><span>${TEXTOS.cor}</span><input type="color" name="cor" value="${livre}"></label>`,
    enviar: TEXTOS.acrescentar,
    async aoEnviar(form) {
      const nome = form.elements.nome.value.trim();
      if (!nome) {
        form.elements.nome.focus();
        throw TEXTOS.nomeVazio;
      }
      await chamar('criar_atividade', { nome, cor: form.elements.cor.value });
    },
  });
}

// ---- eventos ----

function erroDoCampo(nome, msg) {
  const el = raiz.querySelector(`[data-erro="${nome}"]`);
  if (!el) return;
  el.textContent = msg;
  el.hidden = !msg;
}

async function aoMudar(ev) {
  const el = ev.target;
  // o espaço de uma conta do Claude (rodada 28): não é da config
  if (el.matches?.('select[data-conta]')) {
    try {
      await chamar('conta_definir', { conta: el.dataset.conta, espaco_id: el.value === '' ? null : Number(el.value) });
    } catch (e) {
      faixa(textoDoErro(e));
    }
    return;
  }
  const nome = el.name;
  if (!nome) return;
  if (nome === 'esp_nome' || nome === 'esp_cor') {
    const e = (lerSnap()?.espacos ?? []).find((x) => x.id === Number(el.dataset.id));
    if (!e) return;
    const v = nome === 'esp_nome' ? el.value.trim() : el.value;
    if (nome === 'esp_nome' && (v === e.nome || !v)) {
      el.value = e.nome;
      if (!v) faixa(TEXTOS.nomeVazio);
      return;
    }
    return salvarEspaco(e, nome === 'esp_nome' ? { nome: v } : { cor: v });
  }
  if (nome === 'at_nome' || nome === 'at_cor') {
    const a = (lerSnap()?.atividades ?? []).find((x) => x.id === Number(el.dataset.id));
    if (!a) return;
    const v = nome === 'at_nome' ? el.value.trim() : el.value;
    if (nome === 'at_nome' && (v === a.nome || !v)) {
      el.value = a.nome;
      if (!v) faixa(TEXTOS.nomeVazio);
      return;
    }
    return salvarAtividade(a, nome === 'at_nome' ? { nome: v } : { cor: v });
  }
  // os valores são lidos agora, não quando a gravação sair da fila
  if (nome === 'tema' || nome === 'idioma' || nome === 'dock_botoes' || nome === 'dock_espaco' || nome === 'ao_fechar' || nome === 'ao_minimizar' || nome === 'conversa_nova') {
    const v = el.value;
    return salvar((c) => (c[nome] = v));
  }
  if (nome === 'dock_modo') {
    const v = el.value;
    trocarAoMinimizar(v);
    return salvar((c) => (c.dock_modo = v));
  }
  if (nome === 'espera_dock_min' || nome === 'espera_app_min' || nome === 'pausado_dock_min') {
    const v = Number(el.value);
    return salvar((c) => (c[nome] = v));
  }
  if (nome === 'arredondamento_min') {
    const v = Number(el.value);
    return salvar((c) => (c.arredondamento_min = v));
  }
  // as dicas têm o comando delas (o "Salvar" da config não muda as dicas; rodada 16)
  if (nome === 'dicas_ligadas') {
    const v = el.checked;
    return chamar('dicas_ligar', { ligadas: v });
  }
  if (nome === 'dock_situacao' || nome === 'zerar_na_virada' || nome === 'painel_linha' || nome === 'atualizar_auto') {
    const v = el.checked;
    return salvar((c) => (c[nome] = v));
  }
  if (nome === 'modo_dev') {
    const v = el.checked;
    // ligar pede a confirmação explícita; até ela, a caixa fica desmarcada
    if (v) {
      el.checked = false;
      return confirmarModoDev(el);
    }
    mostrarTempos(false);
    // desligado, o núcleo volta os tempos aos padrões
    return salvar((c) => (c.modo_dev = false));
  }
  if (nome === 'programas_claude' || nome === 'pastas_ignoradas') {
    const v = linhas(el.value);
    el.value = v.join('\n');
    return salvar((c) => (c[nome] = v));
  }
  const tempo = TEMPOS.find(([k]) => k === nome);
  if (tempo) {
    const max = tempo[3];
    const s = segundosDeMinutos(el.value, max);
    if (s === null) return erroDoCampo(nome, TEXTOS.minutosDe1a(max));
    erroDoCampo(nome, '');
    return salvar((c) => (c[nome] = s));
  }
  if (nome === 'corte') {
    try {
      const r = await chamar('ler_corte', { texto: el.value });
      el.value = r.texto;
      erroDoCampo('corte', '');
      return salvar((c) => (c.corte_min = r.min));
    } catch (e) {
      return erroDoCampo('corte', textoDoErro(e));
    }
  }
  if (nome === 'iniciar_com_windows') {
    const ligar = el.checked;
    el.disabled = true;
    try {
      await (ligar ? iniciarComWindows.ligar() : iniciarComWindows.desligar());
      dados.autostart = await iniciarComWindows.ligado();
      el.checked = dados.autostart;
      const v = dados.autostart;
      await salvar((c) => (c.iniciar_com_windows = v));
    } catch (e) {
      console.error('worktimer: autostart', e);
      el.checked = !ligar;
      faixa(textoDoErro(e));
    } finally {
      el.disabled = false;
    }
  }
}

/** "Modo desenvolvedor: por sua conta e risco": só liga com "Ligar mesmo assim". */
function confirmarModoDev(el) {
  abrirDialogo({
    titulo: TITULO_MODO_DEV,
    corpo: corpoModoDev(),
    enviar: TEXTOS.ligarMesmoAssim,
    perigo: true,
    async aoEnviar() {
      // só marca se gravou (o erro fica no diálogo)
      await salvar((c) => (c.modo_dev = true), { lancar: true });
      el.checked = true;
      mostrarTempos(true);
    },
  });
}

/** "Restaurar" um backup, com confirmação explícita. */
function confirmarRestaurar(nome) {
  const b = dados.backups.find((x) => x.nome === nome);
  if (!b) return;
  abrirDialogo({
    titulo: TEXTOS.restaurarBackupTitulo(esc(dataDoBackup(b.quando))),
    corpo: corpoRestaurar(b),
    enviar: TEXTOS.restaurar,
    perigo: true,
    async aoEnviar() {
      await esperarGravacoes();
      await chamar('backup_restaurar', { nome });
      [dados.backups, dados.regras] = await Promise.all([chamar('backups_listar'), chamar('regras_pasta')]);
      desenharBackups();
      gruposDesenhados = '';
      desenharGrupos();
      faixa(TEXTOS.backupRestaurado(dataDoBackup(b.quando)));
    },
  });
}

function confirmarArquivar(id) {
  const g = lerSnap()?.grupos.find((x) => x.id === id);
  if (!g) return;
  abrirDialogo({
    titulo: esc(TEXTOS.arquivarGrupoTitulo(g.nome)),
    corpo: `<p class="dlg-p">${TEXTOS.arquivarGrupoTexto2}</p>`,
    enviar: TEXTOS.arquivar,
    perigo: true,
    async aoEnviar() {
      await chamar('editar_grupo', {
        grupo: { ...grupoParaSalvar(g), arquivado: true },
      });
    },
  });
}

function confirmarRemoverRegra(id) {
  const r = dados.regras.find((x) => x.id === id);
  if (!r) return;
  abrirDialogo({
    titulo: TEXTOS.removerRegraTitulo,
    corpo: `<p class="dlg-p">${TEXTOS.removerRegraTexto(`<code class="aj-cam">${esc(r.pasta)}</code>`, esc(r.timer_nome || TEXTOS.timerApagado))}</p>`,
    enviar: TEXTOS.remover,
    perigo: true,
    async aoEnviar() {
      dados.regras = await chamar('remover_regra_pasta', { id: r.id });
      desenharGrupos();
    },
  });
}

async function aoClicar(ev) {
  const alvo = ev.target.closest('[data-aj]');
  if (!alvo || alvo.disabled) return;
  const id = Number(alvo.dataset.id);
  switch (alvo.dataset.aj) {
    case 'voltar':
      voltarFn();
      break;
    case 'atu-verificar':
      await verificarAtualizacoes();
      break;
    case 'atu-instalar':
      abrirAtualizar(verificacao.novidade);
      break;
    case 'link':
      chamar('abrir_link', { qual: alvo.dataset.link }).catch((e) => console.error('worktimer: abrir_link', e));
      break;
    case 'ir': {
      ev.preventDefault();
      raiz.querySelector(`#aj-${alvo.dataset.id}`)?.scrollIntoView({ block: 'start', behavior: 'smooth' });
      break;
    }
    case 'ligar-claude': {
      await ligarAoClaude();
      await atualizarHooks();
      // o diálogo pode ter mudado as pastas que nunca contam
      const campo = raiz?.querySelector('.aj textarea[name="pastas_ignoradas"]');
      if (campo && campo !== document.activeElement) {
        try {
          campo.value = (await chamar('obter_estado')).config.pastas_ignoradas.join('\n');
        } catch (e) {
          console.error('worktimer: obter_estado', e);
        }
      }
      break;
    }
    case 'atualizar-hooks':
      await ligarAoClaude({ atualizar: true });
      await atualizarHooks();
      break;
    case 'remover-hooks':
      await removerHooks();
      await atualizarHooks();
      break;
    case 'wsl-ligar':
      // a distro parada leva uns segundos para subir: o botão diz isso e não aceita outro clique
      if (alvo.dataset.subir === '1') {
        alvo.disabled = true;
        alvo.textContent = TEXTOS.wslLigando;
      }
      if (await ligarDistro(alvo.dataset.distro, alvo.dataset.subir === '1')) await atualizarWsl();
      else if (alvo.isConnected) {
        alvo.disabled = false;
        alvo.textContent = TEXTOS.wslLigar;
      }
      break;
    case 'wsl-desligar':
      if (await desligarDistro(alvo.dataset.distro)) await atualizarWsl();
      break;
    case 'conta-esquecer':
      try {
        await chamar('conta_esquecer', { conta: alvo.dataset.conta });
      } catch (e) {
        faixa(textoDoErro(e));
      }
      break;
    case 'backup-agora':
      alvo.disabled = true;
      try {
        dados.backups = await chamar('backup_agora');
        desenharBackups();
        faixa(TEXTOS.backupFeito);
      } catch (e) {
        console.error('worktimer: backup_agora', e);
        faixa(textoDoErro(e));
        alvo.disabled = false;
      }
      break;
    case 'backup-pasta':
      chamar('backups_abrir_pasta').catch((e) => faixa(textoDoErro(e)));
      break;
    case 'backup-restaurar':
      confirmarRestaurar(alvo.dataset.nome);
      break;
    case 'ver-tour':
      document.dispatchEvent(new CustomEvent('pedir-tour', { detail: { tipo: 'essencial' } }));
      break;
    case 'tour-completo':
      document.dispatchEvent(new CustomEvent('pedir-tour', { detail: { tipo: 'completo' } }));
      break;
    case 'apagar-perfil-teste':
      abrirDialogo({
        titulo: TEXTOS.apagarPerfilTitulo,
        corpo: `<p class="dlg-p">${TEXTOS.apagarPerfilTexto(NOME_APP, '<code class="aj-cam">--perfil teste</code>')}</p>`,
        enviar: TEXTOS.apagarEFechar,
        perigo: true,
        aoEnviar: () => chamar('apagar_perfil_teste'),
      });
      break;
    case 'dicas-de-novo':
      await chamar('dicas_de_novo');
      faixa(TEXTOS.dicasDeNovoFeito);
      break;
    case 'arquivar-grupo':
      confirmarArquivar(id);
      break;
    case 'remover-regra':
      confirmarRemoverRegra(id);
      break;
    case 'arquivar-atividade':
    case 'desarquivar-atividade': {
      const a = (lerSnap()?.atividades ?? []).find((x) => x.id === id);
      if (a) await salvarAtividade(a, { arquivada: alvo.dataset.aj === 'arquivar-atividade' });
      break;
    }
    case 'nova-atividade':
      novaAtividade();
      break;
    // os espaços (rodada 20)
    case 'novo-espaco':
      await novoEspaco(lerSnap());
      break;
    case 'espaco-subir':
    case 'espaco-descer':
      await moverEspaco(id, alvo.dataset.aj === 'espaco-subir' ? -1 : 1);
      break;
    case 'arquivar-espaco':
    case 'desarquivar-espaco': {
      const e = (lerSnap()?.espacos ?? []).find((x) => x.id === id);
      if (e) await salvarEspaco(e, { arquivado: alvo.dataset.aj === 'arquivar-espaco' });
      break;
    }
    case 'apagar-espaco':
      confirmarApagarEspaco(id);
      break;
  }
}

let voltarFn = () => {};

/** Monta a tela em `el`. `voltar()` sai dos Ajustes. */
export async function montarAjustes(el, opcoes) {
  raiz = el;
  lerSnap = opcoes.lerSnap;
  voltarFn = opcoes.voltar;
  el.innerHTML = `<p class="vazio aj-carregando">${TEXTOS.carregando}</p>`;
  await carregar();
  const snap = lerSnap();
  if (raiz !== el || !snap || el.hidden) return;
  el.innerHTML = html(snap);
  atualizarWsl();
  vigiarWsl(el);
  gruposDesenhados = htmlGrupos(snap);
  atividadesDesenhadas = htmlAtividades(snap);
  mostrarTempos(Boolean(snap.config.modo_dev));
  if (!el.dataset.ligado) {
    el.dataset.ligado = '1';
    el.addEventListener('change', (ev) => {
      if (raiz !== el) return;
      const p = aoMudar(ev);
      tratando.add(p);
      p.finally(() => tratando.delete(p));
    });
    el.addEventListener('click', (ev) => raiz === el && aoClicar(ev));
    // as "Pastas deste timer" (editar timer) mudaram as regras: a lista daqui acompanha
    document.addEventListener('regras-mudaram', (ev) => {
      if (raiz !== el || !dados) return;
      dados.regras = ev.detail ?? dados.regras;
      desenharGrupos();
    });
  }
}

/** O "Ao minimizar a janela" com o texto do modo do dock (a opção marcada fica). */
function trocarAoMinimizar(modoDock) {
  const el = raiz?.querySelector('[data-campo="ao_minimizar"]');
  if (!el) return;
  const atual = el.querySelector('input[name="ao_minimizar"]:checked')?.value;
  el.outerHTML = campoAoMinimizar(atual, modoDock);
}

/** Snapshot novo com os Ajustes abertos: acompanha o tema, o dock e os grupos, sem mexer nos campos sendo editados. */
export function atualizarAjustes(snap) {
  if (!raiz || !raiz.querySelector('.aj')) return;
  desenharGrupos();
  desenharAtividades();
  desenharContas();
  if (pendentes > 0) return;
  const c = snap.config;
  for (const [nome, valor] of [
    ['idioma', c.idioma ?? IDIOMA_SISTEMA],
    ['tema', c.tema],
    ['dock_botoes', c.dock_botoes],
    ['dock_espaco', c.dock_espaco ?? 'timer'],
    ['ao_fechar', c.ao_fechar],
    ['ao_minimizar', c.ao_minimizar],
    ['dock_modo', c.dock_modo ?? 'sempre'],
    ['conversa_nova', c.conversa_nova ?? 'confirmar'],
    ['espera_dock_min', c.espera_dock_min ?? 30],
    ['espera_app_min', c.espera_app_min ?? 0],
    ['pausado_dock_min', c.pausado_dock_min ?? 10],
    ['arredondamento_min', c.arredondamento_min],
  ]) {
    const r = raiz.querySelector(`input[name="${nome}"][value="${valor}"]`);
    if (r && !r.checked) r.checked = true;
  }
  for (const nome of ['dock_situacao', 'modo_dev', 'zerar_na_virada', 'painel_linha']) {
    const cx = raiz.querySelector(`input[name="${nome}"]`);
    if (cx && cx !== document.activeElement) cx.checked = Boolean(c[nome]);
  }
  // a busca de atualização (rodada 25): ligada por padrão, e a data da última busca
  const auto = raiz.querySelector('input[name="atualizar_auto"]');
  if (auto && auto !== document.activeElement) auto.checked = c.atualizar_auto !== false;
  const ultima = raiz.querySelector('[data-atu="ultima"]');
  if (ultima) ultima.textContent = textoDaUltima(c.atualizacao_ultima ?? null);
  const dicasLigadas = raiz.querySelector('input[name="dicas_ligadas"]');
  if (dicasLigadas && dicasLigadas !== document.activeElement) dicasLigadas.checked = c.dicas_ligadas !== false;
  mostrarTempos(Boolean(c.modo_dev));
  // os tempos voltam aos padrões ao desligar o modo dev: os campos acompanham
  for (const [k] of TEMPOS) {
    const campo = raiz.querySelector(`input[name="${k}"]`);
    if (campo && campo !== document.activeElement && raiz.querySelector(`[data-erro="${k}"]`)?.hidden !== false) {
      campo.value = minutosDeSegundos(c[k]);
    }
  }
}
