// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! A leitura no terminal sem o título (rodada 28, opção B). O Claude Code no terminal (e no VS Code)
//! não põe o nome da sessão no título da janela, então a conversa aberta não é achada pelo título.
//! O motor liga a sessão à janela em que você escreveu: num `UserPromptSubmit`, a amostra mais
//! próxima do evento (até `LIGAR_JANELA_MS` antes ou depois) diz qual janela estava em foco. Depois,
//! com essa janela em uso e o título sem nome de sessão, a conversa aberta é a sessão ligada a ela
//! que terminou por último.
//!
//! Spec 002: pela janela, só a sessão que espera você, até `ESPERA_RECENTE_MS` depois do fim do
//! turno (o `Stop`), e o `/exit` (`SessionEnd`) solta a janela. Sem isso, a mesma janela usada
//! horas depois para outra coisa (outra aba, um `npm run dev`) contava como leitura.
//!
//! Privacidade: o identificador da janela (o HWND) fica só em memória. Ele nunca vai para o banco,
//! para log, para arquivo nem para mensagem de erro (`Sessao` não tem o campo).
use std::collections::VecDeque;

use super::leitura::ESPERA_RECENTE_MS;
use super::manual::Amostra;
use super::{e_sessao_do_chat, Motor, ORIGEM_APP_DO_CLAUDE, ORIGEM_CHAT};
use crate::modelo::EstadoSessao;
use crate::tempo::{Ms, MIN};

/// A amostra que liga a sessão à janela fica a até isto do `UserPromptSubmit`, antes ou depois.
pub const LIGAR_JANELA_MS: Ms = 6_000;
/// As amostras guardadas para ligar: os últimos 45 s (uns 30 s mais a folga da fila do WSL, lida a
/// cada 30 s).
pub const AMOSTRAS_GUARDADAS_MS: Ms = 45_000;
/// Uma ligação sem evento novo por mais que isto cai (o Windows reaproveita o HWND).
pub const LIGACAO_JANELA_MAX_MS: Ms = 24 * 60 * MIN;

/// O que o motor guarda de cada amostra recente: o instante, a janela (se a amostra vale para
/// ligar: um programa do Claude que não é o app, em foco) e o programa.
#[derive(Clone, Debug, PartialEq)]
pub struct AmostraDaJanela {
    pub t: Ms,
    pub janela: Option<isize>,
    pub processo: Option<String>,
}

/// As amostras recentes, as ligações e os pedidos de ligação esperando a amostra seguinte. Só em
/// memória.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Janelas {
    pub amostras: VecDeque<AmostraDaJanela>,
    /// sessão → (janela, quando foi ligada)
    pub ligadas: std::collections::BTreeMap<String, (isize, Ms)>,
    /// (sessão, instante do `UserPromptSubmit`) ainda sem a amostra seguinte
    pub pendentes: Vec<(String, Ms)>,
}

impl Motor {
    /// A janela da amostra, se ela vale para ligar: em foco, de um programa de `programas_claude`
    /// que não é o app do Claude (`claude`), fora das janelas do Chronn.
    fn janela_que_liga(&self, a: &Amostra) -> Option<isize> {
        let p = a.processo.as_deref()?.to_lowercase();
        let do_claude = p != "claude" && self.cfg.programas_claude.iter().any(|c| c.to_lowercase() == p);
        (a.em_foco && !a.neutra && do_claude).then_some(a.janela).flatten()
    }

    /// Guarda a amostra (só os últimos `AMOSTRAS_GUARDADAS_MS`) e resolve os pedidos de ligação que
    /// esperavam por ela.
    pub(crate) fn guardar_amostra_da_janela(&mut self, a: &Amostra) {
        let janela = self.janela_que_liga(a);
        self.janelas.amostras.push_back(AmostraDaJanela { t: a.t, janela, processo: a.processo.clone() });
        // os pendentes primeiro, com as amostras de antes ainda guardadas (revisão M12: a amostra
        // seguinte pode chegar mais de 45 s depois, com o PC suspenso ou um ciclo lento)
        let pendentes = std::mem::take(&mut self.janelas.pendentes);
        for (sessao, t) in pendentes {
            // a amostra seguinte chegou: decide agora
            if a.t >= t {
                self.ligar_pela_amostra(&sessao, t);
            } else {
                self.janelas.pendentes.push((sessao, t));
            }
        }
        // a poda guarda o que um pendente ainda pode usar
        let mut limite = a.t - AMOSTRAS_GUARDADAS_MS;
        if let Some(t) = self.janelas.pendentes.iter().map(|(_, t)| *t).min() {
            limite = limite.min(t - LIGAR_JANELA_MS);
        }
        let j = &mut self.janelas;
        while j.amostras.front().is_some_and(|x| x.t < limite) {
            j.amostras.pop_front();
        }
    }

    /// Um `UserPromptSubmit` da sessão em `t`: se ela pode ler pela janela (não é do app do Claude
    /// nem do Chat), liga agora se a amostra seguinte já chegou; senão, fica pendente.
    pub(crate) fn pedir_ligacao_da_janela(&mut self, sessao_id: &str, t: Ms) {
        if !self.pode_ler_pela_janela(sessao_id) {
            return;
        }
        let depois = self.janelas.amostras.iter().any(|x| x.t >= t);
        if depois {
            self.ligar_pela_amostra(sessao_id, t);
        } else {
            self.janelas.pendentes.retain(|(s, _)| s != sessao_id);
            self.janelas.pendentes.push((sessao_id.to_string(), t));
        }
    }

    /// A sessão pode ser lida pela janela: existe, não é do Chat e não veio do app do Claude.
    fn pode_ler_pela_janela(&self, sessao_id: &str) -> bool {
        !e_sessao_do_chat(sessao_id)
            && self
                .sessoes
                .get(sessao_id)
                .is_some_and(|s| !matches!(s.origem.as_deref(), Some(ORIGEM_APP_DO_CLAUDE) | Some(ORIGEM_CHAT)))
    }

    /// Liga a sessão à janela da amostra mais próxima de `t` que vale para ligar, a até
    /// `LIGAR_JANELA_MS`. Nenhuma: a ligação de antes (se havia) fica.
    fn ligar_pela_amostra(&mut self, sessao_id: &str, t: Ms) {
        let achada = self
            .janelas
            .amostras
            .iter()
            .filter(|x| (x.t - t).abs() <= LIGAR_JANELA_MS)
            .filter_map(|x| Some(((x.t - t).abs(), x.janela?)))
            .min_by_key(|(d, _)| *d)
            .map(|(_, j)| j);
        if let Some(janela) = achada {
            self.janelas.ligadas.insert(sessao_id.to_string(), (janela, t));
        }
    }

    /// A conversa aberta pela janela: entre as sessões ligadas a `janela` (sem as paradas), a do
    /// evento mais recente; ela só é lida se espera você há menos de `ESPERA_RECENTE_MS` (contados
    /// do fim do turno até a amostra de agora). Nunca uma do app do Claude ou do Chat, uma parada
    /// (`/exit`) ou uma trabalhando (spec 002). Com duas abas na mesma janela, a que trabalha (o
    /// evento mais recente) vence e nada é lido: não dá para saber qual aba está à vista (revisão
    /// I1: melhor faltar do que sobrar).
    pub(crate) fn sessao_pela_janela(&self, janela: Option<isize>) -> Option<String> {
        let janela = janela?;
        let agora = self.ultima_amostra;
        let s = self
            .janelas
            .ligadas
            .iter()
            .filter(|(id, (j, _))| *j == janela && self.pode_ler_pela_janela(id))
            .filter_map(|(id, _)| self.sessoes.get(id))
            .filter(|s| s.estado != EstadoSessao::Parada)
            .max_by_key(|s| (s.ultimo_evento, s.terminou_em))?;
        let fim = s.terminou_em.filter(|_| s.estado == EstadoSessao::EsperandoVoce)?;
        agora.is_none_or(|a| a < fim + ESPERA_RECENTE_MS).then(|| s.id.clone())
    }

    /// O `/exit` (`SessionEnd`): a sessão solta a janela (spec 002).
    pub(crate) fn soltar_janela(&mut self, sessao_id: &str) {
        self.janelas.ligadas.remove(sessao_id);
        self.janelas.pendentes.retain(|(s, _)| s != sessao_id);
    }

    /// As ligações de sessões que saíram da memória caem, e as de mais de 24 h sem evento novo
    /// também (o HWND pode ter sido reaproveitado).
    pub(crate) fn limpar_janelas(&mut self, agora: Ms) {
        let sessoes = &self.sessoes;
        self.janelas.ligadas.retain(|id, (_, em)| {
            sessoes.get(id).is_some_and(|s| agora - (*em).max(s.ultimo_evento) <= LIGACAO_JANELA_MAX_MS)
        });
        self.janelas.pendentes.retain(|(id, _)| sessoes.contains_key(id));
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::{ConfigApp, ConfigMotor};
    use crate::modelo::{EstadoSessao, Mudanca, RegraPasta};
    use crate::motor::{Aberta, Conversa, JanelaEmUso};
    use crate::tempo::ms_local;
    use fila::EventoHook;

    const F: i32 = -180;
    const JANELA: isize = 4242;
    const OUTRA: isize = 777;
    fn h(hh: u32, mm: u32) -> Ms {
        ms_local(2026, 11, 17, hh, mm, F)
    }
    fn motor() -> Motor {
        let app = ConfigApp {
            programas_claude: ["claude", "code", "windowsterminal"].iter().map(|s| s.to_string()).collect(),
            ..ConfigApp::default()
        };
        let mut m = Motor::novo(ConfigMotor::de(&app, F), h(8, 0));
        m.regras_pasta = vec![RegraPasta { id: 1, pasta: "G:\\A".into(), timer_id: 1 }];
        m
    }
    fn ev(evento: &str, sessao: &str, t: Ms, origem: Option<&str>) -> EventoHook {
        EventoHook {
            t,
            evento: evento.into(),
            sessao: sessao.into(),
            cwd: "G:\\A".into(),
            transcript: None,
            notificacao: None,
            origem: origem.map(str::to_string),
            conta: None,
        }
    }
    /// O Windows Terminal em foco, na janela `janela`, com um título sem nome de sessão.
    fn terminal(t: Ms, janela: isize) -> Amostra {
        Amostra {
            t,
            ocioso_ms: 1_000,
            processo: Some("WindowsTerminal".into()),
            titulo: Some("claude".into()),
            janela: Some(janela),
            em_foco: true,
            ..Default::default()
        }
    }
    fn navegador(t: Ms) -> Amostra {
        Amostra { t, ocioso_ms: 1_000, processo: Some("msedge".into()), titulo: Some("Notícias".into()), janela: Some(OUTRA), em_foco: true, ..Default::default() }
    }
    fn aberta(m: &Motor, janela: isize, titulo: &str) -> Aberta {
        m.sessao_aberta(&JanelaEmUso { processo: Some("WindowsTerminal".into()), titulo: Some(titulo.into()), conversa: Conversa::PeloTitulo, janela: Some(janela) })
    }
    const S: Ms = 1_000;

    #[test]
    fn a_sessao_liga_pela_amostra_de_antes_do_evento() {
        let mut m = motor();
        m.amostra(&terminal(h(9, 0) - 2 * S, JANELA));
        m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), Some("cli")));
        assert!(m.janelas.ligadas.is_empty(), "espera a amostra seguinte");
        // a seguinte é de outro programa: vale a de antes, 2 s antes
        m.amostra(&navegador(h(9, 0) + 3 * S));
        assert_eq!(m.janelas.ligadas.get("s1").map(|x| x.0), Some(JANELA));
        m.evento(&ev("Stop", "s1", h(9, 1), Some("cli")));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::Sessao("s1".into()));
        assert_eq!(aberta(&m, OUTRA, "claude"), Aberta::NaoSei, "outra janela: não sei");
    }

    #[test]
    fn a_sessao_liga_pela_amostra_de_depois_do_evento() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), None));
        m.amostra(&terminal(h(9, 0) + 4 * S, JANELA));
        assert_eq!(m.janelas.ligadas.get("s1"), Some(&(JANELA, h(9, 0))));
        // a fila lida atrasada (o WSL): a amostra de depois já está guardada e liga na hora
        let mut m = motor();
        m.amostra(&terminal(h(9, 0) + 4 * S, JANELA));
        m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), Some("wsl")));
        assert_eq!(m.janelas.ligadas.get("s1").map(|x| x.0), Some(JANELA));
    }

    #[test]
    fn o_pendente_liga_pela_amostra_de_antes_mesmo_com_a_seguinte_50_s_depois() {
        // revisão M12: o PC suspenso, ou um ciclo lento, entre as duas amostras
        let mut m = motor();
        m.amostra(&terminal(h(9, 0) - 2 * S, JANELA));
        m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), Some("cli")));
        m.amostra(&navegador(h(9, 0) + 50 * S));
        assert_eq!(m.janelas.ligadas.get("s1").map(|x| x.0), Some(JANELA));
        // e a poda volta ao normal depois
        m.amostra(&navegador(h(9, 2)));
        assert!(m.janelas.amostras.iter().all(|x| x.t >= h(9, 2) - AMOSTRAS_GUARDADAS_MS));
    }

    #[test]
    fn amostra_a_mais_de_seis_segundos_nao_liga() {
        let mut m = motor();
        m.amostra(&terminal(h(9, 0) - 7 * S, JANELA));
        m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), Some("cli")));
        m.amostra(&terminal(h(9, 0) + 7 * S, JANELA));
        assert!(m.janelas.ligadas.is_empty());
        assert!(m.janelas.pendentes.is_empty(), "decidido: nada pendente");
        // a amostra fora de foco também não liga
        m.amostra(&Amostra { em_foco: false, ..terminal(h(9, 1) - S, JANELA) });
        m.evento(&ev("UserPromptSubmit", "s1", h(9, 1), Some("cli")));
        m.amostra(&navegador(h(9, 1) + 4 * S));
        assert!(m.janelas.ligadas.is_empty());
    }

    #[test]
    fn a_amostra_do_app_do_claude_nao_liga() {
        let mut m = motor();
        let app = |t: Ms| Amostra { processo: Some("claude".into()), ..terminal(t, JANELA) };
        m.amostra(&app(h(9, 0) - S));
        m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), Some("cli")));
        m.amostra(&app(h(9, 0) + 4 * S));
        assert!(m.janelas.ligadas.is_empty());
    }

    #[test]
    fn duas_abas_na_mesma_janela_le_a_que_terminou_por_ultimo() {
        let mut m = motor();
        for (s, min) in [("aba1", 0), ("aba2", 2)] {
            m.amostra(&terminal(h(9, min) - S, JANELA));
            m.evento(&ev("UserPromptSubmit", s, h(9, min), Some("cli")));
            m.amostra(&terminal(h(9, min) + 4 * S, JANELA));
        }
        m.evento(&ev("Stop", "aba2", h(9, 5), Some("cli")));
        m.evento(&ev("Stop", "aba1", h(9, 8), Some("cli")));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::Sessao("aba1".into()));
        // a aba2 recebe outra mensagem e termina depois: passa a ser a lida
        m.amostra(&terminal(h(9, 9) - S, JANELA));
        m.evento(&ev("UserPromptSubmit", "aba2", h(9, 9), Some("cli")));
        m.amostra(&terminal(h(9, 9) + 4 * S, JANELA));
        m.evento(&ev("Stop", "aba2", h(9, 10), Some("cli")));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::Sessao("aba2".into()));
        // a nova ligação troca a janela da sessão
        m.amostra(&terminal(h(9, 11) - S, OUTRA));
        m.evento(&ev("UserPromptSubmit", "aba2", h(9, 11), Some("cli")));
        m.amostra(&terminal(h(9, 11) + 4 * S, OUTRA));
        assert_eq!(m.janelas.ligadas["aba2"].0, OUTRA);
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::Sessao("aba1".into()));
    }

    #[test]
    fn o_titulo_continua_tendo_prioridade() {
        let mut m = motor();
        for (s, min) in [("s1", 0), ("s2", 2)] {
            m.amostra(&terminal(h(9, min) - S, JANELA));
            m.evento(&ev("UserPromptSubmit", s, h(9, min), Some("cli")));
            m.amostra(&terminal(h(9, min) + 4 * S, JANELA));
            m.evento(&ev("Stop", s, h(9, min + 1), Some("cli")));
        }
        m.sessoes.get_mut("s1").unwrap().nome = Some("Plano do app".into());
        assert_eq!(aberta(&m, JANELA, "Plano do app - Windows Terminal"), Aberta::Sessao("s1".into()));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::Sessao("s2".into()), "sem o nome no título: a janela");
    }

    #[test]
    fn sessao_do_app_do_claude_nunca_entra_por_janela() {
        let mut m = motor();
        m.amostra(&terminal(h(9, 0) - S, JANELA));
        m.evento(&ev("UserPromptSubmit", "app", h(9, 0), Some("claude-desktop")));
        m.amostra(&terminal(h(9, 0) + 4 * S, JANELA));
        assert!(m.janelas.ligadas.is_empty());
        // nem se a ligação existir por outro caminho
        m.janelas.ligadas.insert("app".into(), (JANELA, h(9, 0)));
        m.evento(&ev("Stop", "app", h(9, 1), Some("claude-desktop")));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::NaoSei);
        // nem o Chat
        m.janelas.ligadas.insert("chat:x".into(), (JANELA, h(9, 0)));
        assert_eq!(m.sessao_pela_janela(Some(JANELA)), None);
    }

    #[test]
    fn a_ligacao_some_com_a_sessao_e_depois_de_24_h_sem_evento() {
        let mut m = motor();
        m.amostra(&terminal(h(9, 0) - S, JANELA));
        m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), Some("cli")));
        m.amostra(&terminal(h(9, 0) + 4 * S, JANELA));
        m.evento(&ev("Stop", "s1", h(9, 1), Some("cli")));
        m.tique(h(9, 2));
        assert!(m.janelas.ligadas.contains_key("s1"));
        m.tique(h(9, 1) + LIGACAO_JANELA_MAX_MS + S);
        assert!(m.janelas.ligadas.is_empty(), "24 h sem evento");
        m.janelas.ligadas.insert("s1".into(), (JANELA, h(9, 0)));
        m.sessoes.remove("s1");
        m.tique(h(9, 3));
        assert!(m.janelas.ligadas.is_empty(), "a sessão saiu da memória");
    }

    /// Spec 002: 9h00 o pedido no Windows Terminal, 9h01 o fim, 9h02 o `/exit`; das 10h às 12h a
    /// mesma janela para outra coisa. Nada de leitura.
    #[test]
    fn terminal_nao_conta_leitura_depois_do_exit() {
        let mut m = motor();
        let mut mud = vec![];
        mud.extend(m.amostra(&terminal(h(9, 0) - S, JANELA)));
        mud.extend(m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), Some("cli"))));
        mud.extend(m.amostra(&terminal(h(9, 0) + 4 * S, JANELA)));
        mud.extend(m.evento(&ev("Stop", "s1", h(9, 1), Some("cli"))));
        mud.extend(m.evento(&ev("SessionEnd", "s1", h(9, 2), Some("cli"))));
        assert!(m.janelas.ligadas.is_empty(), "o /exit solta a janela");
        // dentro da hora do Stop: só o /exit impede a leitura (revisão M8)
        let mut t = h(9, 3);
        while t <= h(9, 55) {
            mud.extend(m.amostra(&terminal(t, JANELA)));
            t += 5 * S;
        }
        assert_eq!(m.lendo_agora(), None, "nada de leitura logo depois do /exit");
        let mut t = h(10, 0);
        while t <= h(12, 0) {
            mud.extend(m.amostra(&terminal(t, JANELA)));
            mud.extend(m.tique(t));
            t += 5 * S;
        }
        assert_eq!(m.lendo_agora(), None);
        assert!(
            !mud.iter().any(|x| matches!(x, Mudanca::Abriu { origem: crate::modelo::Origem::Leitura, .. })),
            "nenhuma leitura aberta"
        );
        // mesmo com a ligação de pé (vinda de antes da correção), a sessão parada não é lida
        m.janelas.ligadas.insert("s1".into(), (JANELA, h(9, 0)));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::NaoSei);
    }

    /// Revisão I1: duas abas do Windows Terminal (uma janela só). A aba 1 termina; a aba 2 recebe
    /// um pedido e trabalha: olhando a janela, nada é lido (não dá para saber qual aba está à vista).
    #[test]
    fn duas_abas_com_uma_trabalhando_nao_le_a_outra() {
        let mut m = motor();
        m.amostra(&terminal(h(9, 0) - S, JANELA));
        m.evento(&ev("UserPromptSubmit", "aba1", h(9, 0), Some("cli")));
        m.amostra(&terminal(h(9, 0) + 4 * S, JANELA));
        m.evento(&ev("Stop", "aba1", h(9, 1), Some("cli")));
        m.amostra(&terminal(h(9, 2) - S, JANELA));
        m.evento(&ev("UserPromptSubmit", "aba2", h(9, 2), Some("cli")));
        m.amostra(&terminal(h(9, 2) + 4 * S, JANELA));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::NaoSei, "a aba 2 trabalha");
        let mut t = h(9, 3);
        while t <= h(9, 10) {
            m.amostra(&terminal(t, JANELA));
            t += 5 * S;
        }
        assert_eq!(m.lendo_agora(), None);
        // a aba 2 termina: ela é a lida
        m.evento(&ev("Stop", "aba2", h(9, 11), Some("cli")));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::Sessao("aba2".into()));
    }

    /// Spec 002: sem o `/exit`, a janela vale só por `ESPERA_RECENTE_MS` depois do fim do turno.
    #[test]
    fn pela_janela_so_ate_uma_hora_depois_do_stop() {
        let mut m = motor();
        m.amostra(&terminal(h(9, 0) - S, JANELA));
        m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), Some("cli")));
        m.amostra(&terminal(h(9, 0) + 4 * S, JANELA));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::NaoSei, "trabalhando: não é lida pela janela");
        m.evento(&ev("Stop", "s1", h(9, 1), Some("cli")));
        m.amostra(&navegador(h(9, 30)));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::Sessao("s1".into()), "29 min depois do fim");
        m.amostra(&navegador(h(10, 0) + 59 * S));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::Sessao("s1".into()), "um segundo antes da hora");
        m.amostra(&navegador(h(10, 1)));
        assert_eq!(aberta(&m, JANELA, "claude"), Aberta::NaoSei, "uma hora depois do fim");
        // lendo no terminal quando a hora vence: a leitura fecha
        let mut m = motor();
        m.amostra(&terminal(h(9, 0) - S, JANELA));
        m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), Some("cli")));
        m.amostra(&terminal(h(9, 0) + 4 * S, JANELA));
        m.evento(&ev("Stop", "s1", h(9, 1), Some("cli")));
        let mut t = h(9, 50);
        while t <= h(10, 10) {
            m.amostra(&terminal(t, JANELA));
            t += 5 * S;
        }
        assert_eq!(m.lendo_agora(), None, "a leitura pela janela para uma hora depois do Stop");
        // uma mensagem nova liga de novo
        m.amostra(&terminal(h(10, 11) - S, JANELA));
        m.evento(&ev("UserPromptSubmit", "s1", h(10, 11), Some("cli")));
        m.amostra(&terminal(h(10, 11) + 4 * S, JANELA));
        m.evento(&ev("Stop", "s1", h(10, 12), Some("cli")));
        m.amostra(&terminal(h(10, 13), JANELA));
        m.amostra(&terminal(h(10, 13) + 5 * S, JANELA));
        assert_eq!(m.lendo_agora(), Some("s1"));
    }

    #[test]
    fn a_leitura_conta_pela_janela_e_o_hwnd_nao_vai_nas_mudancas() {
        let mut m = motor();
        let mut mud = vec![];
        mud.extend(m.amostra(&terminal(h(9, 0) - S, JANELA)));
        mud.extend(m.evento(&ev("UserPromptSubmit", "s1", h(9, 0), Some("cli"))));
        mud.extend(m.amostra(&terminal(h(9, 0) + 4 * S, JANELA)));
        mud.extend(m.evento(&ev("Stop", "s1", h(9, 1), Some("cli"))));
        assert_eq!(m.sessoes["s1"].estado, EstadoSessao::EsperandoVoce);
        // no terminal da sessão, sem o nome no título: a leitura abre (troca suave)
        mud.extend(m.amostra(&terminal(h(9, 2), JANELA)));
        mud.extend(m.amostra(&terminal(h(9, 2) + 5 * S, JANELA)));
        assert_eq!(m.lendo_agora(), Some("s1"));
        assert!(mud.iter().any(|x| matches!(x, Mudanca::Abriu { origem: crate::modelo::Origem::Leitura, .. })));
        let texto = serde_json::to_string(&mud).unwrap();
        assert!(!texto.contains(&JANELA.to_string()), "o HWND fica só em memória: {texto}");
    }
}
