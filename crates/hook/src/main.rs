// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use std::io::{Read, Write};

fn executar() -> Option<()> {
    let mut buf = Vec::new();
    std::io::stdin().take(1_000_000).read_to_end(&mut buf).ok()?;
    let entrada: serde_json::Value = serde_json::from_slice(&buf).ok()?;
    let ignoradas: Vec<String> = std::fs::read(fila::caminhos::hook_json())
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| {
            v.get("pastas_ignoradas")?
                .as_array()
                .map(|a| a.iter().filter_map(|s| s.as_str().map(str::to_string)).collect())
        })
        .unwrap_or_default();
    let agora = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_millis() as i64;
    let linha = fila::linha_do_evento(&entrada, agora, &ignoradas)?;
    let caminho = fila::caminhos::fila();
    std::fs::create_dir_all(caminho.parent()?).ok()?;
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(caminho)
        .ok()?;
    f.write_all(linha.as_bytes()).ok()?;
    Some(())
}

fn main() {
    let _ = std::panic::catch_unwind(executar);
    std::process::exit(0);
}
