// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Quem mostra as dicas (rodada 16, revisão): uma por vez no app inteiro (a janela principal e o
//! dock), nunca durante o tour nem com um diálogo aberto na janela principal. A dica só conta
//! como vista quando sai pela pessoa ("Entendi", "Não mostrar mais dicas") ou pelo prazo; a que
//! foi fechada no meio (o tour começou, um diálogo abriu, a tela mudou) volta depois.

use serde::Serialize;

use crate::config::ConfigApp;

/// A resposta a "posso mostrar a dica agora?".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Vez {
    /// Mostre (ela fica "à vista" até `saiu`).
    Mostrar,
    /// Agora não (outra dica à vista, o tour, um diálogo): tente de novo depois.
    Esperar,
    /// Nunca (desligadas, já vista, ou uma dica que não existe).
    Nao,
}

/// O estado das dicas na memória (não vai para o banco).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Dicas {
    /// A dica à vista agora, em qualquer janela.
    pub mostrando: Option<String>,
    /// A janela que mostra a dica (o label: "main" ou "dock"; re-revisão).
    pub dono: Option<String>,
    /// A janela principal está com o tour ou com um diálogo aberto.
    pub ocupada: bool,
}

impl Dicas {
    /// A janela `janela` quer mostrar a dica `id`.
    pub fn pedir(&mut self, cfg: &ConfigApp, id: &str, janela: &str) -> Vez {
        if !cfg.pode_mostrar_dica(id) {
            return Vez::Nao;
        }
        if self.ocupada || self.mostrando.is_some() {
            return Vez::Esperar;
        }
        self.mostrando = Some(id.to_string());
        self.dono = Some(janela.to_string());
        Vez::Mostrar
    }

    /// A janela sumiu (escondida, fechada ou recarregada): a dica dela sai, sem contar como vista
    /// (volta depois), e, sendo a principal, ela deixa de estar ocupada (re-revisão: a vez não fica
    /// presa). Quando a página volta, ela informa o estado de novo.
    pub fn janela_saiu(&mut self, janela: &str, era_a_principal: bool) {
        if self.dono.as_deref() == Some(janela) {
            self.mostrando = None;
            self.dono = None;
        }
        if era_a_principal {
            self.ocupada = false;
        }
    }

    /// A dica `id` saiu da tela da janela `janela`. `vista`: pela pessoa ou pelo prazo (fica
    /// vista); fechada no meio, não (volta depois). Só vale da janela que mostra a dica (a outra,
    /// ou um aviso atrasado de uma página que já sumiu, não solta a vez nem marca vista;
    /// re-revisão). Devolve se a config mudou (a dica ficou vista).
    pub fn saiu(&mut self, cfg: &mut ConfigApp, id: &str, vista: bool, janela: &str) -> bool {
        if self.mostrando.as_deref() != Some(id) || self.dono.as_deref() != Some(janela) {
            return false;
        }
        self.mostrando = None;
        self.dono = None;
        vista && cfg.marcar_dica_vista(id)
    }

    /// A janela principal ficou ocupada (o tour, um diálogo) ou livre. Ocupada com uma dica à
    /// vista: devolve a dica, que a janela dela fecha (sem contar como vista).
    pub fn ocupar(&mut self, ocupada: bool) -> Option<String> {
        self.ocupada = ocupada;
        if ocupada {
            self.mostrando.clone()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn uma_por_vez_no_app_inteiro_e_nunca_com_a_principal_ocupada() {
        let mut cfg = ConfigApp::default();
        let mut d = Dicas::default();
        assert_eq!(d.pedir(&cfg, "esperando", "main"), Vez::Mostrar);
        // a outra janela (ou a mesma) com outra dica: espera a vez
        assert_eq!(d.pedir(&cfg, "zerar", "main"), Vez::Esperar);
        assert!(d.saiu(&mut cfg, "esperando", true, "main"), "Entendi: fica vista");
        assert_eq!(d.pedir(&cfg, "esperando", "main"), Vez::Nao, "vista não volta");
        // o tour ou um diálogo na principal: espera
        d.ocupar(true);
        assert_eq!(d.pedir(&cfg, "zerar", "main"), Vez::Esperar);
        d.ocupar(false);
        assert_eq!(d.pedir(&cfg, "zerar", "main"), Vez::Mostrar);
        // o tour começa com a dica à vista: ela fecha, sem contar como vista, e volta depois
        assert_eq!(d.ocupar(true), Some("zerar".to_string()));
        assert!(!d.saiu(&mut cfg, "zerar", false, "main"));
        d.ocupar(false);
        assert_eq!(d.pedir(&cfg, "zerar", "main"), Vez::Mostrar, "a interrompida volta");
        d.saiu(&mut cfg, "zerar", true, "main");
        // desligadas: nunca
        cfg.dicas_ligadas = false;
        assert_eq!(d.pedir(&cfg, "pergunta", "main"), Vez::Nao);
        // a saída de uma dica que não está à vista não solta a que está
        cfg.dicas_ligadas = true;
        assert_eq!(d.pedir(&cfg, "pergunta", "main"), Vez::Mostrar);
        d.saiu(&mut cfg, "outra", false, "main");
        assert_eq!(d.mostrando.as_deref(), Some("pergunta"));
        assert_eq!(d.pedir(&cfg, "", "main"), Vez::Nao);
    }

    #[test]
    fn a_janela_que_some_solta_a_vez_sem_marcar_vista() {
        let mut cfg = ConfigApp::default();
        let mut d = Dicas::default();
        // a principal mostra uma dica e é fechada (ou escondida, ou recarregada) sem o clique
        assert_eq!(d.pedir(&cfg, "esperando", "main"), Vez::Mostrar);
        assert_eq!(d.pedir(&cfg, "zerar", "dock"), Vez::Esperar);
        d.janela_saiu("main", true);
        assert_eq!(d, Dicas::default(), "a vez fica livre");
        assert!(cfg.pode_mostrar_dica("esperando"), "a que não foi vista volta depois");
        assert_eq!(d.pedir(&cfg, "zerar", "dock"), Vez::Mostrar, "as dicas não param");
        // a principal some com o tour ou um diálogo: deixa de estar ocupada, e a dica do dock fica
        d.ocupar(true);
        d.janela_saiu("main", true);
        assert!(!d.ocupada);
        assert_eq!(d.mostrando.as_deref(), Some("zerar"));
        // o dock recarregado solta a dele (e não mexe no ocupado da principal)
        d.ocupar(true);
        d.janela_saiu("dock", false);
        assert_eq!((d.mostrando.as_deref(), d.ocupada), (None, true));
        d.saiu(&mut cfg, "zerar", false, "dock");
        assert!(cfg.pode_mostrar_dica("zerar"));
    }

    #[test]
    fn so_a_janela_que_mostra_solta_a_dica() {
        let mut cfg = ConfigApp::default();
        let mut d = Dicas::default();
        assert_eq!(d.pedir(&cfg, "zerar", "dock"), Vez::Mostrar);
        // a principal (ou uma página que já sumiu) avisando por engano: nada muda
        assert!(!d.saiu(&mut cfg, "zerar", true, "main"));
        assert_eq!((d.mostrando.as_deref(), d.dono.as_deref()), (Some("zerar"), Some("dock")));
        assert!(cfg.pode_mostrar_dica("zerar"), "nem fica vista");
        assert_eq!(d.pedir(&cfg, "pergunta", "main"), Vez::Esperar, "a vez continua do dock");
        // a dona avisa: solta e fica vista
        assert!(d.saiu(&mut cfg, "zerar", true, "dock"));
        assert_eq!(d, Dicas::default());
        assert!(!cfg.pode_mostrar_dica("zerar"));
        // depois de solta, um aviso atrasado da mesma janela também não muda nada
        assert_eq!(d.pedir(&cfg, "pergunta", "main"), Vez::Mostrar);
        assert!(!d.saiu(&mut cfg, "zerar", true, "dock"));
        assert_eq!(d.mostrando.as_deref(), Some("pergunta"));
    }
}
