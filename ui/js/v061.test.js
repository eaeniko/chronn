// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// 0.6.1, as correções do teste da 0.6.0: o dock com duas sessões (uma lida e outra esperando), o
// rótulo da conta na pergunta e nos Ajustes, o aviso da pasta "raiz" no modal e o "Ligar" na
// distro parada com a seção do WSL que se atualiza.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { TEXTOS, definirIdioma } from './textos.js';
import { timersParaDock } from './formato.js';
import { perguntaContaNova, perguntaSessaoNova, mudarEscolha, esquecerEscolhas } from './componentes.js';
import { htmlContas, ajustesAVista, WSL_A_CADA_MS } from './ajustes.js';

const fonte = (nome) => readFileSync(new URL(nome, import.meta.url), 'utf8');
const M = 60_000;
const H = 60 * M;

function em(id, f) {
  definirIdioma(id);
  try {
    return f();
  } finally {
    definirIdioma('pt-BR');
  }
}

const timer = (id, extra = {}) => ({
  id,
  nome: `T${id}`,
  cor: '#4f8cff',
  rodando: false,
  situacao: null,
  espera_desde: null,
  espera_ultima: null,
  atividade_padrao_id: 1,
  trechos: [],
  ajustes: [],
  ajuste_ms: 0,
  sessoes: [],
  ...extra,
});

test('0.6.1: o dock mostra junto a sessão lida e a que espera você desde a sua última leitura', () => {
  // "kit" (WSL) terminou o turno há 2 h, mas você leu ela até 5 min atrás (o núcleo manda a
  // `espera_ultima` da última leitura); agora você lê "Chronn" no app
  const agora = 10 * H;
  const lida = timer(1, { nome: 'Chronn', rodando: true, situacao: 'lendo', trechos: [{ inicio: agora - 5 * M, fim: null }] });
  const espera = timer(2, { nome: 'kit', situacao: 'esperando', espera_desde: agora - 2 * H, espera_ultima: agora - 5 * M });
  const snap = { agora, config: { espera_dock_min: 30 }, grupos: [{ id: 1, timers: [lida, espera] }] };
  assert.deepEqual(timersParaDock(snap, agora, true).map((x) => [x.nome, x.situacao]), [['Chronn', 'lendo'], ['kit', 'esperando']]);
  // sem a última leitura (só o fim do turno, há 2 h): sairia do dock, como no teste da 0.6.0
  const velha = { ...snap, grupos: [{ id: 1, timers: [lida, { ...espera, espera_ultima: agora - 2 * H }] }] };
  assert.deepEqual(timersParaDock(velha, agora, true).map((x) => x.nome), ['Chronn']);
});

const pergunta = (o = {}) => ({
  id: 2 ** 52 + 7,
  tipo: 'conta_nova',
  texto: 'De qual espaço é esta conta?',
  opcoes: [
    ['conta:a1b2:espaco:1', 'Trabalho'],
    ['conta:a1b2:espaco:2', 'Pessoal'],
    ['conta:a1b2:depois', 'Depois'],
  ],
  cores: ['#5b8def', '#e8a23a', null],
  sessao_id: null,
  pasta: null,
  pasta_curta: null,
  sessao_nome: null,
  origem: 'wsl',
  pastas: [],
  conta_rotulo: 'Ana · an•••@exemplo.com · Pro',
  conta_curta: 'a1b2',
  pasta_larga: false,
  ...o,
});

test('0.6.1: a pergunta da conta diz qual é a conta (o rótulo; sem ele, a curta)', () => {
  assert.match(perguntaContaNova(pergunta()), /<dt>Conta<\/dt><dd class="sn-nome" title="Ana · an•••@exemplo\.com · Pro">Ana · an•••@exemplo\.com · Pro<\/dd>/);
  assert.match(perguntaContaNova(pergunta({ conta_rotulo: null })), /<dt>Conta<\/dt><dd class="sn-nome" title="Conta ·a1b2">Conta ·a1b2<\/dd>/);
  assert.doesNotMatch(perguntaContaNova(pergunta({ conta_rotulo: null, conta_curta: null })), /<dt>Conta<\/dt>/);
  assert.match(em('en', () => perguntaContaNova(pergunta())), /<dt>Account<\/dt>/);
});

test('0.6.1: os Ajustes mostram o rótulo da conta no lugar do "Conta ·1a2b"', () => {
  const espacos = [{ id: 1, nome: 'Trabalho', cor: '#5b8def', arquivado: false }];
  const c = (o) => ({ id: 'a1b2c3d4e5f60718', curta: 'a1b2', rotulo: null, origem: 'wsl', visto_em: Date.UTC(2026, 10, 22, 12), espaco_id: null, ...o });
  const h = htmlContas([c({ rotulo: 'Ana · an•••@exemplo.com · Max' })], espacos);
  assert.match(h, /<span class="aj-n">Ana · an•••@exemplo\.com · Max<\/span>/);
  assert.match(h, /aria-label="Espaço da conta Ana · an•••@exemplo\.com · Max"/);
  assert.match(htmlContas([c({})], espacos), /<span class="aj-n">Conta ·a1b2<\/span>/);
});

const sessaoNova = (o = {}) => ({
  id: 9,
  tipo: 'sessao_nova',
  texto: 'x',
  opcoes: [['ligar:3', 'Contar em Site'], ['outro', 'Outro timer…'], ['novo', '+ Novo timer…'], ['nao', 'Não contar']],
  sessao_id: 's1',
  pasta: 'D:/Projetos',
  pasta_curta: 'D:/Projetos',
  sessao_nome: 'Arrumar o login',
  origem: 'wsl',
  pastas: [{ pasta: 'D:/Projetos', curta: 'D:/Projetos', comecou: true, agora: true, varios: true }],
  cores: [],
  conta_rotulo: null,
  conta_curta: null,
  pasta_larga: false,
  ...o,
});

test('0.6.1: o modal avisa a pasta com vários projetos (marcada) e a pasta do usuário', () => {
  // desmarcada: nada a avisar
  assert.doesNotMatch(perguntaSessaoNova(sessaoNova()), /vários projetos/);
  // marcada: o aviso no lugar do "A pasta de cima…"
  esquecerEscolhas([]);
  assert.ok(mudarEscolha({ dataset: { pergunta: '9', pgSempre: '', pasta: 'D:/Projetos' }, checked: true }));
  const marcada = perguntaSessaoNova(sessaoNova());
  assert.match(marcada, /<b>Esta pasta tem vários projetos dentro, e a regra pega todos\. Para separar, abra o claude dentro da pasta de cada projeto\.<\/b>/);
  // uma pasta de um projeto só: sem o aviso
  assert.doesNotMatch(perguntaSessaoNova(sessaoNova({ pastas: [{ pasta: 'D:/Projetos', curta: 'D:/Projetos', comecou: true, agora: true, varios: false }] })), /vários projetos/);
  esquecerEscolhas([]);
  // a pasta do usuário: sem a caixa, o aviso de abrir o claude dentro do projeto
  const home = perguntaSessaoNova(sessaoNova({ pasta: 'C:/Users/voce', pasta_curta: 'C:/Users/voce', pastas: [], pasta_larga: true }));
  assert.match(home, /<small class="sn-ajuda"><b>Esta pasta é ampla demais para uma regra \(uma pasta de usuário ou a raiz de um disco\)\. Para cada projeto contar no timer dele, abra o claude dentro da pasta do projeto\.<\/b><\/small>/);
  assert.doesNotMatch(home, /data-pg-sempre/);
  assert.doesNotMatch(perguntaSessaoNova(sessaoNova({ pastas: [], pasta_larga: false })), /ampla demais/);
  assert.match(em('en', () => perguntaSessaoNova(sessaoNova({ pastas: [], pasta_larga: true }))), /open claude inside the project’s folder/);
});

test('0.6.1: a seção do WSL só é lida de novo com os Ajustes à vista, a cada 10 s', () => {
  assert.equal(WSL_A_CADA_MS, 10_000);
  const el = { hidden: false, isConnected: true };
  const doc = { visibilityState: 'visible' };
  assert.ok(ajustesAVista(el, el, doc));
  assert.ok(!ajustesAVista(el, { outro: 1 }, doc), 'remontado noutro lugar');
  assert.ok(!ajustesAVista({ ...el, hidden: true }, null, doc));
  assert.ok(!ajustesAVista(el, el, { visibilityState: 'hidden' }), 'a janela escondida');
  const aj = fonte('./ajustes.js');
  assert.match(aj, /el\.innerHTML = html\(snap\);\n  atualizarWsl\(\);\n  vigiarWsl\(el\);/);
  // com um diálogo aberto, espera; fora dos Ajustes, para
  assert.match(aj, /document\.querySelector\('\.dlg-fundo'\) \|\| subindo\.size\) return;/);
  assert.match(aj, /if \(raiz !== el \|\| el\.hidden \|\| el\.isConnected === false\) \{\n      clearInterval\(relerWsl\);/);
  // o "Ligar" da parada sobe a distro só no clique, na prévia e no Ligar da prévia
  assert.match(aj, /chamar\('wsl_previa', \{ distro: nome, subir \}\)/);
  assert.match(aj, /chamar\('wsl_ligar', \{ distro: nome, subir \}\)/);
  // revisão M4: um segundo clique não sobe a distro de novo, e o botão diz "Ligando…"
  assert.match(aj, /if \(subindo\.has\(nome\)\) return Promise\.resolve\(false\);/);
  assert.match(aj, /alvo\.textContent = TEXTOS\.wslLigando;/);
});

test('0.6.1: as chaves novas existem e mudam nos dois idiomas; o limite do WSL está escrito', () => {
  for (const k of ['avisoPastaLarga', 'avisoVariosProjetos', 'rotuloConta']) {
    assert.equal(typeof TEXTOS[k], 'string', k);
    assert.notEqual(TEXTOS[k], em('en', () => TEXTOS[k]), k);
  }
  assert.notEqual(TEXTOS.wslLigarParada('Ubuntu'), em('en', () => TEXTOS.wslLigarParada('Ubuntu')));
  assert.match(TEXTOS.wslAjuda, /Com a distro parada, os últimos eventos esperam na fila até ela rodar de novo\./);
  assert.match(em('en', () => TEXTOS.wslAjuda), /the last events wait in its queue/);
});

test('0.6.1: a distro ligada cujos hooks saíram do settings.json pede "Ligar de novo"', async () => {
  const { htmlWsl } = await import('./ajustes.js');
  const h = htmlWsl([{ nome: 'Ubuntu', estado: 'sem_hooks', ligada: true }]);
  assert.match(h, /<span class="aj-estado antigo"><i><\/i>os hooks saíram do settings\.json<\/span><small><\/small><button type="button" class="bt mini pri" data-aj="wsl-ligar" data-distro="Ubuntu" title="Outro programa regravou o ~\/\.claude\/settings\.json do Ubuntu sem os hooks do Chronn: as sessões de lá não contam até ligar de novo">Ligar de novo<\/button><button type="button" class="bt mini" data-aj="wsl-desligar"/);
  assert.match(em('en', () => htmlWsl([{ nome: 'Ubuntu', estado: 'sem_hooks', ligada: true }])), /the hooks left settings\.json[^]*Connect again/);
  assert.notEqual(TEXTOS.wslEstado('sem_hooks'), em('en', () => TEXTOS.wslEstado('sem_hooks')));
});

test('0.6.1: a faixa do WSL sem os hooks, com o "Ligar de novo" e o × que esconde até a lista mudar', async () => {
  const { faixaDoWsl } = await import('./ajustes.js');
  assert.equal(faixaDoWsl([]), null);
  assert.equal(faixaDoWsl(undefined), null);
  assert.deepEqual(faixaDoWsl(['Ubuntu']), {
    texto: 'Os hooks do Chronn saíram do ~/.claude/settings.json do Ubuntu (outro programa regravou o arquivo): as sessões de lá não contam.',
    distro: 'Ubuntu',
    chave: 'Ubuntu',
  });
  assert.equal(faixaDoWsl(['Ubuntu', 'Debian']).chave, 'Ubuntu|Debian');
  assert.match(em('en', () => faixaDoWsl(['Ubuntu']).texto), /Chronn’s hooks left/);
  const html = readFileSync(new URL('../index.html', import.meta.url), 'utf8');
  assert.match(html, /<div class="faixa-topo" id="faixa-wsl" role="status" hidden>/);
  assert.match(html, /id="bt-wsl-de-novo" data-texto="wslLigarDeNovo"/);
  const painel = fonte('./painel.js');
  // o app nunca regrava sozinho: a faixa abre a prévia do "Ligar"
  assert.match(painel, /if \(d\) ligarDistro\(d\);/);
  assert.match(painel, /mostrarFaixaHooks\(\);\n  mostrarFaixaWsl\(\);/);
});
