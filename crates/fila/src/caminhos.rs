// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Pasta de dados, dentro da pasta do usuário (`%USERPROFILE%\.worktimer`; no Mac, `~/.worktimer`).
/// Fora do AppData de propósito: o app do Claude para Windows é um pacote MSIX e tudo que roda
/// dentro dele (os hooks, inclusive) vê um `%LOCALAPPDATA%` virtual. A pasta do usuário fora do
/// AppData não é virtualizada, então o app e os hooks enxergam a mesma pasta de qualquer lado.
pub const PASTA_DADOS: &str = ".worktimer";

/// Variável que troca a pasta de dados (testes e execuções de teste).
pub const VAR_BASE: &str = "WORKTIMER_BASE";

/// Nome do executável do hook (sem `.exe`).
pub const HOOK_NOME: &str = "work-timer-hook";

// ---- perfis (rodada 17) ----
// `work-timer.exe --perfil teste` abre o app "zerado", como um colega novo, sem misturar com o de
// uso: tudo o que é da pessoa (os dados, o `settings.json` do Claude Code, o "Iniciar com o
// Windows") passa por este ponto único. O perfil é definido uma vez, no começo do `main`; quem
// não define (o hook, os testes) fica no normal.

/// O perfil desta execução.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Perfil {
    /// O de uso: `~\.worktimer`, o `~\.claude\settings.json` real, o registro "Run".
    #[default]
    Normal,
    /// O de teste: `~\.worktimer-perfil-teste`, um `settings.json` simulado dentro dele, o
    /// "Iniciar com o Windows" só na config. Nunca lê nem grava o que é do normal.
    Teste,
}

/// Pasta de dados do perfil de teste (dentro da pasta do usuário).
pub const PASTA_PERFIL_TESTE: &str = ".worktimer-perfil-teste";
/// O `settings.json` do Claude Code simulado, dentro da pasta do perfil de teste.
pub const SETTINGS_SIMULADO: &str = "settings-claude-simulado.json";

static PERFIL: OnceLock<Perfil> = OnceLock::new();

/// Define o perfil desta execução (vale a primeira vez; devolve se valeu).
pub fn definir_perfil(p: Perfil) -> bool {
    PERFIL.set(p).is_ok()
}

/// O perfil desta execução (o normal, se ninguém definiu).
pub fn perfil() -> Perfil {
    PERFIL.get().copied().unwrap_or_default()
}

/// O perfil pedido na linha de comando (`--perfil teste` ou `--perfil=teste`; o primeiro
/// argumento é o exe). Um valor desconhecido abre o normal e devolve o aviso para o log.
pub fn perfil_da_linha(args: impl IntoIterator<Item = String>) -> (Perfil, Option<String>) {
    let args: Vec<String> = args.into_iter().skip(1).collect();
    let valor = args.iter().enumerate().find_map(|(i, a)| {
        if a == "--perfil" {
            Some(args.get(i + 1).cloned().unwrap_or_default())
        } else {
            a.strip_prefix("--perfil=").map(str::to_string)
        }
    });
    match valor.as_deref() {
        None => (Perfil::Normal, None),
        Some("teste") => (Perfil::Teste, None),
        Some(v) => (Perfil::Normal, Some(format!("perfil desconhecido \"{v}\": abrindo o perfil normal (só existe \"teste\")"))),
    }
}

/// A pasta do usuário (`%USERPROFILE%`; no Mac, `~`).
pub fn pasta_do_usuario() -> PathBuf {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
}

/// A pasta de dados do perfil desta execução.
pub fn base() -> PathBuf {
    match perfil() {
        Perfil::Teste => base_do_perfil(Perfil::Teste, &pasta_do_usuario()),
        Perfil::Normal => {
            match std::env::var_os(VAR_BASE).filter(|p| !p.is_empty()) {
                Some(p) => PathBuf::from(p),
                None => base_do_perfil(Perfil::Normal, &pasta_do_usuario()),
            }
        }
    }
}

/// A pasta de dados de um perfil dentro da pasta do usuário (sem a variável de ambiente: a do
/// teste nunca a segue).
pub fn base_do_perfil(p: Perfil, usuario: &Path) -> PathBuf {
    match p {
        Perfil::Normal => usuario.join(PASTA_DADOS),
        Perfil::Teste => usuario.join(PASTA_PERFIL_TESTE),
    }
}

/// A pasta do Claude Code deste usuário (`~\\.claude`), nos dois perfis: só para ler (se o Claude
/// Code está instalado, e os projetos recentes em `projects`; rodada 18). Quem grava usa o
/// `settings_claude()`, que no perfil de teste é o simulado.
pub fn pasta_claude() -> PathBuf {
    pasta_do_usuario().join(".claude")
}

/// O `settings.json` do Claude Code deste perfil: o real (`~\.claude\settings.json`) ou, no de
/// teste, o simulado, dentro da pasta dele.
pub fn settings_claude() -> PathBuf {
    settings_claude_do_perfil(perfil(), &pasta_do_usuario())
}

pub fn settings_claude_do_perfil(p: Perfil, usuario: &Path) -> PathBuf {
    match p {
        Perfil::Normal => usuario.join(".claude").join("settings.json"),
        Perfil::Teste => base_do_perfil(Perfil::Teste, usuario).join(SETTINGS_SIMULADO),
    }
}

pub fn banco() -> PathBuf {
    banco_em(&base())
}

pub fn banco_em(base: &Path) -> PathBuf {
    base.join("worktimer.db")
}

pub fn fila() -> PathBuf {
    fila_em(&base())
}

pub fn fila_em(base: &Path) -> PathBuf {
    base.join("fila").join("eventos.jsonl")
}

pub fn hook_json() -> PathBuf {
    base().join("hook.json")
}

pub fn backups() -> PathBuf {
    base().join("backups")
}

/// Onde fica a cópia do hook que os comandos do `settings.json` chamam.
pub fn hook_exe() -> PathBuf {
    hook_exe_em(&base())
}

pub fn hook_exe_em(base: &Path) -> PathBuf {
    base.join("bin").join(format!("{HOOK_NOME}{}", std::env::consts::EXE_SUFFIX))
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn perfil_de_teste_nunca_aponta_para_o_de_uso_nem_para_o_claude() {
        let u = Path::new("C:/Users/usuario");
        let t = base_do_perfil(Perfil::Teste, u);
        assert_eq!(t, u.join(".worktimer-perfil-teste"));
        assert_eq!(base_do_perfil(Perfil::Normal, u), u.join(".worktimer"));
        let normal = u.join(".worktimer");
        let claude = u.join(".claude");
        // tudo o que o perfil de teste lê e grava fica dentro da pasta dele
        let s = settings_claude_do_perfil(Perfil::Teste, u);
        assert_eq!(s, t.join("settings-claude-simulado.json"));
        for p in [t.clone(), banco_em(&t), fila_em(&t), hook_exe_em(&t), t.join("hook.json"), t.join("backups"), s] {
            assert!(p.starts_with(&t), "{}", p.display());
            assert!(!p.starts_with(&normal), "{} aponta para o .worktimer", p.display());
            assert!(!p.starts_with(&claude), "{} aponta para o .claude", p.display());
        }
        assert_eq!(settings_claude_do_perfil(Perfil::Normal, u), claude.join("settings.json"));
    }

    #[test]
    fn perfil_pela_linha_de_comando() {
        let a = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(perfil_da_linha(a(&["wt.exe"])), (Perfil::Normal, None));
        assert_eq!(perfil_da_linha(a(&["wt.exe", "--perfil", "teste"])), (Perfil::Teste, None));
        assert_eq!(perfil_da_linha(a(&["wt.exe", "--bandeja", "--perfil=teste"])), (Perfil::Teste, None));
        let (p, aviso) = perfil_da_linha(a(&["wt.exe", "--perfil", "outro"]));
        assert_eq!(p, Perfil::Normal, "desconhecido: o normal");
        assert!(aviso.unwrap().contains("\"outro\""));
        assert_eq!(perfil_da_linha(a(&["wt.exe", "--perfil"])).0, Perfil::Normal);
        assert_eq!(perfil_da_linha(a(&["--perfil", "teste"])).0, Perfil::Normal, "o nome do exe não conta");
        // sem definir, o perfil é o normal (o hook, os testes)
        assert_eq!(perfil(), Perfil::Normal);
    }

    #[test]
    fn caminhos_dentro_da_base() {
        let b = Path::new("C:/u/.worktimer");
        assert_eq!(banco_em(b), b.join("worktimer.db"));
        assert_eq!(fila_em(b), b.join("fila").join("eventos.jsonl"));
        let h = hook_exe_em(b);
        assert!(h.starts_with(b.join("bin")));
        assert!(h.file_name().unwrap().to_string_lossy().starts_with("work-timer-hook"));
    }
}
