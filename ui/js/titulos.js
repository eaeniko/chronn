// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// As dicas nativas (`title`) quebradas em linhas curtas nas três páginas (rodada 19): um
// observador reescreve todo `title` longo pela versão quebrada, inclusive os que entram depois.
import { quebrarDica } from './formato.js';

/** Acima disto, a dica é quebrada em linhas. */
export const LIMITE_DICA = 60;

/** A dica como fica na tela: quebrada se passa do limite. */
export const dicaQuebrada = (texto) => (texto.length > LIMITE_DICA ? quebrarDica(texto, LIMITE_DICA) : texto);

/**
 * Quebra o `title` de `el`, se for longo. Só escreve quando muda: o texto quebrado passa igual
 * pela `quebrarDica`, então o observador não entra em laço. Devolve se escreveu.
 */
export function quebrarTitulo(el) {
  const t = el.getAttribute?.('title');
  if (!t || t.length <= LIMITE_DICA) return false;
  const q = quebrarDica(t, LIMITE_DICA);
  if (q === t) return false;
  el.setAttribute('title', q);
  return true;
}

/** `quebrarTitulo` em `raiz` e em tudo dentro dela com `title`. */
export function quebrarTitulos(raiz) {
  let n = quebrarTitulo(raiz) ? 1 : 0;
  for (const el of raiz.querySelectorAll?.('[title]') ?? []) if (quebrarTitulo(el)) n++;
  return n;
}

/**
 * Põe o `title` de `el` (como `el.title = texto`), já quebrado, só se mudou: os laços de cada
 * segundo comparam com o texto quebrado e não reescrevem a dica a cada volta.
 */
export function trocarTitulo(el, texto) {
  const q = dicaQuebrada(texto);
  if (el.title !== q) el.title = q;
}

/**
 * Liga o observador em `raiz` (a página toda): quebra os `title` que já estão e os que entram ou
 * mudam depois. `Observador` é o `MutationObserver` (trocável nos testes). Devolve o observador.
 */
export function vigiarTitulos(raiz = globalThis.document?.documentElement, Observador = globalThis.MutationObserver) {
  if (!raiz || !Observador) return null;
  quebrarTitulos(raiz);
  const obs = new Observador((registros) => {
    for (const r of registros) {
      if (r.type === 'attributes') quebrarTitulo(r.target);
      else for (const n of r.addedNodes ?? []) if (n.nodeType === 1) quebrarTitulos(n);
    }
  });
  obs.observe(raiz, { subtree: true, childList: true, attributes: true, attributeFilter: ['title'] });
  return obs;
}
