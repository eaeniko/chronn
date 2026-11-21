// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use std::io::Write;
use std::process::{Command, Stdio};

/// Roda o hook com a pasta de dados na variável `WORKTIMER_BASE` (sem o `CLAUDE_CODE_ENTRYPOINT`
/// de quem roda os testes).
fn roda(base: &std::path::Path, entrada: &[u8]) -> std::process::ExitStatus {
    roda_com(base, entrada, None)
}

/// `roda` com a origem que o Claude Code passaria ao hook (`CLAUDE_CODE_ENTRYPOINT`).
fn roda_com(base: &std::path::Path, entrada: &[u8], origem: Option<&str>) -> std::process::ExitStatus {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_work-timer-hook"));
    cmd.env("WORKTIMER_BASE", base).env_remove("CLAUDE_CODE_ENTRYPOINT");
    if let Some(o) = origem {
        cmd.env("CLAUDE_CODE_ENTRYPOINT", o);
    }
    let mut filho = cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    filho.stdin.take().unwrap().write_all(entrada).unwrap();
    filho.wait().unwrap()
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
    assert!(roda_com(d.path(), json, Some("claude-desktop")).success());
    assert!(roda_com(d.path(), json, Some("nome estranho")).success());
    assert!(roda(d.path(), json).success());
    let (evs, _) = fila::ler_novos(&d.path().join("fila").join("eventos.jsonl"), 0).unwrap();
    let origens: Vec<Option<&str>> = evs.iter().map(|e| e.origem.as_deref()).collect();
    assert_eq!(origens, [Some("claude-desktop"), None, None]);
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
