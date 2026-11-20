// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use serde::{Deserialize, Serialize};

use crate::tempo::Ms;

/// Config gravada (JSON). Campos que nao existem mais (ex.: `teto_leitura_s`, sem uso desde que a
/// leitura passou a seguir a conversa aberta) sao ignorados ao carregar e somem ao gravar.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ConfigApp {
    pub tema: String,
    pub dock_botoes: String,
    /// O minimizar da janela principal: `"perguntar"` (padrão), `"bandeja"` (fecha a janela como
    /// o X), `"dock"` (minimiza e mostra o dock) ou `"barra"` (minimiza e só). Substitui o
    /// `dock_ao_minimizar` (verdadeiro vira `"dock"`, falso vira `"barra"`).
    pub ao_minimizar: String,
    pub iniciar_com_windows: bool,
    pub programas_claude: Vec<String>,
    pub uso_s: u32,
    pub inatividade_s: u32,
    pub sessao_parada_s: u32,
    pub corte_min: u32,
    pub arredondamento_min: u32,
    pub pastas_ignoradas: Vec<String>,
    pub dock_x: Option<i32>,
    pub dock_y: Option<i32>,
    pub dock_larg: Option<i32>,
    pub dock_alt: Option<i32>,
    pub dock_expandido: bool,
    pub visao: String,
    /// O X da janela principal: `"perguntar"` (padrão), `"bandeja"` (fecha a janela e segue
    /// na bandeja e no dock) ou `"sair"` (encerra o app).
    pub ao_fechar: String,
    /// Etiqueta com o que cada timer está fazendo no dock ("trabalhando", "lendo", "esperando você").
    pub dock_situacao: bool,
    /// Modo desenvolvedor (Ajustes → Sobre): só com ele os tempos (uso, inatividade, sessão
    /// parada) podem sair dos padrões. Desligado, valem os padrões.
    pub modo_dev: bool,
    /// "Zerar sozinho na virada do dia" (Ajustes → Dia de trabalho), desligado por padrão: no corte
    /// do dia, todos os timers são zerados no instante do corte (rodada 11). Desligado, o número de
    /// cada timer só volta a zero pelo "Zerar".
    pub zerar_na_virada: bool,
    /// Quando o dock aparece (rodada 12): `"sempre"` ("Sempre à vista", o padrão, também para a
    /// config que já existe) ou `"perguntas"` ("Só quando houver uma pergunta": escondido, aparece
    /// sozinho com uma pergunta e se esconde quando não sobra nenhuma).
    pub dock_modo: String,
    /// Quanto tempo o lembrete "esperando você" (e o "segundo plano") fica à vista no dock
    /// recolhido, em minutos (rodada 13): 15, 30 (o padrão), 60 ou 120; 0 = até a virada do dia.
    pub espera_dock_min: u32,
    /// O mesmo no dock expandido e no app: 0 (o padrão) = até a virada do dia.
    pub espera_app_min: u32,
    /// Quanto tempo o timer pausado (o ⏸ ou a inatividade; o ■ não) fica no dock recolhido, com o
    /// ▶ para retomar, em minutos (rodada 14): 0 (sai na hora), 5, 10 (o padrão), 30 ou 60.
    pub pausado_dock_min: u32,
    /// Conversa nova numa pasta com regra (rodada 15): `"confirmar"` ("Confirmar sempre", o
    /// padrão: conta no timer da regra e o dock pergunta "Certo?") ou `"regra"` ("Usar a regra
    /// direto": o aviso "Ligada a X pela pasta · Trocar" da rodada 7).
    pub conversa_nova: String,
    /// O tour da janela principal já foi visto, fechado ou recusado (rodada 16): ele não volta
    /// sozinho (só pelo ? do topo e por Ajustes → Ajuda). Falso numa config de antes: quem já usa
    /// o app vê a faixa "Novo: um tour…" uma vez.
    pub tour_visto: bool,
    /// "Dicas na tela" (Ajustes → Ajuda, rodada 16): ligadas por padrão.
    pub dicas_ligadas: bool,
    /// As dicas já mostradas (cada uma aparece uma vez só); "Mostrar as dicas de novo" esvazia.
    pub dicas_vistas: Vec<String>,
}

impl ConfigApp {
    /// A dica `id` ainda pode aparecer? Ligadas e ainda não vista (quem decide a vez é
    /// `dicas::Dicas`).
    pub fn pode_mostrar_dica(&self, id: &str) -> bool {
        self.dicas_ligadas && !id.is_empty() && !self.dicas_vistas.iter().any(|v| v == id)
    }

    /// A dica `id` fica vista (não aparece de novo). Devolve se mudou.
    pub fn marcar_dica_vista(&mut self, id: &str) -> bool {
        if id.is_empty() || self.dicas_vistas.iter().any(|v| v == id) {
            return false;
        }
        self.dicas_vistas.push(id.to_string());
        true
    }
}

impl Default for ConfigApp {
    fn default() -> Self {
        ConfigApp {
            tema: "escuro".into(),
            dock_botoes: "um".into(),
            ao_minimizar: AO_MINIMIZAR_PERGUNTAR.into(),
            iniciar_com_windows: false,
            programas_claude: ["claude", "code", "windowsterminal", "powershell", "pwsh", "cmd", "antigravity"]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            uso_s: USO_PADRAO_S,
            inatividade_s: INATIVIDADE_PADRAO_S,
            sessao_parada_s: SESSAO_PARADA_PADRAO_S,
            corte_min: 0,
            arredondamento_min: 5,
            pastas_ignoradas: vec![],
            dock_x: None,
            dock_y: None,
            dock_larg: None,
            dock_alt: None,
            dock_expandido: false,
            visao: "painel".into(),
            ao_fechar: AO_FECHAR_PERGUNTAR.into(),
            dock_situacao: true,
            modo_dev: false,
            zerar_na_virada: false,
            dock_modo: DOCK_SEMPRE.into(),
            espera_dock_min: ESPERA_DOCK_PADRAO_MIN,
            espera_app_min: 0,
            pausado_dock_min: PAUSADO_DOCK_PADRAO_MIN,
            conversa_nova: CONVERSA_NOVA_CONFIRMAR.into(),
            tour_visto: false,
            dicas_ligadas: true,
            dicas_vistas: vec![],
        }
    }
}

/// Tempos padrão (os únicos valendo sem o modo desenvolvedor).
pub const USO_PADRAO_S: u32 = 180;
pub const INATIVIDADE_PADRAO_S: u32 = 600;
pub const SESSAO_PARADA_PADRAO_S: u32 = 600;
/// No modo desenvolvedor, a inatividade vai de 1 a 30 min.
pub const INATIVIDADE_MAX_S: u32 = 30 * 60;
/// No modo desenvolvedor, uso recente e sessão parada vão de 1 min a 24 h.
pub const TEMPO_MAX_S: u32 = 24 * 60 * 60;

/// Valores de `ao_fechar`.
pub const AO_FECHAR_PERGUNTAR: &str = "perguntar";
pub const AO_FECHAR_BANDEJA: &str = "bandeja";
pub const AO_FECHAR_SAIR: &str = "sair";

/// Os tempos que o "esperando você" fica à vista (Ajustes → "Esperando você"), em minutos; 0 =
/// até a virada do dia. Fora da lista, vale o padrão de cada lugar.
pub const ESPERA_OPCOES_MIN: [u32; 5] = [15, 30, 60, 120, 0];
pub const ESPERA_DOCK_PADRAO_MIN: u32 = 30;
/// Os tempos do pausado no dock recolhido (Ajustes → "Lembretes no dock"), em minutos; 0 = sai na
/// hora.
pub const PAUSADO_OPCOES_MIN: [u32; 5] = [0, 5, 10, 30, 60];
pub const PAUSADO_DOCK_PADRAO_MIN: u32 = 10;

/// Valores de `conversa_nova`.
pub const CONVERSA_NOVA_CONFIRMAR: &str = "confirmar";
pub const CONVERSA_NOVA_REGRA: &str = "regra";

/// Valores de `dock_modo`.
pub const DOCK_SEMPRE: &str = "sempre";
pub const DOCK_SO_PERGUNTAS: &str = "perguntas";

/// Valores de `ao_minimizar`.
pub const AO_MINIMIZAR_PERGUNTAR: &str = "perguntar";
pub const AO_MINIMIZAR_BANDEJA: &str = "bandeja";
pub const AO_MINIMIZAR_DOCK: &str = "dock";
pub const AO_MINIMIZAR_BARRA: &str = "barra";

/// Menor valor aceito para os tempos (uso, inatividade, sessão parada):
/// zero desligaria a detecção.
pub const TEMPO_MIN_S: u32 = 60;

impl ConfigApp {
    /// Os tempos com nome para mensagens: (rótulo, valor em segundos).
    pub fn tempos(&self) -> [(&'static str, u32); 3] {
        [("uso recente", self.uso_s), ("inatividade", self.inatividade_s), ("sessão parada", self.sessao_parada_s)]
    }

    /// Tempos que valem: sem o modo desenvolvedor, os padrões; com ele, os gravados, subindo para
    /// `TEMPO_MIN_S` os que estão abaixo (config antiga ou corrompida) e descendo a inatividade
    /// para `INATIVIDADE_MAX_S` e os outros para `TEMPO_MAX_S`.
    pub fn normalizada(mut self) -> ConfigApp {
        if !self.modo_dev {
            self.uso_s = USO_PADRAO_S;
            self.inatividade_s = INATIVIDADE_PADRAO_S;
            self.sessao_parada_s = SESSAO_PARADA_PADRAO_S;
            return self;
        }
        for s in [&mut self.uso_s, &mut self.inatividade_s, &mut self.sessao_parada_s] {
            *s = (*s).max(TEMPO_MIN_S);
        }
        self.inatividade_s = self.inatividade_s.min(INATIVIDADE_MAX_S);
        self.uso_s = self.uso_s.min(TEMPO_MAX_S);
        self.sessao_parada_s = self.sessao_parada_s.min(TEMPO_MAX_S);
        self
    }

    /// Algum tempo diferente do padrão?
    pub fn tempos_fora_do_padrao(&self) -> bool {
        (self.uso_s, self.inatividade_s, self.sessao_parada_s)
            != (USO_PADRAO_S, INATIVIDADE_PADRAO_S, SESSAO_PARADA_PADRAO_S)
    }

    /// Config gravada (JSON) → a que vale. Uma config de antes do modo desenvolvedor (sem o campo
    /// `modo_dev`) com tempos personalizados liga o modo sozinha, para não perder os valores.
    /// Uma de antes do `ao_minimizar` traz o `dock_ao_minimizar` para ele.
    pub fn de_json(j: &str) -> serde_json::Result<ConfigApp> {
        let v: serde_json::Value = serde_json::from_str(j)?;
        let antiga = v.get("modo_dev").is_none();
        let minimizar = match (v.get("ao_minimizar"), v.get("dock_ao_minimizar").and_then(|d| d.as_bool())) {
            (None, Some(true)) => Some(AO_MINIMIZAR_DOCK),
            (None, Some(false)) => Some(AO_MINIMIZAR_BARRA),
            _ => None,
        };
        let mut c: ConfigApp = serde_json::from_value(v)?;
        if let Some(m) = minimizar {
            c.ao_minimizar = m.into();
        }
        if antiga && c.tempos_fora_do_padrao() {
            c.modo_dev = true;
        }
        // tempo do "esperando você" fora da lista (config estragada): o padrão
        if !ESPERA_OPCOES_MIN.contains(&c.espera_dock_min) {
            c.espera_dock_min = ESPERA_DOCK_PADRAO_MIN;
        }
        if !ESPERA_OPCOES_MIN.contains(&c.espera_app_min) {
            c.espera_app_min = 0;
        }
        if !PAUSADO_OPCOES_MIN.contains(&c.pausado_dock_min) {
            c.pausado_dock_min = PAUSADO_DOCK_PADRAO_MIN;
        }
        Ok(c.normalizada())
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ConfigMotor {
    pub fuso_min: i32,
    pub corte_min: u32,
    pub uso_ms: Ms,
    pub inatividade_ms: Ms,
    pub sessao_parada_ms: Ms,
    pub programas_claude: Vec<String>,
    pub pastas_ignoradas: Vec<String>,
    /// A conversa nova numa pasta com regra pergunta "Certo?" (`ConfigApp::conversa_nova`).
    pub confirmar_conversa_nova: bool,
}

impl ConfigMotor {
    pub fn de(cfg: &ConfigApp, fuso_min: i32) -> ConfigMotor {
        ConfigMotor {
            fuso_min,
            corte_min: cfg.corte_min,
            uso_ms: cfg.uso_s as Ms * 1000,
            inatividade_ms: cfg.inatividade_s as Ms * 1000,
            sessao_parada_ms: cfg.sessao_parada_s as Ms * 1000,
            programas_claude: cfg.programas_claude.clone(),
            pastas_ignoradas: cfg.pastas_ignoradas.clone(),
            confirmar_conversa_nova: cfg.conversa_nova != CONVERSA_NOVA_REGRA,
        }
    }
}

/// Corte do dia em `HH:MM` → minutos desde a meia-noite (`"03:00"` → 180).
/// Hora com 1 ou 2 dígitos (0–23), minutos com 2 (00–59); qualquer outra coisa é `None`.
pub fn corte_de_texto(texto: &str) -> Option<u32> {
    let (h, m) = texto.trim().split_once(':')?;
    let digitos = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if !digitos(h) || h.len() > 2 || !digitos(m) || m.len() != 2 {
        return None;
    }
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// Minutos desde a meia-noite → `HH:MM` (180 → `"03:00"`).
pub fn texto_de_corte(min: u32) -> String {
    format!("{:02}:{:02}", (min / 60) % 24, min % 60)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn ao_fechar_pergunta_por_padrao_e_vai_e_volta_no_json() {
        let c = ConfigApp::default();
        assert_eq!(c.ao_fechar, "perguntar");
        let j = serde_json::to_value(&c).unwrap();
        assert_eq!(j["ao_fechar"], "perguntar");
        let c2 = ConfigApp { ao_fechar: AO_FECHAR_BANDEJA.into(), ..c };
        let volta: ConfigApp = serde_json::from_str(&serde_json::to_string(&c2).unwrap()).unwrap();
        assert_eq!(volta, c2);
        // config gravada antes do campo existir: carrega com o padrão
        let velha: ConfigApp = serde_json::from_str(r#"{"tema": "claro"}"#).unwrap();
        assert_eq!((velha.ao_fechar.as_str(), velha.tema.as_str()), ("perguntar", "claro"));
    }

    #[test]
    fn ao_minimizar_pergunta_por_padrao_e_traz_o_dock_ao_minimizar() {
        assert_eq!(ConfigApp::default().ao_minimizar, "perguntar");
        assert_eq!(ConfigApp::de_json("{}").unwrap().ao_minimizar, "perguntar");
        let c = ConfigApp::de_json(r#"{"tema": "claro", "dock_ao_minimizar": true}"#).unwrap();
        assert_eq!((c.ao_minimizar.as_str(), c.tema.as_str()), ("dock", "claro"));
        assert_eq!(ConfigApp::de_json(r#"{"dock_ao_minimizar": false}"#).unwrap().ao_minimizar, "barra");
        // o campo novo vale mais que o antigo, e o antigo some ao gravar
        let c = ConfigApp::de_json(r#"{"ao_minimizar": "bandeja", "dock_ao_minimizar": true}"#).unwrap();
        assert_eq!(c.ao_minimizar, "bandeja");
        assert!(serde_json::to_value(&c).unwrap().get("dock_ao_minimizar").is_none());
    }

    #[test]
    fn config_antiga_com_teto_da_leitura_carrega_e_some_ao_gravar() {
        let velha: ConfigApp =
            serde_json::from_str(r#"{"uso_s": 120, "teto_leitura_s": 1200, "tema": "claro"}"#).unwrap();
        assert_eq!((velha.uso_s, velha.tema.as_str()), (120, "claro"));
        let j = serde_json::to_value(&velha).unwrap();
        assert!(j.get("teto_leitura_s").is_none());
    }

    #[test]
    fn sem_modo_dev_os_tempos_sao_os_padroes() {
        let c = ConfigApp { uso_s: 60, inatividade_s: 120, sessao_parada_s: 3600, ..ConfigApp::default() };
        assert!(!c.modo_dev, "desligado por padrão");
        let n = c.clone().normalizada();
        assert_eq!((n.uso_s, n.inatividade_s, n.sessao_parada_s), (180, 600, 600));
        // com o modo dev, valem os gravados
        let n = ConfigApp { modo_dev: true, ..c }.normalizada();
        assert_eq!((n.uso_s, n.inatividade_s, n.sessao_parada_s), (60, 120, 3600));
    }

    #[test]
    fn modo_dev_sobe_o_minimo_e_limita_a_inatividade_a_30_min() {
        let c = ConfigApp { modo_dev: true, uso_s: 0, inatividade_s: 24 * 3600, sessao_parada_s: 59, ..ConfigApp::default() };
        let n = c.normalizada();
        assert_eq!((n.uso_s, n.inatividade_s, n.sessao_parada_s), (60, 1800, 60));
        let n = ConfigApp { modo_dev: true, inatividade_s: 0, ..ConfigApp::default() }.normalizada();
        assert_eq!(n.inatividade_s, 60);
    }

    #[test]
    fn modo_dev_limita_uso_e_sessao_parada_a_24_h() {
        let n = ConfigApp { modo_dev: true, uso_s: 100_000, sessao_parada_s: 9_999_999, ..ConfigApp::default() }.normalizada();
        assert_eq!((n.uso_s, n.sessao_parada_s), (86_400, 86_400));
    }

    #[test]
    fn config_antiga_com_tempos_personalizados_liga_o_modo_dev() {
        // de antes do modo dev, com a inatividade mudada: liga sozinho e guarda os valores
        let c = ConfigApp::de_json(r#"{"inatividade_s": 300, "uso_s": 120, "tema": "claro"}"#).unwrap();
        assert!(c.modo_dev);
        assert_eq!((c.uso_s, c.inatividade_s, c.sessao_parada_s, c.tema.as_str()), (120, 300, 600, "claro"));
        // de antes do modo dev, com os padrões: fica desligado
        let c = ConfigApp::de_json(r#"{"inatividade_s": 600, "uso_s": 180, "sessao_parada_s": 600}"#).unwrap();
        assert!(!c.modo_dev);
        assert!(!ConfigApp::de_json("{}").unwrap().modo_dev);
        // com o campo gravado (desligado de propósito): valem os padrões
        let c = ConfigApp::de_json(r#"{"modo_dev": false, "inatividade_s": 300}"#).unwrap();
        assert_eq!((c.modo_dev, c.inatividade_s), (false, 600));
        // inatividade antiga acima de 30 min: liga e desce para 30
        assert_eq!(ConfigApp::de_json(r#"{"inatividade_s": 86400}"#).unwrap().inatividade_s, 1800);
        assert!(ConfigApp::de_json("{ quebrado").is_err());
    }

    #[test]
    fn situacao_no_dock_ligada_por_padrao_e_config_antiga_sem_os_campos_novos() {
        let c = ConfigApp::default();
        assert!(c.dock_situacao);
        let velha: ConfigApp = serde_json::from_str(r#"{"tema": "claro"}"#).unwrap();
        assert!(velha.dock_situacao);
        assert!(!velha.modo_dev);
    }

    #[test]
    fn config_corte_hhmm() {
        assert_eq!(corte_de_texto("03:00"), Some(180));
        assert_eq!(corte_de_texto("25:00"), None);
        assert_eq!(texto_de_corte(180), "03:00");
        assert_eq!(corte_de_texto("00:00"), Some(0));
        assert_eq!(corte_de_texto(" 23:59 "), Some(23 * 60 + 59));
        assert_eq!(corte_de_texto("3:30"), Some(210));
        for ruim in ["", "03", "03:0", "03:60", "24:00", "-1:00", "a:00", "03:00:00", "003:00", "+3:00"] {
            assert_eq!(corte_de_texto(ruim), None, "{ruim:?}");
        }
        assert_eq!(texto_de_corte(0), "00:00");
        assert_eq!(texto_de_corte(23 * 60 + 59), "23:59");
    }

    #[test]
    fn dock_sempre_a_vista_por_padrao_tambem_na_config_que_ja_existe() {
        assert_eq!(ConfigApp::default().dock_modo, DOCK_SEMPRE);
        let velha = ConfigApp::de_json(r#"{"tema":"claro","dock_botoes":"dois","modo_dev":false}"#).unwrap();
        assert_eq!((velha.dock_modo.as_str(), velha.tema.as_str()), (DOCK_SEMPRE, "claro"));
        let c = ConfigApp { dock_modo: DOCK_SO_PERGUNTAS.into(), ..ConfigApp::default() };
        let volta = ConfigApp::de_json(&serde_json::to_string(&c).unwrap()).unwrap();
        assert_eq!(volta.dock_modo, DOCK_SO_PERGUNTAS);
    }

    #[test]
    fn esperando_voce_30_min_no_dock_e_ate_a_virada_no_app_por_padrao() {
        let c = ConfigApp::default();
        assert_eq!((c.espera_dock_min, c.espera_app_min), (30, 0));
        let velha = ConfigApp::de_json(r#"{"tema":"claro","modo_dev":false}"#).unwrap();
        assert_eq!((velha.espera_dock_min, velha.espera_app_min), (30, 0));
        let c = ConfigApp::de_json(r#"{"modo_dev":false,"espera_dock_min":120,"espera_app_min":15}"#).unwrap();
        assert_eq!((c.espera_dock_min, c.espera_app_min), (120, 15));
        let c = ConfigApp::de_json(r#"{"modo_dev":false,"espera_dock_min":7,"espera_app_min":99}"#).unwrap();
        assert_eq!((c.espera_dock_min, c.espera_app_min), (30, 0), "fora da lista: o padrão");
    }

    #[test]
    fn pausado_fica_10_min_no_dock_por_padrao() {
        assert_eq!(ConfigApp::default().pausado_dock_min, 10);
        assert_eq!(ConfigApp::de_json(r#"{"modo_dev":false}"#).unwrap().pausado_dock_min, 10, "config antiga");
        assert_eq!(ConfigApp::de_json(r#"{"modo_dev":false,"pausado_dock_min":0}"#).unwrap().pausado_dock_min, 0);
        assert_eq!(ConfigApp::de_json(r#"{"modo_dev":false,"pausado_dock_min":7}"#).unwrap().pausado_dock_min, 10);
    }
}
