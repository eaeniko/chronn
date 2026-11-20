// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

// O primeiro uso da rodada 17 (os passos, as pastas por timer, o resumo) mudou na rodada 18:
// os testes dele estão em rodada18.test.js.

const fonte = (nome) => readFileSync(new URL(`./${nome}`, import.meta.url), 'utf8');

test('rodada 17: o perfil de teste tem a marca no dock e o "Apagar o perfil…" só nele', async () => {
  const { marcaDeTeste, PERFIL_TESTE } = await import('./nome.js');
  assert.equal(PERFIL_TESTE, false, 'fora do app: o perfil normal');
  assert.equal(marcaDeTeste(false), '');
  assert.match(marcaDeTeste(true), /<span class="dteste" title="Perfil de teste">teste<\/span>/);
  assert.match(fonte('dock.js'), /<div class="pilula">\$\{marcaDeTeste\(\)\}/);
  assert.match(fonte('dock.js'), /<div class="dx-top">\$\{marcaDeTeste\(\)\}/);
  const { htmlApagarPerfil } = await import('./ajustes.js');
  assert.equal(htmlApagarPerfil(false), '', 'não existe fora do perfil de teste');
  assert.match(htmlApagarPerfil(true), /data-aj="apagar-perfil-teste">Apagar o perfil de teste e recomeçar/);
});

test('rodada 17: o "Iniciar com o Windows" passa pelo núcleo, nunca pelo plugin direto', () => {
  const api = fonte('api.js');
  assert.doesNotMatch(api, /tauri\(\)\.autostart/);
  assert.match(api, /chamar\('inicio_windows', \{ ligar: true \}\)/);
  const caps = readFileSync(new URL('../../src-tauri/capabilities/default.json', import.meta.url), 'utf8');
  assert.doesNotMatch(caps, /autostart/, 'a página não tem permissão para o plugin');
});

test('rodada 17: o tour essencial tem 5 passos e é o padrão; o completo segue com 8', async () => {
  const { PASSOS_ESSENCIAL, PASSOS, passosDoTour, htmlDoBalao } = await import('./tour.js');
  assert.deepEqual(PASSOS_ESSENCIAL.map((p) => p.id), ['cartao', 'claude', 'pastas', 'dock', 'ajuda']);
  assert.equal(PASSOS.length, 8, 'o completo sem mudanças');
  assert.equal(passosDoTour('essencial'), PASSOS_ESSENCIAL);
  assert.equal(passosDoTour(), PASSOS_ESSENCIAL, 'sem tipo, o essencial');
  assert.equal(passosDoTour('completo'), PASSOS);
  assert.equal(PASSOS_ESSENCIAL[3], PASSOS.find((p) => p.id === 'dock'), 'o passo do dock é o mesmo');
  assert.match(htmlDoBalao(4, { passos: PASSOS_ESSENCIAL }), /Passo 5 de 5[\s\S]*Em cada tela, o \? explica o que ela mostra\. O tour completo fica em Ajustes → Ajuda\. Tudo roda neste computador[\s\S]*Concluir/);
  // o passo 3: as pastas, ou "Conectar o Claude Code" com o Claude Code desligado (rodada 18)
  const ligado = htmlDoBalao(2, { passos: PASSOS_ESSENCIAL, claudeLigado: true });
  assert.match(ligado, /<h3 id="tour-t">As pastas<\/h3>/);
  assert.match(ligado, /“Pastas deste timer”/);
  assert.doesNotMatch(ligado, /data-tour="acao"/);
  const semClaude = htmlDoBalao(2, { passos: PASSOS_ESSENCIAL, claudeLigado: false });
  assert.match(semClaude, /O Claude Code ainda não está conectado/);
  assert.match(semClaude, /<button type="button" class="bt" data-tour="acao" data-acao="ligar-claude">Conectar o Claude Code<\/button>/);
  // o tour_visto vale para os dois (o mini-tour de uma tela não grava)
  const painel = fonte('painel.js');
  assert.match(painel, /marcarVisto: !daTela/);
  assert.match(painel, /addEventListener\('pedir-tour', \(ev\) => verTour\(ev\.detail\?\.tipo \?\? 'essencial'\)\)/);
});

test('rodada 17: o menu do ? mostra o nome da tela atual', async () => {
  const { itensDoMenuAjuda, htmlDoMenuAjuda } = await import('./tour.js');
  assert.deepEqual(itensDoMenuAjuda('painel').map((i) => i.rotulo), ['Como começar', 'Esta tela: Painel', 'Tour completo']);
  assert.equal(itensDoMenuAjuda('foco')[1].rotulo, 'Esta tela: Foco');
  assert.equal(itensDoMenuAjuda('cartoes')[1].rotulo, 'Esta tela: Cartões');
  assert.equal(itensDoMenuAjuda('linha')[1].rotulo, 'Esta tela: Linha do tempo');
  assert.equal(itensDoMenuAjuda('ajustes')[1].rotulo, 'Esta tela: Ajustes');
  const h = htmlDoMenuAjuda('linha');
  assert.match(h, /role="menu"/);
  assert.match(h, /data-ajuda="essencial">Como começar<\/button><button type="button" role="menuitem" data-ajuda="tela">Esta tela: Linha do tempo<\/button><button type="button" role="menuitem" data-ajuda="completo">Tour completo/);
});

test('rodada 17: cada mini-tour tem os passos certos, de 2 a 4', async () => {
  const { MINI_TOURS, passosDoTour } = await import('./tour.js');
  const ids = Object.fromEntries(Object.entries(MINI_TOURS).map(([t, ps]) => [t, ps.map((p) => p.id)]));
  assert.deepEqual(ids, {
    painel: ['grupos', 'pausar-tudo', 'painel'],
    foco: ['relogio', 'lista'],
    cartoes: ['cartoes', 'zoho'],
    linha: ['trechos', 'cores', 'ajuste'],
    ajustes: ['dock', 'claude', 'grupos', 'ajuda'],
  });
  for (const ps of Object.values(MINI_TOURS)) {
    assert.ok(ps.length >= 2 && ps.length <= 4);
    for (const p of ps) assert.ok(p.alvo && p.titulo && p.texto, p.id);
  }
  assert.equal(passosDoTour('tela', 'cartoes'), MINI_TOURS.cartoes);
  assert.equal(passosDoTour('tela', 'nada'), MINI_TOURS.painel);
  // os Ajustes: cada seção em uma frase (inatividade, dock, lembretes, Claude Code, grupos e regras, backups, ajuda)
  const textos = MINI_TOURS.ajustes.map((p) => p.texto).join(' ');
  for (const s of ['Inatividade', 'Dock', 'Lembretes', 'Claude Code', 'Grupos e regras', 'Backups', 'dicas']) assert.ok(textos.includes(s), s);
});

test('rodada 17: os alvos que faltam ficam centralizados', async () => {
  const { ladoDoBalao, MINI_TOURS } = await import('./tour.js');
  // o ± só existe com um ajuste: sem ele na tela, o balão vai para o centro
  assert.equal(MINI_TOURS.linha[2].alvo, '#v-linha .mao');
  assert.deepEqual(ladoDoBalao(null, { w: 300, h: 150 }, { w: 1000, h: 700 }), { lado: 'centro', x: 350, y: 275 });
  // o tour procura o alvo na tela e, sem ele (ou escondido), passa nulo
  const tour = fonte('tour.js');
  assert.match(tour, /function alvoNaTela\(passo\) \{\s*if \(!passo\.alvo\) return null;\s*const el = document\.querySelector\(passo\.alvo\);\s*if \(!el \|\| el\.closest\('\[hidden\]'\)/);
});

test('revisão da rodada 17: o passo das pastas relê o Claude Code e se redesenha depois do "Ligar"', () => {
  const tour = fonte('tour.js');
  assert.match(tour, /const ligado = typeof claudeLigado === 'function' \? claudeLigado\(\) : claudeLigado;/);
  assert.match(tour, /export const redesenharTour = \(\) => redesenhar\(\);/);
  assert.match(tour, /redesenhar = \(\) => \{\s*if \(!pausado && tour\.aberto\) tour\.ir\(tour\.passo\);/);
  const painel = fonte('painel.js');
  assert.match(painel, /const claudeLigado = \(\) => estadoHooks === null \|\| estadoHooks === 'ligado';/);
  assert.match(painel, /ligarAoClaude\(\)\.then\(verHooks\)\.then\(redesenharTour\)/);
});

test('revisão da rodada 17: o foco do "Pronto" começa em "Fazer o tour (1 min)"', () => {
  const pu = fonte('primeiro-uso.js');
  assert.match(pu, /enviar: 'Fazer o tour \(1 min\)',[\s\S]*?aoMontar\(form\) \{\s*form\.querySelector\('\[type="submit"\]'\)\?\.setAttribute\('autofocus', ''\);/);
  // o diálogo foca o [autofocus] depois do aoMontar
  assert.match(fonte('editar.js'), /aoMontar\?\.\(form\);\s*const primeiro = form\.querySelector\('\[autofocus\]'\)/);
});
