// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// A janela dos avisos do dock (rodada 15): mostra a faixa, mede, e o núcleo põe a janela do
// tamanho dela, logo abaixo do dock (ou acima), sem pegar o foco. O botão ("Desfazer") avisa o
// dock, que faz o que ele faz.
import { chamar } from './api.js';
import { htmlDoAviso } from './formato.js';
import { vigiarTitulos } from './titulos.js';

// as dicas longas em linhas curtas (rodada 19)
vigiarTitulos();

const raiz = document.getElementById('aviso');
let atual = null;

async function mostrar(aviso) {
  if (!aviso) return;
  atual = aviso;
  if (aviso.tema === 'claro' || aviso.tema === 'escuro') document.documentElement.dataset.tema = aviso.tema;
  // o texto já vem no idioma do dock; o `lang` acompanha (rodada 23)
  if (aviso.lang) document.documentElement.lang = aviso.lang;
  raiz.innerHTML = htmlDoAviso(aviso);
  // mede depois do desenho (as fontes já carregadas)
  await document.fonts?.ready;
  const r = raiz.firstElementChild.getBoundingClientRect();
  if (atual !== aviso) return;
  chamar('aviso_tamanho', { id: aviso.id, larg: Math.ceil(r.width), alt: Math.ceil(r.height) }).catch((e) =>
    console.error('worktimer: aviso_tamanho', e),
  );
}

raiz.addEventListener('click', (ev) => {
  const b = ev.target.closest('button');
  if (!b || !atual) return;
  b.disabled = true;
  chamar('aviso_clicado', { id: atual.id, qual: Number(b.dataset.qual || 1) }).catch((e) => console.error('worktimer: aviso_clicado', e));
});

window.__TAURI__.event.listen('aviso-mostrar', (ev) => mostrar(ev.payload));
// o aviso pode ter chegado antes da página
chamar('aviso_atual').then(mostrar, () => {});
