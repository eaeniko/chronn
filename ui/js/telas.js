// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Rodada 22 (0.3.1): as telas num menu suspenso, no começo da linha de baixo do cabeçalho
// ("Painel ▾"), e a navegação pelas setas nos menus pequenos.
import { esc } from './componentes.js';
import { TEXTOS } from './textos.js';

/** As telas do menu, na ordem. `atalho`: a tecla, se houver (hoje nenhuma tem). */
export const ITENS_DAS_TELAS = [
  { id: 'painel', rotulo: TEXTOS.telaPainel, atalho: null },
  { id: 'foco', rotulo: TEXTOS.telaFoco, atalho: null },
  { id: 'cartoes', rotulo: TEXTOS.telaCartoes, atalho: null },
  { id: 'linha', rotulo: TEXTOS.linhaDoTempo, atalho: null },
];

/** O nome da tela no botão do menu (nos Ajustes, "Ajustes"). */
export function rotuloDaTela(visao) {
  if (visao === 'ajustes') return TEXTOS.telaAjustes;
  return (ITENS_DAS_TELAS.find((t) => t.id === visao) ?? ITENS_DAS_TELAS[0]).rotulo;
}

/** O menu das telas, com a atual marcada (✓ e `aria-checked`). */
export function htmlDoMenuTelas(visao) {
  const itens = ITENS_DAS_TELAS.map((t) => {
    const on = t.id === visao;
    const atalho = t.atalho ? `<kbd>${esc(t.atalho)}</kbd>` : '';
    return `<button type="button" role="menuitemradio" aria-checked="${on}" data-visao="${t.id}"${on ? ' class="on"' : ''}><span class="mt-ok" aria-hidden="true">${on ? '✓' : ''}</span><span class="mt-nome">${esc(t.rotulo)}</span>${atalho}</button>`;
  }).join('');
  return `<div class="menu-ajuda menu-telas" role="menu" aria-label="${esc(TEXTOS.telasRotulo)}">${itens}</div>`;
}

/**
 * Para onde vai o foco num menu com `n` opções, a partir da `i` (-1: nenhuma), pela `tecla`:
 * ↓ e ↑ andam e dão a volta; Home e End vão às pontas. Outra tecla: nulo.
 */
export function proximoNoMenu(n, i, tecla) {
  if (n <= 0) return null;
  if (tecla === 'ArrowDown') return i < 0 ? 0 : (i + 1) % n;
  if (tecla === 'ArrowUp') return i < 0 ? n - 1 : (i - 1 + n) % n;
  if (tecla === 'Home') return 0;
  if (tecla === 'End') return n - 1;
  return null;
}
