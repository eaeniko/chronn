// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Ponte com o núcleo (Tauri). Comandos recebem argumentos em snake_case.

const tauri = () => window.__TAURI__;

/** Chama um comando do núcleo: chamar('play', { timer_id: 1 }). */
export function chamar(nome, args = {}) {
  return tauri().core.invoke(nome, args);
}

/**
 * Chama `fn(snapshot)` agora e a cada evento "estado-mudou".
 * Eventos que chegam durante uma leitura viram uma única leitura a mais.
 * `aoErro(e)` recebe a leitura que falhou (ex.: o banco não abriu); a próxima que der certo
 * chama `aoErro(null)`.
 */
export async function aoMudar(fn, aoErro = () => {}) {
  let lendo = false;
  let pendente = false;
  const ler = async () => {
    if (lendo) {
      pendente = true;
      return;
    }
    lendo = true;
    try {
      do {
        pendente = false;
        fn(await chamar('obter_estado'));
        aoErro(null);
      } while (pendente);
    } catch (e) {
      console.error('worktimer: obter_estado falhou', e);
      aoErro(e);
    } finally {
      lendo = false;
    }
  };
  const parar = await tauri().event.listen('estado-mudou', ler);
  await ler();
  return parar;
}

/** Ouve um evento do núcleo: `fn(payload)`. Devolve (numa promessa) a função que para de ouvir. */
export const ouvir = (nome, fn) => tauri().event.listen(nome, (e) => fn(e.payload));

/** Iniciar com o Windows (pelo núcleo; no perfil de teste, só a config): o estado real. */
export const iniciarComWindows = {
  // pelo núcleo (rodada 17): no perfil normal, o registro;
  // no de teste, só a config do perfil. A página não chama o plugin do autostart direto.
  ligado: () => chamar('inicio_windows', { ligar: null }),
  ligar: () => chamar('inicio_windows', { ligar: true }),
  desligar: () => chamar('inicio_windows', { ligar: false }),
};
