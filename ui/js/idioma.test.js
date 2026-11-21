// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 23 (0.4.0): português (Brasil) e inglês. Os dois dicionários com as mesmas chaves, o
// "Sistema" pelo idioma do Windows, a troca na hora e os formatos nos dois idiomas.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import {
  DICIONARIOS,
  IDIOMAS,
  TEXTOS,
  resolverIdioma,
  definirIdioma,
  idiomaAtual,
  localeAtual,
  aoTrocarIdioma,
  aplicarTextos,
  idiomaInicial,
  idiomaDoEstado,
} from './textos.js';
import { curto, textoDoAjuste, dataDoDia, desdeTexto, tamanho, SITUACAO, etiquetaComposta, tituloDoTrecho } from './formato.js';

const FONTE = readFileSync(new URL('./textos.js', import.meta.url), 'utf8');

/** Roda `f` com o idioma `id` e volta ao português (os outros testes contam com ele). */
function em(id, f) {
  definirIdioma(id);
  try {
    return f();
  } finally {
    definirIdioma('pt-BR');
  }
}

/** Compara a forma de dois valores: mesmas chaves, mesmos tipos, funções com os mesmos parâmetros. */
function mesmaForma(a, b, onde) {
  assert.equal(typeof a, typeof b, `${onde}: tipo`);
  if (typeof a === 'function') {
    assert.equal(a.length, b.length, `${onde}: parâmetros`);
  } else if (a instanceof RegExp) {
    assert.ok(b instanceof RegExp, onde);
  } else if (Array.isArray(a)) {
    assert.ok(Array.isArray(b), `${onde}: lista`);
    assert.equal(a.length, b.length, `${onde}: tamanho`);
    a.forEach((x, i) => {
      // nas listas de opções, o valor (o primeiro item) é o mesmo nos dois idiomas
      if (Array.isArray(x)) assert.equal(x[0], b[i][0], `${onde}[${i}]: valor`);
      mesmaForma(x, b[i], `${onde}[${i}]`);
    });
  } else if (a && typeof a === 'object') {
    assert.deepEqual(Object.keys(a), Object.keys(b), `${onde}: chaves`);
    for (const k of Object.keys(a)) mesmaForma(a[k], b[k], `${onde}.${k}`);
  } else if (typeof a === 'string') {
    assert.ok(a.length > 0 && b.length > 0, `${onde}: vazio`);
  }
}

test('rodada 23: os dois idiomas têm exatamente as mesmas chaves, na mesma ordem, e as funções os mesmos parâmetros', () => {
  const pt = DICIONARIOS['pt-BR'];
  const en = DICIONARIOS.en;
  assert.deepEqual(Object.keys(en), Object.keys(pt));
  for (const k of Object.keys(pt)) mesmaForma(pt[k], en[k], k);
  // as funções aceitam os mesmos parâmetros: chamadas com valores de exemplo, as duas dão texto
  for (const k of Object.keys(pt)) {
    if (typeof pt[k] !== 'function') continue;
    const args = Array.from({ length: pt[k].length }, (_, i) => (i === 0 ? 2 : `x${i}`));
    for (const id of ['pt-BR', 'en']) {
      const r = DICIONARIOS[id][k](...args);
      assert.ok(r === null || typeof r === 'string', `${id}.${k}`);
    }
  }
});

test('rodada 23: nenhuma chave repetida no código dos dicionários (a segunda apagaria a primeira em silêncio)', () => {
  for (const [ini, fim] of [
    ['const PT = {', '// ⟨fim pt⟩'],
    ['const EN = {', '// ⟨fim en⟩'],
  ]) {
    const corpo = FONTE.slice(FONTE.indexOf(ini), FONTE.indexOf(fim));
    const chaves = [...corpo.matchAll(/^ {2}([A-Za-z_][\w]*):/gm)].map((m) => m[1]);
    const vistas = new Set();
    for (const k of chaves) {
      assert.ok(!vistas.has(k), `${ini} repete ${k}`);
      vistas.add(k);
    }
    assert.ok(chaves.length > 100, `${ini}: ${chaves.length} chaves`);
  }
});

test('rodada 23: o nome de cada idioma vem no próprio idioma, e a escolha "Sistema" segue o Windows', () => {
  assert.deepEqual(
    IDIOMAS.map((i) => [i.id, i.nome]),
    [
      ['pt-BR', 'Português (Brasil)'],
      ['en', 'English'],
    ],
  );
  assert.equal(resolverIdioma('sistema', 'pt-BR'), 'pt-BR');
  assert.equal(resolverIdioma('sistema', 'pt-PT'), 'pt-BR');
  assert.equal(resolverIdioma('sistema', 'en-US'), 'en');
  assert.equal(resolverIdioma('sistema', 'es-ES'), 'en', 'outro idioma cai no inglês');
  assert.equal(resolverIdioma(undefined, 'pt'), 'pt-BR');
  // a escolha manda, qualquer que seja o Windows
  assert.equal(resolverIdioma('en', 'pt-BR'), 'en');
  assert.equal(resolverIdioma('pt-BR', 'es-ES'), 'pt-BR');
  // a primeira pintura: o último usado, senão o do Windows
  assert.equal(idiomaInicial('en', 'pt-BR'), 'en');
  assert.equal(idiomaInicial(null, 'pt-PT'), 'pt-BR');
  assert.equal(idiomaInicial('xx', 'de-DE'), 'en');
  // o estado do núcleo diz o idioma que vale (o mesmo das perguntas); sem ele, a config
  assert.equal(idiomaDoEstado({ idioma: 'en', config: { idioma: 'pt-BR' } }, 'pt-BR'), 'en');
  assert.equal(idiomaDoEstado({ config: { idioma: 'sistema' } }, 'es-ES'), 'en');
});

test('rodada 23: trocar o idioma troca o TEXTOS na hora e avisa quem pediu', () => {
  const vistos = [];
  const parar = aoTrocarIdioma((id) => vistos.push(id));
  try {
    assert.equal(idiomaAtual(), 'pt-BR');
    assert.equal(TEXTOS.pausarTudo, 'Pausar tudo');
    assert.equal(definirIdioma('en'), true);
    assert.equal(definirIdioma('en'), false, 'o mesmo idioma não troca');
    assert.equal(TEXTOS.pausarTudo, 'Pause all');
    assert.equal(TEXTOS.rodando(2), '2 running');
    assert.equal(localeAtual(), 'en-US');
    assert.equal(TEXTOS.nTimers(1), '1 timer');
    assert.equal(TEXTOS.nTimers(3), '3 timers');
    // as situações (o Proxy do formato) também
    assert.deepEqual(SITUACAO.esperando, { curta: 'waiting for you', longa: 'waiting for you' });
  } finally {
    definirIdioma('pt-BR');
    parar();
  }
  assert.deepEqual(vistos, ['en', 'pt-BR']);
  assert.equal(TEXTOS.pausarTudo, 'Pausar tudo');
  assert.deepEqual(SITUACAO.esperando, { curta: 'esperando você', longa: 'esperando você' });
});

test('rodada 23: as durações nos dois idiomas ("2h 01min" → "2h 01m")', () => {
  const MIN = 60_000;
  assert.equal(curto(121 * MIN), '2h 01min');
  assert.equal(curto(45 * MIN), '45min');
  assert.equal(textoDoAjuste(15 * MIN), '+15 min');
  assert.equal(textoDoAjuste(-65 * MIN), '−1h 05min');
  assert.equal(tamanho(1.2 * 1024 * 1024), '1,2 MB');
  em('en', () => {
    assert.equal(curto(121 * MIN), '2h 01m');
    assert.equal(curto(45 * MIN), '45m');
    assert.equal(textoDoAjuste(15 * MIN), '+15 min');
    assert.equal(textoDoAjuste(-65 * MIN), '−1h 05m');
    assert.equal(tamanho(1.2 * 1024 * 1024), '1.2 MB');
  });
});

test('rodada 23: as datas e os dias da semana pelo Intl, nos dois idiomas', () => {
  assert.equal(dataDoDia('2026-11-17'), 'Terça, 17 nov');
  assert.equal(dataDoDia('2026-11-21'), 'Sábado, 21 nov');
  assert.equal(dataDoDia('2026-03-01'), 'Domingo, 1 mar');
  em('en', () => {
    assert.equal(dataDoDia('2026-11-17'), 'Tuesday, Nov 17');
    assert.equal(dataDoDia('2026-03-01'), 'Sunday, Mar 1');
  });
  // o "desde …" do número do timer
  const agora = new Date(2026, 10, 19, 10, 0).getTime();
  const ontem = new Date(2026, 10, 18, 14, 20).getTime();
  const antes = new Date(2026, 10, 15, 10, 5).getTime();
  const outroAno = new Date(2025, 10, 15, 10, 5).getTime();
  assert.equal(desdeTexto(ontem, agora), 'desde ontem 14:20');
  assert.equal(desdeTexto(antes, agora), 'desde 15/11 10:05');
  assert.equal(desdeTexto(outroAno, agora), 'desde 15/11/2025 10:05');
  em('en', () => {
    assert.equal(desdeTexto(agora - 60 * 60_000, agora), 'since today 09:00');
    assert.equal(desdeTexto(ontem, agora), 'since yesterday 14:20');
    assert.equal(desdeTexto(antes, agora), 'since Nov 15 10:05');
    assert.equal(desdeTexto(outroAno, agora), 'since Nov 15, 2025 10:05');
  });
});

test('rodada 23: as etiquetas do dock e a dica do trecho nos dois idiomas', () => {
  const t = { manual_atividade_id: 1, situacao_claude: 'esperando', espera_ultima: 0 };
  assert.equal(etiquetaComposta(t, 1000, 0).texto, 'manual + esperando você');
  em('en', () => {
    assert.equal(etiquetaComposta(t, 1000, 0).texto, 'manual + waiting for you');
    assert.equal(etiquetaComposta({ pausado: true }, 1000, 0).texto, 'paused');
    assert.equal(tituloDoTrecho({ inicio: 9 * 3_600_000, fim: null, origem: 'claude' }, 0, 10 * 3_600_000), '09:00–now · Claude working · 1h 00m');
  });
});

// ---- a guarda de texto solto ----

/** As palavras do português que, juntas numa frase, denunciam um texto fora do `textos.js`. */
const PALAVRAS = new Set(['de', 'do', 'da', 'dos', 'das', 'em', 'na', 'nas', 'nos', 'para', 'com', 'sem', 'um', 'uma', 'que', 'ao', 'pelo', 'pela', 'ou', 'seu', 'sua', 'este', 'esta']);
const ACENTO = /[áàâãéêíóôõúüç]/i;
const ROTULOS = new Set(['pausar', 'zerar', 'encerrar', 'salvar', 'cancelar', 'fechar', 'voltar', 'ligar', 'desligar', 'criar', 'apagar', 'arquivar', 'editar', 'mover', 'abrir', 'nenhum', 'nenhuma', 'nada', 'sempre', 'agora', 'hoje', 'ontem', 'desde', 'todos', 'tudo', 'mostrar', 'esconder', 'expandir', 'recolher', 'trocar', 'pular', 'entendi', 'proximo', 'anterior', 'concluir', 'continuar', 'comecar', 'escolher', 'outro', 'novo', 'nova', 'sim', 'pronto', 'passo', 'dica', 'dicas', 'ajustes', 'grupo', 'espaco', 'atividade', 'pasta', 'pastas', 'sessao', 'conversa', 'esperando', 'rodando', 'lendo', 'escrevendo', 'trabalhando', 'reuniao', 'mais', 'menos', 'ate']);

/**
 * Os textos literais de um código JS (sem os comentários): cada string entre aspas, apóstrofos ou
 * crases (sem o que está dentro de `${…}`, que é código e entra de novo na busca) e sem as
 * expressões regulares.
 */
function literais(fonte) {
  const v = [];
  let i = 0;
  const n = fonte.length;
  const regexPode = () => {
    // uma "/" começa uma expressão regular depois de um operador, um parêntese ou o começo
    let j = i - 1;
    while (j >= 0 && /\s/.test(fonte[j])) j--;
    return j < 0 || /[(,=:[!&|?{};+\-*%<>~^]/.test(fonte[j]) || /\breturn$/.test(fonte.slice(Math.max(0, j - 6), j + 1));
  };
  function lerString(q) {
    // i aponta para a aspa de abertura
    i++;
    let atual = '';
    while (i < n && fonte[i] !== q) {
      if (fonte[i] === '\\') {
        atual += fonte[i + 1] ?? '';
        i += 2;
        continue;
      }
      if (q === '`' && fonte[i] === '$' && fonte[i + 1] === '{') {
        v.push(atual);
        atual = ' ';
        i += 2;
        lerCodigo('}');
        i++;
        continue;
      }
      atual += fonte[i++];
    }
    i++;
    v.push(atual);
  }
  function lerCodigo(fim) {
    let fundo = 0;
    while (i < n) {
      const c = fonte[i];
      if (fim && c === fim && fundo === 0) return;
      if (c === '{') fundo++;
      else if (c === '}') fundo--;
      if (c === '/' && fonte[i + 1] === '/') {
        while (i < n && fonte[i] !== '\n') i++;
        continue;
      }
      if (c === '/' && fonte[i + 1] === '*') {
        i = fonte.indexOf('*/', i + 2) + 2;
        continue;
      }
      if (c === '"' || c === "'" || c === '`') {
        lerString(c);
        continue;
      }
      if (c === '/' && regexPode()) {
        i++;
        let classe = false;
        while (i < n && (fonte[i] !== '/' || classe)) {
          if (fonte[i] === '\\') i++;
          else if (fonte[i] === '[') classe = true;
          else if (fonte[i] === ']') classe = false;
          i++;
        }
        i++;
        continue;
      }
      i++;
    }
  }
  lerCodigo(null);
  return v;
}

/** O texto parece português (um acento, ou duas palavras comuns numa frase de três ou mais)? */
function pareceProtugues(texto) {
  const t = texto.replace(/<[^>]*>/g, ' ');
  if (ACENTO.test(t)) return true;
  // um rótulo curto com maiúscula ("Pausar", "Nenhum timer"): as palavras que só existem em português
  if (/^\s*[A-Z]/.test(t) && t.trim().split(/\s+/).some((p) => ROTULOS.has(p.toLowerCase().replace(/[^a-z]/g, '')))) return true;
  const palavras = t.toLowerCase().split(/[^a-z]+/).filter(Boolean);
  return palavras.length >= 3 && palavras.filter((p) => PALAVRAS.has(p)).length >= 2;
}

/** Os textos em português de um HTML, fora dos comentários e dos textos que a página troca. */
function textosDoHtml(html) {
  const sem = html
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<script[\s\S]*?<\/script>/g, '')
    // os textos que a página troca pelo idioma de agora (data-texto, data-dica, data-rotulo)
    .replace(/(<[^>]*\bdata-texto="\w+"[^>]*>)[^<]*/g, '$1')
    .replace(/title="[^"]*"(?=[^>]*\bdata-dica=)/g, '')
    .replace(/aria-label="[^"]*"(?=[^>]*\bdata-rotulo=)/g, '');
  const v = [...sem.matchAll(/>([^<]+)</g)].map((m) => m[1]);
  for (const m of sem.matchAll(/\b(?:title|aria-label|placeholder|alt)="([^"]*)"/g)) v.push(m[1]);
  return v.filter((t) => pareceProtugues(t));
}

test('rodada 23: a guarda acha texto solto (o teste do teste)', () => {
  assert.deepEqual(literais("const a = 'Não deu certo'; // comentário\nconst b = `x ${f('y')} z`; const r = /[\"']/g;"), ['Não deu certo', 'x ', 'y', '  z']);
  assert.ok(pareceProtugues('Pausar tudo da aba'));
  assert.ok(pareceProtugues('Nada para encerrar'));
  assert.ok(!pareceProtugues('Pause all'));
  assert.ok(!pareceProtugues('menu-ajuda menu-zerar'));
  assert.deepEqual(textosDoHtml('<b data-texto="x">Pausar</b><i title="Olá" data-dica="y">.</i><p>Não</p>'), ['Não']);
});

/** Os arquivos que esta rodada ainda vai passar para o textos.js (a lista esvazia no fim). */
const AINDA_NAO = [];

test('rodada 23: não sobra texto em português fora do textos.js (JS e HTML da interface)', () => {
  const pasta = new URL('./', import.meta.url);
  const achados = [];
  for (const nome of readdirSync(pasta)) {
    if (!nome.endsWith('.js') || nome.endsWith('.test.js') || nome === 'textos.js' || nome === 'teste-nome.js') continue;
    if (AINDA_NAO.includes(nome)) continue;
    const fonte = readFileSync(new URL(nome, pasta), 'utf8');
    // as linhas de console (diagnóstico, não aparecem na tela) ficam de fora
    const semConsole = fonte.replace(/console\.(?:error|info|warn|log)\([^\n]*/g, '');
    for (const t of literais(semConsole)) if (pareceProtugues(t)) achados.push(`${nome}: ${t.trim().slice(0, 70)}`);
  }
  for (const nome of ['index.html', 'dock.html', 'aviso.html']) {
    const html = readFileSync(new URL(`../${nome}`, pasta), 'utf8');
    for (const t of textosDoHtml(html)) achados.push(`${nome}: ${t.trim().slice(0, 70)}`);
  }
  assert.deepEqual(achados, []);
});

test('rodada 23: aplicarTextos troca os textos fixos, as dicas, os rótulos e o lang', () => {
  const el = (dados) => ({ dataset: dados, textContent: '', attrs: {}, setAttribute(k, v) { this.attrs[k] = v; }, getAttribute(k) { return this.attrs[k] ?? null; } });
  const texto = el({ texto: 'pausarTudo' });
  const dica = el({ dica: 'telasDica' });
  const rotulo = el({ rotulo: 'telasRotulo' });
  const app = el({ textoApp: 'faixaTourTexto' });
  const doc = {
    documentElement: { lang: 'pt-BR' },
    querySelectorAll: (sel) =>
      ({ '[data-texto]': [texto], '[data-dica]': [dica], '[data-rotulo]': [rotulo], '[data-texto-app]': [app] })[sel] ?? [],
  };
  em('en', () => aplicarTextos(doc, 'Chronn'));
  assert.equal(doc.documentElement.lang, 'en');
  assert.equal(texto.textContent, 'Pause all');
  assert.equal(dica.attrs.title, 'Switch screen');
  assert.equal(rotulo.attrs['aria-label'], 'Screens');
  assert.equal(app.textContent, 'New: a 1-minute tour of Chronn');
  aplicarTextos(doc, 'Chronn');
  assert.equal(doc.documentElement.lang, 'pt-BR');
  assert.equal(texto.textContent, 'Pausar tudo');
});

test('rodada 23: Ajustes → Idioma: Sistema, Português (Brasil) e English, cada nome no próprio idioma', async () => {
  const { htmlIdioma } = await import('./ajustes.js');
  const h = htmlIdioma('sistema', 'pt-BR');
  const ops = [...h.matchAll(/value="([^"]+)"( checked)?><span>(.*?)<\/span>/g)].map((m) => [m[1], Boolean(m[2]), m[3].replace(/<[^>]+>/g, '')]);
  assert.deepEqual(ops, [
    ['sistema', true, 'Sistema'],
    ['pt-BR', false, 'Português (Brasil)'],
    ['en', false, 'English'],
  ]);
  // o nome de cada idioma leva o lang dele; o Sistema diz qual vale agora
  assert.match(h, /<bdi lang="en">English<\/bdi>/);
  assert.match(h, /O idioma do Windows \(agora: Português \(Brasil\)\)/);
  em('en', () => {
    const e = htmlIdioma('pt-BR', 'en', 'en-US');
    assert.match(e, /value="pt-BR" checked><span><bdi lang="pt-BR">Português \(Brasil\)<\/bdi>/, 'o nome do português fica em português');
    assert.match(e, /value="sistema"><span>System<\/span>/);
    assert.match(e, /Windows language: English\./);
    // com uma escolha fixa, a dica diz o idioma do Windows, não o escolhido
    assert.match(htmlIdioma('en', 'en', 'pt-BR'), /Windows language: Português \(Brasil\)\./);
  });
  // a seção vem primeiro nos Ajustes, e a escolha grava em config.idioma
  const AJ = readFileSync(new URL('./ajustes.js', import.meta.url), 'utf8');
  assert.match(AJ, /\$\{secao\('idioma', TEXTOS\.secoes\.idioma, htmlIdioma\(c\.idioma, snap\.idioma\), TEXTOS\.idiomaDica\)\}\s*\$\{secao\('aparencia'/);
  assert.match(AJ, /nome === 'tema' \|\| nome === 'idioma' \|\|[^\n]*\{\s*const v = el\.value;\s*return salvar\(\(c\) => \(c\[nome\] = v\)\);/);
  assert.match(AJ, /\['idioma', c\.idioma \?\? IDIOMA_SISTEMA\]/);
});

test('rodada 23: as três janelas seguem o idioma do estado e trocam na hora', () => {
  const PAINEL = readFileSync(new URL('./painel.js', import.meta.url), 'utf8');
  const DOCK = readFileSync(new URL('./dock.js', import.meta.url), 'utf8');
  const AVISO = readFileSync(new URL('./aviso.js', import.meta.url), 'utf8');
  // a primeira pintura: o último usado (ou o do Windows); depois, o do estado
  for (const f of [PAINEL, DOCK]) {
    assert.match(f, /definirIdioma\(idiomaInicial\([^)]*\), navigator\.language\)\);\s*(?:\/\/[^\n]*\n\s*)?aplicarTextos\(document\);/);
    assert.match(f, /idiomaDoEstado\(s, navigator\.language\)/);
  }
  // a principal: troca os textos fixos, fecha o menu aberto, remonta os Ajustes e redesenha o tour
  assert.match(PAINEL, /if \(!definirIdioma\(id\)\) return;\s*aplicarTextos\(document\);[\s\S]*?menuAberto\?\.fechar\(\);\s*if \(visao === 'ajustes'\) remontarAjustesNoIdioma\(\);\s*if \(tourAtivo\(\)\) redesenharTour\(\);/);
  assert.match(PAINEL, /aoMudar\(\(s\) => \{\s*snap = s;\s*seguirIdioma\(s\);/);
  // o aviso recebe o lang do dock
  assert.match(DOCK, /lang: document\.documentElement\.lang/);
  assert.match(AVISO, /if \(aviso\.lang\) document\.documentElement\.lang = aviso\.lang;/);
});
