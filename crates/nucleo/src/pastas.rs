// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Regras de pasta (rodada 15): a pasta-mãe (uma pasta com vários projetos dentro), o que vale
//! como regra e o "Trocar por uma subpasta…".

use std::path::Path;

use fila::pasta_ignorada;
use serde::Serialize;

use crate::modelo::RegraPasta;
use crate::motor::{chave_de_pasta, pasta_larga, raiz_do_projeto};

/// Texto da pasta larga demais para regra.
#[allow(non_snake_case)]
pub fn ERRO_PASTA_LARGA() -> &'static str {
    crate::idioma::ta("erro.pasta_larga")
}
/// Texto da pasta que nunca conta.
#[allow(non_snake_case)]
pub fn ERRO_PASTA_IGNORADA() -> &'static str {
    crate::idioma::ta("erro.pasta_ignorada")
}

/// O que a tela precisa saber antes de gravar uma regra de pasta (rodada 15).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Conferencia {
    /// Onde a regra fica: a raiz do projeto (num worktree do Claude Code, a pasta antes do
    /// `\.claude\worktrees\`), como a regra grava.
    pub pasta: String,
    /// Pasta com vários projetos dentro: a regra vale, mas a caixa vem desmarcada, com o aviso.
    pub pasta_mae: bool,
    /// A regra desta pasta já é de outro timer (o id dele): gravar passa a regra para este.
    pub outro_timer: Option<i64>,
    /// A regra não pode ficar aqui (raiz de drive, pasta do usuário, pasta que nunca conta).
    pub erro: Option<String>,
}

/// Confere uma regra para `pasta` no timer `timer_id` (nenhum: o timer ainda não existe).
/// `perfil`: a pasta do usuário (`%USERPROFILE%`).
pub fn conferir(
    pasta: &str,
    timer_id: Option<i64>,
    regras: &[RegraPasta],
    ignoradas: &[String],
    perfil: Option<&str>,
) -> Conferencia {
    let raiz = raiz_do_projeto(pasta.trim()).trim_end_matches(['\\', '/']);
    // `G:\` sem a barra vira `G:`, que a tela mostra assim mesmo
    let pasta = raiz.to_string();
    let erro = if pasta_larga(&pasta, perfil) {
        Some(ERRO_PASTA_LARGA().to_string())
    } else if pasta_ignorada(&pasta, ignoradas) {
        Some(ERRO_PASTA_IGNORADA().to_string())
    } else {
        None
    };
    let chave = chave_de_pasta(&pasta);
    let outro_timer = regras
        .iter()
        .find(|r| chave_de_pasta(&r.pasta) == chave && Some(r.timer_id) != timer_id)
        .map(|r| r.timer_id);
    let pasta_mae = erro.is_none() && pasta_mae(&pasta, regras);
    Conferencia { pasta, pasta_mae, outro_timer, erro }
}

/// `pasta` é ancestral de `outra` (sem diferenciar maiúsculas nem barras; a própria pasta não)?
pub fn ancestral(pasta: &str, outra: &str) -> bool {
    let (p, o) = (chave_de_pasta(pasta), chave_de_pasta(outra));
    !p.is_empty() && o.len() > p.len() + 1 && o.starts_with(&p) && o.as_bytes()[p.len()] == b'/'
}

/// A pasta está em `outra` ou é ela?
pub fn dentro_de(pasta: &str, outra: &str) -> bool {
    chave_de_pasta(pasta) == chave_de_pasta(outra) || ancestral(outra, pasta)
}

/// Pasta-mãe pela regra (a): ela é ancestral da pasta de outra regra.
pub fn ancestral_de_outra_regra(pasta: &str, regras: &[RegraPasta]) -> bool {
    regras.iter().any(|r| ancestral(pasta, &r.pasta))
}

/// Pasta-mãe pela regra (b): 2 ou mais subpastas diretas com `.git` ou `.claude`. Lê só os nomes
/// das subpastas diretas, sem descer mais (e confere se cada uma tem uma dessas duas).
pub fn varios_projetos(pasta: &Path) -> bool {
    let Ok(dir) = std::fs::read_dir(pasta) else { return false };
    dir.filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| e.path().join(".git").exists() || e.path().join(".claude").exists())
        .nth(1)
        .is_some()
}

/// A pasta é uma pasta-mãe (uma regra nela pegaria vários projetos)? Regra (a) ou (b).
pub fn pasta_mae(pasta: &str, regras: &[RegraPasta]) -> bool {
    ancestral_de_outra_regra(pasta, regras) || varios_projetos(Path::new(pasta))
}

/// O "Trocar por uma subpasta…" da sessão: só vale a pasta da regra ou uma subpasta dela que
/// contenha a pasta da conversa.
pub fn subpasta_valida(regra: &str, nova: &str, cwd: &str) -> bool {
    chave_de_pasta(regra) == chave_de_pasta(nova) || (ancestral(regra, nova) && dentro_de(cwd, nova))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn r(id: i64, pasta: &str) -> RegraPasta {
        RegraPasta { id, pasta: pasta.into(), timer_id: id }
    }

    #[test]
    fn ancestral_sem_maiusculas_nem_barras() {
        assert!(ancestral("G:\\Empresa\\Projeto", "G:\\Empresa\\Projeto\\Flow"));
        assert!(ancestral("g:/empresa/projeto/", "G:\\Empresa\\Projeto\\Flow\\sub"));
        assert!(!ancestral("G:\\Empresa\\Projeto", "G:\\Empresa\\Projeto"), "a própria pasta não");
        assert!(!ancestral("G:\\Empresa\\Projeto", "G:\\Empresa\\Projeto A"), "outro nome que começa igual");
        assert!(!ancestral("G:\\Empresa\\Projeto\\Flow", "G:\\Empresa\\Projeto"));
    }

    #[test]
    fn pasta_mae_pela_regra_a() {
        let regras = [r(1, "G:\\Empresa\\Projeto\\Flow"), r(2, "D:\\Outro")];
        assert!(ancestral_de_outra_regra("G:\\Empresa\\Projeto", &regras));
        assert!(ancestral_de_outra_regra("G:\\Empresa", &regras));
        assert!(!ancestral_de_outra_regra("G:\\Empresa\\Projeto\\Flow", &regras), "ela mesma");
        assert!(!ancestral_de_outra_regra("G:\\Empresa\\Projeto\\Cliente A", &regras));
        assert!(!ancestral_de_outra_regra("G:\\Empresa\\Projeto", &[]));
    }

    #[test]
    fn pasta_mae_pela_regra_b_numa_pasta_temporaria() {
        let d = tempfile::tempdir().unwrap();
        let mae = d.path();
        // um projeto com .git: ainda não
        std::fs::create_dir_all(mae.join("Flow").join(".git")).unwrap();
        std::fs::create_dir_all(mae.join("docs")).unwrap();
        std::fs::write(mae.join("leia.txt"), "x").unwrap();
        assert!(!varios_projetos(mae));
        // o segundo, com .claude: pasta-mãe
        std::fs::create_dir_all(mae.join("Cliente A").join(".claude")).unwrap();
        assert!(varios_projetos(mae));
        assert!(pasta_mae(&mae.to_string_lossy(), &[]));
        // só as subpastas diretas: um projeto mais fundo não conta
        let outra = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(outra.path().join("a").join("b").join(".git")).unwrap();
        std::fs::create_dir_all(outra.path().join("c").join(".git")).unwrap();
        assert!(!varios_projetos(outra.path()));
        assert!(!varios_projetos(&mae.join("nao-existe")));
    }

    #[test]
    fn conferir_corta_o_worktree_e_recusa_o_que_nao_pode() {
        let regras = [r(1, "G:\\Empresa\\Projeto\\Flow"), r(2, "G:\\Projeto A")];
        let ign = ["G:\\Pessoal".to_string()];
        let perfil = Some("C:\\Users\\usuario");
        let c = conferir("G:\\Projeto A\\.claude\\worktrees\\x\\sub", Some(2), &regras, &ign, perfil);
        assert_eq!(c, Conferencia { pasta: "G:\\Projeto A".into(), pasta_mae: false, outro_timer: None, erro: None });
        // a regra da pasta é de outro timer
        assert_eq!(conferir("g:/projeto a/", Some(5), &regras, &ign, perfil).outro_timer, Some(2));
        assert_eq!(conferir("G:\\Projeto A", None, &regras, &ign, perfil).outro_timer, Some(2), "timer novo");
        // ancestral de outra regra: pasta-mãe
        assert!(conferir("G:\\Empresa\\Projeto", Some(5), &regras, &ign, perfil).pasta_mae);
        // raiz de drive, pasta do usuário, pasta que nunca conta
        for (p, e) in [
            ("G:\\", ERRO_PASTA_LARGA()),
            ("G:", ERRO_PASTA_LARGA()),
            ("C:\\Users\\usuario", ERRO_PASTA_LARGA()),
            ("C:\\Users\\outro", ERRO_PASTA_LARGA()),
            ("G:\\Pessoal\\x", ERRO_PASTA_IGNORADA()),
        ] {
            let c = conferir(p, Some(1), &regras, &ign, perfil);
            assert_eq!(c.erro.as_deref(), Some(e), "{p}");
            assert!(!c.pasta_mae);
        }
    }

    #[test]
    fn subpasta_que_contem_a_conversa() {
        let regra = "G:\\Empresa\\Projeto";
        let cwd = "G:\\Empresa\\Projeto\\Flow\\app";
        assert!(subpasta_valida(regra, "G:\\Empresa\\Projeto", cwd), "a própria pasta da regra");
        assert!(subpasta_valida(regra, "G:\\Empresa\\Projeto\\Flow", cwd));
        assert!(subpasta_valida(regra, "g:/empresa/projeto/flow/app", cwd), "a pasta da conversa");
        assert!(!subpasta_valida(regra, "G:\\Empresa\\Projeto\\Cliente A", cwd), "não contém a conversa");
        assert!(!subpasta_valida(regra, "G:\\Empresa", cwd), "acima da regra");
        assert!(!subpasta_valida(regra, "D:\\Flow", cwd));
    }
}
