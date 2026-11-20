// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { campoGrupo, grupoDoForm, grupoComNome, NOVO_GRUPO, PALETA, textoDaFaixaEncerrado, DESFAZER_ENCERRAR_MS, camposDaSessao, corpoLigarATimer, quandoLivre, campoRegra } from './editar.js';

test('rodada 15: pasta-mãe desmarca o "sempre nesta pasta" e avisa; marcar continua possível', () => {
  const h = campoRegra('G:\\Empresa\\Projeto', { pasta_mae: true }, 'Projeto A');
  assert.match(h, /<input type="checkbox" name="regra"><span>Sempre que for nesta pasta/);
  assert.match(h, /<p class="dlg-aviso" role="note">Esta pasta tem vários projetos dentro\. Toda conversa aberta nela vai para Projeto A\.<\/p>/);
  // sem pasta-mãe: marcado, sem aviso
  const normal = campoRegra('G:\\Projeto A', { pasta_mae: false }, 'Projeto A');
  assert.match(normal, /name="regra" checked/);
  assert.doesNotMatch(normal, /dlg-aviso/);
  // a regra da pasta é de outro timer: desmarcado, dizendo que passa
  const outro = campoRegra('G:\\Projeto A', { outro_timer_nome: 'Flow <x>' }, 'Projeto A');
  assert.doesNotMatch(outro, /checked/);
  assert.match(outro, /A regra desta pasta é de Flow &lt;x&gt;: marcar passa a regra para Projeto A\./);
  // o "+ Novo timer…" da sessão numa pasta-mãe
  const pedido = camposDaSessao({ sessao_id: 's1', nome: 'Empresa', pasta: 'G:\\Projeto', pasta_mae: true });
  assert.match(pedido, /name="regra"><span>/);
  assert.match(pedido, /vai para o timer novo\./);
});

test('"+ Novo timer…" de uma sessão: a pasta e o "sempre nesta pasta" já marcado', () => {
  const h = camposDaSessao({ sessao_id: 's1', nome: 'Flow', pasta: 'G:\\Projeto <Solution>\\Flow' });
  assert.match(h, /na pasta <code class="aj-cam">G:\\Projeto &lt;Solution&gt;\\Flow<\/code>/);
  assert.match(h, /<input type="checkbox" name="regra" checked><span>Sempre que for nesta pasta<small>G:\\Projeto &lt;Solution&gt;\\Flow<\/small>/);
});

test('"Ligar a um timer": "+ Novo timer…" no fim, também sem nenhum timer', () => {
  const sessao = { id: 's1', nome: 'Sessão', cwd: 'D:\\x', timer_id: 5 };
  const vazio = corpoLigarATimer({ grupos: [] }, sessao, true);
  assert.match(vazio, /Nenhum timer ainda\.<\/p><div class="lista"><button type="button" class="it it-novo" data-novo="1">/);
  const com = corpoLigarATimer({ grupos: [{ id: 1, nome: 'G', timers: [{ id: 5, nome: 'A', cor: '#a00' }, { id: 6, nome: 'B', cor: '#b00' }] }] }, sessao, true);
  assert.match(com, /data-timer="5"[^>]* disabled>.*já ligada/);
  assert.match(com, /data-timer="6".*<button type="button" class="it it-novo" data-novo="1"><span class="it-n">\+ Novo timer…<\/span><\/button><\/div>$/s);
  // sem quem abra o "Novo timer", fica como era
  assert.doesNotMatch(corpoLigarATimer({ grupos: [] }, sessao), /it-novo/);
  assert.match(corpoLigarATimer({ grupos: [] }, sessao), /<p class="dlg-p">Nenhum timer ainda\.<\/p>$/);
});

test('"+ Novo timer…" na vez dele: descarta se a pergunta saiu, oferece ligar ao de mesmo nome', async () => {
  const { decidirPedido, perguntaDaSessao, timerComNome } = await import('./editar.js');
  const pergunta = { id: 7, tipo: 'sessao_nova', sessao_id: 's1', opcoes: [] };
  const projeto_a = { id: 3, nome: ' Projeto A ', cor: '#a00' };
  const snap = (perguntas, timers = []) => ({ perguntas, grupos: [{ id: 1, nome: 'G', timers }] });
  const pedido = { sessao_id: 's1', nome: 'projeto a', pasta: 'G:\\projeto a' };
  // a pergunta saiu (o Trazer ligou a sessão pela regra, ou ela foi ligada pelo "Escolher timer…")
  assert.deepEqual(decidirPedido(snap([], [projeto_a]), pedido), { descartar: true });
  assert.deepEqual(decidirPedido(snap([{ ...pergunta, sessao_id: 'outra' }]), pedido), { descartar: true });
  assert.deepEqual(decidirPedido(snap([{ ...pergunta, tipo: 'volta_inatividade' }]), pedido), { descartar: true });
  // a pergunta segue e já há "Projeto A" (sem diferenciar maiúsculas nem espaços): ligar a ele
  assert.deepEqual(decidirPedido(snap([pergunta], [projeto_a]), pedido), { ligarA: projeto_a });
  assert.deepEqual(decidirPedido(snap([pergunta], [{ ...projeto_a, nome: 'Outro' }]), pedido), { criar: true });
  assert.equal(perguntaDaSessao(snap([pergunta]), 's1'), pergunta);
  // a da conversa nova numa pasta com regra também é da sessão (rodada 15)
  const conversa = { id: 8, tipo: 'conversa_nova', sessao_id: 's2', opcoes: [] };
  assert.equal(perguntaDaSessao(snap([pergunta, conversa]), 's2'), conversa);
  assert.equal(perguntaDaSessao(snap([{ id: 9, tipo: 'volta_inatividade', opcoes: [] }]), 's1'), null);
  assert.equal(timerComNome(snap([], [projeto_a]), ''), null, 'nome vazio não acha nada');
});

test('quandoLivre: sem diálogo aberto, roda na hora', () => {
  let rodou = false;
  quandoLivre(() => (rodou = true));
  assert.equal(rodou, true);
});

const snap = { grupos: [{ id: 1, nome: 'Clientes', cor: PALETA[0], timers: [] }, { id: null, nome: 'Sem grupo', cor: '#8a8f98', timers: [] }] };

test('seletor de grupo: "+ Novo grupo…" no fim, com nome e cor escondidos até escolher', () => {
  const h = campoGrupo(snap, 1);
  assert.match(h, /<option value="1" selected>Clientes<\/option><option value="">Sem grupo<\/option><option value="novo">\+ Novo grupo…<\/option><\/select>/);
  assert.match(h, /<div class="grupo-novo" hidden>/);
  assert.match(h, /name="grupo_nome"/);
  // a cor do grupo novo começa na primeira que nenhum grupo usa
  assert.match(h, new RegExp(`name="grupo_cor" value="${PALETA[1]}" checked`));
  assert.doesNotMatch(h, /name="cor"/, 'não mistura com a cor do timer');
  assert.match(campoGrupo(snap, null), /<option value="" selected>Sem grupo/);
});

test('grupoDoForm: grupo que existe, sem grupo ou novo (com nome)', () => {
  assert.deepEqual(grupoDoForm('1', '', ''), { id: 1 });
  assert.deepEqual(grupoDoForm('', 'x', ''), { id: null });
  assert.deepEqual(grupoDoForm(NOVO_GRUPO, '  Pessoal ', '#7b88ff'), { novo: { nome: 'Pessoal', cor: '#7b88ff' } });
  assert.throws(() => grupoDoForm(NOVO_GRUPO, '   ', '#7b88ff'), (e) => e === 'Dê um nome ao grupo novo');
});

test('grupoComNome: o mesmo nome sem diferenciar maiúsculas nem espaços; nunca o "Sem grupo"', () => {
  assert.equal(grupoComNome(snap.grupos, ' clientes ')?.id, 1);
  assert.equal(grupoComNome(snap.grupos, 'Sem grupo'), null);
  assert.equal(grupoComNome(snap.grupos, 'Outro'), null);
  assert.equal(grupoComNome([{ id: 7, nome: 'Pessoal' }], 'PESSOAL')?.id, 7);
});

test('faixa do ■: "<timer> encerrado." por 10 s', () => {
  assert.equal(textoDaFaixaEncerrado('Cliente F'), 'Cliente F encerrado.');
  assert.equal(textoDaFaixaEncerrado(undefined), 'Encerrado.');
  assert.equal(DESFAZER_ENCERRAR_MS, 10_000);
});

test('mover uma sessão: "desde o início" marcado se ela não tem timer ou começou há menos de 15 min', async () => {
  const { desdeInicioPadrao, MOVER_DESDE_INICIO_MS } = await import('./editar.js');
  const agora = 1_000_000_000;
  assert.equal(desdeInicioPadrao({ timer_id: null, criada_em: 0 }, agora), true, 'sem timer: sempre');
  assert.equal(desdeInicioPadrao({ timer_id: 3, criada_em: agora - 14 * 60_000 }, agora), true);
  assert.equal(desdeInicioPadrao({ timer_id: 3, criada_em: agora - MOVER_DESDE_INICIO_MS }, agora), false);
  assert.equal(desdeInicioPadrao({ timer_id: 3 }, agora), false, 'sem saber quando começou: daqui para frente');
  assert.equal(desdeInicioPadrao({ id: 's1', nome: 'Sessão do Claude', cwd: '' }, agora), true);
});

test('"Mais opções" fechado por padrão, aberto quando pedido, e diz o que já tem dentro', async () => {
  const { maisOpcoes, resumoDoTimer } = await import('./editar.js');
  assert.match(maisOpcoes('<i>x</i>'), /^<details class="mais"><summary>Mais opções<\/summary><div class="mais-corpo"><i>x<\/i><\/div><\/details>$/);
  assert.match(maisOpcoes('x', true), /^<details class="mais" open>/);
  assert.match(maisOpcoes('x', false, 'card <do> Zoho'), /<summary>Mais opções <small>card &lt;do&gt; Zoho<\/small><\/summary>/);
  assert.equal(resumoDoTimer({ link_card: null, ajuste_ms: 0, ajustes: [] }), '');
  assert.equal(resumoDoTimer({ link_card: 'https://x', ajuste_ms: 0, ajustes: [] }), 'card do Zoho');
});

test('"Trocar" sem a sessão na lista: a reserva leva o timer e, sem o início, move "daqui para frente"', async () => {
  const { sessaoDeReserva, desdeInicioPadrao } = await import('./editar.js');
  const r = sessaoDeReserva('s1', { nome: 'Conversa', cwd: 'D:/x', timer_id: 4 });
  assert.deepEqual(r, { id: 's1', nome: 'Conversa', cwd: 'D:/x', timer_id: 4 });
  assert.equal(desdeInicioPadrao(r, Date.now()), false);
  // sem nada: sessão sem timer, como antes
  assert.deepEqual(sessaoDeReserva('s2'), { id: 's2', nome: 'Sessão do Claude', cwd: '', timer_id: null });
  assert.equal(desdeInicioPadrao(sessaoDeReserva('s2'), Date.now()), true);
});

test('rodada 15: "Pastas deste timer" lista só as regras dele, com Trocar e Tirar', async () => {
  const { corpoPastasDoTimer } = await import('./editar.js');
  const regras = [
    { id: 1, pasta: 'G:/Projeto A', timer_id: 5, timer_nome: 'Projeto A' },
    { id: 2, pasta: 'G:/Flow <x>', timer_id: 6, timer_nome: 'Flow' },
    { id: 3, pasta: 'D:/Projeto A2', timer_id: 5, timer_nome: 'Projeto A' },
  ];
  const h = corpoPastasDoTimer(regras, 5);
  assert.equal([...h.matchAll(/<li>/g)].length, 2);
  assert.match(h, /<code class="aj-cam" title="G:\/Projeto A">G:\/Projeto A<\/code><button type="button" class="bt mini" data-pasta="trocar" data-id="1">Trocar pasta…<\/button><button type="button" class="bt mini" data-pasta="tirar" data-id="1">Tirar<\/button>/);
  assert.doesNotMatch(h, /Flow/);
  assert.match(h, /data-pasta="acrescentar">\+ Acrescentar pasta…/);
  assert.match(h, /as que já estão ligadas ficam onde estão/);
  // sem regras: o vazio e o acrescentar
  const vazio = corpoPastasDoTimer(regras, 7);
  assert.match(vazio, /Nenhuma pasta\./);
  assert.match(vazio, /data-pasta="acrescentar"/);
  // o erro da última pasta escolhida
  assert.match(corpoPastasDoTimer([], 5, null, 'Uma regra não pode <x>'), /<p class="pt-erro" role="alert">Uma regra não pode &lt;x&gt;<\/p>/);
});

test('rodada 15: confirmar a regra numa pasta-mãe ou que já é de outro timer', async () => {
  const { confirmacaoDaRegra, corpoPastasDoTimer } = await import('./editar.js');
  assert.equal(confirmacaoDaRegra({ pasta_mae: false, outro_timer_nome: null }, 'Projeto A'), null);
  const mae = confirmacaoDaRegra({ pasta_mae: true }, 'Projeto A');
  assert.deepEqual(mae, { textos: ['Esta pasta tem vários projetos dentro. Toda conversa aberta nela vai para Projeto A.'], botao: 'Pôr a regra assim mesmo' });
  const outro = confirmacaoDaRegra({ outro_timer_nome: 'Flow <x>' }, 'Projeto A');
  assert.deepEqual(outro, { textos: ['A regra desta pasta é de Flow &lt;x&gt;. Passar para Projeto A?'], botao: 'Passar para Projeto A' });
  const os2 = confirmacaoDaRegra({ pasta_mae: true, outro_timer_nome: 'Flow' }, 'Projeto A');
  assert.equal(os2.textos.length, 2);
  assert.equal(os2.botao, 'Passar para Projeto A');
  const h = corpoPastasDoTimer([], 5, { ...mae, pasta: 'G:/Projeto', trocar: null });
  assert.match(h, /<div class="dlg-aviso pt-conf" role="alert"><code class="aj-cam">G:\/Projeto<\/code><p>Esta pasta tem vários projetos dentro\./);
  assert.match(h, /data-pasta="confirmar">Pôr a regra assim mesmo<\/button><button type="button" class="bt mini" data-pasta="cancelar">Cancelar/);
});

test('rodada 15: o ✎ da sessão diz como ela entrou', async () => {
  const { comoEntrou } = await import('./editar.js');
  assert.equal(comoEntrou({ ligacao: 'regra', regra_pasta: 'G:/Projeto' }), 'pela pasta G:/Projeto');
  assert.equal(comoEntrou({ ligacao: 'mao' }), 'ligada à mão');
  assert.equal(comoEntrou({ ligacao: 'herdada', nome: 'Projeto: Cliente B' }), 'continuação de “Projeto: Cliente B”');
  assert.equal(comoEntrou({ ligacao: 'leitura' }), 'pela leitura');
  assert.equal(comoEntrou({ ligacao: 'deduzida' }), 'ligada antes desta versão do app');
  assert.equal(comoEntrou({ ligacao: null }), 'ligada antes desta versão do app');
});

test('rodada 15: o ✎ da sessão mostra nome e pasta só para ler, e as ações da regra só pela regra', async () => {
  const { corpoSessao } = await import('./editar.js');
  const pelaRegra = corpoSessao({ id: 's1', nome: 'Flow <x>', cwd: 'G:/Projeto/Flow', ligacao: 'regra', regra_id: 3, regra_pasta: 'G:/Projeto' });
  assert.match(pelaRegra, /<input type="text" value="Flow &lt;x&gt;" readonly>/);
  assert.match(pelaRegra, /class="mono" value="G:\/Projeto\/Flow" readonly/);
  assert.match(pelaRegra, /<p class="ses-como">pela pasta G:\/Projeto<\/p>/);
  for (const a of ['mover', 'desligar', 'subpasta', 'apagar-regra']) assert.match(pelaRegra, new RegExp(`data-ses="${a}"`));
  assert.match(pelaRegra, /Mover para outro timer…[\s\S]*Desligar deste timer[\s\S]*Trocar por uma subpasta…[\s\S]*Apagar esta regra/);
  const aMao = corpoSessao({ id: 's2', nome: 'B', cwd: 'D:/x', ligacao: 'mao', regra_id: null });
  assert.match(aMao, /data-ses="mover"/);
  assert.match(aMao, /data-ses="desligar"/);
  assert.doesNotMatch(aMao, /data-ses="subpasta"|data-ses="apagar-regra"/);
  assert.match(aMao, /volta a perguntar na próxima mensagem; o tempo já contado fica onde está/);
});
