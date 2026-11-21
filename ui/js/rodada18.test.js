// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 18: o onboarding genérico e local ("N de 6 · …", "Conectar apps") e os projetos
// recentes deste PC no passo das pastas.
import { TEXTOS } from './textos.js';
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const fonte = (nome) => readFileSync(new URL(`./${nome}`, import.meta.url), 'utf8');

test('rodada 18: os seis passos, numerados no título ("1 de 6 · Boas-vindas")', async () => {
  const { PASSOS_DO_PRIMEIRO_USO, tituloDoPasso } = await import('./primeiro-uso.js');
  assert.deepEqual(PASSOS_DO_PRIMEIRO_USO, ['boas-vindas', 'timers', 'claude', 'pastas', 'windows', 'pronto']);
  assert.deepEqual(PASSOS_DO_PRIMEIRO_USO.map(tituloDoPasso), [
    '1 de 6 · Boas-vindas',
    '2 de 6 · Grupo e timers',
    '3 de 6 · Conectar apps',
    '4 de 6 · Pastas dos projetos',
    '5 de 6 · Iniciar com o Windows',
    '6 de 6 · Pronto',
  ]);
  // cada passo usa o título numerado; o "Passo N de 6" solto saiu
  const pu = fonte('primeiro-uso.js');
  for (const id of PASSOS_DO_PRIMEIRO_USO) assert.ok(pu.includes(`titulo: tituloDoPasso('${id}')`), id);
  assert.doesNotMatch(pu, /Passo \$\{|pu-passo/);
});

test('rodada 18: as boas-vindas dizem o que o app é e que tudo roda neste computador', async () => {
  const { TEXTO_BOAS_VINDAS, TEXTO_LOCAL, TEXTO_GRUPO } = await import('./primeiro-uso.js');
  assert.equal(
    TEXTO_BOAS_VINDAS,
    'O Chronn conta o seu tempo por timer: no play, ou sozinho quando um app conectado trabalha numa pasta ligada ao timer.',
  );
  assert.equal(
    TEXTO_LOCAL,
    'Tudo roda neste computador. O Chronn não usa IA, não coleta dados e não envia nada para lugar nenhum. Você decide o que conectar e quais pastas contam. É para o seu controle pessoal.',
  );
  assert.equal(TEXTO_GRUPO, 'Um grupo junta timers, como um cliente ou uma área. Cada timer é um projeto ou atividade.');
  const pu = fonte('primeiro-uso.js');
  // (rodada 23: os textos vêm do textos.js; a quebra de linha do placeholder vira &#10;)
  assert.equal(TEXTOS.puGrupoPlaceholder, 'Ex.: Cliente A');
  assert.match(pu, /placeholder="\$\{esc\(TEXTOS\.puGrupoPlaceholder\)\}"/);
  assert.equal(TEXTOS.puTimersPlaceholder.replace(/\n/g, '&#10;'), 'Um por linha. Ex.:&#10;Projeto X&#10;Reuniões&#10;Estudos');
  assert.match(pu, /placeholder="\$\{esc\(TEXTOS\.puTimersPlaceholder\)\.replace\(\/\\n\/g, '&#10;'\)\}"/);
});

test('rodada 18: "Conectar apps" mostra o cartão do Claude Code nos estados', async () => {
  const { TEXTO_CONECTAR, corpoConectarApps } = await import('./primeiro-uso.js');
  assert.equal(TEXTO_CONECTAR, 'Conecte os apps que você usa e o Chronn conta sozinho enquanto eles trabalham. Por enquanto: Claude Code.');
  const s = 'C:\\Users\\usuario\\.claude\\settings.json';
  // desligado: o cartão (sem o botão dele), o caminho abreviado e a prévia exata recolhida
  const d = corpoConectarApps({ estado: 'desligado', settings: s, previa: '{ "x": "C:\\\\Users\\\\usuario" }' }, false);
  assert.match(d, /Conecte os apps que você usa/);
  assert.match(d, /Não conectado/);
  assert.match(d, /~\\\.claude\\settings\.json/);
  assert.match(d, /<details class="mais aj-ver-muda"><summary>Ver o que muda/);
  assert.doesNotMatch(d, /<details[^>]* open/);
  assert.doesNotMatch(d, /data-hooks=|data-conectar/, 'o botão é o do passo');
  assert.doesNotMatch(d, /Perfil de teste/);
  // conectado: o cartão diz, sem prévia
  const l = corpoConectarApps({ estado: 'ligado', settings: s }, false);
  assert.match(l, /Conectado/);
  assert.doesNotMatch(l, /Ver o que muda/);
  // sem o Claude Code neste PC: o cartão diz, sem prévia
  const n = corpoConectarApps({ estado: 'desligado', encontrado: false, settings: s }, false);
  assert.match(n, /Claude Code não encontrado neste PC/);
  assert.match(n, /Instale o Claude Code e volte aqui/);
  assert.doesNotMatch(n, /Ver o que muda/);
  // perfil de teste: o aviso
  assert.match(corpoConectarApps({ estado: 'desligado', settings: s }, true), /Perfil de teste: o Claude Code real não é alterado/);
  // o botão do passo: Conectar / Atualizar, ou só Continuar (conectado ou não encontrado)
  const pu = fonte('primeiro-uso.js');
  assert.match(pu, /enviar: conectar \? \(estado === 'antigo' \? TEXTOS\.atualizar : TEXTOS\.conectar\) : TEXTOS\.continuar,\s*cancelar: conectar \? TEXTOS\.pular : null,/);
  assert.deepEqual([TEXTOS.atualizar, TEXTOS.conectar, TEXTOS.continuar, TEXTOS.pular], ['Atualizar', 'Conectar', 'Continuar', 'Pular']);
  assert.match(pu, /const conectar = encontrado && estado !== 'ligado';/);
  assert.match(pu, /chamar\('conexao_claude'\)/);
});

test('rodada 18: o resumo do "Pronto" diz "Claude Code conectado"', async () => {
  const { resumoDoPronto } = await import('./primeiro-uso.js');
  assert.equal(resumoDoPronto({ timers: 2, claude: true, pastas: 1 }), '2 timers · Claude Code conectado · 1 pasta');
  assert.equal(resumoDoPronto({ timers: 1, claude: true, pastas: 0 }), '1 timer · Claude Code conectado · nenhuma pasta');
  assert.equal(resumoDoPronto({ timers: 3, claude: false, pastas: 2 }), '3 timers · nenhum app conectado · 2 pastas');
  assert.equal(resumoDoPronto({}), 'nenhum timer · nenhum app conectado · nenhuma pasta');
});

test('rodada 18: o passo das pastas aparece com timers criados; a lista, só com o Claude Code conectado', async () => {
  const { temPassoDasPastas, TEXTO_PASTAS } = await import('./primeiro-uso.js');
  assert.equal(temPassoDasPastas({ timers: [{ id: 1, nome: 'Projeto X' }] }), true);
  assert.equal(temPassoDasPastas({ timers: [] }), false);
  assert.equal(temPassoDasPastas({}), false);
  assert.equal(
    TEXTO_PASTAS,
    'Cada app conectado trabalha numa pasta (por exemplo, a do projeto aberto no Claude Code). Ligue a pasta a um timer e o tempo dela conta lá sozinho.',
  );
  assert.match(fonte('primeiro-uso.js'), /async function recentesDoPc\(\) \{\s*if \(!feito\.claude\) return \[\];\s*try \{\s*return await chamar\('projetos_recentes'\);/);
});

test('rodada 18: "Projetos recentes neste PC": o nome curto, o caminho na dica, quando e o seletor', async () => {
  const { corpoDasPastas, textoUsadoHa, TEXTO_NAO_ACHOU } = await import('./primeiro-uso.js');
  const agora = Date.UTC(2026, 10, 19, 12);
  const dia = 24 * 60 * 60 * 1000;
  assert.equal(textoUsadoHa(agora - 2 * dia, agora), 'usado há 2 dias');
  assert.equal(textoUsadoHa(agora - dia, agora), 'usado ontem');
  assert.equal(textoUsadoHa(agora - 1000, agora), 'usado hoje');
  assert.equal(textoUsadoHa(null, agora), 'escolhida agora');
  const timers = [{ id: 1, nome: 'Projeto X' }, { id: 2, nome: 'Reuniões <1>' }];
  const linhas = [
    { pasta: 'D:\\trabalho\\app-web', nome: 'app-web', usado_em: agora - 2 * dia, timer_id: null },
    { pasta: 'D:\\trabalho', nome: 'trabalho', usado_em: agora - 5 * dia, timer_id: 2, conf: { pasta_mae: true } },
  ];
  const h = corpoDasPastas(linhas, timers, '', agora);
  assert.match(h, /<p class="pu-sub">Projetos recentes neste PC<\/p>/);
  assert.match(h, /<span class="pu-recente-nome" title="D:\\trabalho\\app-web">app-web<\/span>/);
  assert.match(h, /usado há 2 dias/);
  // o seletor: — e os timers criados; a escolha marcada
  assert.match(h, /<select data-recente="0" aria-label="Timer de app-web"><option value="" selected>—<\/option><option value="1">Projeto X<\/option><option value="2">Reuniões &lt;1&gt;<\/option><\/select>/);
  assert.match(h, /<select data-recente="1"[^>]*><option value="">—<\/option><option value="1">Projeto X<\/option><option value="2" selected>/);
  // a pasta-mãe avisa (a conferência da rodada 15)
  assert.match(h, /Esta pasta tem vários projetos dentro\. Toda conversa aberta nela vai para Reuniões &lt;1&gt;\./);
  // "Escolher outra pasta…" e o "Não achou?", sempre
  assert.match(h, /data-pu="outra">Escolher outra pasta…/);
  assert.ok(h.endsWith(`<p class="pu-nao-achou">${TEXTO_NAO_ACHOU}</p>`));
  assert.equal(TEXTO_NAO_ACHOU, 'Não achou? Sem problema: na primeira conversa nova, o Chronn pergunta em qual timer contar e pode lembrar a pasta.');
});

test('rodada 18: sem lista, só o texto, "Escolher outra pasta…" e o "Não achou?"', async () => {
  const { corpoDasPastas, TEXTO_PASTAS, TEXTO_NAO_ACHOU } = await import('./primeiro-uso.js');
  const h = corpoDasPastas([], [{ id: 1, nome: 'Projeto X' }]);
  assert.doesNotMatch(h, /Projetos recentes|<select|pu-recente/);
  assert.match(h, new RegExp(TEXTO_PASTAS.replace(/[.()]/g, '\\$&')));
  assert.match(h, /Escolher outra pasta…/);
  assert.match(h, /Não achou\? Sem problema/);
  assert.ok(h.includes(TEXTO_NAO_ACHOU));
  // o erro de "Escolher outra pasta…" (a raiz, a pasta do usuário…) aparece no passo
  assert.match(corpoDasPastas([], [], 'Uma regra não pode ficar na raiz de um drive'), /role="alert">Uma regra não pode ficar na raiz de um drive/);
});

test('rodada 18: as regras são criadas ao continuar, uma por pasta com timer escolhido', async () => {
  const { regrasDasPastas, criarRegrasDasPastas, acrescentarPasta } = await import('./primeiro-uso.js');
  const chamadas = [];
  const falso = async (nome, args) => chamadas.push([nome, args]);
  // pular, ou continuar sem nada escolhido
  assert.equal(await criarRegrasDasPastas([], falso), 0);
  assert.equal(await criarRegrasDasPastas([{ pasta: 'D:\\a', timer_id: null }], falso), 0);
  assert.deepEqual(chamadas, []);
  const linhas = [
    { pasta: 'D:\\trabalho\\app-web', timer_id: 1 },
    { pasta: 'D:\\notas', timer_id: null },
    { pasta: 'C:\\', timer_id: 2, erro: 'Uma regra não pode ficar na raiz de um drive' },
    { pasta: 'D:\\trabalho', timer_id: 2 },
  ];
  assert.deepEqual(regrasDasPastas(linhas), [
    { timer_id: 1, pasta: 'D:\\trabalho\\app-web' },
    { timer_id: 2, pasta: 'D:\\trabalho' },
  ]);
  assert.equal(await criarRegrasDasPastas(linhas, falso), 2);
  assert.deepEqual(chamadas, [
    ['salvar_regra', { pasta: 'D:\\trabalho\\app-web', timer_id: 1, trocar: null }],
    ['salvar_regra', { pasta: 'D:\\trabalho', timer_id: 2, trocar: null }],
  ]);
  // a mesma pasta não vira duas linhas (nem dois timers): "Escolher outra pasta…" com uma da lista
  // devolve a linha que já está lá
  const l = [{ pasta: 'D:\\trabalho\\app-web', nome: 'app-web', timer_id: 1 }];
  assert.equal(acrescentarPasta(l, { pasta: 'd:/trabalho/app-web/' }), 0);
  assert.equal(l.length, 1);
  assert.equal(acrescentarPasta(l, { pasta: 'D:\\notas' }), 1);
  assert.deepEqual(l[1], { pasta: 'D:\\notas', nome: 'notas', usado_em: null, timer_id: null });
  // e, se chegarem duas iguais, uma regra só
  assert.deepEqual(regrasDasPastas([{ pasta: 'D:\\X', timer_id: 1 }, { pasta: 'd:/x', timer_id: 2 }]), [{ timer_id: 1, pasta: 'D:\\X' }]);
  // cada escolha passa pela conferência da rodada 15 (com o timer escolhido)
  const pu = fonte('primeiro-uso.js');
  assert.match(pu, /chamar\('conferir_regra', \{ pasta: linha\.pasta, timer_id: linha\.timer_id \}\)/);
  assert.match(pu, /feito\.pastas = await criarRegrasDasPastas\(linhasRecentes\);/);
});

test('rodada 18: sem o Claude Code neste PC, nem a faixa nem a linha oferecem "Conectar"', async () => {
  const { faixaDosHooks } = await import('./ajustes.js');
  assert.equal(faixaDosHooks('desligado', true, false), null);
  assert.equal(faixaDosHooks('desligado', true, true).botao, 'Conectar');
  assert.equal(faixaDosHooks('antigo', true, false).botao, 'Atualizar', 'hooks antigos: o arquivo existe');
  assert.match(fonte('painel.js'), /const c = await chamar\('conexao_claude'\);\s*claudeEncontrado = c\.encontrado !== false;\s*mostrarFaixaHooks\(c\.estado\);/);
  const { linhaClaudeDesconectado } = await import('./editar.js');
  assert.equal(linhaClaudeDesconectado('ausente'), '');
  assert.match(fonte('editar.js'), /return c\.encontrado === false \? 'ausente' : c\.estado;/);
});

test('rodada 18: o tour fala de apps conectados e de rodar neste computador', async () => {
  const { PASSOS_ESSENCIAL } = await import('./tour.js');
  assert.equal(PASSOS_ESSENCIAL[1].titulo, 'Apps conectados contam sozinhos');
  assert.match(PASSOS_ESSENCIAL[1].texto, /Claude/);
  assert.match(PASSOS_ESSENCIAL[4].texto, /Tudo roda neste computador/);
});
