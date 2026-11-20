// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Projetos recentes (rodada 18): as pastas em que o Claude Code trabalhou nos últimos dias, para
//! o passo "Pastas dos projetos" do primeiro uso. A pessoa não precisa saber onde fica a pasta de
//! cada conversa: o app acha.
//!
//! Lê `~\.claude\projects\*`: em cada subpasta, o `.jsonl` mais novo, só as primeiras linhas, até
//! achar o campo `cwd` (no máximo 64 KB). Só a pasta é guardada, e só em memória, para o diálogo:
//! nenhum texto de conversa.

use std::io::Read;
use std::path::Path;
use std::time::UNIX_EPOCH;

use fila::pasta_ignorada;
use serde::{Deserialize, Serialize};

use crate::modelo::RegraPasta;
use crate::motor::{chave_de_pasta, pasta_larga, raiz_do_projeto, regra_para, sugestao_de_pasta};
use crate::tempo::Ms;

/// Quantos dias para trás contam.
pub const DIAS_RECENTES: u32 = 30;
/// Quantos projetos, no máximo.
pub const MAX_RECENTES: usize = 12;
/// Quanto de cada transcript é lido, no máximo, procurando o `cwd`.
pub const LIMITE_LEITURA: u64 = 64 * 1024;

/// Uma pasta de projeto usada há pouco.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ProjetoRecente {
    /// A pasta (a raiz do projeto: num worktree, a pasta antes do `\.claude\worktrees\`).
    pub pasta: String,
    /// O nome curto (o último nome da pasta).
    pub nome: String,
    /// Quando ela foi usada pela última vez (ms).
    pub usado_em: Ms,
}

/// O que fica de fora da lista: as pastas que nunca contam, as já ligadas por uma regra, e a
/// pasta do usuário (`perfil`) ou a raiz de um drive.
pub struct Filtro<'a> {
    pub regras: &'a [RegraPasta],
    pub ignoradas: &'a [String],
    pub perfil: Option<&'a str>,
}

/// Uma linha do transcript vista só pelo `cwd` (revisão da rodada 18): o resto da linha (o texto
/// da conversa) é pulado pelo serde sem ser montado em memória.
#[derive(Deserialize)]
struct SoOCwd {
    cwd: Option<String>,
}

/// O `cwd` das primeiras linhas de um transcript (até `limite` bytes; a linha cortada no fim não
/// conta). Nenhum outro campo é lido.
pub fn cwd_do_transcript(arquivo: &Path, limite: u64) -> Option<String> {
    let mut bytes = Vec::new();
    std::fs::File::open(arquivo).ok()?.take(limite).read_to_end(&mut bytes).ok()?;
    let texto = String::from_utf8_lossy(&bytes);
    texto.lines().find_map(|linha| {
        let l: SoOCwd = serde_json::from_str(linha).ok()?;
        l.cwd.filter(|c| !c.trim().is_empty())
    })
}

fn modificado_em(p: &Path) -> Option<Ms> {
    let m = std::fs::metadata(p).ok()?.modified().ok()?;
    Some(m.duration_since(UNIX_EPOCH).ok()?.as_millis() as Ms)
}

/// Os projetos recentes em `pasta_projetos` (`~\.claude\projects`), do mais novo para o mais
/// antigo: usados nos últimos `dias` antes de `agora`, no máximo `max`, as pastas iguais juntas
/// (fica a data mais recente) e sem o que o `filtro` tira.
pub fn projetos_recentes(pasta_projetos: &Path, agora: Ms, dias: u32, max: usize, filtro: &Filtro) -> Vec<ProjetoRecente> {
    let desde = agora - Ms::from(dias) * 24 * 60 * 60 * 1000;
    let Ok(dir) = std::fs::read_dir(pasta_projetos) else { return vec![] };
    let mut achados: Vec<ProjetoRecente> = vec![];
    for sub in dir.filter_map(Result::ok).filter(|e| e.file_type().is_ok_and(|t| t.is_dir())) {
        // o .jsonl mais novo da subpasta
        let Ok(arqs) = std::fs::read_dir(sub.path()) else { continue };
        let Some((arquivo, quando)) = arqs
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x.eq_ignore_ascii_case("jsonl")))
            .filter_map(|p| modificado_em(&p).map(|m| (p, m)))
            .max_by_key(|(_, m)| *m)
        else {
            continue;
        };
        if quando < desde {
            continue;
        }
        let Some(cwd) = cwd_do_transcript(&arquivo, LIMITE_LEITURA) else { continue };
        let pasta = raiz_do_projeto(&cwd).trim_end_matches(['\\', '/']).to_string();
        if pasta.is_empty()
            || pasta_larga(&pasta, filtro.perfil)
            || pasta_ignorada(&pasta, filtro.ignoradas)
            || regra_para(filtro.regras, &pasta).is_some()
        {
            continue;
        }
        let chave = chave_de_pasta(&pasta);
        match achados.iter_mut().find(|a| chave_de_pasta(&a.pasta) == chave) {
            Some(a) => a.usado_em = a.usado_em.max(quando),
            None => achados.push(ProjetoRecente { nome: sugestao_de_pasta(&pasta), pasta, usado_em: quando }),
        }
    }
    achados.sort_by(|a, b| b.usado_em.cmp(&a.usado_em).then_with(|| a.pasta.cmp(&b.pasta)));
    achados.truncate(max);
    achados
}

#[cfg(test)]
mod testes {
    use super::*;
    use std::time::{Duration, SystemTime};

    const DIA: Ms = 24 * 60 * 60 * 1000;

    /// Um transcript falso: as linhas dadas, com a data de modificação de `dias_atras`.
    fn transcript(base: &Path, sub: &str, nome: &str, linhas: &[String], dias_atras: u64) {
        let d = base.join(sub);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join(nome);
        std::fs::write(&p, linhas.join("\n")).unwrap();
        let quando = SystemTime::now() - Duration::from_secs(dias_atras * 24 * 60 * 60 + 60);
        std::fs::File::options().write(true).open(&p).unwrap().set_modified(quando).unwrap();
    }

    fn com_cwd(cwd: &str) -> Vec<String> {
        vec![
            r#"{"type":"summary","summary":"x"}"#.to_string(),
            serde_json::json!({ "type": "user", "cwd": cwd, "sessionId": "s" }).to_string(),
        ]
    }

    fn agora() -> Ms {
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis() as Ms
    }

    fn sem_filtro() -> Filtro<'static> {
        Filtro { regras: &[], ignoradas: &[], perfil: Some("C:\\Users\\usuario") }
    }

    #[test]
    fn le_o_cwd_normaliza_o_worktree_e_ordena_do_mais_novo() {
        let d = tempfile::tempdir().unwrap();
        transcript(d.path(), "a", "1.jsonl", &com_cwd("D:\\Projetos\\app-web"), 2);
        transcript(d.path(), "b", "1.jsonl", &com_cwd("D:\\Projetos\\api\\.claude\\worktrees\\feat-1"), 1);
        let v = projetos_recentes(d.path(), agora(), DIAS_RECENTES, MAX_RECENTES, &sem_filtro());
        let pastas: Vec<&str> = v.iter().map(|p| p.pasta.as_str()).collect();
        assert_eq!(pastas, ["D:\\Projetos\\api", "D:\\Projetos\\app-web"], "o worktree vira a raiz; o mais novo primeiro");
        assert_eq!(v[0].nome, "api");
        assert!(v[0].usado_em > v[1].usado_em);
    }

    #[test]
    fn o_jsonl_mais_novo_de_cada_pasta_e_so_os_ultimos_dias() {
        let d = tempfile::tempdir().unwrap();
        // na mesma subpasta, o mais novo vale
        transcript(d.path(), "a", "velho.jsonl", &com_cwd("D:\\Velho"), 10);
        transcript(d.path(), "a", "novo.jsonl", &com_cwd("D:\\Novo"), 3);
        // há 40 dias: fora dos 30
        transcript(d.path(), "b", "1.jsonl", &com_cwd("D:\\Antigo"), 40);
        // outro arquivo que não é .jsonl não conta
        std::fs::write(d.path().join("b").join("notas.txt"), "{\"cwd\":\"D:\\\\Txt\"}").unwrap();
        let v = projetos_recentes(d.path(), agora(), 30, 12, &sem_filtro());
        assert_eq!(v.iter().map(|p| p.pasta.as_str()).collect::<Vec<_>>(), ["D:\\Novo"]);
        // com 60 dias, o antigo entra
        assert_eq!(projetos_recentes(d.path(), agora(), 60, 12, &sem_filtro()).len(), 2);
    }

    #[test]
    fn pastas_iguais_juntas_e_o_maximo() {
        let d = tempfile::tempdir().unwrap();
        transcript(d.path(), "a", "1.jsonl", &com_cwd("D:\\Projetos\\app-web"), 5);
        transcript(d.path(), "b", "1.jsonl", &com_cwd("d:/projetos/app-web/"), 1);
        transcript(d.path(), "c", "1.jsonl", &com_cwd("D:\\Projetos\\app-web\\.claude\\worktrees\\x"), 3);
        let v = projetos_recentes(d.path(), agora(), 30, 12, &sem_filtro());
        assert_eq!(v.len(), 1, "a mesma pasta escrita de três jeitos: {v:?}");
        let mais_novo = agora() - DIA;
        assert!(v[0].usado_em >= mais_novo - 5 * 60 * 1000, "fica a data mais recente");
        for i in 0..15 {
            transcript(d.path(), &format!("p{i}"), "1.jsonl", &com_cwd(&format!("D:\\Projetos\\p{i}")), 2);
        }
        assert_eq!(projetos_recentes(d.path(), agora(), 30, 12, &sem_filtro()).len(), 12);
        assert_eq!(projetos_recentes(d.path(), agora(), 30, 3, &sem_filtro()).len(), 3);
    }

    #[test]
    fn ficam_de_fora_as_ignoradas_a_raiz_o_usuario_e_as_com_regra() {
        let d = tempfile::tempdir().unwrap();
        transcript(d.path(), "a", "1.jsonl", &com_cwd("D:\\pessoal\\notas"), 1);
        transcript(d.path(), "b", "1.jsonl", &com_cwd("D:\\"), 1);
        transcript(d.path(), "c", "1.jsonl", &com_cwd("C:\\Users\\usuario"), 1);
        transcript(d.path(), "d", "1.jsonl", &com_cwd("G:\\Empresa\\Projeto A\\sub"), 1);
        transcript(d.path(), "e", "1.jsonl", &com_cwd("D:\\Fica"), 1);
        let regras = [RegraPasta { id: 1, pasta: "G:\\Empresa\\Projeto A".into(), timer_id: 1 }];
        let ignoradas = ["D:\\pessoal".to_string()];
        let filtro = Filtro { regras: &regras, ignoradas: &ignoradas, perfil: Some("C:\\Users\\usuario") };
        let v = projetos_recentes(d.path(), agora(), 30, 12, &filtro);
        assert_eq!(v.iter().map(|p| p.pasta.as_str()).collect::<Vec<_>>(), ["D:\\Fica"]);
    }

    #[test]
    fn sem_cwd_nas_primeiras_linhas_ou_depois_de_64_kb_fica_de_fora() {
        let d = tempfile::tempdir().unwrap();
        transcript(d.path(), "a", "1.jsonl", &[r#"{"type":"summary"}"#.to_string(), "não é json".to_string()], 1);
        // o cwd depois de 64 KB de linhas sem ele: não é lido
        let mut longas: Vec<String> = (0..80).map(|i| format!("{{\"type\":\"x\",\"n\":{i},\"t\":\"{}\"}}", "a".repeat(1000))).collect();
        longas.push(serde_json::json!({ "cwd": "D:\\Tarde" }).to_string());
        transcript(d.path(), "b", "1.jsonl", &longas, 1);
        assert!(projetos_recentes(d.path(), agora(), 30, 12, &sem_filtro()).is_empty());
        // o mesmo cwd antes dos 64 KB é lido
        assert_eq!(cwd_do_transcript(&d.path().join("b").join("1.jsonl"), 200 * 1024).as_deref(), Some("D:\\Tarde"));
        // sem a pasta de projetos: nada
        assert!(projetos_recentes(&d.path().join("nao-existe"), agora(), 30, 12, &sem_filtro()).is_empty());
    }

    #[test]
    fn so_o_cwd_de_cima_da_linha_conta() {
        let d = tempfile::tempdir().unwrap();
        let linhas = vec![
            // cwd nulo, vazio ou só dentro da mensagem: a linha não dá a pasta
            serde_json::json!({ "type": "user", "cwd": null }).to_string(),
            serde_json::json!({ "type": "user", "cwd": "  " }).to_string(),
            serde_json::json!({ "message": { "content": [{ "cwd": "D:\\Errada" }] } }).to_string(),
            "[1, 2, 3]".to_string(),
            serde_json::json!({ "type": "user", "message": { "content": "texto longo" }, "cwd": "D:\\Certa", "x": [1, { "y": 2 }] }).to_string(),
        ];
        transcript(d.path(), "a", "1.jsonl", &linhas, 1);
        assert_eq!(cwd_do_transcript(&d.path().join("a").join("1.jsonl"), LIMITE_LEITURA).as_deref(), Some("D:\\Certa"));
    }
}
