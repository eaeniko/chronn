// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { pedidoDeLigar } from './componentes.js';
import { criarFila, DICAS } from './dicas.js';

const fonte = (nome) => readFileSync(new URL(`./${nome}`, import.meta.url), 'utf8');

test('rodada 16: o dock não abre diálogos; pede o "Ligar a um timer" à janela principal', () => {
  const dock = fonte('dock.js');
  assert.doesNotMatch(dock, /abrirDialogo|ligarATimer/, 'nenhum diálogo na página do dock');
  assert.match(dock, /agir\('pedir_ligar_sessao', \{ pedido \}\)/);
  assert.match(dock, /case 'outro':\s*await escolherTimer\(pedidoDeLigar\(d\)\);/);
  assert.match(dock, /case 'novo-timer':[\s\S]*?await novoTimerDaSessao\(d\.sessao\)/);
  assert.match(dock, /const novoTimerDaSessao = \(sessaoId\) => agir\('pedir_novo_timer'/);
  // a janela principal atende o pedido e devolve a janela ao estado de antes
  const painel = fonte('painel.js');
  assert.match(painel, /listen\('ligar-sessao', verLigarSessao\)/);
  // revisão: cada pedido avisa o núcleo uma vez, quando o diálogo dele acaba (sem prazo)
  assert.match(painel, /ligarATimer\(lerSnap, p\.sessao_id, aoNovo, reserva\);[\s\S]*?quandoSemDialogo\(fimDoDialogoDoDock\);/);
  assert.match(painel, /const fimDoDialogoDoDock = \(\) =>\s*chamar\('dialogo_do_dock_fechou'\)/);
  assert.match(painel, /if \(pedido\) abrirNovoTimerDaSessao\(pedido, fimDoDialogoDoDock\);/);
  assert.match(painel, /if \(pedidosAbertos\.has\(pedido\.sessao_id\)\) return depois\?\.\(\);/, 'o repetido também avisa');
  assert.doesNotMatch(fonte('editar.js'), /FOLGA_SEM_DIALOGO|800/);
});

test('rodada 16: o pedido leva a sessão, a pergunta e o que mostrar fora das recentes', () => {
  // o "Outro timer…" da pergunta: com o "+ Novo timer…"
  assert.deepEqual(pedidoDeLigar({ sessao: 's1', pergunta: '7' }), {
    sessao_id: 's1', pergunta_id: 7, nome: null, cwd: null, timer_id: null, com_novo: true,
  });
  // o "Trocar" do aviso de ligada sozinha: a sessão já tem timer, sem "+ Novo timer…"
  const aviso = { sessao_id: 's2', sessao_nome: 'Flow: spec', cwd: 'G:/Flow', timer_id: 4 };
  assert.deepEqual(pedidoDeLigar({ sessao: 's2' }, aviso), {
    sessao_id: 's2', pergunta_id: null, nome: 'Flow: spec', cwd: 'G:/Flow', timer_id: 4, com_novo: false,
  });
});

test('rodada 16: os passos do tour, na ordem', async () => {
  const { PASSOS, htmlDoBalao } = await import('./tour.js');
  assert.deepEqual(PASSOS.map((p) => p.id), ['cartao', 'etiquetas', 'hoje', 'perguntas', 'painel', 'dock', 'novo', 'zerar', 'ajustes']);
  assert.equal(PASSOS.find((p) => p.dock).id, 'dock', 'o dock pisca só no passo dele');
  for (const p of PASSOS) assert.ok(p.texto.split(/[.!?](\s|$)/).filter((x) => x && x.trim()).length <= 3, p.id);
  const primeiro = htmlDoBalao(0);
  assert.match(primeiro, /Passo 1 de 9/);
  assert.match(primeiro, /<h3 id="tour-t">O cartão do timer<\/h3>/);
  assert.match(primeiro, /data-tour="voltar" disabled>Voltar/);
  assert.match(primeiro, /data-tour="proximo">Próximo/);
  assert.match(primeiro, /data-tour="pular">Pular tour/);
  assert.match(primeiro, /data-tour="fechar"[^>]*aria-label="Fechar o tour">×/);
  assert.match(htmlDoBalao(8), /data-tour="proximo">Concluir/);
  assert.doesNotMatch(htmlDoBalao(3), /disabled/);
});

test('rodada 16: o balão escolhe o lado com espaço e, sem alvo ou sem lado, fica no centro', async () => {
  const { ladoDoBalao } = await import('./tour.js');
  const janela = { w: 1000, h: 700 };
  const balao = { w: 300, h: 150 };
  const r = (left, top, right, bottom) => ({ left, top, right, bottom });
  // embaixo do alvo, centralizado nele
  assert.deepEqual(ladoDoBalao(r(400, 50, 600, 90), balao, janela), { lado: 'abaixo', x: 350, y: 102 });
  // o alvo embaixo da janela: acima
  assert.deepEqual(ladoDoBalao(r(400, 600, 600, 640), balao, janela), { lado: 'acima', x: 350, y: 438 });
  // alvo alto encostado à esquerda: à direita
  assert.deepEqual(ladoDoBalao(r(20, 20, 200, 680), balao, janela), { lado: 'direita', x: 212, y: 275 });
  // alvo alto encostado à direita: à esquerda
  assert.deepEqual(ladoDoBalao(r(800, 20, 980, 680), balao, janela), { lado: 'esquerda', x: 488, y: 275 });
  // o alvo ocupa a janela toda: não cabe em lado nenhum, centro
  assert.deepEqual(ladoDoBalao(r(10, 10, 990, 690), balao, janela), { lado: 'centro', x: 350, y: 275 });
  // sem alvo (sem timer, sem pergunta, painel fechado, o passo do dock): centro
  assert.deepEqual(ladoDoBalao(null, balao, janela), { lado: 'centro', x: 350, y: 275 });
  // nas bordas, o balão não sai da janela
  assert.deepEqual(ladoDoBalao(r(0, 40, 30, 70), balao, janela), { lado: 'abaixo', x: 12, y: 82 });
});

test('rodada 16: Esc, Pular, o X e o Concluir gravam tour_visto; as setas andam', async () => {
  const { criarTour, PASSOS } = await import('./tour.js');
  const novo = () => {
    const r = { desenhos: [], gravou: 0, pisca: [] };
    r.t = criarTour({ desenhar: (i) => r.desenhos.push(i), gravar: () => r.gravou++, piscarDock: (s) => r.pisca.push(s) });
    r.t.ir(0);
    return r;
  };
  let a = novo();
  assert.equal(a.t.tecla('ArrowRight'), true);
  assert.equal(a.t.passo, 1);
  a.t.tecla('ArrowLeft');
  a.t.tecla('ArrowLeft');
  assert.equal(a.t.passo, 0, 'não passa do primeiro');
  assert.equal(a.t.tecla('a'), false);
  a.t.tecla('Escape');
  assert.equal(a.gravou, 1);
  assert.equal(a.t.aberto, false);
  assert.equal(a.desenhos.at(-1), null, 'some');
  a.t.tecla('Escape');
  a.t.proximo();
  assert.equal(a.gravou, 1, 'fechado não volta');
  // pular (e o X: o mesmo fechar)
  a = novo();
  a.t.proximo();
  a.t.fechar();
  assert.equal(a.gravou, 1);
  // até o fim: o Enter no último passo conclui; o dock pisca só no passo dele
  a = novo();
  for (let k = 0; k < PASSOS.length - 1; k++) a.t.tecla('Enter');
  assert.equal(a.t.passo, PASSOS.length - 1);
  assert.equal(a.gravou, 0);
  a.t.tecla('Enter');
  assert.equal(a.gravou, 1);
  assert.deepEqual(a.pisca, [true, false]);
});

test('rodada 16: a faixa "Novo: um tour…" só para quem já usa e nunca viu', async () => {
  const { mostrarFaixaTour } = await import('./tour.js');
  assert.equal(mostrarFaixaTour({ tour_visto: false }, true), true);
  assert.equal(mostrarFaixaTour({}, true), true, 'config de antes');
  assert.equal(mostrarFaixaTour({ tour_visto: false }, false), false, 'sem timers: o primeiro uso oferece');
  assert.equal(mostrarFaixaTour({ tour_visto: true }, true), false, 'respondida, não volta');
  const painel = readFileSync(new URL('./painel.js', import.meta.url), 'utf8');
  assert.match(painel, /\{ once: true \}/);
  assert.match(painel, /chamar\('marcar_tour_visto'\)/);
});

/**
 * Um núcleo de mentira com a regra de `nucleo::dicas::Dicas`: uma dica à vista no app inteiro,
 * nenhuma com a principal ocupada, e a vista só quando sai pela pessoa ou pelo prazo.
 */
function nucleoDeDicas() {
  const n = { ligadas: true, vistas: [], mostrando: null, ocupada: false };
  n.pedir = async (id) => {
    if (!n.ligadas || n.vistas.includes(id)) return 'nao';
    if (n.ocupada || n.mostrando) return 'esperar';
    n.mostrando = id;
    return 'mostrar';
  };
  n.saiu = async (id, vista) => {
    if (n.mostrando === id) n.mostrando = null;
    if (vista && !n.vistas.includes(id)) n.vistas.push(id);
  };
  return n;
}

/** Uma janela com a fila de dicas; `fechar(r)` tira a dica à vista dela. */
function janelaDeDicas(n, podeMostrar = () => true) {
  const j = { mostradas: [], fechar: null };
  j.fila = criarFila({
    ligadas: () => n.ligadas,
    vistas: () => n.vistas,
    podeMostrar,
    reivindicar: n.pedir,
    mostrar: (id) =>
      new Promise((ok) => {
        j.mostradas.push(id);
        j.fechar = (r) => {
          j.fechar = null;
          ok(r);
        };
      }),
    concluir: n.saiu,
  });
  return j;
}

const tique = async () => {
  for (let k = 0; k < 5; k++) await new Promise((ok) => setTimeout(ok, 0));
};

test('rodada 16: cada dica dispara uma vez só, uma por vez, e nunca com o tour ou um diálogo', async () => {
  const n = nucleoDeDicas();
  let livre = false;
  const j = janelaDeDicas(n, () => livre);
  // durante o tour (ou com um diálogo aberto): espera
  j.fila.oferecer('zerar');
  j.fila.oferecer('pergunta');
  j.fila.oferecer('zerar');
  await tique();
  assert.deepEqual(j.mostradas, []);
  assert.deepEqual(j.fila.fila, ['zerar', 'pergunta'], 'a repetida não entra duas vezes');
  // livre: uma por vez, na ordem
  livre = true;
  j.fila.tentar();
  await tique();
  assert.deepEqual(j.mostradas, ['zerar']);
  j.fila.tentar();
  await tique();
  assert.deepEqual(j.mostradas, ['zerar'], 'a segunda espera a vez');
  j.fechar('entendi');
  await tique();
  assert.deepEqual(j.mostradas, ['zerar', 'pergunta']);
  j.fechar('venceu');
  await tique();
  assert.deepEqual(n.vistas, ['zerar', 'pergunta'], 'o prazo também conta como vista');
  // vista não volta
  j.fila.oferecer('zerar');
  await tique();
  assert.deepEqual(j.mostradas, ['zerar', 'pergunta']);
  // desligadas: nada aparece
  n.ligadas = false;
  j.fila.oferecer('esperando');
  await tique();
  assert.deepEqual(j.mostradas, ['zerar', 'pergunta']);
  assert.deepEqual(j.fila.fila, []);
  // "Mostrar as dicas de novo" (e ligadas): todas voltam, na hora
  n.ligadas = true;
  n.vistas = [];
  j.fila.oferecer('zerar');
  await tique();
  assert.deepEqual(j.mostradas, ['zerar', 'pergunta', 'zerar']);
  j.fechar('entendi');
  await tique();
  // uma dica que não existe não entra
  j.fila.oferecer('nada');
  assert.deepEqual(j.fila.fila, []);
  assert.equal(Object.keys(DICAS).length, 7);
});

test('revisão da rodada 16: uma dica por vez no app inteiro; a interrompida volta sem ficar vista', async () => {
  const n = nucleoDeDicas();
  const principal = janelaDeDicas(n);
  const dock = janelaDeDicas(n);
  principal.fila.oferecer('zerar');
  await tique();
  dock.fila.oferecer('claude_contando');
  await tique();
  assert.deepEqual(principal.mostradas, ['zerar']);
  assert.deepEqual(dock.mostradas, [], 'o dock espera a dica da principal sair');
  assert.deepEqual(dock.fila.fila, ['claude_contando']);
  principal.fechar('entendi');
  await tique();
  dock.fila.tentar();
  await tique();
  assert.deepEqual(dock.mostradas, ['claude_contando']);
  // o tour começa na principal: o núcleo segura as dicas e a do dock fecha no meio
  n.ocupada = true;
  dock.fechar('interrompida');
  await tique();
  assert.ok(!n.vistas.includes('claude_contando'), 'a interrompida não fica vista');
  assert.deepEqual(dock.fila.fila, ['claude_contando'], 'e volta para a fila');
  dock.fila.tentar();
  principal.fila.oferecer('esperando');
  await tique();
  assert.deepEqual(dock.mostradas, ['claude_contando'], 'nada durante o tour');
  assert.deepEqual(principal.mostradas, ['zerar']);
  // o tour acabou: a interrompida aparece de novo
  n.ocupada = false;
  dock.fila.tentar();
  await tique();
  assert.deepEqual(dock.mostradas, ['claude_contando', 'claude_contando']);
});

test('revisão da rodada 16: "Mostrar as dicas de novo" vale na hora, sem recomeçar a página', () => {
  // as duas janelas oferecem as dicas a cada estado (sem um "já oferecida" por página)
  const painel = fonte('painel.js');
  const dock = fonte('dock.js');
  assert.doesNotMatch(painel, /dicasOferecidas/);
  assert.doesNotMatch(dock, /const oferecidas/);
  assert.match(painel, /for \(const id of gatilhos\(s\)\) if \(!DICAS\[id\]\.soNoDock\) dicas\.oferecer\(id\);/);
});

test('rodada 16: os gatilhos das dicas pelo estado', async () => {
  const { gatilhos, DICAS } = await import('./dicas.js');
  const snap = (timers, perguntas = []) => ({ grupos: [{ timers }], perguntas, config: { inatividade_s: 600 } });
  assert.deepEqual(gatilhos(snap([{ id: 1, situacao: 'trabalhando' }])), ['claude_contando']);
  assert.deepEqual(gatilhos(snap([{ id: 1, situacao: 'esperando' }])), ['esperando']);
  assert.deepEqual(gatilhos(snap([{ id: 1, situacao: 'manual', situacao_claude: 'esperando' }])), ['manual_mais']);
  assert.deepEqual(gatilhos(snap([{ id: 1, situacao: 'manual' }])), [], 'só o manual não é "manual + …"');
  assert.deepEqual(gatilhos(snap([], [{ tipo: 'conversa_nova' }])), ['pergunta']);
  assert.deepEqual(gatilhos(snap([], [{ tipo: 'sessao_nova' }, { tipo: 'volta_inatividade' }])), ['pergunta', 'inatividade']);
  assert.equal(DICAS.claude_contando.texto(), 'Este timer anda sozinho quando o Claude Code trabalha numa pasta ligada a ele.');
  assert.equal(DICAS.zerar.texto(), 'Zerar não apaga nada: o histórico do dia continua.');
  assert.match(DICAS.inatividade.texto(snap([])), /Depois de 10 min sem mexer no PC/);
  assert.equal(DICAS.claude_contando.alvo(snap([{ id: 4, situacao: 'trabalhando' }])), '#grupos .k[data-timer="4"]');
  assert.equal(DICAS.dock_expande.soNoDock, true);
});

test('rodada 16: a dica no aviso do dock tem "Entendi" e "Não mostrar mais dicas"; Ajustes → Ajuda', async () => {
  const { htmlDoAviso } = await import('./formato.js');
  const h = htmlDoAviso({ texto: 'Arraste o dock.', botao: 'Entendi', botao2: 'Não mostrar mais dicas' });
  assert.match(h, /<div class="faixa dupla"[\s\S]*data-qual="2">Não mostrar mais dicas<\/button><button type="button" data-qual="1">Entendi<\/button>/);
  const { htmlDaDica } = await import('./dicas.js');
  assert.match(htmlDaDica('Zerar <não>'), /<p id="dica-t">Zerar &lt;não&gt;<\/p>[\s\S]*data-dica="desligar">Não mostrar mais dicas[\s\S]*data-dica="entendi">Entendi/);
  const { htmlAjuda } = await import('./ajustes.js');
  const a = htmlAjuda({});
  assert.match(a, /name="dicas_ligadas" checked[^>]*><span>Dicas na tela/);
  assert.match(a, /data-aj="dicas-de-novo">Mostrar as dicas de novo/);
  // rodada 17: "Como começar" (o essencial) e "Tour completo"
  assert.match(a, /data-aj="ver-tour">Como começar/);
  assert.match(a, /data-aj="tour-completo">Tour completo/);
  assert.doesNotMatch(htmlAjuda({ dicas_ligadas: false }), /name="dicas_ligadas" checked/);
});

test('revisão da rodada 16: a dica à vista fecha com o tour, um diálogo ou outra tela, e o núcleo sabe', () => {
  const painel = fonte('painel.js');
  // o tour e os diálogos entram no body: a janela avisa o núcleo e fecha a dica (sem contar como vista)
  assert.match(painel, /document\.querySelector\('body > \.dlg-fundo, body > \.tour'\)/);
  assert.match(painel, /if \(ocupada\) fecharDicaAVista\(\);/);
  assert.match(painel, /chamar\('principal_ocupada', \{ ocupada \}\)/);
  assert.match(painel, /async function trocarVisao\(v\) \{[\s\S]*?fecharDicaAVista\(\);/);
  assert.match(painel, /listen\('dica-fechar'/);
  assert.match(fonte('dock.js'), /listen\('dica-fechar'[\s\S]*?fecharAviso\(dicaAtual\.aviso, 'interrompido'\)/);
});

test('revisão da rodada 16: com timers e os hooks desligados, a faixa "Conectar" com "Agora não" (rodada 18)', async () => {
  const { faixaDosHooks, HOOKS_ANTIGOS } = await import('./ajustes.js');
  const desligado = faixaDosHooks('desligado', true);
  assert.deepEqual(desligado, {
    texto: 'O Claude Code não está conectado: o tempo do Claude não está contando.',
    botao: 'Conectar',
    atualizar: false,
    esconder: 'Agora não',
  });
  // sem timers, quem liga é o primeiro uso
  assert.equal(faixaDosHooks('desligado', false), null);
  // a da versão antiga continua
  assert.deepEqual(faixaDosHooks('antigo', false), { texto: HOOKS_ANTIGOS, botao: 'Atualizar', atualizar: true, esconder: '×' });
  assert.equal(faixaDosHooks('ligado', true), null);
  assert.equal(faixaDosHooks(null, true), null, 'sem leitura dos hooks, nada');
  // "Agora não" esconde até a próxima abertura (só na memória da página) e o "Ligar" liga
  const painel = fonte('painel.js');
  assert.match(painel, /\$\('#bt-hooks-esconder'\)\?\.addEventListener\('click', \(\) => \{\s*faixaHooksEscondida = true;/);
  assert.match(painel, /ligarAoClaude\(\{ atualizar: \$\('#faixa-hooks'\)\.dataset\.atualizar === '1' \}\)/);
  assert.match(painel, /faixaDosHooks\(estadoHooks, Boolean\(snap\) && !semTimers\(snap\), claudeEncontrado\)/);
});

test('revisão da rodada 16: um diálogo por cima pausa o tour, que volta no mesmo passo', async () => {
  const { criarTour } = await import('./tour.js');
  const r = { desenhos: [], gravou: 0, pisca: [] };
  const t = criarTour({ desenhar: (i) => r.desenhos.push(i), gravar: () => r.gravou++, piscarDock: (s) => r.pisca.push(s) });
  t.ir(5);
  assert.deepEqual(r.pisca, [true], 'o passo do dock pisca');
  t.pausar();
  assert.deepEqual(r.pisca, [true, false], 'pausado: o dock para de piscar');
  assert.equal(t.aberto, true);
  assert.equal(r.gravou, 0, 'pausar não fecha');
  t.retomar();
  assert.equal(t.passo, 5, 'o mesmo passo');
  assert.equal(r.desenhos.at(-1), 5);
  assert.deepEqual(r.pisca, [true, false, true]);
  // a tela pausa sozinha com um diálogo no body e solta o teclado
  const tour = fonte('tour.js');
  assert.match(tour, /document\.querySelector\('body > \.dlg-fundo'\)/);
  assert.match(tour, /camada\.hidden = true;\s*document\.removeEventListener\('keydown', teclado, true\);\s*tour\.pausar\(\);/);
  assert.match(tour, /camada\.hidden = false;\s*document\.addEventListener\('keydown', teclado, true\);\s*tour\.retomar\(\);/);
});

test('revisão da rodada 16: o passo do dock com o dock escondido não fala em piscar; o tour começado já conta como visto', async () => {
  const { criarTour, htmlDoBalao, PASSOS } = await import('./tour.js');
  const i = PASSOS.findIndex((p) => p.dock);
  assert.match(htmlDoBalao(i), /piscando agora/);
  const escondido = htmlDoBalao(i, { dockEscondido: true });
  assert.doesNotMatch(escondido, /piscando/);
  assert.match(escondido, /aparece só quando há uma pergunta/);
  assert.match(escondido, /Ajustes → Dock/);
  // escondido: nada pisca
  const pisca = [];
  const t = criarTour({ desenhar: () => {}, gravar: () => {}, piscarDock: (s) => pisca.push(s), semPiscar: true });
  t.ir(i);
  assert.deepEqual(pisca, []);
  // o tour grava tour_visto ao começar (o do primeiro uso interrompido, a janela fechada no meio)
  const tour = fonte('tour.js');
  // (rodada 17: o mini-tour de uma tela não grava; o essencial e o completo, sim)
  assert.match(tour, /export function iniciarTour\(\{[^}]*marcarVisto = true[^}]*\} = \{\}\) \{\s*if \(ativo\) return;[\s\S]*?const gravar = marcarVisto[\s\S]*?\n\s*gravar\(\);/);
  assert.match(tour, /addEventListener\('pagehide', aoSairDaPagina\)/);
  assert.match(fonte('painel.js'), /iniciarTour\(\{ passos, dockEscondido, claudeLigado, marcarVisto: !daTela, aoAcao \}\)/);
});

test('re-revisão da rodada 16: ir para os Ajustes fecha a dica; a página que some ou recomeça solta a vez', () => {
  const painel = fonte('painel.js');
  assert.match(painel, /function abrirAjustes\(secao = null\) \{[\s\S]*?fecharDicaAVista\(\);[\s\S]*?visao = 'ajustes';/);
  // ao começar, o núcleo solta o que era da página e ela informa o estado de novo
  assert.match(painel, /chamar\('dicas_pagina_comecou'\)[\s\S]*?\.finally\(\(\) => conferirOcupada\(true\)\)/);
  // escondida: a dica dela fecha (sem contar como vista); de volta, informa o ocupado
  assert.match(painel, /visibilitychange[\s\S]*?if \(document\.hidden\) fecharDicaAVista\(\);\s*else conferirOcupada\(true\);/);
  assert.match(fonte('dock.js'), /chamar\('dicas_pagina_comecou'\)/);
});
