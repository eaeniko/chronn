// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 25: o `latest.json` do atualizador (os campos, a URL com a versão, a assinatura lida do
// `.sig` e as notas do CHANGELOG). Revisão da rodada 25: a assinatura conferida contra a chave
// pública do app e a versão assinada, o CHANGELOG ausente e a linha de comando por uma junção.
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, writeFileSync, readFileSync, rmSync, rmdirSync, existsSync, symlinkSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';
import {
  urlDoInstalador,
  notasDoChangelog,
  montarLatestJson,
  gerar,
  dataRfc3339,
  numeroDaChave,
  versaoAssinada,
  conferirAssinatura,
  chavePublicaDoApp,
  eALinhaDeComando,
} from './latest-json.mjs';

const DATA = new Date(Date.UTC(2026, 10, 20, 15, 4, 5, 678));
const SCRIPT = fileURLToPath(new URL('./latest-json.mjs', import.meta.url));

// Chaves e assinaturas de exemplo no formato do minisign (base64 de um texto cuja segunda linha é
// base64 do algoritmo, do número da chave e dos bytes). Os bytes da chave e da assinatura são de
// mentira: o script só lê o número da chave e o comentário confiável.
/** O número da chave pública do app (o `tauri.conf.json`), como os bytes 2 a 9 a guardam. */
const DO_APP = 'aa7e4a8970e90cac';
const OUTRA = '0102030405060708';
const b64 = (s) => Buffer.from(s, 'utf8').toString('base64');
const caixa = (alg, keynum, n) => Buffer.concat([Buffer.from(alg), Buffer.from(keynum, 'hex'), Buffer.alloc(n, 7)]).toString('base64');
const pubkeyDe = (keynum) =>
  b64(`untrusted comment: minisign public key: ${Buffer.from(keynum, 'hex').reverse().toString('hex').toUpperCase()}\n${caixa('Ed', keynum, 32)}\n`);
const sigDe = (keynum, versao) =>
  b64(
    `untrusted comment: signature from tauri secret key\n${caixa('ED', keynum, 64)}\n` +
      `trusted comment: timestamp:1763650000\tfile:Work Timer_${versao ?? '0.4.1'}_x64-setup.exe${versao ? `\tversion:${versao}` : ''}\n` +
      `${Buffer.alloc(64, 9).toString('base64')}\n`,
  );

test('rodada 25: a URL do instalador leva a versão no caminho da release e no nome do arquivo', () => {
  assert.equal(urlDoInstalador('0.4.1'), 'https://github.com/eaeniko/chronn/releases/download/v0.4.1/Chronn_0.4.1_x64-setup.exe');
});

test('rodada 25: o latest.json tem a versão, as notas, a data e o Windows com a assinatura e a URL', () => {
  const j = montarLatestJson({ versao: '0.4.1', notas: 'Fixes.', data: DATA, assinatura: '  c2lnbmF0dXJl\n' });
  assert.deepEqual(j, {
    version: '0.4.1',
    notes: 'Fixes.',
    pub_date: '2026-11-20T15:04:05Z',
    platforms: {
      'windows-x86_64': {
        signature: 'c2lnbmF0dXJl',
        url: 'https://github.com/eaeniko/chronn/releases/download/v0.4.1/Chronn_0.4.1_x64-setup.exe',
      },
    },
  });
  assert.equal(dataRfc3339(DATA), '2026-11-20T15:04:05Z');
  // sem notas, um texto curto
  assert.equal(montarLatestJson({ versao: '0.4.1', notas: null, data: DATA, assinatura: 'x' }).notes, 'Chronn 0.4.1');
  // sem assinatura ou com a versão errada, não gera
  assert.throws(() => montarLatestJson({ versao: '0.4.1', notas: '', data: DATA, assinatura: ' ' }), /assinatura vazia/);
  assert.throws(() => montarLatestJson({ versao: '0.4.1-alpha', notas: '', data: DATA, assinatura: 'x' }), /versão inválida/);
});

test('rodada 25: as notas vêm da entrada da versão no CHANGELOG', () => {
  const ch = '# Changelog\r\n\r\n## 0.4.1 - 2026-11-21\r\n\r\n- Fix A.\r\n- Fix B.\r\n\r\n## [0.4.0]\n\n- English.\n\n## 0.3.1\n- Header.\n';
  assert.equal(notasDoChangelog(ch, '0.4.1'), '- Fix A.\n- Fix B.');
  assert.equal(notasDoChangelog(ch, '0.4.0'), '- English.');
  assert.equal(notasDoChangelog(ch, '0.3.1'), '- Header.');
  assert.equal(notasDoChangelog(ch, '0.4'), null, 'não casa um prefixo da versão');
  assert.equal(notasDoChangelog(ch, '0.4.10'), null);
  assert.equal(notasDoChangelog('', '0.4.1'), null);
});

test('revisão da rodada 25: o número da chave lido da pubkey do app e do .sig, e a versão assinada', () => {
  // a pubkey real do tauri.conf.json: o número bate com o do comentário (que o minisign escreve
  // de trás para frente)
  const pubkey = chavePublicaDoApp();
  assert.equal(numeroDaChave(pubkey), DO_APP);
  const comentario = Buffer.from(pubkey, 'base64').toString('utf8').split('\n')[0];
  assert.ok(comentario.endsWith(Buffer.from(DO_APP, 'hex').reverse().toString('hex').toUpperCase()), comentario);
  assert.equal(numeroDaChave(pubkeyDe(OUTRA)), OUTRA);
  assert.equal(numeroDaChave(sigDe(DO_APP, '0.4.1')), DO_APP);
  assert.equal(versaoAssinada(sigDe(DO_APP, '0.4.1')), '0.4.1');
  assert.equal(versaoAssinada(sigDe(DO_APP, null)), null, 'assinatura de antes, sem a versão');
  // o que não é do minisign não passa calado
  assert.throws(() => numeroDaChave(b64('qualquer coisa')), /ilegível/);
  assert.throws(() => numeroDaChave('c2lnbmF0dXJl', 'assinatura'), /assinatura ilegível/);
});

test('revisão da rodada 25: a build para se a chave privada não é o par da pubkey do app', () => {
  const pubkey = chavePublicaDoApp();
  assert.doesNotThrow(() => conferirAssinatura(sigDe(DO_APP, '0.4.1'), pubkey, '0.4.1'));
  // outra chave: o tauri-cli só avisa no log; aqui a build para
  assert.throws(() => conferirAssinatura(sigDe(OUTRA, '0.4.1'), pubkey, '0.4.1'), /não é da chave pública do app \(chave 0102030405060708, o app espera aa7e4a8970e90cac\)/);
  assert.throws(() => conferirAssinatura(sigDe(DO_APP, '0.4.1'), pubkeyDe(OUTRA), '0.4.1'), /não é da chave pública do app/);
  // o app exige a versão assinada (requireSignedVersion): sem ela, ou com outra, também para
  assert.throws(() => conferirAssinatura(sigDe(DO_APP, null), pubkey, '0.4.1'), /da versão \(nenhuma\), não da 0\.4\.1/);
  assert.throws(() => conferirAssinatura(sigDe(DO_APP, '0.4.0'), pubkey, '0.4.1'), /da versão 0\.4\.0, não da 0\.4\.1/);
});

test('rodada 25: gerar lê o .sig e o CHANGELOG, confere a assinatura e grava o latest.json', () => {
  const d = mkdtempSync(join(tmpdir(), 'chronn-latest-'));
  try {
    const sig = join(d, 'Chronn_0.4.1_x64-setup.exe.sig');
    const ch = join(d, 'CHANGELOG.md');
    const saida = join(d, 'latest.json');
    const pubkey = pubkeyDe(DO_APP);
    const assinatura = sigDe(DO_APP, '0.4.1');
    writeFileSync(sig, `${assinatura}\n`);
    writeFileSync(ch, '## 0.4.1\n\n- Updates from the app.\n');
    const avisos = [];
    const avisar = (m) => avisos.push(m);
    gerar({ versao: '0.4.1', sig, saida, pubkey, changelog: ch, data: DATA, avisar });
    const j = JSON.parse(readFileSync(saida, 'utf8'));
    assert.equal(j.platforms['windows-x86_64'].signature, assinatura);
    assert.equal(j.notes, '- Updates from the app.');
    assert.equal(j.version, '0.4.1');
    assert.deepEqual(avisos, []);
    // sem CHANGELOG pedido, o texto curto, sem aviso
    gerar({ versao: '0.4.1', sig, saida, pubkey, data: DATA, avisar });
    assert.equal(JSON.parse(readFileSync(saida, 'utf8')).notes, 'Chronn 0.4.1');
    assert.deepEqual(avisos, []);
    // o CHANGELOG sem a entrada da versão: o texto curto, com um aviso
    writeFileSync(ch, '## 0.4.0\n\n- Old.\n');
    gerar({ versao: '0.4.1', sig, saida, pubkey, changelog: ch, data: DATA, avisar });
    assert.equal(JSON.parse(readFileSync(saida, 'utf8')).notes, 'Chronn 0.4.1');
    assert.match(avisos.join('\n'), /não tem a entrada da 0\.4\.1/);
    // o CHANGELOG pedido e ausente é erro, e nada é gravado
    rmSync(saida);
    assert.throws(() => gerar({ versao: '0.4.1', sig, saida, pubkey, changelog: join(d, 'nao-existe.md'), data: DATA, avisar }), /CHANGELOG não encontrado/);
    assert.equal(existsSync(saida), false);
    // assinado por outra chave: erro, e nada é gravado
    assert.throws(() => gerar({ versao: '0.4.1', sig, saida, pubkey: pubkeyDe(OUTRA), data: DATA, avisar }), /não é da chave pública do app/);
    assert.equal(existsSync(saida), false);
  } finally {
    rmSync(d, { recursive: true, force: true });
  }
});

test('revisão da rodada 25: a linha de comando roda também por uma junção, e para com a chave errada', () => {
  const url = pathToFileURL(SCRIPT).href;
  assert.equal(eALinhaDeComando(SCRIPT, url), true);
  assert.equal(eALinhaDeComando(join(dirname(SCRIPT), 'latest-json.test.mjs'), url), false);
  assert.equal(eALinhaDeComando(undefined, url), false);
  assert.equal(eALinhaDeComando(join(dirname(SCRIPT), 'nao-existe.mjs'), url), false);
  const d = mkdtempSync(join(tmpdir(), 'chronn-latest-cli-'));
  const juncao = join(d, 'scripts');
  let temJuncao = true;
  try {
    // pela junção (como a pasta antiga do repositório), o caminho real é o mesmo
    try {
      symlinkSync(dirname(SCRIPT), juncao, 'junction');
    } catch {
      temJuncao = false;
    }
    if (temJuncao) assert.equal(eALinhaDeComando(join(juncao, 'latest-json.mjs'), url), true);
    // a linha de comando de verdade: com a chave do app grava; com outra, sai com erro sem gravar
    const sig = join(d, 'a.exe.sig');
    const saida = join(d, 'latest.json');
    const rodar = () =>
      spawnSync(process.execPath, [temJuncao ? join(juncao, 'latest-json.mjs') : SCRIPT, '0.4.1', sig, saida], { encoding: 'utf8' });
    writeFileSync(sig, sigDe(DO_APP, '0.4.1'));
    let r = rodar();
    assert.equal(r.status, 0, r.stderr);
    assert.equal(JSON.parse(readFileSync(saida, 'utf8')).version, '0.4.1');
    rmSync(saida);
    writeFileSync(sig, sigDe(OUTRA, '0.4.1'));
    r = rodar();
    assert.equal(r.status, 1);
    assert.match(r.stderr, /não é da chave pública do app/);
    assert.equal(existsSync(saida), false);
  } finally {
    // a junção sai antes (só o link: a pasta dos scripts fica)
    if (temJuncao) rmdirSync(juncao);
    rmSync(d, { recursive: true, force: true });
  }
});
