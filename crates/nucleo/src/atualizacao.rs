// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! O atualizador (rodada 25): as regras que não dependem da rede. A busca e a instalação ficam no
//! app (`tauri-plugin-updater`); aqui, quando a busca automática roda e o rótulo da versão.
//!
//! A busca é a única consulta à internet do app: ela lê o `latest.json` da última release.

use crate::CANAL;

/// A busca automática roda no máximo uma vez a cada 24 h.
pub const INTERVALO_MS: i64 = 24 * 60 * 60 * 1000;

/// A busca automática roda agora (ao abrir o app)? Só com a opção ligada, fora do perfil de teste
/// e se a última busca que deu certo (`ultima`, ms desde 1970) tem 24 h ou mais. Nunca buscou:
/// sim. Uma última busca "no futuro" (o relógio voltou) também conta como vencida, para a busca
/// não ficar presa.
pub fn busca_automatica_devida(ligada: bool, ultima: Option<i64>, agora: i64, perfil_teste: bool) -> bool {
    if perfil_teste || !ligada {
        return false;
    }
    match ultima {
        None => true,
        Some(u) => u > agora || agora - u >= INTERVALO_MS,
    }
}

/// A versão com o canal: "0.4.0 · alpha" (o "Sobre" dos Ajustes e a bandeja).
pub fn versao_com_canal(versao: &str) -> String {
    format!("{versao} · {CANAL}")
}

#[cfg(test)]
mod testes {
    use super::*;

    const H: i64 = 60 * 60 * 1000;
    const AGORA: i64 = 1_800_000_000_000;

    #[test]
    fn a_regra_das_24_horas() {
        // nunca buscou
        assert!(busca_automatica_devida(true, None, AGORA, false));
        // recente: não
        assert!(!busca_automatica_devida(true, Some(AGORA - H), AGORA, false));
        assert!(!busca_automatica_devida(true, Some(AGORA - 24 * H + 1), AGORA, false));
        // antiga: sim (24 h em ponto já vale)
        assert!(busca_automatica_devida(true, Some(AGORA - 24 * H), AGORA, false));
        assert!(busca_automatica_devida(true, Some(AGORA - 30 * 24 * H), AGORA, false));
        // o relógio voltou: a última "no futuro" não prende a busca
        assert!(busca_automatica_devida(true, Some(AGORA + H), AGORA, false));
    }

    #[test]
    fn desligada_ou_no_perfil_de_teste_nunca_busca_sozinha() {
        for ultima in [None, Some(AGORA - 48 * H), Some(AGORA - H)] {
            assert!(!busca_automatica_devida(false, ultima, AGORA, false));
            assert!(!busca_automatica_devida(true, ultima, AGORA, true));
        }
    }

    #[test]
    fn o_rotulo_alpha_vem_da_constante() {
        assert_eq!(CANAL, "alpha");
        assert_eq!(versao_com_canal("0.4.0"), "0.4.0 · alpha");
        assert_eq!(versao_com_canal("1.2.3"), format!("1.2.3 · {CANAL}"));
    }
}
