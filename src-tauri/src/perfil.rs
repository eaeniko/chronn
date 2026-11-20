// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! O perfil desta execução (rodada 17): o normal ou o de teste (`work-timer.exe --perfil teste`).
//! Os caminhos ficam em `fila::caminhos` (o ponto único: a pasta de dados e o `settings.json` do
//! Claude Code); aqui ficam o nome na tela, o identificador do app (que separa a instância única
//! e os dados do WebView) e o "Iniciar com o Windows", que no de teste fica só na config.

pub use fila::caminhos::Perfil;

use crate::APP_NOME;

/// O perfil desta execução (o normal, se ninguém definiu: os testes).
pub fn atual() -> Perfil {
    fila::caminhos::perfil()
}

/// É o perfil de teste?
pub fn e_teste() -> bool {
    atual() == Perfil::Teste
}

/// O nome na bandeja e nos títulos das janelas: "Chronn (teste)" no perfil de teste.
pub fn nome_na_tela() -> String {
    nome_na_tela_de(atual())
}

pub fn nome_na_tela_de(p: Perfil) -> String {
    match p {
        Perfil::Normal => APP_NOME.to_string(),
        Perfil::Teste => format!("{APP_NOME} (teste)"),
    }
}

/// O identificador do app no perfil: o de teste ganha um sufixo. A instância única (o plugin usa
/// o identificador no nome do mutex) e os dados do WebView2 ficam separados, e as duas instâncias
/// rodam juntas.
pub fn identificador(base: &str, p: Perfil) -> String {
    match p {
        Perfil::Normal => base.to_string(),
        Perfil::Teste => format!("{base}.perfil-teste"),
    }
}

/// O nome do mutex da instância única para um identificador (o que o
/// `tauri-plugin-single-instance` usa no Windows: `<identificador>-sim`). Documenta, nos testes,
/// que o identificador do perfil separa a instância.
#[cfg(test)]
pub fn chave_de_instancia(identificador: &str) -> String {
    format!("{identificador}-sim")
}

/// Onde fica o "Iniciar com o Windows" do perfil.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Inicio {
    /// O registro "Run" do Windows (pelo plugin do autostart).
    Registro,
    /// Só a config do perfil (o de teste: o registro nunca muda).
    Config,
}

pub fn inicio_do_perfil(p: Perfil) -> Inicio {
    match p {
        Perfil::Normal => Inicio::Registro,
        Perfil::Teste => Inicio::Config,
    }
}

/// O "Iniciar com o Windows" do perfil: lê (`ligar = None`) ou liga/desliga, pelo registro ou só
/// pela config. Cada lado só é chamado no perfil dele (o de teste nunca chama o do registro).
pub fn aplicar_inicio<R, C>(p: Perfil, ligar: Option<bool>, registro: R, config: C) -> Result<bool, String>
where
    R: FnOnce(Option<bool>) -> Result<bool, String>,
    C: FnOnce(Option<bool>) -> Result<bool, String>,
{
    match inicio_do_perfil(p) {
        Inicio::Registro => registro(ligar),
        Inicio::Config => config(ligar),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn nome_e_identificador_do_perfil() {
        assert_eq!(nome_na_tela_de(Perfil::Normal), APP_NOME);
        assert_eq!(nome_na_tela_de(Perfil::Teste), format!("{APP_NOME} (teste)"));
        assert_eq!(nome_na_tela(), APP_NOME, "sem definir, o normal");
        let id = "io.github.eaeniko.worktimer";
        assert_eq!(identificador(id, Perfil::Normal), id);
        assert_eq!(identificador(id, Perfil::Teste), "io.github.eaeniko.worktimer.perfil-teste");
    }

    #[test]
    fn chave_de_instancia_diferente_no_perfil_de_teste() {
        let id = "io.github.eaeniko.worktimer";
        let normal = chave_de_instancia(&identificador(id, Perfil::Normal));
        let teste = chave_de_instancia(&identificador(id, Perfil::Teste));
        assert_eq!(normal, "io.github.eaeniko.worktimer-sim");
        assert_ne!(normal, teste, "as duas instâncias rodam juntas");
    }

    #[test]
    fn inicio_simulado_nao_chama_o_registro() {
        let registro = |_: Option<bool>| -> Result<bool, String> { panic!("o perfil de teste chamou o registro") };
        let mut guardado = false;
        let r = aplicar_inicio(Perfil::Teste, Some(true), registro, |v| {
            guardado = v.unwrap();
            Ok(guardado)
        });
        assert_eq!(r, Ok(true));
        assert!(guardado);
        assert_eq!(inicio_do_perfil(Perfil::Teste), Inicio::Config);
        // o normal vai pelo registro (e não pela config)
        let r = aplicar_inicio(Perfil::Normal, None, |_| Ok(false), |_| panic!("o normal usou a config"));
        assert_eq!(r, Ok(false));
    }
}
