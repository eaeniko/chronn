// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use std::io::Write;
use std::process::{Command, Stdio};

/// Como o hook saiu: o status e o que ele escreveu no stderr (revisão M1: uma gravação que
/// falhou diz por quê).
struct Saida {
    status: std::process::ExitStatus,
    erro: String,
}

impl Saida {
    fn success(&self) -> bool {
        self.status.success()
    }
}

/// Roda o hook com a pasta de dados na variável `WORKTIMER_BASE` (sem o `CLAUDE_CODE_ENTRYPOINT`
/// nem o `CLAUDE_CODE_ACCOUNT_UUID` de quem roda os testes).
fn roda(base: &std::path::Path, entrada: &[u8]) -> Saida {
    roda_com(base, entrada, None)
}

/// `roda` com a origem que o Claude Code passaria ao hook (`CLAUDE_CODE_ENTRYPOINT`).
fn roda_com(base: &std::path::Path, entrada: &[u8], origem: Option<&str>) -> Saida {
    roda_com_conta(base, entrada, origem, None)
}

/// `roda_com` com a conta do Claude (`CLAUDE_CODE_ACCOUNT_UUID`).
fn roda_com_conta(base: &std::path::Path, entrada: &[u8], origem: Option<&str>, conta: Option<&str>) -> Saida {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_work-timer-hook"));
    cmd.env("WORKTIMER_BASE", base).env_remove("CLAUDE_CODE_ENTRYPOINT").env_remove("CLAUDE_CODE_ACCOUNT_UUID");
    if let Some(o) = origem {
        cmd.env("CLAUDE_CODE_ENTRYPOINT", o);
    }
    if let Some(c) = conta {
        cmd.env("CLAUDE_CODE_ACCOUNT_UUID", c);
    }
    let mut filho = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    filho.stdin.take().unwrap().write_all(entrada).unwrap();
    let s = filho.wait_with_output().unwrap();
    Saida { status: s.status, erro: String::from_utf8_lossy(&s.stderr).into_owned() }
}

#[test]
fn hook_grava_uma_linha_e_sai_com_zero() {
    let d = tempfile::tempdir().unwrap();
    let json = br#"{"hook_event_name":"Stop","session_id":"s1","cwd":"C:\\x","prompt":"segredo"}"#;
    assert!(roda(d.path(), json).success());
    let txt = std::fs::read_to_string(d.path().join("fila").join("eventos.jsonl")).unwrap();
    assert_eq!(txt.lines().count(), 1);
    assert!(!txt.contains("segredo"));
}

#[test]
fn hook_manda_a_origem_que_herda_do_claude_code() {
    let d = tempfile::tempdir().unwrap();
    let json = br#"{"hook_event_name":"Stop","session_id":"s1","cwd":"C:\\x"}"#;
    let saidas = [roda_com(d.path(), json, Some("claude-desktop")), roda_com(d.path(), json, Some("nome estranho")), roda(d.path(), json)];
    let erros: Vec<&str> = saidas.iter().map(|s| s.erro.as_str()).filter(|e| !e.is_empty()).collect();
    assert!(saidas.iter().all(Saida::success) && erros.is_empty(), "o stderr do hook: {erros:?}");
    let (evs, _) = fila::ler_novos(&d.path().join("fila").join("eventos.jsonl"), 0).unwrap();
    let origens: Vec<Option<&str>> = evs.iter().map(|e| e.origem.as_deref()).collect();
    assert_eq!(origens, [Some("claude-desktop"), None, None]);
}

#[test]
fn hook_manda_so_o_resumo_da_conta_e_nunca_o_id() {
    let d = tempfile::tempdir().unwrap();
    let uuid = "5d1c0e7a-90b2-4f61-8c3e-2a7b9f04d6e1";
    let json = br#"{"hook_event_name":"UserPromptSubmit","session_id":"s1","cwd":"C:\\x"}"#;
    let saidas = [
        roda_com_conta(d.path(), json, Some("cli"), Some(&format!(" {} ", uuid.to_uppercase()))),
        roda_com_conta(d.path(), json, Some("cli"), Some("")),
        roda(d.path(), json),
    ];
    let erros: Vec<&str> = saidas.iter().map(|s| s.erro.as_str()).filter(|e| !e.is_empty()).collect();
    assert!(saidas.iter().all(Saida::success) && erros.is_empty(), "o stderr do hook: {erros:?}");
    let caminho = d.path().join("fila").join("eventos.jsonl");
    let txt = std::fs::read_to_string(&caminho).unwrap();
    assert!(!txt.to_lowercase().contains(uuid), "o id da conta nunca vai para a fila");
    assert!(!txt.to_lowercase().contains("5d1c0e7a"));
    let (evs, _) = fila::ler_novos(&caminho, 0).unwrap();
    let contas: Vec<Option<String>> = evs.iter().map(|e| e.conta.clone()).collect();
    assert_eq!(contas, [fila::resumo_da_conta(Some(uuid)), None, None]);
}

#[test]
fn hook_com_lixo_no_stdin_sai_com_zero_e_nao_grava() {
    let d = tempfile::tempdir().unwrap();
    assert!(roda(d.path(), b"isto nao e json").success());
    assert!(!d.path().join("fila").join("eventos.jsonl").exists());
}

#[test]
fn hook_grava_o_instante_em_milissegundos() {
    let d = tempfile::tempdir().unwrap();
    let antes = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let json = br#"{"hook_event_name":"Stop","session_id":"s1","cwd":"C:\\x"}"#;
    assert!(roda(d.path(), json).success());
    let depois = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64;
    let (evs, _) = fila::ler_novos(&d.path().join("fila").join("eventos.jsonl"), 0).unwrap();
    assert_eq!(evs.len(), 1);
    assert!(evs[0].t >= antes && evs[0].t <= depois, "t = {} fora de [{antes}, {depois}]", evs[0].t);
}
