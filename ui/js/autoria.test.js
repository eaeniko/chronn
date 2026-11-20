// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 18, seção 5: os créditos com link e as marcas de autoria.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const fonte = (nome) => readFileSync(new URL(`./${nome}`, import.meta.url), 'utf8');
const pagina = (nome) => readFileSync(new URL(`../${nome}`, import.meta.url), 'utf8');

test('autoria: o "@eaeniko" e o "Código no GitHub" abrem fora do app, pelo núcleo', async () => {
  const { CREDITOS, htmlCreditos } = await import('./ajustes.js');
  assert.equal(CREDITOS, 'Criado por @eaeniko (Nikolai Cinotti)');
  const h = htmlCreditos('chronn-eaeniko-7f3a9c');
  assert.match(h, /Criado por <button type="button" class="aj-link" data-aj="link" data-link="autor" title="eaeniko\.okin\.club">@eaeniko<\/button> \(Nikolai Cinotti\)/);
  assert.match(h, /data-link="codigo" title="github\.com\/eaeniko\/chronn">Código no GitHub<\/button>/);
  // a página só diz qual link: o endereço é o do núcleo (nunca um <a href> dentro do WebView)
  assert.doesNotMatch(h, /href=/);
  assert.match(fonte('ajustes.js'), /case 'link':\s*chamar\('abrir_link', \{ qual: alvo\.dataset\.link \}\)/);
  // a marca de autoria no Sobre
  assert.match(h, /<p class="aj-assinatura mono">chronn-eaeniko-7f3a9c<\/p>/);
  assert.match(fonte('ajustes.js'), /\$\{htmlCreditos\(dados\.info\?\.assinatura\)\}/);
});

test('autoria: o "Sobre o …" da bandeja abre os Ajustes no Sobre', () => {
  const painel = fonte('painel.js');
  assert.match(painel, /const secaoDoPedido = \(pedido\) => \(pedido === 'sobre' \? 'sobre' : null\);/);
  assert.match(painel, /if \(snap && visao\) abrirAjustes\(secaoDoPedido\(pedido\)\);/);
  assert.match(painel, /montarAjustes\(\$\('#v-ajustes'\), \{ lerSnap, voltar: \(\) => trocarVisao\(visaoAntes\) \}\)\.then\(irPara\);/);
});

test('autoria: as páginas têm author e generator; o console mostra a linha ao abrir', async () => {
  for (const p of ['index.html', 'dock.html', 'aviso.html']) {
    const h = pagina(p);
    assert.match(h, /<meta name="author" content="Nikolai Cinotti \(@eaeniko\)">/, p);
    assert.match(h, /<meta name="generator" content="Chronn">/, p);
  }
  const { linhaDoConsole } = await import('./nome.js');
  assert.equal(linhaDoConsole(), 'Chronn · © 2026 Nikolai Cinotti (@eaeniko) · eaeniko.okin.club');
  assert.match(fonte('painel.js'), /\nconsole\.info\(linhaDoConsole\(\)\);/);
});
