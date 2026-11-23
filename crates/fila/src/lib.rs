// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
pub mod caminhos;

use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventoHook {
    pub t: i64,
    pub evento: String,
    pub sessao: String,
    pub cwd: String,
    #[serde(default)]
    pub transcript: Option<String>,
    #[serde(default)]
    pub notificacao: Option<String>,
    /// De onde a sessão veio (rodada 27): o `CLAUDE_CODE_ENTRYPOINT` que o hook herda do Claude
    /// Code (`claude-desktop`, `cli`, `claude-vscode`…), ou `chat` nos eventos do Chat.
    #[serde(default)]
    pub origem: Option<String>,
    /// A conta do Claude (rodada 28): só o resumo do `CLAUDE_CODE_ACCOUNT_UUID` (`resumo_da_conta`,
    /// 16 caracteres hexadecimais). O id da conta nunca sai do hook.
    #[serde(default)]
    pub conta: Option<String>,
}

/// O resumo da conta do Claude que vai no evento (rodada 28): o id sem os espaços e em
/// minúsculas, passado pelo FNV-1a de 64 bits, em 16 caracteres hexadecimais. Vazio ou ausente:
/// sem conta. O id nunca vai para a fila.
pub fn resumo_da_conta(id: Option<&str>) -> Option<String> {
    let id: String = id?.chars().filter(|c| !c.is_whitespace()).collect::<String>().to_lowercase();
    if id.is_empty() {
        return None;
    }
    Some(format!("{:016x}", fnv1a_64(id.as_bytes())))
}

/// FNV-1a de 64 bits.
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// O resumo de conta lido da fila vale? Exatamente 16 caracteres hexadecimais minúsculos (outro
/// valor fica de fora).
pub fn conta_valida(v: Option<&str>) -> Option<String> {
    let v = v?;
    (v.len() == 16 && v.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))).then(|| v.to_string())
}

/// A origem que vai no evento: até 32 caracteres de `[a-z0-9_-]` (outro valor, ou vazio, fica de
/// fora).
pub fn origem_valida(v: Option<&str>) -> Option<String> {
    let v = v?.trim();
    let ok = !v.is_empty()
        && v.len() <= 32
        && v.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    ok.then(|| v.to_string())
}

/// A pasta comparável: `/` = `\`, sem diferenciar maiúsculas, e `\\wsl$\<d>\` como
/// `\\wsl.localhost\<d>\` (rodada 28: os dois nomes da mesma pasta do WSL).
pub fn normaliza(s: &str) -> String {
    let n = s.replace('\\', "/").to_lowercase();
    match n.strip_prefix("//wsl$/") {
        Some(resto) => format!("//wsl.localhost/{resto}"),
        None if n == "//wsl$" => "//wsl.localhost".to_string(),
        None => n,
    }
}

pub fn pasta_ignorada(cwd: &str, ignoradas: &[String]) -> bool {
    let c = normaliza(cwd);
    ignoradas.iter().any(|i| {
        let i = normaliza(i);
        let i = i.trim_end_matches('/');
        !i.is_empty() && (c == i || c.starts_with(&format!("{i}/")))
    })
}

/// A linha da fila do evento do hook. `origem`: o `CLAUDE_CODE_ENTRYPOINT` do processo do hook
/// (conferido por `origem_valida`). `conta`: o `CLAUDE_CODE_ACCOUNT_UUID` dele, que vai só como
/// resumo (`resumo_da_conta`).
pub fn linha_do_evento(
    entrada: &serde_json::Value,
    agora: i64,
    ignoradas: &[String],
    origem: Option<&str>,
    conta: Option<&str>,
) -> Option<String> {
    let texto = |k: &str| entrada.get(k).and_then(|v| v.as_str()).map(str::to_string);
    let evento = texto("hook_event_name").filter(|s| !s.is_empty())?;
    let sessao = texto("session_id").filter(|s| !s.is_empty())?;
    let cwd = texto("cwd").unwrap_or_default();
    if pasta_ignorada(&cwd, ignoradas) {
        return None;
    }
    let ev = EventoHook {
        t: agora,
        evento,
        sessao,
        cwd,
        transcript: texto("transcript_path"),
        notificacao: texto("notification_type"),
        origem: origem_valida(origem),
        conta: resumo_da_conta(conta),
    };
    let mut l = serde_json::to_string(&ev).ok()?;
    l.push('\n');
    Some(l)
}

pub fn ler_novos(caminho: &Path, offset: u64) -> std::io::Result<(Vec<EventoHook>, u64)> {
    let mut f = match std::fs::File::open(caminho) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((vec![], offset)),
        Err(e) => return Err(e),
    };
    f.seek(SeekFrom::Start(offset))?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf)?;
    let mut eventos = Vec::new();
    let mut consumido = 0usize;
    while let Some(pos) = buf[consumido..].iter().position(|&b| b == b'\n') {
        let linha = &buf[consumido..consumido + pos];
        if let Ok(ev) = serde_json::from_slice::<EventoHook>(linha) {
            eventos.push(ev);
        }
        consumido += pos + 1;
    }
    Ok((eventos, offset + consumido as u64))
}

/// Acima disto a fila já lida é esvaziada (5 MB).
pub const LIMITE_FILA: u64 = 5 * 1024 * 1024;

/// Esvazia a fila se ela passou de `limite` e já foi lida inteira (`offset` = tamanho).
/// Devolve se esvaziou: quem lê recomeça do zero. O hook abre em modo de acréscimo a cada
/// evento, então segue escrevendo no arquivo vazio.
pub fn esvaziar_se_lida(caminho: &Path, offset: u64, limite: u64) -> std::io::Result<bool> {
    let tamanho = match std::fs::metadata(caminho) {
        Ok(m) => m.len(),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(e),
    };
    if tamanho <= limite || tamanho != offset {
        return Ok(false);
    }
    let f = std::fs::OpenOptions::new().write(true).open(caminho)?;
    // confere de novo com o arquivo aberto: evento que chegou agora fica para a próxima vez
    if f.metadata()?.len() != offset {
        return Ok(false);
    }
    f.set_len(0)?;
    Ok(true)
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::io::Write;

    #[test]
    fn fila_lida_e_grande_e_esvaziada() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("e.jsonl");
        assert!(!esvaziar_se_lida(&p, 0, 10).unwrap(), "sem arquivo");
        std::fs::write(&p, format!("{}{}", linha(1), linha(2))).unwrap();
        let (_, off) = ler_novos(&p, 0).unwrap();
        assert!(!esvaziar_se_lida(&p, off, 10_000).unwrap(), "pequena fica");
        assert!(!esvaziar_se_lida(&p, off - 1, 10).unwrap(), "com linha por ler fica");
        assert_eq!(std::fs::metadata(&p).unwrap().len(), off);
        assert!(esvaziar_se_lida(&p, off, 10).unwrap());
        assert_eq!(std::fs::metadata(&p).unwrap().len(), 0);
        // o hook segue acrescentando e a leitura recomeça do zero
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(linha(3).as_bytes()).unwrap();
        let (evs, _) = ler_novos(&p, 0).unwrap();
        assert_eq!(evs.iter().map(|e| e.t).collect::<Vec<_>>(), [3]);
    }

    #[test]
    fn linha_tem_campos_e_nao_tem_prompt() {
        let e = serde_json::json!({"hook_event_name":"UserPromptSubmit","session_id":"s1","cwd":"G:\\Empresa\\Projeto","transcript_path":"C:\\x.jsonl","prompt":"segredo do cliente"});
        let l = linha_do_evento(&e, 1000, &[], None, None).unwrap();
        assert!(!l.contains("segredo"));
        let ev: EventoHook = serde_json::from_str(l.trim()).unwrap();
        assert_eq!((ev.t, ev.evento.as_str(), ev.sessao.as_str()), (1000, "UserPromptSubmit", "s1"));
        assert_eq!(ev.origem, None);
    }

    #[test]
    fn a_origem_vai_no_evento_so_se_for_curta_e_simples() {
        let e = serde_json::json!({"hook_event_name":"Stop","session_id":"s1","cwd":"G:\\Projeto"});
        let origem = |o: Option<&str>| -> Option<String> {
            let l = linha_do_evento(&e, 1, &[], o, None).unwrap();
            serde_json::from_str::<EventoHook>(l.trim()).unwrap().origem
        };
        assert_eq!(origem(Some("claude-desktop")).as_deref(), Some("claude-desktop"));
        assert_eq!(origem(Some("cli")).as_deref(), Some("cli"));
        assert_eq!(origem(Some("sdk_ts-2")).as_deref(), Some("sdk_ts-2"));
        assert_eq!(origem(Some(&"a".repeat(32))).map(|o| o.len()), Some(32));
        for ruim in ["", "  ", "CLI", "claude desktop", "c:\\x", &"a".repeat(33), "café"] {
            assert_eq!(origem(Some(ruim)), None, "{ruim:?}");
        }
        assert_eq!(origem(None), None);
        // uma linha antiga, sem a origem, ainda é lida
        let velha: EventoHook = serde_json::from_str(linha(1).trim()).unwrap();
        assert_eq!(velha.origem, None);
    }

    #[test]
    fn a_conta_vai_so_como_resumo() {
        let uuid = "0A1B2C3D-4E5F-6789-ABCD-EF0123456789";
        let e = serde_json::json!({"hook_event_name":"Stop","session_id":"s1","cwd":"G:\\Projeto"});
        let l = linha_do_evento(&e, 1, &[], None, Some(uuid)).unwrap();
        assert!(!l.to_lowercase().contains(&uuid.to_lowercase()), "o id da conta nunca vai para a fila: {l}");
        assert!(!l.to_lowercase().contains("0a1b2c3d"), "nem um pedaço dele: {l}");
        let ev: EventoHook = serde_json::from_str(l.trim()).unwrap();
        let resumo = ev.conta.unwrap();
        assert_eq!(resumo.len(), 16);
        assert_eq!(conta_valida(Some(&resumo)), Some(resumo.clone()));
        // os espaços e as maiúsculas não mudam o resumo
        assert_eq!(resumo_da_conta(Some(&format!("  {}\n", uuid.to_lowercase()))), Some(resumo));
        assert_eq!(resumo_da_conta(Some("")), None);
        assert_eq!(resumo_da_conta(Some("   ")), None);
        assert_eq!(resumo_da_conta(None), None);
        // o FNV-1a de 64 bits conhecido
        assert_eq!(fnv1a_64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a_64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(resumo_da_conta(Some("a")).as_deref(), Some("af63dc4c8601ec8c"));
        for ruim in ["", "abc", "AF63DC4C8601EC8C", "af63dc4c8601ec8g", "af63dc4c8601ec8c0"] {
            assert_eq!(conta_valida(Some(ruim)), None, "{ruim:?}");
        }
        // a linha antiga, sem a conta, ainda é lida
        let velha: EventoHook = serde_json::from_str(linha(1).trim()).unwrap();
        assert_eq!(velha.conta, None);
    }

    #[test]
    fn wsl_cifrao_e_wsl_localhost_sao_a_mesma_pasta() {
        let ign = vec![r"\\wsl$\Ubuntu\home\u\pessoal".to_string()];
        assert!(pasta_ignorada(r"\\wsl.localhost\Ubuntu\home\u\pessoal\x", &ign));
        assert!(pasta_ignorada(r"\\WSL$\ubuntu\home\u\pessoal", &ign));
        assert!(!pasta_ignorada(r"\\wsl.localhost\Debian\home\u\pessoal", &ign));
        assert_eq!(normaliza(r"\\wsl$\Ubuntu\x"), "//wsl.localhost/ubuntu/x");
        assert_eq!(normaliza(r"C:\X"), "c:/x");
    }

    #[test]
    fn pasta_ignorada_descarta() {
        let e = serde_json::json!({"hook_event_name":"Stop","session_id":"s9","cwd":"G:\\pessoal\\notas"});
        assert_eq!(linha_do_evento(&e, 1, &["g:/pessoal".into()], Some("cli"), None), None);
    }

    #[test]
    fn pasta_ignorada_respeita_limite_de_caminho() {
        let ign = vec!["g:/pessoal".to_string()];
        assert!(!pasta_ignorada("G:\\pessoal2\\x", &ign));
        assert!(!pasta_ignorada("G:\\pessoal-backup", &ign));
        assert!(pasta_ignorada("G:\\pessoal", &ign));
        let barra = vec!["g:/pessoal/".to_string()];
        assert!(pasta_ignorada("G:\\pessoal\\x", &barra));
        assert!(pasta_ignorada("G:\\pessoal", &barra));
        assert!(!pasta_ignorada("G:\\pessoal2", &barra));
    }

    fn linha(t: i64) -> String {
        format!("{{\"t\":{t},\"evento\":\"Stop\",\"sessao\":\"s\",\"cwd\":\"c\"}}\n")
    }

    #[test]
    fn ler_novos_nao_consome_linha_incompleta() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("e.jsonl");
        std::fs::write(&p, format!("{}{}{{\"t\":3", linha(1), linha(2))).unwrap();
        let (evs, off) = ler_novos(&p, 0).unwrap();
        assert_eq!(evs.len(), 2);
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b",\"evento\":\"Stop\",\"sessao\":\"s\",\"cwd\":\"c\"}\n").unwrap();
        let (evs, off2) = ler_novos(&p, off).unwrap();
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].t, 3);
        assert_eq!(off2, std::fs::metadata(&p).unwrap().len());
    }

    #[test]
    fn linha_corrompida_e_pulada() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("e.jsonl");
        std::fs::write(&p, format!("{}lixo\n{}", linha(1), linha(2))).unwrap();
        let (evs, off) = ler_novos(&p, 0).unwrap();
        assert_eq!(evs.len(), 2);
        assert_eq!(off, std::fs::metadata(&p).unwrap().len());
        assert_eq!(ler_novos(&d.path().join("nada"), 7).unwrap(), (vec![], 7));
    }
}
