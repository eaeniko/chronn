// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// O `latest.json` do atualizador (rodada 25): o arquivo que o app lê na última release do GitHub
// (`releases/latest/download/latest.json`) para saber se há versão nova. Sobe na release junto do
// instalador. O `build.ps1` chama a linha de comando; as funções ficam testáveis no Node.
//
// Antes de gravar, confere a assinatura contra a chave pública do app (`plugins.updater.pubkey`
// do `tauri.conf.json`): o `tauri-cli` assina com qualquer chave e só avisa no log quando ela não é
// o par da pública, e uma release assim nunca seria aceita pelas cópias instaladas.
//
// Uso: node latest-json.mjs <versão> <instalador.exe.sig> <saída.json> [CHANGELOG.md]
import { readFileSync, writeFileSync, existsSync, realpathSync } from 'node:fs';
import { fileURLToPath } from 'node:url';

/** O repositório público, onde ficam as releases. */
export const REPO = 'eaeniko/chronn';
/** O nome de exibição no arquivo do instalador (o `nucleo::APP_NOME`). */
export const NOME = 'Chronn';
/** A plataforma que o app procura no `latest.json` (Windows 64 bits). */
export const PLATAFORMA = 'windows-x86_64';

/** O endereço do instalador da versão na release: `…/download/v0.4.1/Chronn_0.4.1_x64-setup.exe`. */
export const urlDoInstalador = (versao) =>
  `https://github.com/${REPO}/releases/download/v${versao}/${NOME}_${versao}_x64-setup.exe`;

/**
 * A entrada da versão no CHANGELOG (o texto entre o título dela e o próximo título de versão),
 * sem espaços nas pontas; nula se não houver. Aceita "## 0.4.1", "## [0.4.1]", "## v0.4.1" e
 * um resto no título ("## 0.4.1 - 2026-11-20").
 */
export function notasDoChangelog(changelog, versao) {
  const linhas = String(changelog ?? '').replace(/\r\n/g, '\n').split('\n');
  const esc = versao.replace(/\./g, '\\.');
  const titulo = new RegExp(`^#{1,3}\\s*\\[?v?${esc}\\]?(\\s|$|[^\\d.])`);
  const i = linhas.findIndex((l) => titulo.test(l));
  if (i < 0) return null;
  const fim = linhas.findIndex((l, j) => j > i && /^#{1,3}\s*\[?v?\d+\.\d+/.test(l));
  const texto = linhas.slice(i + 1, fim < 0 ? undefined : fim).join('\n').trim();
  return texto || null;
}

/**
 * As linhas de um texto do minisign. A chave pública e o `.sig` do Tauri são base64 de um texto
 * do minisign: "untrusted comment: …", a linha com a chave ou a assinatura (base64 de novo) e, só
 * na assinatura, "trusted comment: …" e a assinatura global.
 */
function linhasDoMinisign(b64, oque) {
  const texto = Buffer.from(String(b64 ?? '').trim(), 'base64').toString('utf8');
  const linhas = texto.replace(/\r\n/g, '\n').split('\n');
  if (!linhas[0]?.startsWith('untrusted comment:') || !linhas[1]?.trim()) throw new Error(`${oque} ilegível: não é do minisign`);
  return linhas;
}

/**
 * O número da chave (o `keynum` do minisign, 8 bytes em hexadecimal) de uma chave pública ou de
 * uma assinatura em base64: os bytes 2 a 9 da segunda linha (os dois primeiros são o algoritmo).
 */
export function numeroDaChave(b64, oque = 'chave') {
  const bytes = Buffer.from(linhasDoMinisign(b64, oque)[1].trim(), 'base64');
  if (bytes.length < 10) throw new Error(`${oque} ilegível: curta demais`);
  return bytes.subarray(2, 10).toString('hex');
}

/**
 * A versão que a assinatura leva no comentário confiável (`…\tversion:0.4.1`, que o `tauri-cli`
 * grava e o `requireSignedVersion` do app exige), ou nula.
 */
export function versaoAssinada(assinatura) {
  const confiavel = linhasDoMinisign(assinatura, 'assinatura').find((l) => l.startsWith('trusted comment:'));
  if (!confiavel) return null;
  const campo = confiavel.slice('trusted comment:'.length).trim().split('\t').find((c) => c.startsWith('version:'));
  return campo ? campo.slice('version:'.length) : null;
}

/**
 * Para a build se a assinatura não vale para o app: feita com outra chave que não o par da
 * `pubkey`, ou sem a versão (ou com outra) no comentário confiável.
 */
export function conferirAssinatura(assinatura, pubkey, versao) {
  const daAssinatura = numeroDaChave(assinatura, 'assinatura');
  const doApp = numeroDaChave(pubkey, 'chave pública');
  if (daAssinatura !== doApp) {
    throw new Error(
      `a assinatura não é da chave pública do app (chave ${daAssinatura}, o app espera ${doApp}): ` +
        'a chave privada em TAURI_SIGNING_PRIVATE_KEY não é o par da pubkey do tauri.conf.json',
    );
  }
  const assinada = versaoAssinada(assinatura);
  if (assinada !== versao) {
    throw new Error(`a assinatura é da versão ${assinada ?? '(nenhuma)'}, não da ${versao}: o app recusaria a atualização`);
  }
}

/** O texto curto quando o CHANGELOG não tem a versão. */
export const notasPadrao = (versao) => `${NOME} ${versao}`;

/** A data no formato que o atualizador lê (RFC 3339, sem os milissegundos). */
export const dataRfc3339 = (data) => data.toISOString().replace(/\.\d{3}Z$/, 'Z');

/**
 * O `latest.json`: a versão, as notas, a data e, para o Windows, a assinatura (o conteúdo do
 * `.sig` que a build gera ao lado do instalador) e o endereço do instalador.
 */
export function montarLatestJson({ versao, notas, data, assinatura }) {
  if (!/^\d+\.\d+\.\d+$/.test(versao)) throw new Error(`versão inválida: ${versao}`);
  const sig = String(assinatura ?? '').trim();
  if (!sig) throw new Error('assinatura vazia: a build gerou o .sig?');
  return {
    version: versao,
    notes: notas || notasPadrao(versao),
    pub_date: dataRfc3339(data),
    platforms: {
      [PLATAFORMA]: { signature: sig, url: urlDoInstalador(versao) },
    },
  };
}

/** O `tauri.conf.json` do app, ao lado deste script. */
export const CONF = fileURLToPath(new URL('../src-tauri/tauri.conf.json', import.meta.url));

/** A chave pública do atualizador no `tauri.conf.json` (`plugins.updater.pubkey`). */
export function chavePublicaDoApp(conf = CONF) {
  const pubkey = JSON.parse(readFileSync(conf, 'utf8'))?.plugins?.updater?.pubkey;
  if (!pubkey) throw new Error(`pubkey ausente em ${conf}`);
  return pubkey;
}

/**
 * Lê o `.sig` e o CHANGELOG (se pedido), confere a assinatura contra a `pubkey` e grava o
 * `latest.json`. Devolve o objeto gravado. O CHANGELOG pedido e ausente é erro; sem a entrada da
 * versão, as notas ficam no texto curto, com um aviso.
 */
export function gerar({ versao, sig, saida, pubkey, changelog = null, data = new Date(), avisar = console.warn }) {
  const assinatura = readFileSync(sig, 'utf8');
  conferirAssinatura(assinatura, pubkey, versao);
  let notas = null;
  if (changelog) {
    if (!existsSync(changelog)) throw new Error(`CHANGELOG não encontrado: ${changelog}`);
    notas = notasDoChangelog(readFileSync(changelog, 'utf8'), versao);
    if (!notas) avisar(`aviso: o CHANGELOG não tem a entrada da ${versao}; as notas ficam "${notasPadrao(versao)}"`);
  }
  const json = montarLatestJson({ versao, notas, data, assinatura });
  writeFileSync(saida, `${JSON.stringify(json, null, 2)}\n`);
  return json;
}

/** Este arquivo foi chamado pela linha de comando? Pelo caminho real (junção e `subst` contam). */
export function eALinhaDeComando(argv1 = process.argv[1], url = import.meta.url) {
  if (!argv1) return false;
  try {
    return realpathSync.native(fileURLToPath(url)) === realpathSync.native(argv1);
  } catch {
    return false;
  }
}

if (eALinhaDeComando()) {
  const [versao, sig, saida, changelog] = process.argv.slice(2);
  if (!versao || !sig || !saida) {
    console.error('uso: node latest-json.mjs <versão> <instalador.exe.sig> <saída.json> [CHANGELOG.md]');
    process.exit(2);
  }
  try {
    gerar({ versao, sig, saida, pubkey: chavePublicaDoApp(), changelog: changelog ?? null });
  } catch (e) {
    console.error(`latest.json: ${e.message}`);
    process.exit(1);
  }
  console.log(saida);
}
