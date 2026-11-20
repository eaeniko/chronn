// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Tela de Ajustes (⚙), na janela principal. Cada campo grava sozinho ao mudar: `salvar_config`
// leva a config inteira, lida do núcleo na hora (assim não desfaz o tema trocado no topo, a
// visão ou o dock). As gravações vão em fila, uma de cada vez.
// Também mora aqui o "Conectar o Claude Code" (prévia exata → Conectar), usado no primeiro uso.
import { chamar, iniciarComWindows } from './api.js';
import { esc, cor } from './componentes.js';
import { abrirDialogo, faixa, textoDoErro, grupoParaSalvar, PALETA, novoEspaco } from './editar.js';
import { linhas, minutosDeSegundos, segundosDeMinutos, tamanho, abreviarCaminho } from './formato.js';
import { NOME_APP, PERFIL_TESTE } from './nome.js';
import { TEXTOS } from './textos.js';
import { podeApagarEspaco, textoAoApagar } from './espacos.js';

export const CREDITOS = 'Criado por @eaeniko (Nikolai Cinotti)';

/**
 * Os créditos do Sobre (rodada 18): o `@eaeniko` abre o site do autor e "Código no GitHub" o
 * código, no navegador padrão (o núcleo abre; a página só diz qual dos dois). Embaixo, a
 * versão e a marca de autoria.
 */
export function htmlCreditos(assinatura = '') {
  return `<p class="aj-cred">Criado por <button type="button" class="aj-link" data-aj="link" data-link="autor" title="eaeniko.okin.club">@eaeniko</button> (Nikolai Cinotti) · <button type="button" class="aj-link" data-aj="link" data-link="codigo" title="github.com/eaeniko/chronn">Código no GitHub</button></p>
${assinatura ? `<p class="aj-assinatura mono">${esc(assinatura)}</p>` : ''}`;
}

const SECOES = [
  ['aparencia', 'Aparência'],
  ['dock', 'Dock'],
  ['espera', 'Lembretes no dock'],
  ['sistema', 'Sistema'],
  ['claude', 'Conexões'],
  ['pastas', 'Pastas que nunca contam'],
  ['dia', 'Dia de trabalho'],
  ['tempos', 'Tempos'],
  ['arredondamento', 'Arredondamento'],
  ['atividades', 'Atividades'],
  ['grupos', 'Grupos e regras'],
  ['backups', 'Backups'],
  ['ajuda', 'Ajuda'],
  ['sobre', 'Sobre'],
];

/** "Apagar o perfil de teste e recomeçar" (rodada 17): só existe no perfil de teste. */
export const htmlApagarPerfil = (teste = PERFIL_TESTE) =>
  teste
    ? `<div class="aj-bts"><button type="button" class="bt perigo" data-aj="apagar-perfil-teste">Apagar o perfil de teste e recomeçar</button></div>
<small class="aj-dica">Perfil de teste: fecha o app de teste e apaga a pasta dele (os dados, a config e o Claude Code simulado). O perfil de uso não muda.</small>`
    : '';

/** Ajustes → Ajuda (rodada 16): as dicas na tela, mostrá-las de novo e o tour. */
export const htmlAjuda = (c) =>
  `${caixa('dicas_ligadas', 'Dicas na tela', 'Uma dica curta na primeira vez que cada coisa acontece: o Claude contando, uma pergunta, o ↺ Zerar…', c?.dicas_ligadas !== false)}
<div class="aj-bts">
  <button type="button" class="bt" data-aj="dicas-de-novo">Mostrar as dicas de novo</button>
  <button type="button" class="bt pri" data-aj="ver-tour">Como começar</button>
  <button type="button" class="bt" data-aj="tour-completo">Tour completo</button>
</div>
<small class="aj-dica">"Como começar" leva um minuto pelo essencial: os cartões, o Claude contando, as pastas e o dock. O tour completo passa por todas as partes da janela. Em cada tela, o ? do topo explica o que ela mostra.</small>`;

/**
 * Campos de tempo (só no modo desenvolvedor; sem ele valem os padrões): config em segundos, tela
 * em minutos. O último número é o máximo em minutos.
 */
export const TEMPOS = [
  ['uso_s', 'Uso recente', 'A leitura da conversa aberta no Claude só conta se houve mouse ou teclado neste tempo. Padrão: 3 min.', 1440],
  ['inatividade_s', 'Inatividade', 'Sem mouse e teclado por este tempo, os timers manuais pausam. Padrão: 10 min.', 30],
  ['sessao_parada_s', 'Sessão parada', 'Claude trabalhando sem nenhum evento por este tempo vira parada. Padrão: 10 min.', 1440],
];

/** O interruptor discreto do Sobre. */
export const campoModoDev = (ligado) => `<label class="chk aj-dev">
  <input type="checkbox" name="modo_dev"${ligado ? ' checked' : ''}><span>Modo desenvolvedor<small>Mostra a seção Tempos. Ao desligar, os tempos voltam aos padrões.</small></span>
</label>`;

/** Título e corpo da confirmação ao ligar o modo desenvolvedor. */
export const TITULO_MODO_DEV = 'Modo desenvolvedor: por sua conta e risco';
export const corpoModoDev = () => `<p class="dlg-p">Ele libera a seção Tempos, que muda quando o app conta uso, inatividade e sessão parada.</p>
<p class="dlg-p">Tempos fora do padrão podem contar horas a mais ou a menos e deixar os seus números diferentes dos colegas.</p>
<p class="dlg-p">Só ligue se você sabe o que está fazendo. Ao desligar, os tempos voltam aos padrões.</p>`;

/** Faixa fixa da seção Tempos. */
export const AVISO_TEMPOS =
  'Por sua conta e risco: mudar estes tempos pode contar horas a mais ou a menos e deixar seus números diferentes dos colegas.';

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

const DICA_PASTAS = 'Uma por linha. Sessões do Claude nestas pastas (e subpastas) ficam de fora: o hook nem registra.';

/** Campo "Pastas que nunca contam" (Ajustes e o diálogo de ligar ao Claude Code). */
const campoPastas = (pastas, rotulo) => `<label class="campo">${rotulo ? `<span>${rotulo}</span>` : ''}
  <textarea name="pastas_ignoradas" rows="2" spellcheck="false" aria-label="Pastas que nunca contam" placeholder="Ex.: G:\\Projetos pessoais">${esc(pastas.join('\n'))}</textarea>
  <small>${DICA_PASTAS}</small>
</label>`;

/** Texto do estado "antigo" dos hooks (Ajustes e a faixa da janela principal). */
export const HOOKS_ANTIGOS = 'Os hooks apontam para a versão antiga. Atualizar?';

/** A faixa dos hooks desligados com timers (revisão da rodada 16; "conectado" desde a rodada 18). */
export const HOOKS_DESLIGADOS = 'O Claude Code não está conectado: o tempo do Claude não está contando.';

/**
 * A faixa dos hooks na janela principal, ou nula: com os hooks da versão antiga, "Atualizar" (e o
 * ×); desligados com timers (uma reinstalação, alguém tirou à mão), "Ligar" e "Agora não". Sem
 * timers, quem liga é o primeiro uso. As duas somem até a próxima abertura pelo × ou "Agora não".
 */
export function faixaDosHooks(estado, temTimers, encontrado = true) {
  if (estado === 'antigo') return { texto: HOOKS_ANTIGOS, botao: 'Atualizar', atualizar: true, esconder: '×' };
  // sem o Claude Code neste PC, a faixa não oferece conectar (rodada 18)
  if (estado === 'desligado' && temTimers && encontrado) return { texto: HOOKS_DESLIGADOS, botao: 'Conectar', atualizar: false, esconder: 'Agora não' };
  return null;
}

/**
 * Corpo com a prévia exata do settings.json (bloco monoespaçado com rolagem) e, antes de ligar,
 * as pastas que nunca contam: elas valem desde o primeiro evento. `atualizar`: os hooks da
 * versão antiga saem e entram os de agora.
 */
export function corpoPrevia(previa, settings, pastas = [], atualizar = false) {
  const o = atualizar ? 'trocar os hooks da versão antiga pelos de agora' : 'acrescentar os hooks dele';
  // o caminho abreviado no texto (rodada 18); a prévia, exata e recolhida em "Ver o que muda"
  return `<p class="dlg-p">O ${NOME_APP} vai ${o} em <code class="aj-cam">${esc(abreviarCaminho(settings || 'settings.json'))}</code>. O que já existe no arquivo fica como está, e antes de gravar ele faz uma cópia de segurança.</p>
<details class="mais aj-ver-muda"><summary>Ver o que muda</summary><div class="mais-corpo"><p class="aj-dica">Os caminhos são os do seu usuário neste PC.</p>
<pre class="aj-previa" tabindex="0" aria-label="Prévia da mudança">${esc(previa)}</pre></div></details>
${campoPastas(pastas, 'Pastas que nunca contam (opcional)')}`;
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
    ? `<p class="dlg-p">Cópia de segurança do arquivo anterior:</p><code class="aj-cam aj-bloco">${esc(abreviarCaminho(copia))}</code>`
    : '<p class="dlg-p">Não havia configuração antes: arquivo novo criado.</p>';
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
  const titulo = atualizar ? 'Atualizar a conexão do Claude Code' : 'Conectar o Claude Code';
  const { previa, settings, pastas, erro } = await lerPrevia();
  return new Promise((ok) => {
    if (erro) {
      abrirDialogo({
        titulo,
        corpo: `<p class="dlg-p">Não deu para preparar a mudança: ${esc(erro)}</p>`,
        enviar: null,
        cancelar: 'Fechar',
        aoFechar: () => ok(false),
      });
      return;
    }
    let copia = null;
    abrirDialogo({
      titulo,
      classe: 'largo',
      corpo: corpoPrevia(previa, settings, pastas, atualizar),
      enviar: atualizar ? 'Atualizar' : 'Conectar',
      async aoEnviar(form) {
        // as pastas primeiro: o hook já nasce sabendo o que não registrar
        await gravarPastas(form);
        copia = await chamar('hooks_instalar');
      },
      aoFechar(enviado) {
        if (!enviado) return ok(false);
        abrirDialogo({
          titulo: atualizar ? 'Conexão atualizada' : 'Claude Code conectado',
          corpo: corpoInstalado(copia),
          enviar: 'OK',
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
      titulo: 'Desconectar o Claude Code?',
      corpo: `<p class="dlg-p">Os hooks do ${NOME_APP} saem de <code class="aj-cam">${esc(abreviarCaminho(dados.info?.settings ?? 'settings.json'))}</code> (antes, uma cópia de segurança). O resto do arquivo fica como está. As sessões do Claude deixam de entrar sozinhas nos timers.</p>`,
      enviar: 'Desconectar',
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

/** "Ao fechar a janela": o que o X da janela principal faz. */
export const AO_FECHAR = [
  ['perguntar', 'Perguntar'],
  ['bandeja', 'Manter na bandeja'],
  ['sair', 'Sair'],
];

export const campoAoFechar = (atual) => `<div class="campo"><span id="aj-ao-fechar">Ao fechar a janela</span>
<div class="seg" role="radiogroup" aria-labelledby="aj-ao-fechar">${AO_FECHAR.map(([v, r]) => radio('ao_fechar', v, r, atual)).join('')}</div>
<small>Na bandeja o ${NOME_APP} continua contando, e a janela sai da memória até você abrir de novo.</small></div>`;

/** "Ao minimizar a janela": o que o minimizar da janela principal faz. */
export const AO_MINIMIZAR = [
  ['perguntar', 'Perguntar'],
  ['bandeja', 'Bandeja'],
  ['dock', 'Abrir o dock'],
  ['barra', 'Barra de tarefas'],
];

/**
 * `modoDock`: com o dock "Só quando houver uma pergunta", a opção do dock não promete o dock
 * (rodada 12): o minimizar só minimiza.
 */
export const campoAoMinimizar = (atual, modoDock = 'sempre') => {
  const escondido = modoDock === 'perguntas';
  const ops = AO_MINIMIZAR.map(([v, r]) => [v, v === 'dock' && escondido ? 'Dock (escondido)' : r]);
  const dica = escondido
    ? 'Bandeja: como fechar, a janela sai da memória. Dock (escondido): com o dock só nas perguntas, só minimiza. Barra de tarefas: só minimiza.'
    : 'Bandeja: como fechar, a janela sai da memória. Abrir o dock: minimiza e mostra o dock. Barra de tarefas: só minimiza.';
  return `<div class="campo" data-campo="ao_minimizar"><span id="aj-ao-minimizar">Ao minimizar a janela</span>
<div class="seg" role="radiogroup" aria-labelledby="aj-ao-minimizar">${ops.map(([v, r]) => radio('ao_minimizar', v, r, atual)).join('')}</div>
<small>${dica}</small></div>`;
};

/** Os tempos do "esperando você" (rodada 13), em minutos; 0 = até a virada do dia. */
export const ESPERA_OPCOES = [
  [15, '15 min'],
  [30, '30 min'],
  [60, '1 h'],
  [120, '2 h'],
  [0, 'Até a virada do dia'],
];

/** Os tempos do pausado no dock recolhido (rodada 14), em minutos; 0 = sai na hora. */
export const PAUSADO_OPCOES = [
  [0, 'Sai na hora'],
  [5, '5 min'],
  [10, '10 min'],
  [30, '30 min'],
  [60, '1 h'],
];

/**
 * "Lembretes no dock" (era "Esperando você"): o "esperando você" no dock recolhido (30 min) e no
 * dock expandido e no app (até a virada), e o pausado no dock recolhido (10 min; rodada 14).
 */
export const campoEspera = (c) =>
  [
    ['espera_dock_min', 'Esperando você, no dock recolhido', c.espera_dock_min ?? 30, ESPERA_OPCOES],
    ['espera_app_min', 'Esperando você, no dock expandido e no app', c.espera_app_min ?? 0, ESPERA_OPCOES],
    ['pausado_dock_min', 'Pausado, no dock recolhido', c.pausado_dock_min ?? 10, PAUSADO_OPCOES],
  ]
    .map(
      ([nome, rotulo, atual, opcoes]) => `<div class="campo"><span id="aj-${nome}">${rotulo}</span>
<div class="seg" role="radiogroup" aria-labelledby="aj-${nome}">${opcoes.map(([v, r]) => radio(nome, v, r, atual)).join('')}</div></div>`,
    )
    .join('');

/**
 * "Conversa nova numa pasta com regra" (rodada 15): a conversa conta no timer da regra e o dock
 * pergunta "Certo?" (o padrão), ou só a regra, com o aviso "Ligada a X pela pasta".
 */
export const CONVERSA_NOVA = [
  ['confirmar', 'Confirmar sempre'],
  ['regra', 'Usar a regra direto'],
];

export const campoConversaNova = (atual) => `<div class="campo"><span id="aj-conversa-nova">Conversa nova numa pasta com regra</span>
<div class="seg" role="radiogroup" aria-labelledby="aj-conversa-nova">${CONVERSA_NOVA.map(([v, r]) => radio('conversa_nova', v, r, atual ?? 'confirmar')).join('')}</div>
<small>Confirmar sempre: a conversa já conta no timer da regra e o dock pergunta se está certo (Certo, Outro timer…, + Novo timer…); sem resposta, fica na regra. Usar a regra direto: só o aviso "Ligada a … pela pasta". Uma conversa continuada (o mesmo nome, na mesma pasta) segue o timer da anterior sem perguntar.</small></div>`;

/** "Dock": sempre à vista (o padrão) ou só quando houver uma pergunta (rodada 12). */
export const DOCK_MODO = [
  ['sempre', 'Sempre à vista'],
  ['perguntas', 'Só quando houver uma pergunta'],
];

export const campoDockModo = (atual) => `<div class="campo"><span id="aj-dock-modo">Quando o dock aparece</span>
<div class="seg" role="radiogroup" aria-labelledby="aj-dock-modo">${DOCK_MODO.map(([v, r]) => radio('dock_modo', v, r, atual ?? 'sempre')).join('')}</div>
<small>Só quando houver uma pergunta: o dock fica escondido e aparece sozinho com uma pergunta (sessão nova do Claude, volta depois de um tempo sem mexer no PC, app fechado), sem tirar o foco do que você está fazendo; respondida, ele some de novo. As perguntas aparecem também na janela principal. Para ver o dock a qualquer hora: "Mostrar o dock agora", no ícone da bandeja.</small></div>`;

// ---- backups ----

/** Lista dos backups (o mais recente primeiro), com o "Restaurar" de cada um. */
export function htmlBackups(lista) {
  const itens = lista?.length
    ? lista
        .map(
          (b) => `<li><span class="aj-n mono">${esc(b.quando)}</span><small>${tamanho(b.bytes)}</small><button type="button" class="bt mini" data-aj="backup-restaurar" data-nome="${esc(b.nome)}">Restaurar</button></li>`,
        )
        .join('')
    : '<li class="aj-vazio">Nenhum backup ainda.</li>';
  return `<ul class="aj-lista" data-lista="backups">${itens}</ul>
<div class="aj-bts">
  <button type="button" class="bt pri" data-aj="backup-agora">Fazer backup agora</button>
  <button type="button" class="bt" data-aj="backup-pasta">Abrir pasta</button>
</div>`;
}

/** Corpo da confirmação do "Restaurar". */
export const corpoRestaurar = (b) =>
  `<p class="dlg-p">O banco volta a ser como estava em <b>${esc(b.quando)}</b>. As horas contadas depois disso deixam de aparecer.</p>
<p class="dlg-p">Antes, o estado de agora vira um backup novo: dá para voltar a ele por esta mesma lista. O app reabre o banco na hora, sem reiniciar.</p>`;

const secao = (id, titulo, corpo, dica = '') =>
  `<section class="aj-sec" id="aj-${id}" aria-labelledby="aj-${id}-t"><h3 id="aj-${id}-t">${titulo}</h3>${dica ? `<p class="aj-dica">${dica}</p>` : ''}${corpo}</section>`;

/** A frase do cartão do Claude Code (rodada 18). */
export const FRASE_CLAUDE = 'Com o Claude Code conectado, o timer da pasta conta sozinho enquanto o Claude trabalha.';

/**
 * O estado do cartão do Claude Code (Ajustes → Conexões e "Conectar apps"): `h` = "desligado" |
 * "antigo" | "ligado" (ou nulo, sem leitura); `encontrado`: a pasta `~\\.claude` existe neste PC.
 */
export function estadoDaConexao(h, encontrado = true) {
  if (!encontrado) {
    return {
      id: 'ausente',
      rotulo: 'Claude Code não encontrado neste PC',
      classe: 'ausente',
      botao: { aj: 'ligar-claude', texto: 'Conectar', desabilitado: true, dica: 'Instale o Claude Code e volte aqui' },
    };
  }
  if (h === 'ligado') return { id: 'ligado', rotulo: 'Conectado', classe: 'on', botao: { aj: 'remover-hooks', texto: 'Desconectar' } };
  if (h === 'antigo') return { id: 'antigo', rotulo: 'Precisa atualizar', classe: 'antigo', botao: { aj: 'atualizar-hooks', texto: 'Atualizar', pri: true } };
  if (h === 'desligado') return { id: 'desligado', rotulo: 'Não conectado', classe: '', botao: { aj: 'ligar-claude', texto: 'Conectar', pri: true } };
  return { id: 'sem-leitura', rotulo: 'Não deu para ler o estado', classe: '', botao: { aj: 'ligar-claude', texto: 'Conectar', pri: true } };
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
    const uso = `<small>${nGrupos === 1 ? '1 grupo' : `${nGrupos} grupos`} · ${nTimers === 1 ? '1 timer' : `${nTimers} timers`}</small>`;
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
    return `<li class="aj-at aj-esp${e.arquivado ? ' arq' : ''}"><input type="color" name="esp_cor" data-id="${e.id}" value="${esc(cor(e.cor))}" aria-label="Cor de ${esc(e.nome)}"><input type="text" class="aj-at-nome" name="esp_nome" data-id="${e.id}" value="${esc(e.nome)}" maxlength="${ESPACO_NOME_MAX}" autocomplete="off" aria-label="Nome do espaço"${e.arquivado ? ' disabled' : ''}>${uso}${ordem}${arquivar}${apagar}</li>`;
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
          (g) => `<li style="--c:${cor(g.cor)}"><i></i><span class="aj-n">${esc(g.nome)}</span><small>${g.timers.length === 1 ? '1 timer' : `${g.timers.length} timers`}</small><button type="button" class="bt mini" data-aj="arquivar-grupo" data-id="${g.id}">Arquivar</button></li>`,
        )
        .join('')
    : '<li class="aj-vazio">Nenhum grupo.</li>';
  const lr = dados.regras.length
    ? dados.regras
        .map(
          (r) => `<li><span class="aj-n mono" title="${esc(r.pasta)}">${esc(r.pasta)}</span><small>→ ${esc(r.timer_nome || 'timer apagado')}</small><button type="button" class="bt mini" data-aj="remover-regra" data-id="${r.id}">Remover</button></li>`,
        )
        .join('')
    : '<li class="aj-vazio">Nenhuma regra ainda. Elas nascem ao ligar uma sessão com “Sempre que for nesta pasta” e no editar timer, em “Pastas deste timer”.</li>';
  return `${htmlEspacos(snap)}<h4>Grupos</h4><ul class="aj-lista" data-lista="grupos">${lg}</ul>
<h4>Regras de pasta</h4><p class="aj-dica">Sessões do Claude nestas pastas (e subpastas) entram sozinhas no timer.</p><ul class="aj-lista" data-lista="regras">${lr}</ul>`;
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
    const uso = n ? `<small>padrão de ${n === 1 ? '1 timer' : `${n} timers`}</small>` : '<small></small>';
    const ultima = !a.arquivada && ativas.length === 1;
    const bt = a.arquivada
      ? `<button type="button" class="bt mini" data-aj="desarquivar-atividade" data-id="${a.id}">Desarquivar</button>`
      : `<button type="button" class="bt mini" data-aj="arquivar-atividade" data-id="${a.id}"${ultima ? ' disabled title="Precisa sobrar pelo menos uma atividade"' : ''}>Arquivar</button>`;
    return `<li class="aj-at${a.arquivada ? ' arq' : ''}"><input type="color" name="at_cor" data-id="${a.id}" value="${esc(cor(a.cor))}" aria-label="Cor de ${esc(a.nome)}"><input type="text" class="aj-at-nome" name="at_nome" data-id="${a.id}" value="${esc(a.nome)}" maxlength="${ATIVIDADE_NOME_MAX}" autocomplete="off" aria-label="Nome da atividade"${a.arquivada ? ' disabled' : ''}>${uso}${bt}</li>`;
  };
  return `<ul class="aj-lista" data-lista="atividades">${[...ativas, ...todas.filter((a) => a.arquivada)].map(item).join('')}</ul>
<button type="button" class="bt" data-aj="nova-atividade">+ Acrescentar atividade</button>`;
}

function html(snap) {
  const c = snap.config;
  const tempos = TEMPOS.map(
    ([k, rotulo, dica]) => `<label class="campo aj-tempo">
  <span>${rotulo}</span>
  <div class="aj-un"><input type="text" inputmode="decimal" name="${k}" value="${minutosDeSegundos(c[k])}" autocomplete="off"><em>min</em></div>
  <small>${dica}</small><small class="aj-erro" data-erro="${k}" hidden></small>
</label>`,
  ).join('');
  const auto = dados.autostart;
  return `<div class="aj">
<nav class="aj-nav" aria-label="Seções dos ajustes">
  <button type="button" class="aj-voltar" data-aj="voltar">← Voltar</button>
  ${SECOES.map(([id, t]) => `<a href="#aj-${id}" data-aj="ir" data-id="${id}">${t}</a>`).join('')}
</nav>
<div class="aj-corpo" data-rolagem="ajustes">
  <header class="aj-topo"><h2>Ajustes</h2><span class="aj-salvo" role="status">Salvo</span></header>
  ${secao(
    'aparencia',
    'Aparência',
    `<div class="seg" role="radiogroup" aria-label="Tema">${radio('tema', 'escuro', 'Escuro', c.tema)}${radio('tema', 'claro', 'Claro', c.tema)}${radio('tema', 'windows', 'Seguir o Windows', c.tema)}</div>`,
  )}
  ${secao(
    'dock',
    'Dock',
    `${campoDockModo(c.dock_modo)}
<div class="seg" role="radiogroup" aria-label="Botões do dock">${radio('dock_botoes', 'um', 'Um botão, em etapas', c.dock_botoes)}${radio('dock_botoes', 'dois', 'Dois botões', c.dock_botoes)}</div>
${caixa('dock_situacao', 'Mostrar o que cada timer está fazendo no dock', 'Claude trabalhando, você lendo (ou escrevendo na conversa) ou esperando você. Uma sessão que terminou aparece como lembrete, mesmo com o timer parado, pelo tempo de "Esperando você".', c.dock_situacao)}`,
  )}
  ${secao('espera', 'Lembretes no dock', campoEspera(c), 'Quanto tempo o "esperando você" fica à vista depois que o Claude termina (ou "segundo plano", com uma tarefa ainda rodando), e quanto tempo o timer pausado fica no dock recolhido, com o ▶ para retomar. Só a tela muda: a contagem do tempo segue igual.')}
  ${secao(
    'sistema',
    'Sistema',
    caixa(
      'iniciar_com_windows',
      'Iniciar com o Windows',
      auto === null ? 'Não deu para ler o estado atual.' : `O ${NOME_APP} abre na bandeja quando você entra no Windows.`,
      Boolean(auto),
      auto === null ? 'disabled' : '',
    ) +
      campoAoFechar(c.ao_fechar) +
      campoAoMinimizar(c.ao_minimizar, c.dock_modo),
  )}
  ${secao(
    'claude',
    'Conexões',
    `<div data-bloco="hooks">${htmlConexaoClaude(dados.hooks, dados.encontrado !== false, dados.info?.settings)}</div>
<label class="campo"><span>Programas do Claude</span><textarea name="programas_claude" rows="4" spellcheck="false">${esc(c.programas_claude.join('\n'))}</textarea><small>Um por linha, o nome do programa sem “.exe”. A leitura só conta com um deles em uso.</small></label>`,
  )}
  ${secao('pastas', 'Pastas que nunca contam', campoPastas(c.pastas_ignoradas, ''), 'Para projetos pessoais, por exemplo.')}
  ${secao(
    'dia',
    'Dia de trabalho',
    `<div class="aj-grade">
<label class="campo aj-tempo"><span>Corte do dia</span><div class="aj-un"><input type="text" name="corte" value="${esc(dados.info?.corte ?? '00:00')}" autocomplete="off" placeholder="HH:MM"><em>HH:MM</em></div><small>O trabalho antes deste horário conta para o dia anterior.</small><small class="aj-erro" data-erro="corte" hidden></small></label>
</div>
${caixa('zerar_na_virada', 'Zerar sozinho na virada do dia', 'No corte do dia, todos os timers voltam a 0:00. Desligado, cada timer só volta a 0:00 pelo ↺ Zerar (ou pelo "Zerar todos", no topo). O total do dia, o relatório e o "Lançar o dia" seguem por dia de qualquer jeito.', c.zerar_na_virada)}`,
  )}
  ${secao('tempos', 'Tempos', `<p class="aj-risco" role="note">${AVISO_TEMPOS}</p><div class="aj-grade">${tempos}</div>`, 'Modo desenvolvedor. A inatividade vai de 1 a 30 min; os outros, de 1 min a 24 h.')}
  ${secao(
    'arredondamento',
    'Arredondamento',
    `<div class="seg" role="radiogroup" aria-label="Arredondamento">${[1, 5, 15].map((m) => radio('arredondamento_min', m, `${m} min`, c.arredondamento_min)).join('')}</div>`,
    'Usado ao lançar o dia.',
  )}
  ${secao(
    'atividades',
    'Atividades',
    `<div data-bloco="atividades">${htmlAtividades(snap)}</div>`,
    'O tipo de cada hora de um timer. O Claude trabalhando, a leitura e o ▶ usam a atividade padrão do timer (no editar, em "Mais opções"); o botão de reunião, ao lado do ▶, conta como Reunião. Arquivar a Reunião tira esse botão dos timers.',
  )}
  ${secao('grupos', 'Grupos e regras de pasta', `${campoConversaNova(c.conversa_nova)}<div data-bloco="grupos">${htmlGrupos(snap)}</div>`)}
  ${secao(
    'backups',
    'Backups',
    `<div data-bloco="backups">${htmlBackups(dados.backups)}</div>`,
    'Uma cópia do banco na primeira abertura de cada dia e a cada 6 h com o app aberto. Ficam as 14 mais recentes.',
  )}
  ${secao('ajuda', 'Ajuda', `<div data-bloco="ajuda">${htmlAjuda(c)}${htmlApagarPerfil()}</div>`)}
  ${secao(
    'sobre',
    'Sobre',
    `<div class="aj-sobre"><div class="logo"><b aria-hidden="true"></b>${esc(NOME_APP)}</div>
<p>Versão <span class="mono">${esc(dados.info?.versao ?? '?')}</span></p>
${htmlCreditos(dados.info?.assinatura)}
${campoModoDev(c.modo_dev)}</div>`,
  )}
</div>
</div>`;
}

// ---- carregar e redesenhar partes ----

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
  };
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
    if (nova.arquivada && !a.arquivada && /atividade padrão de/.test(msg)) {
      const sobra = (lerSnap()?.atividades ?? []).find((x) => x.id !== a.id && !x.arquivada);
      abrirDialogo({
        titulo: `Arquivar ${esc(a.nome)}?`,
        corpo: `<p class="dlg-p">${esc(msg)}. Esses timers passam a usar ${esc(sobra?.nome ?? 'outra atividade')}; as horas já contadas continuam em ${esc(a.nome)}.</p>`,
        enviar: 'Arquivar',
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
    titulo: 'Nova atividade',
    corpo: `<label class="campo"><span>Nome</span><input type="text" name="nome" maxlength="${ATIVIDADE_NOME_MAX}" autocomplete="off" autofocus placeholder="Ex.: Suporte"></label>
<label class="campo"><span>Cor</span><input type="color" name="cor" value="${livre}"></label>`,
    enviar: 'Acrescentar',
    async aoEnviar(form) {
      const nome = form.elements.nome.value.trim();
      if (!nome) {
        form.elements.nome.focus();
        throw 'O nome não pode ficar vazio';
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
  const nome = el.name;
  if (!nome) return;
  if (nome === 'esp_nome' || nome === 'esp_cor') {
    const e = (lerSnap()?.espacos ?? []).find((x) => x.id === Number(el.dataset.id));
    if (!e) return;
    const v = nome === 'esp_nome' ? el.value.trim() : el.value;
    if (nome === 'esp_nome' && (v === e.nome || !v)) {
      el.value = e.nome;
      if (!v) faixa('O nome não pode ficar vazio');
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
      if (!v) faixa('O nome não pode ficar vazio');
      return;
    }
    return salvarAtividade(a, nome === 'at_nome' ? { nome: v } : { cor: v });
  }
  // os valores são lidos agora, não quando a gravação sair da fila
  if (nome === 'tema' || nome === 'dock_botoes' || nome === 'ao_fechar' || nome === 'ao_minimizar' || nome === 'conversa_nova') {
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
  if (nome === 'dock_situacao' || nome === 'zerar_na_virada') {
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
    if (s === null) return erroDoCampo(nome, `Use um número de minutos de 1 a ${max}.`);
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
    enviar: 'Ligar mesmo assim',
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
    titulo: `Restaurar o backup de ${esc(b.quando)}?`,
    corpo: corpoRestaurar(b),
    enviar: 'Restaurar',
    perigo: true,
    async aoEnviar() {
      await esperarGravacoes();
      await chamar('backup_restaurar', { nome });
      [dados.backups, dados.regras] = await Promise.all([chamar('backups_listar'), chamar('regras_pasta')]);
      desenharBackups();
      gruposDesenhados = '';
      desenharGrupos();
      faixa(`Backup de ${b.quando} restaurado.`);
    },
  });
}

function confirmarArquivar(id) {
  const g = lerSnap()?.grupos.find((x) => x.id === id);
  if (!g) return;
  abrirDialogo({
    titulo: `Arquivar o grupo ${esc(g.nome)}?`,
    corpo: '<p class="dlg-p">O grupo e os timers dele saem das listas. O tempo já contado fica guardado.</p>',
    enviar: 'Arquivar',
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
    titulo: 'Remover a regra de pasta?',
    corpo: `<p class="dlg-p">Sessões novas em <code class="aj-cam">${esc(r.pasta)}</code> deixam de entrar sozinhas em ${esc(r.timer_nome || 'timer apagado')}. As sessões já ligadas continuam.</p>`,
    enviar: 'Remover',
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
    case 'backup-agora':
      alvo.disabled = true;
      try {
        dados.backups = await chamar('backup_agora');
        desenharBackups();
        faixa('Backup feito.');
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
        titulo: 'Apagar o perfil de teste?',
        corpo: `<p class="dlg-p">O ${NOME_APP} de teste fecha e a pasta dele é apagada: os timers, as horas, a config e o Claude Code simulado. Ao abrir de novo com <code class="aj-cam">--perfil teste</code>, ele começa do zero. O perfil de uso não muda.</p>`,
        enviar: 'Apagar e fechar',
        perigo: true,
        aoEnviar: () => chamar('apagar_perfil_teste'),
      });
      break;
    case 'dicas-de-novo':
      await chamar('dicas_de_novo');
      faixa('As dicas vão aparecer de novo, cada uma na primeira vez que a coisa acontecer.');
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
  el.innerHTML = '<p class="vazio aj-carregando">Carregando…</p>';
  await carregar();
  const snap = lerSnap();
  if (raiz !== el || !snap || el.hidden) return;
  el.innerHTML = html(snap);
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
  if (pendentes > 0) return;
  const c = snap.config;
  for (const [nome, valor] of [
    ['tema', c.tema],
    ['dock_botoes', c.dock_botoes],
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
  for (const nome of ['dock_situacao', 'modo_dev', 'zerar_na_virada']) {
    const cx = raiz.querySelector(`input[name="${nome}"]`);
    if (cx && cx !== document.activeElement) cx.checked = Boolean(c[nome]);
  }
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
