// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 25: atualizar pelo app. O ícone no cabeçalho só com versão nova, o diálogo (as notas e o
// progresso), os estados do "Verificar atualizações", a busca automática nos Ajustes → Sobre e a
// versão com o canal ("0.4.0 · alpha").
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import {
  estadoDoIcone,
  textoDaVersao,
  corpoAtualizar,
  progresso,
  linhaDaVerificacao,
  htmlAtualizacao,
  textoDaUltima,
  dataHoraLocal,
} from './atualizacao.js';
import { DICIONARIOS, definirIdioma } from './textos.js';

const fonte = (nome) => readFileSync(new URL(`./${nome}`, import.meta.url), 'utf8');

/** Roda `f` com o idioma `id` e volta ao português (os outros testes contam com ele). */
function em(id, f) {
  definirIdioma(id);
  try {
    return f();
  } finally {
    definirIdioma('pt-BR');
  }
}

const NOVA = { versao: '0.4.1', notas: '- Correções.\n- <b>Nada</b> de HTML.' };

test('rodada 25: o ícone do cabeçalho só aparece com versão nova, com a dica da versão', () => {
  assert.deepEqual(estadoDoIcone(null), { visivel: false, dica: '' });
  assert.deepEqual(estadoDoIcone(undefined), { visivel: false, dica: '' });
  assert.deepEqual(estadoDoIcone({ versao: '' }), { visivel: false, dica: '' });
  assert.deepEqual(estadoDoIcone(NOVA), { visivel: true, dica: 'Atualizar para 0.4.1' });
  em('en', () => assert.equal(estadoDoIcone(NOVA).dica, 'Update to 0.4.1'));
  // no HTML, escondido até o núcleo achar a versão; junto do ? e do ⚙, antes deles
  const html = fonte('../index.html');
  assert.match(html, /<div class="l1-ajuda">\s*<!--[^>]*-->\s*<button class="ic atu" id="bt-atualizar" data-acao="atualizar" hidden>/);
  // a página pergunta o que a busca desta abertura achou e ouve as buscas seguintes
  const p = fonte('painel.js');
  assert.match(p, /chamar\('atualizacao_estado'\)\.then\(mostrarAtualizacao,/);
  assert.match(p, /ouvir\('atualizacao', mostrarAtualizacao\)/);
  assert.match(p, /case 'atualizar':\s*abrirAtualizar\(novidade\);/);
  assert.match(p, /b\.hidden = !visivel;\s*b\.title = dica;\s*b\.setAttribute\('aria-label', dica\);/);
  // a troca de idioma refaz a dica
  assert.match(p, /mostrarFaixaHooks\(\);\s*mostrarAtualizacao\(novidade\);\s*\}/);
  // o ícone na cor de destaque
  const css = fonte('../css/app.css');
  assert.match(css, /\.ic\.atu \{ position: relative; color: var\(--acento\); \}/);
  // o `.ic` tem display próprio: sem esta regra, o `hidden` não esconderia o ícone
  assert.match(css, /\.ic\.atu\[hidden\] \{ display: none; \}/);
});

test('rodada 25: o diálogo de atualizar: as notas (sem HTML), os botões e o progresso', () => {
  const c = corpoAtualizar(NOVA);
  assert.match(c, /A versão nova é baixada e instalada, e o app abre de novo sozinho\./);
  assert.match(c, /<b>O que mudou<\/b><div class="atu-notas-tx">- Correções\.\n- &lt;b&gt;Nada&lt;\/b&gt; de HTML\.<\/div>/);
  assert.match(c, /<div class="atu-prog" role="status" hidden>/);
  assert.doesNotMatch(corpoAtualizar({ versao: '0.4.1', notas: '  ' }), /atu-notas/, 'sem notas, sem o bloco');
  const a = fonte('atualizacao.js');
  assert.match(a, /enviar: TEXTOS\.atualizarEReiniciar,\s*cancelar: TEXTOS\.depois,/);
  assert.match(a, /await chamar\('atualizacao_instalar'\);/);
  // erro (rede, assinatura): a mensagem no diálogo, e o app segue como está
  assert.match(a, /throw TEXTOS\.naoDeuAtualizar\(textoDoErro\(e\)\);/);
  assert.equal(DICIONARIOS['pt-BR'].naoDeuAtualizar('sem rede'), 'Não foi possível atualizar: sem rede. O app segue como está.');
  em('en', () => {
    assert.match(corpoAtualizar(NOVA), /<b>What’s new<\/b>/);
    assert.equal(DICIONARIOS.en.atualizarEReiniciar, 'Update and restart');
    assert.equal(DICIONARIOS.en.depois, 'Later');
  });
});

test('rodada 25: o progresso do download: a porcentagem, o que já veio e o instalando', () => {
  assert.deepEqual(progresso(0, 1000), { pct: 0, texto: 'Baixando… 0%' });
  assert.deepEqual(progresso(455, 1000), { pct: 45, texto: 'Baixando… 45%' });
  assert.deepEqual(progresso(999, 1000), { pct: 99, texto: 'Baixando… 99%' });
  assert.deepEqual(progresso(1000, 1000), { pct: 100, texto: 'Instalando… o app abre de novo sozinho.' });
  assert.deepEqual(progresso(0, null), { pct: null, texto: 'Baixando…' });
  assert.deepEqual(progresso(3 * 1024 * 1024, null), { pct: null, texto: 'Baixando… 3,0 MB' });
  em('en', () => assert.equal(progresso(455, 1000).texto, 'Downloading… 45%'));
});

test('rodada 25: os estados do "Verificar atualizações", nos dois idiomas', () => {
  assert.equal(linhaDaVerificacao(null), '<span class="aj-atu-res"></span>');
  assert.match(linhaDaVerificacao('procurando'), />Procurando…</);
  assert.match(linhaDaVerificacao('recente'), /class="aj-atu-res ok">Você está na versão mais recente</);
  assert.match(linhaDaVerificacao('disponivel', NOVA), /Versão 0\.4\.1 disponível · <button type="button" class="aj-link" data-aj="atu-instalar">Atualizar<\/button>/);
  assert.match(linhaDaVerificacao('erro'), /Não foi possível verificar \(sem internet\?\)/);
  em('en', () => {
    assert.match(linhaDaVerificacao('procurando'), />Checking…</);
    assert.match(linhaDaVerificacao('recente'), />You’re on the latest version</);
    assert.match(linhaDaVerificacao('disponivel', NOVA), /Version 0\.4\.1 available · <button[^>]*>Update<\/button>/);
    assert.match(linhaDaVerificacao('erro'), /Couldn’t check \(no internet\?\)/);
  });
  // os Ajustes: o botão procura e a linha mostra o resultado; o "Atualizar" abre o mesmo diálogo
  const a = fonte('ajustes.js');
  assert.match(a, /Object\.assign\(verificacao, \{ estado: 'procurando', novidade: null \}\);/);
  assert.match(a, /const n = await chamar\('atualizacao_procurar'\);\s*Object\.assign\(verificacao, \{ estado: n \? 'disponivel' : 'recente', novidade: n \?\? null \}\);/);
  assert.match(a, /Object\.assign\(verificacao, \{ estado: 'erro', novidade: null \}\);/);
  assert.match(a, /case 'atu-instalar':\s*abrirAtualizar\(verificacao\.novidade\);/);
});

test('rodada 25: Ajustes → Sobre: a versão com o canal, a busca automática ligada e a última busca', () => {
  assert.equal(textoDaVersao('0.4.0', 'alpha'), '0.4.0 · alpha');
  assert.equal(textoDaVersao('0.4.0', ''), '0.4.0', 'sem o canal, só o número');
  // o canal vem do núcleo (`nucleo::CANAL`, no `ajustes_info`), não de um texto da página
  const a = fonte('ajustes.js');
  assert.match(a, /textoDaVersao\(dados\.info\?\.versao \?\? '\?', dados\.info\?\.canal\)/);
  assert.match(a, /htmlAtualizacao\(\{ ligada: c\.atualizar_auto !== false, ultima: c\.atualizacao_ultima \?\? null,/);
  assert.match(a, /nome === 'painel_linha' \|\| nome === 'atualizar_auto'\) \{/);
  assert.match(a, /if \(auto && auto !== document\.activeElement\) auto\.checked = c\.atualizar_auto !== false;/);
  const h = htmlAtualizacao({ ligada: true, ultima: null, app: 'Chronn' });
  assert.match(h, /<button type="button" class="bt" data-aj="atu-verificar">Verificar atualizações<\/button>/);
  assert.match(h, /<input type="checkbox" name="atualizar_auto" checked><span>Procurar atualizações automaticamente<small>Uma vez por dia, ao abrir o app\. Procurar e baixar atualizações é o único uso da internet do Chronn\.<\/small>/);
  assert.match(h, /data-atu="ultima">Ainda não houve verificação</);
  assert.doesNotMatch(htmlAtualizacao({ ligada: false, ultima: null }), /name="atualizar_auto" checked/);
  assert.match(htmlAtualizacao({ ligada: true, estado: 'procurando' }), /data-aj="atu-verificar" disabled>/);
  // a data da última busca, no formato das datas dos backups
  const ms = new Date(2026, 10, 20, 15, 4).getTime();
  assert.equal(dataHoraLocal(ms), '20/11/2026 15:04');
  assert.equal(textoDaUltima(ms), 'Última verificação: 20/11/2026 15:04');
  em('en', () => {
    assert.equal(textoDaUltima(ms), 'Last checked: Nov 20, 2026 15:04');
    const e = htmlAtualizacao({ ligada: true, ultima: null, app: 'Chronn' });
    assert.match(e, />Check for updates</);
    assert.match(e, /Check for updates automatically<small>Once a day, when the app opens\. Checking for and downloading updates is the only thing Chronn uses the internet for\./);
  });
});

test('rodada 25: as chaves novas existem nos dois idiomas', () => {
  const chaves = [
    'versaoComCanal', 'atualizarPara', 'atualizarTitulo', 'atualizarTexto', 'notasDaVersao', 'atualizarEReiniciar', 'depois',
    'baixando', 'instalando', 'naoDeuAtualizar', 'verificarAtualizacoes', 'atuProcurando', 'atuRecente', 'atuDisponivel',
    'atuAtualizar', 'atuErro', 'atualizarAuto', 'atualizarAutoDica', 'atuUltima', 'atuNunca',
  ];
  for (const k of chaves) assert.ok(DICIONARIOS['pt-BR'][k] !== undefined && DICIONARIOS.en[k] !== undefined, k);
});

test('revisão da rodada 25: no perfil de teste, o "Verificar" mostra a versão, e nada instala', () => {
  // a linha do "Verificar": a versão achada, sem o "Atualizar", com a frase
  const t = linhaDaVerificacao('disponivel', NOVA, true);
  assert.equal(t, '<span class="aj-atu-res nova">Versão 0.4.1 disponível · no perfil de teste, a instalação fica desligada</span>');
  assert.doesNotMatch(t, /atu-instalar/);
  em('en', () => assert.match(linhaDaVerificacao('disponivel', NOVA, true), />Version 0\.4\.1 available · installing is turned off in the test profile</));
  // fora do perfil de teste, o "Atualizar" de sempre
  assert.match(linhaDaVerificacao('disponivel', NOVA, false), /data-aj="atu-instalar"/);
  // os outros estados não mudam
  assert.equal(linhaDaVerificacao('recente', null, true), linhaDaVerificacao('recente', null, false));
  // o ícone do cabeçalho (que abre o diálogo de instalar) não aparece
  assert.deepEqual(estadoDoIcone(NOVA, true), { visivel: false, dica: '' });
  assert.equal(estadoDoIcone(NOVA, false).visivel, true);
  // o diálogo também não abre, e o núcleo recusa a instalação
  assert.match(fonte('atualizacao.js'), /export function abrirAtualizar\(novidade, teste = PERFIL_TESTE\) \{\s*if \(!novidade\?\.versao \|\| teste\) return;/);
  const rs = readFileSync(new URL('../../src-tauri/src/atualizar.rs', import.meta.url), 'utf8');
  assert.match(rs, /if crate::perfil::e_teste\(\) \{\s*return Err\(ta\("erro\.atualizar_teste"\)\.into\(\)\);\s*\}/);
  for (const d of [DICIONARIOS['pt-BR'], DICIONARIOS.en]) assert.ok(d.atuSoNoUso);
});
