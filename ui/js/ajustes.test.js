// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
import { test } from 'node:test'; import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { CREDITOS, corpoPrevia, corpoInstalado, campoAoFechar, campoModoDev, TEMPOS } from './ajustes.js';
import { corpoFechar, dicaDoCodigo, campoLink, campoAjuste, ajusteDoForm, grupoParaSalvar, timerParaSalvar, nomeAoTrocarLink, linkComNome } from './editar.js';
import { textoDoCard, ajusteDoTimer, marcaAjuste, cabecalhoGrupo, destinoDoGrupo, linksDoGrupo } from './componentes.js';
import { semTimers } from './primeiro-uso.js';
import { NOME_APP } from './nome.js';
test('créditos exatos do Sobre (rodada 18)', () => assert.equal(CREDITOS, 'Criado por @eaeniko (Nikolai Cinotti)'));
test('prévia escapada em bloco monoespaçado', () => {
  const h = corpoPrevia('Antes:\n(sem hooks)\n"<x>"', 'C:/p/settings.json');
  assert.match(h, /<pre class="aj-previa"[^>]*>Antes:\n\(sem hooks\)\n&quot;&lt;x&gt;&quot;<\/pre>/);
  assert.ok(h.includes('<code class="aj-cam">C:/p/settings.json</code>'), 'fora da pasta do usuário, o caminho como está');
});
test('prévia traz o campo das pastas que nunca contam', () => {
  const h = corpoPrevia('x', 's', ['G:\\Pessoal', 'D:\\<notas>']);
  assert.match(h, /<textarea name="pastas_ignoradas"[^>]*>G:\\Pessoal\nD:\\&lt;notas&gt;<\/textarea>/);
  assert.ok(h.indexOf('aj-previa') < h.indexOf('pastas_ignoradas'));
  assert.match(corpoPrevia('x', 's'), /<textarea name="pastas_ignoradas"[^>]*><\/textarea>/);
});
test('depois de instalar: cópia ou arquivo novo', () => {
  assert.match(corpoInstalado('C:/p/settings.json.bak'), /settings\.json\.bak/);
  assert.match(corpoInstalado(''), /arquivo novo criado/);
});
test('primeiro uso só sem nenhum timer', () => {
  assert.equal(semTimers({ grupos: [] }), true);
  assert.equal(semTimers({ grupos: [{ id: 1, timers: [] }] }), true);
  assert.equal(semTimers({ grupos: [{ id: null, timers: [{ id: 1 }] }] }), false);
});
test('Ao fechar a janela: três opções, a atual marcada', () => {
  const h = campoAoFechar('bandeja');
  assert.equal((h.match(/name="ao_fechar"/g) ?? []).length, 3);
  assert.match(h, /value="bandeja" checked><span>Manter na bandeja/);
  assert.match(h, /value="perguntar"><span>Perguntar/);
  assert.match(h, /value="sair"><span>Sair/);
  assert.match(h, /Ao fechar a janela/);
});
test('Fechar o app?: fala da bandeja e traz "Não perguntar de novo"', () => {
  const h = corpoFechar();
  assert.match(h, /bandeja \(os ícones ao lado do relógio\)/);
  assert.match(h, /<input type="checkbox" name="lembrar"><span>Não perguntar de novo/);
});
test('modo desenvolvedor: interruptor no Sobre e limites dos tempos', () => {
  assert.match(campoModoDev(false), /<input type="checkbox" name="modo_dev"><span>Modo desenvolvedor/);
  assert.match(campoModoDev(true), /name="modo_dev" checked/);
  const max = Object.fromEntries(TEMPOS.map(([k, , , m]) => [k, m]));
  assert.deepEqual(max, { uso_s: 1440, inatividade_s: 30, sessao_parada_s: 1440 });
});
const CARD = 'https://sprints.zoho.com/workspace/exemplo#P146/itemdetails/I7';
test('nome do destino: dica com o código e campos com o nome', () => {
  assert.match(dicaDoCodigo(CARD), /item I7 do projeto P146/);
  assert.match(dicaDoCodigo('https://sprints.zoho.com/workspace/exemplo#P146'), /projeto P146/);
  assert.match(dicaDoCodigo(''), /^Opcional/);
  const h = campoLink('link_card', 'Link do card', CARD, '', 'link_card_nome', 'Nome do card', 'INV <x>');
  assert.match(h, /name="link_card" value="https:[^"]*I7"[^>]*data-link="link_card_nome"/);
  assert.match(h, /name="link_card_nome" value="INV &lt;x&gt;"/);
  assert.match(h, /data-codigo="link_card_nome">O link traz só o código \(item I7/);
});
test('o card na tela: código e nome, e o nome do card padrão vale para quem herda', () => {
  const grupo = { card_padrao: CARD, card_padrao_nome: 'INV - Inovação' };
  assert.equal(textoDoCard({ link_card: CARD }, null).texto, 'Card: I7');
  assert.equal(textoDoCard({ link_card: CARD, link_card_nome: 'Painel' }, grupo).texto, 'Card: Painel · I7');
  assert.deepEqual(textoDoCard({ link_card: null }, grupo), { texto: 'Card padrão do grupo: INV - Inovação · I7', herdado: true });
  const cab = cabecalhoGrupo({ id: 1, nome: 'INV', timers: [], ...grupo }, 0, false);
  assert.match(cab, /card padrão: INV - Inovação · I7/);
});
test('salvar grupo e timer leva os nomes dos links (sem eles, sumiriam)', () => {
  const g = grupoParaSalvar({ id: 1, nome: 'G', cor: '#111', link_projeto: 'p', card_padrao: 'c', link_projeto_nome: 'P', card_padrao_nome: 'C', ordem: 2, timers: [] });
  assert.deepEqual(g, { id: 1, nome: 'G', cor: '#111', link_projeto: 'p', card_padrao: 'c', link_projeto_nome: 'P', card_padrao_nome: 'C', ordem: 2, arquivado: false, espaco_id: 1 });
  const t = timerParaSalvar({ id: 3, grupo_id: 1, nome: 'T', cor: '#222', link_card: 'l', link_card_nome: 'L', ordem: 0, rodando: true, trechos: [] });
  assert.deepEqual(t, { id: 3, grupo_id: 1, nome: 'T', cor: '#222', link_card: 'l', link_card_nome: 'L', ordem: 0, arquivado: false, atividade_padrao_id: 1, espaco_id: 1 });
  // rodada 20: o espaço não some ao salvar
  assert.equal(grupoParaSalvar({ id: 1, espaco_id: 2 }).espaco_id, 2);
  assert.equal(timerParaSalvar({ id: 3, espaco_id: 2 }).espaco_id, 2);
  assert.equal(timerParaSalvar({ id: 3, atividade_padrao_id: 3 }).atividade_padrao_id, 3, 'a atividade padrão não some ao salvar');
});
test('ajuste à mão: marca no painel do timer e o "Ajustar hoje"', () => {
  const t = { ajuste_ms: 15 * 60e3, ajustes: [{ id: 1, delta_ms: 15 * 60e3, motivo: 'reunião <sem> timer' }] };
  assert.deepEqual(ajusteDoTimer(t), { texto: '+15 min', dica: 'ajustado à mão: +15 min · reunião <sem> timer' });
  assert.equal(ajusteDoTimer({ ajuste_ms: 0, ajustes: [] }), null);
  assert.match(marcaAjuste(t), /<div class="mao" title="ajustado à mão: \+15 min · reunião &lt;sem&gt; timer">ajustado à mão: \+15 min<\/div>/);
  assert.equal(marcaAjuste({}), '');
  assert.match(campoAjuste(t), /Hoje já tem \+15 min à mão/);
  assert.match(campoAjuste({}), /name="aj_sinal"[\s\S]*name="aj_min"[\s\S]*name="aj_motivo" maxlength="80"/);
  const form = (sinal, min, motivo = '') => ({ elements: { aj_sinal: { value: sinal }, aj_min: { value: min, focus() {} }, aj_motivo: { value: motivo } } });
  assert.equal(ajusteDoForm(form('+', '')), null);
  assert.deepEqual(ajusteDoForm(form('-', '20', ' almoço longo ')), { delta_ms: -20 * 60e3, motivo: 'almoço longo', atividade_id: null });
  assert.deepEqual(ajusteDoForm(form('+', '15')), { delta_ms: 15 * 60e3, motivo: null, atividade_id: null });
  assert.throws(() => ajusteDoForm(form('+', 'x')), /de 1 a 1440/);
  assert.throws(() => ajusteDoForm(form('+', '', 'só o motivo')), /quantos minutos/);
});
const PROJ = 'https://sprints.zoho.com/workspace/exemplo#P146';
test('o grupo mostra o projeto e o card padrão; quem herda usa o nome do grupo', () => {
  const g = { id: 1, nome: 'INV', link_projeto: PROJ, link_projeto_nome: 'INV - Inovação', card_padrao: CARD, card_padrao_nome: null, timers: [] };
  assert.equal(linksDoGrupo(g), 'projeto: INV - Inovação · P146 · card padrão: I7');
  assert.match(cabecalhoGrupo(g, 0, true), /projeto: INV - Inovação · P146 · card padrão: I7/);
  // card padrão sem nome: herda o nome do projeto; sem nenhum, o nome do grupo
  assert.deepEqual(destinoDoGrupo(g), { tipo: 'card', rotulo: 'INV - Inovação · I7' });
  assert.equal(textoDoCard({ link_card: null }, { ...g, link_projeto_nome: null }).texto, 'Card padrão do grupo: INV · I7');
  // só o link do projeto: as horas vão para o projeto (hora geral)
  const so = { id: 2, nome: 'Cursos', link_projeto: PROJ, timers: [] };
  assert.deepEqual(textoDoCard({ link_card: null }, so), { texto: 'Projeto do grupo: Cursos · P146', herdado: true });
  assert.equal(linksDoGrupo(so), 'projeto: P146');
  assert.equal(destinoDoGrupo({ id: null, nome: 'Sem grupo', timers: [] }), null);
  assert.equal(linksDoGrupo({}), '');
});
test('card sem item: aviso; nome só vale com link', () => {
  assert.match(dicaDoCodigo(PROJ, true), /^Atenção: este link não aponta para um item/);
  assert.doesNotMatch(dicaDoCodigo(PROJ, false), /Atenção/);
  assert.doesNotMatch(dicaDoCodigo(CARD, true), /Atenção/);
  assert.match(dicaDoCodigo('https://exemplo.com/x', true), /^Atenção/);
  assert.doesNotMatch(dicaDoCodigo('', true), /Atenção/);
  assert.match(campoLink('link_card', 'L', PROJ, '', 'link_card_nome', 'N', '', true), /data-card="1"[\s\S]*Atenção/);
  const form = (link, nome) => ({ elements: { l: { value: link }, n: { value: nome } } });
  assert.deepEqual(linkComNome(form(' x ', ' Nome '), 'l', 'n'), ['x', 'Nome']);
  assert.deepEqual(linkComNome(form('', 'Nome'), 'l', 'n'), [null, null], 'nome sem link não é guardado');
});
test('trocar o link tira o nome antigo, a menos que a pessoa mexa nele', () => {
  const CARD9 = CARD.replace('I7', 'I9');
  const base = { linkAntes: CARD, nomeAntes: 'Painel' };
  const r = nomeAoTrocarLink({ ...base, linkAgora: CARD9, nomeAgora: 'Painel' });
  assert.equal(r.nome, '');
  assert.match(r.aviso, /o nome "Painel" saiu/);
  assert.equal(r.limpo, true);
  // de volta ao link de antes: o nome volta
  assert.deepEqual(nomeAoTrocarLink({ ...base, linkAgora: CARD, nomeAgora: '', limpo: true }), { nome: 'Painel', aviso: null, limpo: false });
  // a pessoa escreveu outro nome: fica
  assert.equal(nomeAoTrocarLink({ ...base, linkAgora: CARD9, nomeAgora: 'Novo', limpo: true }).nome, 'Novo');
  // mesmo destino (ex.: só mudou o "exemplo"): nada muda
  assert.equal(nomeAoTrocarLink({ ...base, linkAgora: CARD.replace('exemplo', 'outro'), nomeAgora: 'Painel' }).nome, 'Painel');
  // sem nome antes: nada a fazer
  assert.equal(nomeAoTrocarLink({ linkAntes: '', nomeAntes: '', linkAgora: CARD9, nomeAgora: 'X' }).nome, 'X');
});
test('rodada 18: o cartão do Claude Code em Ajustes → Conexões, nos quatro estados', async () => {
  const { htmlConexaoClaude, HOOKS_ANTIGOS, FRASE_CLAUDE } = await import('./ajustes.js');
  assert.equal(FRASE_CLAUDE, 'Com o Claude Code conectado, o timer da pasta conta sozinho enquanto o Claude trabalha.');
  // desligado: "Não conectado" e Conectar
  const desligado = htmlConexaoClaude('desligado', true, 'C:\\Users\\usuario\\.claude\\settings.json');
  assert.match(desligado, /aj-estado "><i><\/i>Não conectado/);
  assert.match(desligado, /class="bt pri" data-aj="ligar-claude">Conectar</);
  assert.ok(desligado.includes(FRASE_CLAUDE));
  assert.ok(desligado.includes('~\\.claude\\settings.json'), 'o caminho abreviado');
  assert.doesNotMatch(desligado, /Users/);
  // ligado: "Conectado" (verde) e Desconectar; o botão de ligar some
  const ligado = htmlConexaoClaude('ligado');
  assert.match(ligado, /aj-estado on"><i><\/i>Conectado/);
  assert.match(ligado, /data-aj="remover-hooks">Desconectar</);
  assert.doesNotMatch(ligado, /ligar-claude/);
  // antigo: "Precisa atualizar" e Atualizar
  const antigo = htmlConexaoClaude('antigo');
  assert.match(antigo, /aj-estado antigo"><i><\/i>Precisa atualizar/);
  assert.match(antigo, /data-aj="atualizar-hooks">Atualizar</);
  assert.ok(antigo.includes(HOOKS_ANTIGOS));
  // sem o Claude Code neste PC: o Conectar desabilitado, com a dica
  const ausente = htmlConexaoClaude('desligado', false, 'C:/u/.claude/settings.json');
  assert.match(ausente, /Claude Code não encontrado neste PC/);
  assert.match(ausente, /data-aj="ligar-claude" disabled title="Instale o Claude Code e volte aqui">Conectar</);
  assert.match(ausente, /Instale o Claude Code e volte aqui\./);
  assert.match(htmlConexaoClaude(null), /Não deu para ler o estado/);
  const { corpoPrevia } = await import('./ajustes.js');
  assert.match(corpoPrevia('x', 's', [], true), /trocar os hooks da versão antiga/);
});
test('rodada 18: o caminho do usuário abreviado no texto, e a prévia exata recolhida', async () => {
  const { abreviarCaminho } = await import('./formato.js');
  assert.equal(abreviarCaminho('C:\\Users\\usuario\\.claude\\settings.json'), '~\\.claude\\settings.json');
  assert.equal(abreviarCaminho('c:/users/Fulano de Tal/.claude/settings.json'), '~/.claude/settings.json');
  assert.equal(abreviarCaminho('D:\\Outro\\settings.json'), 'D:\\Outro\\settings.json');
  const { corpoPrevia } = await import('./ajustes.js');
  const h = corpoPrevia('{"cwd":"C:\\Users\\usuario"}', 'C:\\Users\\usuario\\.claude\\settings.json');
  assert.ok(h.includes('<code class="aj-cam">~\\.claude\\settings.json</code>'));
  assert.match(h, /<details class="mais aj-ver-muda"><summary>Ver o que muda<\/summary>/);
  assert.match(h, /Os caminhos são os do seu usuário neste PC\./);
  // a prévia, exata (com o caminho inteiro)
  assert.ok(h.includes('<pre class="aj-previa" tabindex="0" aria-label="Prévia da mudança">{&quot;cwd&quot;:&quot;C:\\Users\\usuario&quot;}</pre>'));
});
test('rodada 18: "O Claude Code não está conectado · Conectar" nos diálogos de ligar', async () => {
  const { linhaClaudeDesconectado, corpoLigarATimer, corpoPastasDoTimer } = await import('./editar.js');
  assert.match(linhaClaudeDesconectado('desligado'), /O Claude Code não está conectado · <button type="button" class="cx-link" data-conectar-claude>Conectar<\/button>/);
  for (const e of ['ligado', 'antigo', null]) assert.equal(linhaClaudeDesconectado(e), '');
  const sessao = { id: 's1', nome: 'Sessão', cwd: 'D:/x', timer_id: null };
  assert.match(corpoLigarATimer({ grupos: [] }, sessao, false, 'desligado'), /data-conectar-claude/);
  assert.doesNotMatch(corpoLigarATimer({ grupos: [] }, sessao, false, 'ligado'), /data-conectar-claude/);
  assert.match(corpoPastasDoTimer([], 5, null, '', 'desligado'), /<span>Pastas deste timer<\/span><p class="cx-aviso"/);
  // o "+ Ligar uma sessão" também; e o Conectar pede o diálogo à janela principal
  const editar = readFileSync(new URL('./editar.js', import.meta.url), 'utf8');
  assert.match(editar, /corpo: linhaClaudeDesconectado\(estadoClaude\) \+ \(itens \?/);
  assert.match(readFileSync(new URL('./painel.js', import.meta.url), 'utf8'), /addEventListener\('conectar-claude', \(\) => ligarAoClaude\(\)\.then\(verHooks\)\)/);
});
test('Ao minimizar a janela: quatro opções, a atual marcada', async () => {
  const { campoAoMinimizar } = await import('./ajustes.js');
  const h = campoAoMinimizar('dock');
  assert.equal((h.match(/name="ao_minimizar"/g) ?? []).length, 4);
  assert.match(h, /value="dock" checked><span>Abrir o dock/);
  for (const v of ['perguntar', 'bandeja', 'barra']) assert.match(h, new RegExp(`value="${v}"><span>`));
});
test('Minimizar para a bandeja?: explica os dois e traz "Não perguntar de novo"', async () => {
  const { corpoMinimizar } = await import('./editar.js');
  const h = corpoMinimizar();
  assert.match(h, /Na bandeja a janela fecha/);
  assert.match(h, /<input type="checkbox" name="lembrar"><span>Não perguntar de novo/);
});
test('backups: lista com data e tamanho, restaurar e confirmação', async () => {
  const { htmlBackups, corpoRestaurar } = await import('./ajustes.js');
  const b = { nome: '2026-11-18_0930.db', quando: '2026-11-18T09:30', bytes: 49152 };
  const h = htmlBackups([b]);
  assert.match(h, /18\/11\/2026 09:30<\/span><small>48 KB<\/small><button[^>]*data-aj="backup-restaurar" data-nome="2026-11-18_0930\.db">Restaurar/);
  assert.match(h, /data-aj="backup-agora">Fazer backup agora/);
  assert.match(h, /data-aj="backup-pasta">Abrir pasta/);
  assert.match(htmlBackups([]), /Nenhum backup ainda/);
  assert.doesNotMatch(h, /trazer-antigo|versão anterior/, 'a importação da versão anterior saiu (rodada 19)');
  const c = corpoRestaurar(b);
  assert.match(c, /como estava em <b>18\/11\/2026 09:30<\/b>/);
  assert.match(c, /o estado de agora vira um backup novo/);
  assert.match(c, /sem reiniciar/);
});
test('modo desenvolvedor: confirmação em 3 linhas e o risco na seção Tempos', async () => {
  const { TITULO_MODO_DEV, corpoModoDev, AVISO_TEMPOS } = await import('./ajustes.js');
  assert.equal(TITULO_MODO_DEV, 'Modo desenvolvedor: por sua conta e risco');
  const c = corpoModoDev();
  assert.equal((c.match(/<p class="dlg-p">/g) ?? []).length, 3);
  assert.match(c, /horas a mais ou a menos/);
  assert.match(c, /diferentes dos colegas/);
  assert.match(c, /Só ligue se você sabe o que está fazendo/);
  assert.match(AVISO_TEMPOS, /^Por sua conta e risco/);
});
test('rodada 15: conversa nova numa pasta com regra, confirmar sempre por padrão', async () => {
  const { campoConversaNova, CONVERSA_NOVA } = await import('./ajustes.js');
  assert.deepEqual(CONVERSA_NOVA.map((o) => o[1]), ['Confirmar sempre', 'Usar a regra direto']);
  const padrao = campoConversaNova(undefined);
  assert.match(padrao, /Conversa nova numa pasta com regra/);
  assert.match(padrao, /value="confirmar" checked/);
  assert.doesNotMatch(padrao, /value="regra" checked/);
  assert.match(campoConversaNova('regra'), /value="regra" checked/);
});
