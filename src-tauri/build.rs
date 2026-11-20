// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use std::path::Path;

/// O nome de exibição (a constante única `nucleo::APP_NOME`) entra no modelo do instalador:
/// `nsis/instalador.nsi` vira `gen/nsis/instalador.nsi`, que o `tauri.conf.json` aponta.
fn main() {
    gerar_instalador();
    tauri_build::build()
}

fn gerar_instalador() {
    let lib = "../crates/nucleo/src/lib.rs";
    let modelo = "nsis/instalador.nsi";
    println!("cargo:rerun-if-changed={lib}");
    println!("cargo:rerun-if-changed={modelo}");
    let fonte = std::fs::read_to_string(lib).expect("ler o nucleo/src/lib.rs");
    let nome = nome_de_exibicao(&fonte).expect("APP_NOME em nucleo/src/lib.rs");
    // vai entre aspas no NSIS: só letras, números, espaço e pontuação simples
    assert!(
        !nome.is_empty() && nome.chars().all(|c| c.is_alphanumeric() || " -_.()".contains(c)),
        "APP_NOME com caractere que o instalador não aceita: {nome:?}"
    );
    let texto = std::fs::read_to_string(modelo).expect("ler o modelo do instalador");
    assert!(texto.contains("__NOME_APP__"), "o modelo do instalador perdeu o __NOME_APP__");
    let gerado = texto.replace("__NOME_APP__", &nome);
    let destino = Path::new("gen/nsis/instalador.nsi");
    if std::fs::read_to_string(destino).ok().as_deref() != Some(gerado.as_str()) {
        std::fs::create_dir_all(destino.parent().expect("pasta")).expect("criar gen/nsis");
        std::fs::write(destino, gerado).expect("gravar gen/nsis/instalador.nsi");
    }
}

/// O texto de `pub const APP_NOME: &str = "...";`.
fn nome_de_exibicao(fonte: &str) -> Option<String> {
    let linha = fonte.lines().find(|l| l.trim_start().starts_with("pub const APP_NOME: &str = \""))?;
    let depois = &linha[linha.find('"')? + 1..];
    Some(depois[..depois.find('"')?].to_string())
}
