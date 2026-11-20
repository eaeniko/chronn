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
}

fn normaliza(s: &str) -> String {
    s.replace('\\', "/").to_lowercase()
}

pub fn pasta_ignorada(cwd: &str, ignoradas: &[String]) -> bool {
    let c = normaliza(cwd);
    ignoradas.iter().any(|i| {
        let i = normaliza(i);
        let i = i.trim_end_matches('/');
        !i.is_empty() && (c == i || c.starts_with(&format!("{i}/")))
    })
}

pub fn linha_do_evento(
    entrada: &serde_json::Value,
    agora: i64,
    ignoradas: &[String],
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
        let l = linha_do_evento(&e, 1000, &[]).unwrap();
        assert!(!l.contains("segredo"));
        let ev: EventoHook = serde_json::from_str(l.trim()).unwrap();
        assert_eq!((ev.t, ev.evento.as_str(), ev.sessao.as_str()), (1000, "UserPromptSubmit", "s1"));
    }

    #[test]
    fn pasta_ignorada_descarta() {
        let e = serde_json::json!({"hook_event_name":"Stop","session_id":"s9","cwd":"G:\\pessoal\\notas"});
        assert_eq!(linha_do_evento(&e, 1, &["g:/pessoal".into()]), None);
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
