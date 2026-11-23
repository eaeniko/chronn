// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! O Chat do app do Claude (rodada 27). A conversa do Chat vira uma sessão `"chat:<id>"`, sem
//! pasta (`cwd` vazio), que entra pelos mesmos caminhos das sessões do Code: a leitura (a conversa
//! aberta, achada pelo id), a pergunta de sessão nova e o "Claude trabalhando". Sem hooks, o
//! "trabalhando" vem dos sinais da janela (o "Stop response" da conversa aberta e o " Running"
//! dos itens da barra lateral), que viram eventos sintéticos pelo `Motor::evento`.
//!
//! Privacidade: o título da conversa fica só em memória (o `Sessao.nome` do motor, para a tela);
//! o banco grava o nome vazio nas sessões do Chat.
use std::collections::{BTreeMap, BTreeSet};

use fila::EventoHook;

use super::Motor;
use crate::modelo::{EstadoSessao, Mudanca, Sessao};
use crate::tempo::Ms;

/// Começo do id da sessão de uma conversa do Chat.
pub const PREFIXO_CHAT: &str = "chat:";
/// A origem das sessões do Chat (`Sessao.origem`, a coluna `origem` do banco).
pub const ORIGEM_CHAT: &str = "chat";

/// O id da sessão da conversa do Chat `id_conversa`.
pub fn sessao_do_chat(id_conversa: &str) -> String {
    format!("{PREFIXO_CHAT}{id_conversa}")
}

/// A sessão é de uma conversa do Chat?
pub fn e_sessao_do_chat(sessao_id: &str) -> bool {
    sessao_id.starts_with(PREFIXO_CHAT)
}

/// A conversa do Chat à vista e se o Claude responde nela ("Stop response").
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ChatAberto {
    pub id: String,
    pub titulo: String,
    pub parando: bool,
}

/// Os sinais do Chat numa amostra (a janela do app do Claude, à vista ou não). Só valem com
/// `aba_do_chat`: com a aba do Code à vista, nada muda.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SinaisDoChat {
    pub aba_do_chat: bool,
    pub aberta: Option<ChatAberto>,
    /// Os títulos dos itens da barra lateral com o Claude respondendo.
    pub rodando: Vec<String>,
    /// A barra lateral tem itens (sem ela, as conversas que não estão abertas ficam como estavam).
    pub barra: bool,
    /// Os títulos que aparecem em dois ou mais itens da barra lateral (rodando ou não): o
    /// " Running" deles não diz qual conversa responde, mesmo que só uma seja conhecida.
    pub repetidos: Vec<String>,
}

/// Entre duas amostras com sinal, mais que isto sem amostra nenhuma (o laço roda a cada 5 s) conta
/// como lacuna: a resposta que acabou nesse meio fecha no último sinal, não na volta.
pub const LACUNA_SEM_AMOSTRA_MS: Ms = 15_000;

/// Uma sessão do Chat com o Claude trabalhando, pelos sinais (`Motor::chat_trabalhando`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ChatVisto {
    /// A hora do último sinal conclusivo em que ela trabalhava.
    pub ultimo: Ms,
    /// Depois dele veio uma amostra sem sinal conclusivo dela (a aba do Code, a janela fechada, a
    /// consulta atrasada, a barra fechada): se ela sair, a resposta acabou em algum momento da
    /// lacuna, e o `Stop` sai em `ultimo`, nunca na volta.
    pub lacuna: bool,
}

/// O evento sintético de uma conversa do Chat (sem pasta).
fn evento_do_chat(evento: &str, sessao: &str, t: Ms) -> EventoHook {
    EventoHook { t, evento: evento.into(), sessao: sessao.into(), cwd: String::new(), transcript: None, notificacao: None, origem: Some(ORIGEM_CHAT.into()), conta: None }
}

fn chave(titulo: &str) -> String {
    titulo.trim().to_lowercase()
}

impl Motor {
    /// A conversa do Chat foi vista (lida ou nos sinais): a primeira vez, cria a sessão (parada,
    /// sem timer e sem pergunta, para ela aparecer na lista de ligar); depois, só o título em
    /// memória muda.
    pub(crate) fn conhecer_chat(&mut self, id_conversa: &str, titulo: &str, t: Ms) -> Vec<Mudanca> {
        let id = sessao_do_chat(id_conversa);
        let titulo = titulo.trim();
        let nome = (!titulo.is_empty()).then(|| titulo.to_string());
        match self.sessoes.get_mut(&id) {
            Some(s) => {
                let mut mudou = false;
                if nome.is_some() && s.nome != nome {
                    s.nome = nome;
                    mudou = true;
                }
                // a sessão de antes da versão 10 do banco, sem origem
                if s.origem.is_none() {
                    s.origem = Some(ORIGEM_CHAT.into());
                    mudou = true;
                }
                if mudou { vec![Mudanca::Sessao(s.clone())] } else { vec![] }
            }
            None => {
                let s = Sessao {
                    id: id.clone(),
                    cwd: String::new(),
                    nome,
                    transcript: None,
                    timer_id: None,
                    estado: EstadoSessao::Parada,
                    ultimo_evento: t,
                    terminou_em: None,
                    criada_em: t,
                    ligacao: None,
                    desligada_em: None,
                    origem: Some(ORIGEM_CHAT.into()),
                    conta: self.conta_do_app.clone(),
                };
                self.sessoes.insert(id, s.clone());
                vec![Mudanca::Sessao(s)]
            }
        }
    }

    /// A sessão do Chat com este título, se só uma tem (título repetido entre as conhecidas ou na
    /// barra lateral, `repetidos`: nenhuma).
    fn chat_pelo_titulo(&self, titulo: &str, repetidos: &[String]) -> Option<String> {
        let t = chave(titulo);
        if t.is_empty() || repetidos.iter().any(|r| chave(r) == t) {
            return None;
        }
        let mut iguais =
            self.sessoes.values().filter(|s| e_sessao_do_chat(&s.id) && s.nome.as_deref().is_some_and(|n| chave(n) == t));
        let uma = iguais.next()?;
        iguais.next().is_none().then(|| uma.id.clone())
    }

    /// "Claude trabalhando" no Chat, sem hooks. Com a aba do Chat à vista, as que trabalham são a
    /// conversa aberta com o "Stop response" e as da barra lateral com " Running" cujo título é de
    /// uma conversa do Chat conhecida (título repetido: ignora). Comparando com a amostra anterior:
    /// a que entrou ganha um `UserPromptSubmit` (abre o trecho do Claude e, sem timer, a pergunta);
    /// a que continua, um `PostToolUse` (para a regra dos 10 min sem evento não fechar uma resposta
    /// longa); a que saiu, um `Stop`. Sem a aba do Chat, nada (se a resposta acabou fora da vista,
    /// a regra dos 10 min fecha no último evento). Sem a barra lateral, as que não estão abertas
    /// ficam como estavam, sem evento. A que sai depois de uma lacuna (amostras sem sinal dela)
    /// ganha o `Stop` na hora do último sinal em que ela ainda trabalhava, não na volta.
    pub fn sinais_do_chat(&mut self, s: &SinaisDoChat, t: Ms) -> Vec<Mudanca> {
        if !s.aba_do_chat {
            self.sem_sinais_do_chat();
            return vec![];
        }
        let mut out = vec![];
        let aberta = s.aberta.as_ref().map(|a| {
            out.extend(self.conhecer_chat(&a.id, &a.titulo, t));
            sessao_do_chat(&a.id)
        });
        let mut agora: BTreeSet<String> = BTreeSet::new();
        if let (Some(a), Some(id)) = (&s.aberta, &aberta) {
            if a.parando {
                agora.insert(id.clone());
            }
        }
        if s.barra {
            agora.extend(s.rodando.iter().filter_map(|titulo| self.chat_pelo_titulo(titulo, &s.repetidos)));
        }
        let antes = std::mem::take(&mut self.chat_trabalhando);
        let ids: BTreeSet<String> = antes.keys().cloned().chain(agora.iter().cloned()).collect();
        // as que trabalhavam e cujo título passou a se repetir na barra: a barra não diz mais nada
        // delas (re-revisão N3)
        let repetidas: BTreeSet<String> = antes
            .keys()
            .filter(|id| {
                self.sessoes.get(*id).and_then(|x| x.nome.as_deref()).is_some_and(|n| s.repetidos.iter().any(|r| chave(r) == chave(n)))
            })
            .cloned()
            .collect();
        // sem a barra, só a aberta diz algo: as outras seguem como estavam (com a lacuna anotada)
        let sabida = |id: &str| aberta.as_deref() == Some(id) || (s.barra && !repetidas.contains(id));
        let mut depois = BTreeMap::new();
        for id in &ids {
            let visto = antes.get(id).copied();
            let (evento, quando) = match (visto, agora.contains(id)) {
                (Some(v), _) if !sabida(id) => {
                    depois.insert(id.clone(), ChatVisto { lacuna: true, ..v });
                    continue;
                }
                (None, _) if !sabida(id) => continue,
                (None, true) => ("UserPromptSubmit", t),
                (Some(_), true) => ("PostToolUse", t),
                // sem amostra nenhuma por mais que `LACUNA_SEM_AMOSTRA_MS` (o PC suspenso, um ciclo
                // com erro) também é lacuna (re-revisão N1)
                (Some(v), false) => ("Stop", if v.lacuna || t - v.ultimo > LACUNA_SEM_AMOSTRA_MS { v.ultimo } else { t }),
                (None, false) => continue,
            };
            if evento != "Stop" {
                depois.insert(id.clone(), ChatVisto { ultimo: t, lacuna: false });
            }
            out.extend(self.evento(&evento_do_chat(evento, id, quando)));
        }
        self.chat_trabalhando = depois;
        out
    }

    /// Uma amostra sem sinal conclusivo do Chat: nada muda, mas as que trabalhavam ficam com a
    /// lacuna anotada (`ChatVisto::lacuna`).
    pub(crate) fn sem_sinais_do_chat(&mut self) {
        for v in self.chat_trabalhando.values_mut() {
            v.lacuna = true;
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::{ConfigApp, ConfigMotor};
    use crate::modelo::{MotivoFim, Origem, Pergunta};
    use crate::motor::{Amostra, Conversa};
    use crate::tempo::{ms_local, MIN};

    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms {
        ms_local(2026, 11, 17, hh, mm, F)
    }
    fn motor() -> Motor {
        let app = ConfigApp {
            programas_claude: ["claude", "code", "windowsterminal"].iter().map(|s| s.to_string()).collect(),
            pastas_ignoradas: vec!["G:\\pessoal".into()],
            ..ConfigApp::default()
        };
        Motor::novo(ConfigMotor::de(&app, F), h(8, 0))
    }
    fn aberta(id: &str, titulo: &str, parando: bool) -> SinaisDoChat {
        SinaisDoChat {
            aba_do_chat: true,
            aberta: Some(ChatAberto { id: id.into(), titulo: titulo.into(), parando }),
            rodando: vec![],
            barra: true,
            repetidos: vec![],
        }
    }
    fn barra(rodando: &[&str]) -> SinaisDoChat {
        SinaisDoChat {
            aba_do_chat: true,
            aberta: None,
            rodando: rodando.iter().map(|t| t.to_string()).collect(),
            barra: true,
            repetidos: vec![],
        }
    }
    /// O app do Claude em uso com a conversa do Chat `id` aberta.
    fn le(t: Ms, id: &str, titulo: &str) -> Amostra {
        Amostra {
            t,
            ocioso_ms: 1_000,
            processo: Some("claude".into()),
            titulo: Some("Claude".into()),
            conversa: Conversa::Chat { id: id.into(), titulo: titulo.into() },
            em_foco: true,
            ..Default::default()
        }
    }
    fn trechos(v: &[Mudanca]) -> Vec<Mudanca> {
        v.iter().filter(|m| matches!(m, Mudanca::Abriu { .. } | Mudanca::Fechou { .. })).cloned().collect()
    }
    fn claude(m: &Motor, id: &str) -> bool {
        m.abertos.iter().any(|t| t.origem == Origem::Claude && t.sessao_id.as_deref() == Some(id))
    }
    fn estado(m: &Motor, id: &str) -> EstadoSessao {
        m.sessoes[id].estado
    }

    #[test]
    fn conversa_aberta_pela_primeira_vez_cria_a_sessao_sem_timer_nem_pergunta() {
        let mut m = motor();
        let mud = m.amostra(&le(h(9, 0), "abc", "Receita de bolo"));
        let s = &m.sessoes["chat:abc"];
        assert_eq!((s.cwd.as_str(), s.nome.as_deref(), s.timer_id, s.estado), ("", Some("Receita de bolo"), None, EstadoSessao::Parada));
        assert!(mud.iter().any(|x| matches!(x, Mudanca::Sessao(s) if s.id == "chat:abc")));
        assert!(m.perguntas.is_empty(), "ver a conversa não pergunta nada");
        // o título muda em memória; sem mudança, nada
        assert!(m.amostra(&le(h(9, 1), "abc", "Receita de bolo")).iter().all(|x| !matches!(x, Mudanca::Sessao(_))));
        m.amostra(&le(h(9, 2), "abc", "Receita de torta"));
        assert_eq!(m.sessoes["chat:abc"].nome.as_deref(), Some("Receita de torta"));
    }

    #[test]
    fn a_conta_da_sessao_a_primeira_fica_e_o_chat_novo_recebe_a_do_app() {
        let mut m = motor();
        let ev = |sessao: &str, origem: &str, conta: Option<&str>, t: Ms| EventoHook {
            t,
            evento: "UserPromptSubmit".into(),
            sessao: sessao.into(),
            cwd: "D:/Projetos/x".into(),
            transcript: None,
            notificacao: None,
            origem: Some(origem.into()),
            conta: conta.map(str::to_string),
        };
        // sem nenhuma conta do app: a conversa do Chat fica sem conta
        m.amostra(&le(h(8, 30), "antes", "Antes"));
        assert_eq!(m.sessoes["chat:antes"].conta, None);
        let mud = m.evento(&ev("t1", "cli", Some("1111111111111111"), h(9, 0)));
        assert!(mud.contains(&Mudanca::Conta { id: "1111111111111111".into(), origem: Some("cli".into()), visto_em: h(9, 0) }));
        assert_eq!(m.sessoes["t1"].conta.as_deref(), Some("1111111111111111"));
        // a primeira não vazia fica; um valor que não é resumo fica de fora
        m.evento(&ev("t1", "cli", Some("2222222222222222"), h(9, 1)));
        assert_eq!(m.sessoes["t1"].conta.as_deref(), Some("1111111111111111"));
        let mud = m.evento(&ev("t2", "cli", Some("0A1B-uuid-de-verdade"), h(9, 2)));
        assert!(!mud.iter().any(|x| matches!(x, Mudanca::Conta { .. })));
        assert_eq!(m.sessoes["t2"].conta, None);
        assert_eq!(m.conta_do_app, None, "o terminal não é o app");
        // a do app do Claude passa às conversas do Chat novas (as de antes ficam como estavam)
        m.evento(&ev("a1", "claude-desktop", Some("3333333333333333"), h(9, 3)));
        assert_eq!(m.conta_do_app.as_deref(), Some("3333333333333333"));
        m.amostra(&le(h(9, 4), "nova", "Nova"));
        assert_eq!(m.sessoes["chat:nova"].conta.as_deref(), Some("3333333333333333"));
        assert_eq!(m.sessoes["chat:antes"].conta, None);
    }

    #[test]
    fn conversa_do_chat_ligada_conta_leitura_e_escrita_como_as_do_code() {
        let mut m = motor();
        m.amostra(&le(h(9, 0), "abc", "Receita"));
        m.ligar_sessao("chat:abc", 4, false, h(9, 1));
        let suave = crate::motor::TROCA_SUAVE_MS;
        m.amostra(&le(h(9, 2), "abc", "Receita"));
        m.amostra(&le(h(9, 2) + suave, "abc", "Receita"));
        assert_eq!(m.lendo_agora(), Some("chat:abc"));
        assert_eq!(m.abertos.iter().find(|t| t.origem == Origem::Leitura).and_then(|t| t.timer_id), Some(4));
        // escrevendo na caixa da conversa do Chat: só a etiqueta
        let escreve = |t: Ms| Amostra { escrevendo: true, ..le(t, "abc", "Receita") };
        m.amostra(&escreve(h(9, 3)));
        m.amostra(&escreve(h(9, 3) + suave));
        assert!(m.escrevendo_agora());
        // achada pelo id; a conversa do Chat sem sessão é desconhecida, e uma do Code com o mesmo
        // título não é a do Chat
        use crate::motor::{Aberta, JanelaEmUso};
        let janela = |conversa: Conversa| JanelaEmUso { conversa, ..Default::default() };
        let chat = |id: &str| Conversa::Chat { id: id.into(), titulo: "Outro título".into() };
        assert_eq!(m.sessao_aberta(&janela(chat("abc"))), Aberta::Sessao("chat:abc".into()));
        assert_eq!(m.sessao_aberta(&janela(chat("zzz"))), Aberta::Desconhecida);
        assert_eq!(m.sessao_aberta(&janela(Conversa::App(Some("Receita".into())))), Aberta::Desconhecida);
        let terminal = JanelaEmUso { titulo: Some("Receita de bolo".into()), ..janela(Conversa::PeloTitulo) };
        m.conhecer_chat("abc", "Receita de bolo", h(9, 4));
        assert_eq!(m.sessao_aberta(&terminal), Aberta::NaoSei, "o título da janela não acha a do Chat");
    }

    #[test]
    fn trabalhando_pelo_stop_response_abre_pergunta_mantem_e_fecha() {
        let mut m = motor();
        let mud = m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 0));
        assert!(claude(&m, "chat:abc"));
        assert_eq!(estado(&m, "chat:abc"), EstadoSessao::Trabalhando);
        // sem timer: a pergunta de sessão nova, sem pasta
        let p = m.perguntas.iter().find_map(|(_, p)| match p {
            Pergunta::SessaoNova { sessao_id, cwd, sugestao, pasta_mae } => Some((sessao_id.clone(), cwd.clone(), sugestao.clone(), *pasta_mae)),
            _ => None,
        });
        assert_eq!(p, Some(("chat:abc".into(), String::new(), String::new(), false)));
        assert!(trechos(&mud).iter().any(|x| matches!(x, Mudanca::Abriu { origem: Origem::Claude, inicio, .. } if *inicio == h(9, 0))));
        // uma resposta longa: o PostToolUse a cada amostra não deixa a regra dos 10 min fechar
        for min in 1..=15 {
            m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, min));
            m.tique(h(9, min));
        }
        assert!(claude(&m, "chat:abc"));
        assert_eq!(m.sessoes["chat:abc"].ultimo_evento, h(9, 15));
        // acabou: Stop (na amostra seguinte, 5 s depois)
        let mud = m.sinais_do_chat(&aberta("abc", "Receita", false), h(9, 15) + 5_000);
        assert!(!claude(&m, "chat:abc"));
        assert_eq!(estado(&m, "chat:abc"), EstadoSessao::EsperandoVoce);
        assert!(trechos(&mud).iter().any(|x| matches!(x, Mudanca::Fechou { origem: Origem::Claude, fim, motivo: MotivoFim::Terminou, .. } if *fim == h(9, 15) + 5_000)));
        assert_eq!(m.perguntas.len(), 1, "a pergunta sai uma vez só");
    }

    #[test]
    fn trabalhando_pela_barra_lateral_so_de_titulo_conhecido_e_unico() {
        let mut m = motor();
        m.conhecer_chat("a", "Receita", h(9, 0));
        m.conhecer_chat("b", "Plano", h(9, 0));
        m.conhecer_chat("c", "Plano", h(9, 0));
        m.sinais_do_chat(&barra(&["Receita", "Plano", "Desconhecida"]), h(9, 1));
        assert!(claude(&m, "chat:a"));
        assert!(!claude(&m, "chat:b") && !claude(&m, "chat:c"), "título repetido: ignora");
        assert_eq!(m.sessoes.len(), 3, "o título desconhecido não cria sessão");
        m.sinais_do_chat(&barra(&[]), h(9, 2));
        assert!(!claude(&m, "chat:a"));
        assert_eq!(estado(&m, "chat:a"), EstadoSessao::EsperandoVoce);
        // a aberta e a da barra juntas
        let mut s = aberta("b", "Plano novo", true);
        s.rodando = vec!["Receita".into(), "Plano novo".into()];
        m.sinais_do_chat(&s, h(9, 3));
        assert!(claude(&m, "chat:a") && claude(&m, "chat:b"));
    }

    #[test]
    fn titulo_repetido_na_barra_nao_vira_a_conversa_conhecida() {
        // duas conversas "Receita" na barra, só uma conhecida nesta execução: a outra responde
        let mut m = motor();
        m.conhecer_chat("a", "Receita", h(9, 0));
        let mut s = barra(&["Receita"]);
        s.repetidos = vec!["receita".into()];
        m.sinais_do_chat(&s, h(9, 1));
        assert!(!claude(&m, "chat:a"), "o Running não diz qual das duas");
        // com o título único na barra, vale
        m.sinais_do_chat(&barra(&["Receita"]), h(9, 2));
        assert!(claude(&m, "chat:a"));
    }

    #[test]
    fn aba_do_code_nao_fecha_nada_e_dez_min_sem_sinal_fecham_no_ultimo_evento() {
        let mut m = motor();
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 0));
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 2));
        // a aba do Code (ou a janela minimizada): nenhum evento
        let code = SinaisDoChat::default();
        for min in 3..=11 {
            assert!(m.sinais_do_chat(&code, h(9, min)).is_empty());
            m.tique(h(9, min));
        }
        assert!(claude(&m, "chat:abc"), "ainda dentro dos 10 min");
        let mud = m.tique(h(9, 12));
        assert!(!claude(&m, "chat:abc"));
        assert!(trechos(&mud).iter().any(|x| matches!(x, Mudanca::Fechou { origem: Origem::Claude, fim, motivo: MotivoFim::SessaoParou, .. } if *fim == h(9, 2))));
        // de volta à aba do Chat, ainda respondendo: abre de novo
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 20));
        assert!(claude(&m, "chat:abc"));
    }

    /// O fim do trecho do Claude da sessão `id` nas mudanças, se fechou.
    fn fim_do_claude(mud: &[Mudanca], id: &str) -> Option<Ms> {
        mud.iter().find_map(|x| match x {
            Mudanca::Fechou { sessao_id: Some(s), origem: Origem::Claude, fim, .. } if s == id => Some(*fim),
            _ => None,
        })
    }

    #[test]
    fn resposta_que_acabou_fora_da_vista_fecha_no_ultimo_sinal_e_nao_na_volta() {
        let mut m = motor();
        let s: Ms = 1_000;
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 0));
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 0) + 30 * s);
        // a aba do Code até 9:08 (a resposta acaba no meio, fora da vista)
        for min in 1..8 {
            assert!(m.sinais_do_chat(&SinaisDoChat::default(), h(9, min)).is_empty());
            m.tique(h(9, min));
        }
        assert!(claude(&m, "chat:abc"), "ainda dentro dos 10 min");
        // de volta à aba do Chat, sem o "Stop response": o Stop sai no último sinal (9:00:30)
        let mud = m.sinais_do_chat(&aberta("abc", "Receita", false), h(9, 8));
        assert!(!claude(&m, "chat:abc"));
        assert_eq!(fim_do_claude(&mud, "chat:abc"), Some(h(9, 0) + 30 * s));
        assert_eq!(m.sessoes["chat:abc"].terminou_em, Some(h(9, 0) + 30 * s));
        assert_eq!(estado(&m, "chat:abc"), EstadoSessao::EsperandoVoce);
    }

    #[test]
    fn sem_amostra_nenhuma_por_mais_de_15_s_tambem_e_lacuna() {
        // re-revisão N1: o PC suspenso entre 9:00 e 9:06, sem nenhuma amostra no meio
        let mut m = motor();
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 0));
        let mud = m.sinais_do_chat(&aberta("abc", "Receita", false), h(9, 6));
        assert_eq!(fim_do_claude(&mud, "chat:abc"), Some(h(9, 0)));
    }

    #[test]
    fn titulo_que_passa_a_se_repetir_na_barra_nao_da_stop_falso() {
        // re-revisão N3: "Plano" trabalhando pela barra; outro item passa a ter o mesmo título
        let mut m = motor();
        m.conhecer_chat("b", "Plano", h(9, 0));
        m.sinais_do_chat(&barra(&["Plano"]), h(9, 0));
        assert!(claude(&m, "chat:b"));
        let mut dois = barra(&["Plano"]);
        dois.repetidos = vec!["Plano".into()];
        assert!(m.sinais_do_chat(&dois, h(9, 0) + 5_000).is_empty(), "nem Stop nem início novo");
        assert!(claude(&m, "chat:b"), "segue como estava");
        // volta a ser único e já parou: o Stop no último sinal em que trabalhava
        let mud = m.sinais_do_chat(&barra(&[]), h(9, 0) + 10_000);
        assert_eq!(fim_do_claude(&mud, "chat:b"), Some(h(9, 0)));
    }

    #[test]
    fn lacuna_pela_consulta_sem_resposta_ou_pela_barra_fechada_tambem_vale() {
        let mut m = motor();
        let s: Ms = 1_000;
        // a consulta sem resposta (a amostra sem sinais): o Stop no último sinal
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 0));
        m.amostra(&Amostra { t: h(9, 1), ocioso_ms: 1_000, ..Default::default() });
        let mud = m.sinais_do_chat(&aberta("abc", "Receita", false), h(9, 3));
        assert_eq!(fim_do_claude(&mud, "chat:abc"), Some(h(9, 0)));
        // sem lacuna, o Stop sai na amostra em que ela saiu (a seguinte)
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 10));
        let mud = m.sinais_do_chat(&aberta("abc", "Receita", false), h(9, 10) + 5 * s);
        assert_eq!(fim_do_claude(&mud, "chat:abc"), Some(h(9, 10) + 5 * s));
        // a da barra, com a barra fechada no meio: o Stop no último " Running" visto
        m.conhecer_chat("b", "Plano", h(9, 20));
        m.sinais_do_chat(&barra(&["Plano"]), h(9, 20));
        let mut fechada = aberta("abc", "Receita", false);
        fechada.barra = false;
        m.sinais_do_chat(&fechada, h(9, 21));
        assert!(claude(&m, "chat:b"), "sem a barra, segue como estava");
        let mud = m.sinais_do_chat(&barra(&[]), h(9, 25));
        assert_eq!(fim_do_claude(&mud, "chat:b"), Some(h(9, 20)));
    }

    #[test]
    fn sem_a_barra_lateral_as_que_nao_estao_abertas_ficam_como_estavam() {
        let mut m = motor();
        m.conhecer_chat("a", "Receita", h(9, 0));
        m.sinais_do_chat(&barra(&["Receita"]), h(9, 1));
        assert!(claude(&m, "chat:a"));
        // a barra fechada, outra conversa aberta: nada da "a"
        let mut s = aberta("b", "Plano", false);
        s.barra = false;
        let mud = m.sinais_do_chat(&s, h(9, 2));
        assert!(claude(&m, "chat:a"));
        assert!(mud.iter().all(|x| !matches!(x, Mudanca::Sessao(s) if s.id == "chat:a")));
        // a barra de volta, sem a "a" rodando: Stop
        m.sinais_do_chat(&barra(&[]), h(9, 3));
        assert!(!claude(&m, "chat:a"));
    }

    #[test]
    fn pausar_a_resposta_do_chat_so_religa_na_proxima() {
        let mut m = motor();
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 0));
        m.ligar_sessao("chat:abc", 4, true, h(9, 0));
        m.pausar(4, h(9, 1));
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 2));
        assert!(!claude(&m, "chat:abc"), "a mesma resposta não religa");
        m.sinais_do_chat(&aberta("abc", "Receita", false), h(9, 3));
        m.sinais_do_chat(&aberta("abc", "Receita", true), h(9, 4) + MIN);
        assert!(claude(&m, "chat:abc"), "a próxima resposta religa");
    }

    #[test]
    fn sem_pasta_nada_de_regra_nem_heranca() {
        let mut m = motor();
        m.regras_pasta = vec![crate::modelo::RegraPasta { id: 1, pasta: "G:\\".into(), timer_id: 1 }];
        m.conhecer_chat("a", "Receita", h(9, 0));
        m.conhecer_chat("b", "Receita", h(9, 1));
        m.ligar_sessao("chat:a", 3, false, h(9, 2));
        m.sinais_do_chat(&aberta("b", "Receita", true), h(9, 3));
        assert_eq!(m.sessoes["chat:b"].timer_id, None, "nenhuma regra casa com o cwd vazio");
        assert_eq!(m.heranca("chat:b", h(9, 3), |_| true), None, "sem herança entre conversas do Chat");
    }
}
