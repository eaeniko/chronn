// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
/// Nome de exibição do app (provisório): janelas, bandeja, dock, Sobre, textos e o instalador
/// (título, textos e atalhos). A constante única: o Rust usa direto, as páginas recebem em
/// `globalThis.NOME_APP` antes de qualquer script (`main.rs`) e o `build.rs` põe no modelo do
/// instalador. Trocar o nome é trocar esta linha.
pub const APP_NOME: &str = "Chronn";

/// A autoria (rodada 18): o "Sobre" dos Ajustes, a bandeja e os metadados.
pub const AUTOR: &str = "Nikolai Cinotti (@eaeniko)";
/// O site do autor (o `@eaeniko` dos créditos) e o código.
pub const LINK_AUTOR: &str = "https://eaeniko.okin.club";
pub const LINK_CODIGO: &str = "https://github.com/eaeniko/chronn";
/// Marca única pesquisável (rodada 18): acha cópias do código pela busca. Usada de verdade no
/// "Sobre" (os Ajustes) e no log de abertura.
pub const ASSINATURA: &str = "chronn-eaeniko-7f3a9c";
/// O `PRAGMA application_id` do banco: "CHRN" em ASCII (rodada 18). Não muda o esquema.
pub const APPLICATION_ID: i32 = 0x4348_524E;

/// Nome interno, o `productName` do `tauri.conf.json`: pasta de instalação, chaves do instalador
/// no registro e a entrada do "Iniciar com o Windows". Fica até o lançamento, para o instalador
/// atualizar a instalação existente no mesmo lugar. Os executáveis (`work-timer.exe`,
/// `work-timer-hook.exe`), o identificador e a pasta de dados (`fila::caminhos::PASTA_DADOS`)
/// também ficam.
pub const NOME_INTERNO: &str = "Work Timer";

pub mod claude_registros;
pub mod hooks_claude;
pub mod banco;
pub mod config;
pub mod dados;
pub mod dicas;
pub mod fim_do_transcript;
pub mod modelo;
pub mod pastas;
pub mod recentes;
pub mod motor;
pub mod relatorio;
pub mod snapshot;
pub mod tempo;
