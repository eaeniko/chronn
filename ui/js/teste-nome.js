// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
// Testes no Node (`node --import ./js/teste-nome.js --test`): o nome de exibição vem da mesma
// constante que o app usa (`nucleo::APP_NOME`), como o Rust faz nas páginas.
import { readFileSync } from 'node:fs';

const fonte = readFileSync(new URL('../../crates/nucleo/src/lib.rs', import.meta.url), 'utf8');
const achado = /pub const APP_NOME: &str = "([^"]+)";/.exec(fonte);
if (!achado) throw new Error('APP_NOME não encontrado em crates/nucleo/src/lib.rs');
globalThis.NOME_APP = achado[1];
