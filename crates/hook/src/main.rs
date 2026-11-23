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
    // de onde a sessão veio (o app do Claude, o terminal, o VS Code…): o hook herda a variável do
    // Claude Code (rodada 27)
    let origem = std::env::var("CLAUDE_CODE_ENTRYPOINT").ok();
    // a conta do Claude (rodada 28): só o resumo sai daqui (`fila::resumo_da_conta`)
    let conta = std::env::var("CLAUDE_CODE_ACCOUNT_UUID").ok();
    let linha = fila::linha_do_evento(&entrada, agora, &ignoradas, origem.as_deref(), conta.as_deref())?;
    let caminho = fila::caminhos::fila();
    // revisão M1: no Windows, um antivírus ou um indexador pode segurar o arquivo recém-criado por
    // um instante (violação de compartilhamento); a gravação é tentada de novo, e o erro vai para o
    // stderr (o Claude Code o ignora; os testes o mostram)
    // só a pasta e a abertura são tentadas de novo: depois de uma escrita pela metade, outra
    // tentativa juntaria dois pedaços numa linha inválida (re-revisão N5)
    let mut f = match abrir_com_tentativas(&caminho) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("work-timer-hook: não abri a fila ({TENTATIVAS} tentativas): {e}");
            return None;
        }
    };
    if let Err(e) = f.write_all(linha.as_bytes()) {
        eprintln!("work-timer-hook: não gravei o evento na fila: {e}");
        return None;
    }
    Some(())
}

/// Quantas vezes a abertura da fila é tentada.
const TENTATIVAS: usize = 3;

/// Cria a pasta (se faltar) e abre a fila para acrescentar, tentando de novo quando falha: nada
/// foi escrito ainda.
fn abrir_com_tentativas(caminho: &std::path::Path) -> std::io::Result<std::fs::File> {
    let abrir = || -> std::io::Result<std::fs::File> {
        if let Some(pai) = caminho.parent() {
            std::fs::create_dir_all(pai)?;
        }
        std::fs::OpenOptions::new().create(true).append(true).open(caminho)
    };
    let mut ultimo = None;
    for tentativa in 0..TENTATIVAS {
        match abrir() {
            Ok(f) => return Ok(f),
            Err(e) => {
                ultimo = Some(e);
                if tentativa + 1 < TENTATIVAS {
                    std::thread::sleep(std::time::Duration::from_millis(25));
                }
            }
        }
    }
    Err(ultimo.unwrap_or_else(|| std::io::Error::other("sem tentativa")))
}

fn main() {
    let _ = std::panic::catch_unwind(executar);
    std::process::exit(0);
}
