// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Leitura do nome das sessões do Claude Code (só o `customTitle`).

use std::path::Path;

use serde::Deserialize;
use std::io::{Read, Seek, SeekFrom};

const CAUDA: u64 = 256 * 1024;

#[derive(Deserialize)]
struct Linha {
    #[serde(rename = "type")]
    tipo: Option<String>,
    #[serde(rename = "customTitle")]
    titulo: Option<String>,
}

fn ultimo_titulo(texto: &str) -> Option<String> {
    texto
        .lines()
        .filter(|l| l.contains("\"custom-title\""))
        .filter_map(|l| serde_json::from_str::<Linha>(l).ok())
        .filter(|l| l.tipo.as_deref() == Some("custom-title"))
        .filter_map(|l| l.titulo)
        .filter(|t| !t.trim().is_empty())
        .last()
}

pub fn titulo_da_sessao(transcript: &Path) -> Option<String> {
    let mut f = std::fs::File::open(transcript).ok()?;
    let tamanho = f.metadata().ok()?.len();
    if tamanho > CAUDA {
        f.seek(SeekFrom::Start(tamanho - CAUDA)).ok()?;
        let mut bytes = Vec::new();
        f.read_to_end(&mut bytes).ok()?;
        let texto = String::from_utf8_lossy(&bytes);
        // a primeira linha da cauda pode estar cortada ao meio
        let inteiro = texto.split_once('\n').map_or("", |(_, resto)| resto);
        if let Some(t) = ultimo_titulo(inteiro) {
            return Some(t);
        }
        f.seek(SeekFrom::Start(0)).ok()?;
    }
    let mut bytes = Vec::new();
    f.read_to_end(&mut bytes).ok()?;
    ultimo_titulo(&String::from_utf8_lossy(&bytes))
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::fs;

    #[test]
    fn titulo_vem_do_ultimo_custom_title() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        fs::write(
            &p,
            concat!(
                "{\"type\":\"user\",\"message\":\"oi\"}\n",
                "{\"type\":\"custom-title\",\"customTitle\":\"Primeiro\",\"sessionId\":\"a\"}\n",
                "{\"type\":\"assistant\",\"message\":\"ola\"}\n",
                "{\"type\":\"custom-title\",\"customTitle\":\"Spec 018: segundo\",\"sessionId\":\"a\"}\n",
                "{\"type\":\"user\",\"message\":\"fim\"}\n",
            ),
        )
        .unwrap();
        assert_eq!(titulo_da_sessao(&p).as_deref(), Some("Spec 018: segundo"));
    }

    #[test]
    fn sem_titulo_retorna_none() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        fs::write(&p, "{\"type\":\"user\",\"message\":\"oi\"}\n").unwrap();
        assert_eq!(titulo_da_sessao(&p), None);
        assert_eq!(titulo_da_sessao(&d.path().join("nao-existe.jsonl")), None);
    }

    #[test]
    fn titulo_antigo_fora_da_cauda_e_achado_no_arquivo_inteiro() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("s.jsonl");
        let mut s = String::from("{\"type\":\"custom-title\",\"customTitle\":\"Antigo\"}\n");
        let linha = format!("{{\"type\":\"user\",\"message\":\"{}\"}}\n", "x".repeat(1000));
        for _ in 0..400 {
            s.push_str(&linha);
        }
        fs::write(&p, s).unwrap();
        assert_eq!(titulo_da_sessao(&p).as_deref(), Some("Antigo"));
    }
}
