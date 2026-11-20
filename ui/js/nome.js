// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
/**
 * Nome de exibição do app (provisório). A constante única é `nucleo::APP_NOME`, no Rust: o app
 * põe o nome em `globalThis.NOME_APP` antes de qualquer script das páginas (`main.rs`), e os
 * testes no Node leem do mesmo arquivo (`teste-nome.js`). Trocar o nome é trocar aquela linha.
 */
export const NOME_APP = globalThis.NOME_APP ?? '';

/** A autoria (rodada 18; a mesma de `nucleo::AUTOR`). */
export const AUTOR = 'Nikolai Cinotti (@eaeniko)';

/** A linha do console do WebView ao abrir a janela principal (rodada 18). */
export const linhaDoConsole = () => `${NOME_APP} · © 2026 ${AUTOR} · eaeniko.okin.club`;

/**
 * O app abriu no perfil de teste (`work-timer.exe --perfil teste`, rodada 17)? O `main.rs` põe
 * `globalThis.PERFIL_TESTE` antes dos scripts, como o nome.
 */
export const PERFIL_TESTE = globalThis.PERFIL_TESTE === true;

/** A marca pequena do perfil de teste (o dock). */
export const marcaDeTeste = (teste = PERFIL_TESTE) => (teste ? '<span class="dteste" title="Perfil de teste">teste</span>' : '');

/** Põe o nome nos lugares fixos do HTML (`data-nome-app`) e no título da página. */
export function nomearPagina(doc, sufixo = '') {
  doc.title = NOME_APP + sufixo;
  for (const el of doc.querySelectorAll('[data-nome-app]')) el.textContent = NOME_APP;
}
