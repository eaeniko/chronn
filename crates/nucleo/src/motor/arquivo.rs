// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Spec 004: um timer escondido (arquivado, ou de grupo ou espaço arquivado) nunca recebe tempo.
//! A regra de pasta dele não vale (sobra a da pasta de cima) e a sessão ligada a ele fica sem
//! timer: a próxima atividade dela pergunta "Em qual timer contar?".
use std::collections::BTreeSet;

use super::{regra_para, Motor};
use crate::modelo::{MotivoFim, Mudanca, Origem, RegraPasta};
use crate::tempo::Ms;

impl Motor {
    /// O timer está escondido (spec 004)?
    pub fn escondido(&self, timer_id: Option<i64>) -> bool {
        timer_id.is_some_and(|t| self.escondidos.contains(&t))
    }

    /// O timer da regra de pasta que vale para `cwd`, sem as regras dos escondidos: sobra a da
    /// pasta de cima, se houver.
    pub fn regra_viva(&self, cwd: &str) -> Option<i64> {
        if self.escondidos.is_empty() {
            return regra_para(&self.regras_pasta, cwd);
        }
        regra_para(&self.regras_vivas(), cwd)
    }

    /// As regras de pasta que valem: sem as dos escondidos (a de um escondido fica no banco, inerte).
    pub fn regras_vivas(&self) -> Vec<RegraPasta> {
        self.regras_pasta.iter().filter(|r| !self.escondidos.contains(&r.timer_id)).cloned().collect()
    }

    /// O "Desfazer" do apagar (spec 004, revisão I2): a sessão que o apagar soltou, e que segue sem
    /// timer, volta ao timer e à ligação de antes, daqui para frente (o que ela contou sem timer
    /// nesse meio tempo fica sem timer; o apagar com Desfazer nunca tem conversa trabalhando), e a
    /// pergunta "Em qual timer contar?" dela sai.
    pub fn religar(&mut self, sessao_id: &str, timer_id: i64, ligacao: Option<crate::modelo::Ligacao>, agora: Ms) -> Vec<Mudanca> {
        if self.escondido(Some(timer_id)) || self.sessoes.get(sessao_id).is_none_or(|s| s.timer_id.is_some()) {
            return vec![];
        }
        let mut out = self.ligar_sessao(sessao_id, timer_id, false, agora);
        if let Some(s) = self.sessoes.get_mut(sessao_id) {
            s.ligacao = ligacao;
            out.push(Mudanca::Sessao(s.clone()));
        }
        self.perguntadas.insert(sessao_id.to_string());
        self.perguntas.retain(|(id, p)| {
            let dela = matches!(p, crate::modelo::Pergunta::SessaoNova { sessao_id: s, .. } if s == sessao_id);
            if dela {
                out.push(Mudanca::PerguntaResolvida(*id));
            }
            !dela
        });
        out
    }

    /// Troca o conjunto dos escondidos (ao abrir e a cada arquivar, desarquivar ou apagar). O que
    /// está aberto num escondido fecha em `agora` (sem as travas do ⏸), as sessões ligadas a eles
    /// ficam sem timer (`soltar_do_escondido`), e eles saem do ■, das perguntas de "contar" e dos
    /// mapas por timer.
    pub fn esconder(&mut self, escondidos: BTreeSet<i64>, agora: Ms) -> Vec<Mudanca> {
        self.escondidos = escondidos;
        let mut out = self.corte_do_dia(agora);
        let ligadas: Vec<String> =
            self.sessoes.values().filter(|s| self.escondido(s.timer_id)).map(|s| s.id.clone()).collect();
        for id in ligadas {
            out.extend(self.soltar_do_escondido(&id, agora));
        }
        let ids = self.escondidos.clone();
        let dele = move |t: &crate::modelo::TrechoAberto| t.timer_id.is_some_and(|x| ids.contains(&x));
        out.extend(self.fechar_se(agora, MotivoFim::Pausa, dele));
        let mut tocados: BTreeSet<i64> = BTreeSet::new();
        tocados.extend(self.pausado_desde.keys().filter(|t| self.escondidos.contains(t)));
        tocados.extend(self.voltar_para.keys().filter(|t| self.escondidos.contains(t)));
        tocados.extend(self.play_armado.keys().filter(|t| self.escondidos.contains(t)));
        tocados.extend(self.desfazer.keys().filter(|t| self.escondidos.contains(t)));
        if let Some((_, ids, _)) = &self.ausencia {
            tocados.extend(ids.iter().filter(|t| self.escondidos.contains(t)));
        }
        for p in &self.perguntas {
            if let crate::modelo::Pergunta::VoltaInatividade { timer_ids, .. } | crate::modelo::Pergunta::AppFechado { timer_ids, .. } = &p.1 {
                tocados.extend(timer_ids.iter().filter(|t| self.escondidos.contains(t)));
            }
        }
        for id in tocados {
            self.pausado_desde.remove(&id);
            self.voltar_para.remove(&id);
            self.play_armado.remove(&id);
            self.desfazer.remove(&id);
            self.tirar_da_ausencia(id);
            out.extend(self.tirar_das_perguntas(Some(id)));
        }
        let antes = self.encerrados.len();
        let escondidos = &self.escondidos;
        self.encerrados.retain(|t| !escondidos.contains(t));
        if self.encerrados.len() != antes {
            out.push(self.mudanca_encerrados());
        }
        out
    }

    /// A sessão ligada a um escondido fica sem timer (nem ligação: não é decisão sua, e a herança
    /// pode ligá-la de novo). O Claude trabalhando nela segue contando, sem timer, a partir de
    /// `agora`, e a pergunta "Em qual timer contar?" sai já; parada, a pergunta sai na próxima
    /// atividade. A leitura dela fecha.
    pub(super) fn soltar_do_escondido(&mut self, sessao_id: &str, agora: Ms) -> Vec<Mudanca> {
        let Some(s) = self.sessoes.get_mut(sessao_id) else {
            return vec![];
        };
        s.timer_id = None;
        s.ligacao = None;
        // revisão C1: ligá-la depois "desde o início" (a resposta da pergunta) leva só o que veio
        // daqui em diante; o que ela contou no escondido fica nele, no histórico
        s.desligada_em = Some(agora);
        let s = s.clone();
        self.ligadas_sozinhas.remove(sessao_id);
        let mut out = self.tirar_conversa_nova(sessao_id);
        let claude = self.abertos.iter().any(|t| t.origem == Origem::Claude && t.sessao_id.as_deref() == Some(sessao_id));
        out.extend(self.fechar_sessao(sessao_id, agora, MotivoFim::Desligada, false));
        if claude {
            out.extend(self.abrir_claude(sessao_id, None, agora));
            self.perguntadas.insert(sessao_id.to_string());
            let cwd = self.pasta_agora.get(sessao_id).cloned().unwrap_or_else(|| s.cwd.clone());
            out.push(self.pergunta_sessao_nova(sessao_id, &cwd));
        } else {
            self.perguntadas.remove(sessao_id);
        }
        out.push(Mudanca::Sessao(s));
        out
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::{ConfigApp, ConfigMotor};
    use crate::modelo::{EstadoSessao, Pergunta};
    use crate::motor::{Amostra, Conversa};
    use crate::tempo::ms_local;
    use fila::EventoHook;

    const F: i32 = -180;
    const SPRINT: i64 = 3;
    fn h(hh: u32, mm: u32) -> Ms {
        ms_local(2026, 11, 20, hh, mm, F)
    }
    fn motor() -> Motor {
        let app = ConfigApp { programas_claude: vec!["claude".into()], ..ConfigApp::default() };
        let mut m = Motor::novo(ConfigMotor::de(&app, F), h(8, 0));
        m.regras_pasta = vec![RegraPasta { id: 1, pasta: r"D:\proj".into(), timer_id: SPRINT }];
        m
    }
    fn ev(evento: &str, sessao: &str, cwd: &str, t: Ms) -> EventoHook {
        EventoHook { t, evento: evento.into(), sessao: sessao.into(), cwd: cwd.into(), transcript: None, notificacao: None, origem: None, conta: None }
    }
    fn trechos(v: &[Mudanca]) -> Vec<Mudanca> {
        v.iter().filter(|m| matches!(m, Mudanca::Abriu { .. } | Mudanca::Fechou { .. })).cloned().collect()
    }
    fn abriu(timer: Option<i64>, t: Ms) -> Mudanca {
        Mudanca::Abriu { timer_id: timer, sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: t, atividade_id: 1 }
    }
    fn perguntas_novas(v: &[Mudanca]) -> usize {
        v.iter().filter(|m| matches!(m, Mudanca::Pergunta(_, Pergunta::SessaoNova { .. }))).count()
    }

    /// O cenário da spec: o "Sprint 12" arquivado tem a regra de `D:\proj`; a conversa nova lá não
    /// conta nele, fica sem timer e pergunta, e nada fica aberto nele (a bandeja não diz "1 rodando").
    #[test]
    fn conversa_nova_na_pasta_do_arquivado_nao_conta_nele_e_pergunta() {
        let mut m = motor();
        assert!(m.esconder([SPRINT].into(), h(8, 0)).is_empty());
        let v = m.evento(&ev("UserPromptSubmit", "s1", r"D:\proj\api", h(9, 0)));
        assert_eq!(trechos(&v), vec![abriu(None, h(9, 0))]);
        assert_eq!(perguntas_novas(&v), 1);
        assert_eq!(m.sessoes["s1"].timer_id, None);
        assert!(!m.abertos.iter().any(|t| t.timer_id == Some(SPRINT)));
    }

    #[test]
    fn a_regra_do_escondido_cai_para_a_da_pasta_de_cima() {
        let mut m = motor();
        m.regras_pasta.push(RegraPasta { id: 2, pasta: "D:\\".into(), timer_id: 1 });
        assert_eq!(m.regra_viva(r"D:\proj\api"), Some(SPRINT));
        m.esconder([SPRINT].into(), h(8, 0));
        assert_eq!(m.regra_viva(r"D:\proj\api"), Some(1));
        let v = m.evento(&ev("UserPromptSubmit", "s1", r"D:\proj", h(9, 0)));
        assert_eq!(trechos(&v), vec![abriu(Some(1), h(9, 0))]);
    }

    /// A conversa trabalhando no timer que foi escondido segue contando, sem timer, do instante
    /// do arquivar, e pergunta já; os eventos seguintes não perguntam de novo.
    #[test]
    fn esconder_solta_a_sessao_trabalhando_sem_buraco() {
        let mut m = motor();
        assert_eq!(trechos(&m.evento(&ev("UserPromptSubmit", "s1", r"D:\proj", h(9, 5)))), vec![abriu(Some(SPRINT), h(9, 5))]);
        let v = m.esconder([SPRINT].into(), h(9, 10));
        assert_eq!(
            trechos(&v),
            vec![
                Mudanca::Fechou { timer_id: Some(SPRINT), sessao_id: Some("s1".into()), origem: Origem::Claude, fim: h(9, 10), motivo: MotivoFim::Desligada },
                abriu(None, h(9, 10)),
            ]
        );
        assert_eq!(perguntas_novas(&v), 1);
        assert!(v.iter().any(|x| matches!(x, Mudanca::Sessao(s) if s.id == "s1" && s.timer_id.is_none() && s.ligacao.is_none())));
        let v = m.evento(&ev("PostToolUse", "s1", r"D:\proj", h(9, 12)));
        assert_eq!(perguntas_novas(&v), 0);
        assert!(trechos(&v).is_empty(), "{v:#?}");
        assert_eq!(m.abertos.len(), 1);
        assert_eq!(m.abertos[0].timer_id, None);
        // revisão C1: a resposta "Contar no 5" leva só o que veio desde o arquivar (9:10), nunca o
        // que a conversa contou no escondido
        assert_eq!(m.sessoes["s1"].desligada_em, Some(h(9, 10)));
        let (id, _) = m.perguntas.iter().find(|(_, p)| matches!(p, Pergunta::SessaoNova { .. })).cloned().unwrap();
        let v = m.responder(id, crate::modelo::Resposta::LigarA { timer_id: 5, criar_regra: false }, h(9, 15));
        assert!(v.contains(&Mudanca::ReatribuirDesde { sessao_id: "s1".into(), timer_id: 5, desde: h(9, 10) }), "{v:#?}");
        assert!(!v.iter().any(|x| matches!(x, Mudanca::Reatribuir { .. })));
    }

    /// O "Desfazer" do apagar religa a sessão que segue sem timer, tira a pergunta dela, e não
    /// mexe na que já foi ligada a outro timer.
    #[test]
    fn religar_volta_a_sessao_solta_e_tira_a_pergunta() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", r"D:\proj", h(9, 5)));
        m.esconder([SPRINT].into(), h(9, 10));
        assert!(m.perguntas.iter().any(|(_, p)| matches!(p, Pergunta::SessaoNova { .. })));
        m.esconder(BTreeSet::new(), h(9, 11));
        let v = m.religar("s1", SPRINT, Some(crate::modelo::Ligacao::Regra), h(9, 12));
        assert_eq!(m.sessoes["s1"].timer_id, Some(SPRINT));
        assert_eq!(m.sessoes["s1"].ligacao, Some(crate::modelo::Ligacao::Regra));
        assert!(m.perguntas.is_empty());
        assert!(v.iter().any(|x| matches!(x, Mudanca::PerguntaResolvida(_))));
        assert_eq!(m.abertos[0].timer_id, Some(SPRINT), "o Claude segue, de novo no timer");
        // já ligada a outro: nada
        assert!(m.religar("s1", 7, None, h(9, 13)).is_empty());
    }

    /// A parada fica sem timer e pergunta na próxima mensagem; a leitura dela não abre.
    #[test]
    fn esconder_solta_a_sessao_parada_e_a_proxima_mensagem_pergunta() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", r"D:\proj", h(9, 0)));
        m.evento(&ev("Stop", "s1", r"D:\proj", h(9, 5)));
        let v = m.esconder([SPRINT].into(), h(9, 10));
        assert!(trechos(&v).is_empty());
        assert_eq!(perguntas_novas(&v), 0);
        assert_eq!(m.sessoes["s1"].estado, EstadoSessao::EsperandoVoce);
        let v = m.evento(&ev("UserPromptSubmit", "s1", r"D:\proj", h(9, 20)));
        assert_eq!(trechos(&v), vec![abriu(None, h(9, 20))]);
        assert_eq!(perguntas_novas(&v), 1);
    }

    /// Defesa: a sessão ligada a um escondido sem passar pelo `esconder` (o conjunto trocado direto)
    /// não é lida, e o próximo evento a solta.
    #[test]
    fn ligada_a_escondido_nao_e_lida_e_o_evento_solta() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "s1", r"D:\proj", h(9, 0)));
        m.evento(&ev("Stop", "s1", r"D:\proj", h(9, 5)));
        m.sessoes.get_mut("s1").unwrap().nome = Some("Conversa".into());
        m.escondidos = [SPRINT].into();
        let amostra = |t| Amostra {
            t,
            ocioso_ms: 1_000,
            processo: Some("claude".into()),
            titulo: Some("Claude".into()),
            conversa: Conversa::App(Some("Conversa".into())),
            em_foco: true,
            ..Default::default()
        };
        for t in [h(9, 6), h(9, 7), h(9, 8)] {
            assert!(trechos(&m.amostra(&amostra(t))).is_empty(), "sem leitura num escondido");
        }
        let v = m.evento(&ev("UserPromptSubmit", "s1", r"D:\proj", h(9, 20)));
        assert_eq!(trechos(&v), vec![abriu(None, h(9, 20))]);
        assert_eq!(m.sessoes["s1"].timer_id, None);
    }

    #[test]
    fn o_play_e_o_ligar_recusam_um_escondido_e_o_esconder_fecha_o_play() {
        let mut m = motor();
        m.cfg.um_play_por_vez = false;
        m.play(SPRINT, h(9, 0));
        m.pausado_desde.insert(SPRINT, h(9, 0));
        let v = m.esconder([SPRINT].into(), h(9, 30));
        assert_eq!(
            trechos(&v),
            vec![Mudanca::Fechou { timer_id: Some(SPRINT), sessao_id: None, origem: Origem::Manual, fim: h(9, 30), motivo: MotivoFim::Pausa }]
        );
        assert!(!m.pausado_desde.contains_key(&SPRINT));
        assert!(m.play(SPRINT, h(9, 40)).is_empty());
        m.evento(&ev("UserPromptSubmit", "s2", r"C:\x", h(9, 41)));
        assert!(m.ligar_sessao("s2", SPRINT, false, h(9, 42)).is_empty());
        assert_eq!(m.sessoes["s2"].timer_id, None);
        // desescondido, volta a valer
        m.esconder(BTreeSet::new(), h(10, 0));
        assert!(!m.play(SPRINT, h(10, 1)).is_empty());
    }
}
