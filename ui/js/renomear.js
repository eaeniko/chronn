// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Renomear no lugar (rodada 24): o "Renomear" do ⋯ do grupo troca o nome do cabeçalho por um
// campo, com o texto selecionado. Enter grava, Esc cancela, o foco saindo do campo grava (a
// janela perdendo o foco, não); nome vazio ou igual volta ao anterior sem gravar. Sem DOM próprio: o campo vem de quem chama (testado com
// node --test, com um EventTarget no lugar do campo).

/** O nome a gravar: o texto sem os espaços das pontas, ou nulo (vazio ou igual ao de antes). */
export function nomeNovo(original, valor) {
  const n = String(valor ?? '').trim();
  return n && n !== original ? n : null;
}

/**
 * Liga as teclas e o foco de um campo de renomear. `fim(nome, comTeclado)` é chamado uma vez só:
 * `nome` é o nome a gravar (nulo: volta ao anterior sem gravar), e `comTeclado` diz se veio do
 * Enter ou do Esc (o foco volta ao botão ⋯) ou do foco saindo (o foco fica onde a pessoa pôs).
 * O campo que sai da página porque a tela se redesenhou não grava: quem chama recria o campo.
 */
export function ligarRenomear(campo, original, fim) {
  let feito = false;
  const acabar = (gravar, comTeclado) => {
    if (feito) return;
    feito = true;
    fim(gravar ? nomeNovo(original, campo.value) : null, comTeclado);
  };
  campo.addEventListener('keydown', (ev) => {
    if (ev.key !== 'Enter' && ev.key !== 'Escape') return;
    ev.preventDefault();
    // o Esc daqui não fecha mais nada (nem o Enter clica em outro lugar)
    ev.stopPropagation();
    acabar(ev.key === 'Enter', true);
  });
  campo.addEventListener('blur', () => {
    if (campo.isConnected === false) return;
    // a janela inteira perdeu o foco (Alt+Tab, outro app): o campo continua, e o foco volta a
    // ele quando a janela voltar (revisão da rodada 24). Só o foco saindo do campo dentro da
    // página termina.
    if (janelaSemFoco()) return;
    acabar(true, false);
  });
  return acabar;
}

/** A janela do app está sem o foco? (No Node, sem `document`, nunca.) */
function janelaSemFoco(doc = globalThis.document) {
  return typeof doc?.hasFocus === 'function' && !doc.hasFocus();
}

/** Quanto tempo depois do `mousedown` o `blur` ainda é do mesmo clique (ms). */
export const JANELA_DO_CLIQUE = 150;
/** O limite para esperar o clique terminar (o botão segurado, um arrasto): depois, segue. */
export const ESPERA_MAX = 3000;

/**
 * Espera o clique que tirou o foco do campo terminar, antes de gravar e redesenhar (revisão da
 * rodada 24). O `blur` vem no `mousedown`; se a tela se redesenhasse ali, o elemento do clique
 * (um ▶, um cartão) sairia da página antes do `mouseup`, e o `click` não chegaria a ninguém.
 *
 * `ultimoMousedown`: o instante (`agora()`) do último `mousedown` na página. Sem um recente (o
 * foco saiu pelo Tab), resolve já. Com ele, resolve logo depois do `click` (as ações da página
 * rodam antes), ou logo depois do `mouseup` sem `click`, ou no `ESPERA_MAX`.
 */
export function depoisDoClique(doc, ultimoMousedown, agora = () => performance.now()) {
  if (!(agora() - ultimoMousedown <= JANELA_DO_CLIQUE)) return Promise.resolve();
  return new Promise((ok) => {
    let feito = false;
    const fim = () => {
      if (feito) return;
      feito = true;
      clearTimeout(limite);
      doc.removeEventListener('click', aoClicar, true);
      doc.removeEventListener('mouseup', aoSoltar, true);
      ok();
    };
    const aoClicar = () => setTimeout(fim, 0);
    // o `click` vem logo depois do `mouseup`; sem ele (soltou fora do alvo), segue assim mesmo
    const aoSoltar = () => setTimeout(fim, 50);
    const limite = setTimeout(fim, ESPERA_MAX);
    doc.addEventListener('click', aoClicar, true);
    doc.addEventListener('mouseup', aoSoltar, true);
  });
}
