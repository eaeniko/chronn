// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Motor: maquina de estados pura. Sem IO e sem relogio: todo metodo recebe o instante
//! e devolve as `Mudanca`s que o banco aplica depois.
use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;

use crate::config::ConfigMotor;
use crate::modelo::{
    EstadoSessao, Ligacao, MotivoFim, Mudanca, Origem, Pergunta, RegraPasta, Resposta, Sessao, TrechoAberto,
    ATIVIDADE_TRABALHO,
};
use crate::tempo::{dia_de, inicio_do_dia, proximo_corte, Ms, MIN};

/// App reaberto em menos que isto (reinicio, atualizacao): os manuais seguem sem perguntar.
pub const RETOMADA_MS: Ms = 2 * MIN;

mod arquivo;
mod chat;
mod claude;
mod janela;
mod leitura;
mod manual;

/// A origem das sessões do app do Claude (o `CLAUDE_CODE_ENTRYPOINT` dele).
pub const ORIGEM_APP_DO_CLAUDE: &str = "claude-desktop";

pub use chat::{e_sessao_do_chat, sessao_do_chat, ChatAberto, SinaisDoChat, ORIGEM_CHAT, PREFIXO_CHAT};
pub use claude::{chave_de_pasta, pasta_larga, pastas_entre, raiz_do_projeto, regra_da_pasta, regra_para, sugestao_de_pasta, LigadaSozinha, AVISO_LIGADA_MS, HERANCA_MS};
pub use leitura::{
    Aberta, Conversa, JanelaEmUso, Troca, ESPERA_RECENTE_MS, NEUTRA_MAX_MS, NOME_MIN_NO_TITULO, SAIDAS_PARA_LIBERAR,
    TROCA_SUAVE_MS,
};
pub use janela::{AMOSTRAS_GUARDADAS_MS, LIGACAO_JANELA_MAX_MS, LIGAR_JANELA_MS};
pub use manual::{Amostra, DesfazerEncerrar, DESFAZER_ENCERRAR_MS};

pub struct Motor {
    pub cfg: ConfigMotor,
    pub dia: NaiveDate,
    pub abertos: Vec<TrechoAberto>,
    pub sessoes: BTreeMap<String, Sessao>,
    pub regras_pasta: Vec<RegraPasta>,
    /// Sessoes pausadas (regra 8), com o instante do pause (ou do ■): o Claude so as religa na
    /// proxima mensagem sua. Mensagem de antes do clique (fila lida depois dele) nao libera.
    pub bloqueadas: BTreeMap<String, Ms>,
    /// Leitura pausada (regra 8), com quantas amostras seguidas mostraram que voce saiu da conversa:
    /// so volta depois de sair e voltar, ou de escrever nela.
    pub leitura_bloqueada: BTreeMap<String, u32>,
    /// Timers encerrados hoje (■, "terminei isso por agora"): parados, fora do dock recolhido, sem
    /// o lembrete "esperando voce" e sem leitura das sessoes deles. A proxima mensagem numa sessao
    /// do timer, o play ou a virada do dia desfazem. Grava no banco (`Mudanca::Encerrados`).
    pub encerrados: BTreeSet<i64>,
    /// O que cada ■ recente mudou, para o "Desfazer" (so em memoria, por `DESFAZER_ENCERRAR_MS`).
    pub desfazer: BTreeMap<i64, DesfazerEncerrar>,
    /// Sessao com a conversa aberta no Claude, pela ultima amostra que soube dizer.
    pub conversa_aberta: Option<String>,
    /// Voce escreve na conversa que esta sendo lida (o foco na caixa de mensagem dela, no app do
    /// Claude), ja confirmado por `TROCA_SUAVE_MS`. So a etiqueta muda: conta como leitura.
    pub escrevendo: bool,
    /// Desde quando as amostras dizem o contrario de `escrevendo` (a troca da etiqueta espera
    /// `TROCA_SUAVE_MS`, como a da leitura).
    pub escrevendo_desde: Option<Ms>,
    /// Troca da leitura esperando confirmacao (troca suave): a situacao nova so vale depois de
    /// `TROCA_SUAVE_MS` e entao e aplicada no instante em que aconteceu.
    pub troca: Option<Troca>,
    /// Desde quando a janela em uso e do app (neutra por ate `NEUTRA_MAX_MS`).
    pub neutra_desde: Option<Ms>,
    /// Nesta amostra voce acabou de sair do app, onde tinha entrado neste instante.
    pub saiu_do_app: Option<Ms>,
    pub ultima_amostra: Option<Ms>,
    /// Ausencia por inatividade em curso: instante em que comecou, timers manuais que estavam
    /// rodando e a atividade do play manual de cada um (o "Contar" reabre nela).
    pub ausencia: Option<(Ms, Vec<i64>, BTreeMap<i64, i64>)>,
    /// A atividade padrao de cada timer (Claude, leitura, escrita e ▶ usam ela); timer fora do
    /// mapa usa Trabalho. O app copia do banco ao abrir e ao editar.
    pub atividades_padrao: BTreeMap<i64, i64>,
    /// O play manual que uma troca de atividade substituiu (▶ trocado por 👥): ao parar a outra
    /// atividade (`parar_atividade`), ele volta no mesmo instante. So em memoria.
    pub voltar_para: BTreeMap<i64, i64>,
    pub perguntas: Vec<(u64, Pergunta)>,
    pub prox_pergunta: u64,
    /// Sessoes sem timer que ja geraram `Pergunta::SessaoNova` (a pergunta sai uma vez so), e as
    /// ligadas pela regra que ja geraram o aviso "Ligada a X pela pasta".
    pub perguntadas: BTreeSet<String>,
    /// Sessoes que uma regra de pasta (ou a heranca) acabou de ligar sozinha: o dock avisa por
    /// `AVISO_LIGADA_MS` (so em memoria).
    pub ligadas_sozinhas: BTreeMap<String, LigadaSozinha>,
    /// Sessões com uma tarefa em segundo plano pendente (um comando de shell ou um subagente
    /// lançado e ainda sem a `<task-notification>`; rodada 13) e até quando ela vale (2 h do
    /// lançamento; a que vence por último). Esperando você, a etiqueta é "segundo plano". O app
    /// refaz a cada ciclo pelo que leu dos transcripts (`fim_do_transcript`). Só em memória; não
    /// conta tempo.
    pub segundo_plano: BTreeMap<String, Ms>,
    /// O instante da ação que pausou o play manual de cada timer (revisão da rodada 14): o clique
    /// do ⏸ e, na pausa por inatividade, a volta (quando você mexe de novo) ou a resposta "Deixar
    /// pausado". O tempo do pausado no dock recolhido conta dele. Sai no play, no ■ e na virada.
    /// Só em memória: ao reabrir o app, vale o fim do trecho (`snapshot::pausado_em`).
    pub pausado_desde: BTreeMap<i64, Ms>,
    /// As sessões do Chat com o Claude trabalhando pela última amostra conclusiva dos sinais do
    /// Chat (rodada 27): a próxima amostra compara com elas para gerar os eventos sintéticos. De
    /// cada uma, a hora do último sinal conclusivo em que ela trabalhava e se veio uma amostra sem
    /// sinal dela depois (o `Stop` de uma resposta que acabou fora da vista sai naquela hora).
    pub chat_trabalhando: BTreeMap<String, chat::ChatVisto>,
    /// A pasta de agora de cada sessão do Code (rodada 27): o `cwd` do último evento do hook. Só
    /// em memória; o `Sessao.cwd` gravado segue sendo onde ela começou. A pergunta de sessão nova
    /// oferece as pastas entre as duas para o "Sempre que for na pasta".
    pub pasta_agora: BTreeMap<String, String>,
    /// A conta do Claude do último evento do app do Claude (`claude-desktop`; rodada 28): o Chat não
    /// diz a conta, e as conversas do Chat novas ficam com esta. Só em memória.
    pub conta_do_app: Option<String>,
    /// A leitura no terminal pela janela (rodada 28): as amostras recentes, as sessões ligadas a uma
    /// janela (o HWND) e os pedidos esperando a amostra seguinte. Só em memória.
    pub janelas: janela::Janelas,
    /// A última amostra em que cada sessão estava sendo lida (0.6.1): o lembrete "esperando você"
    /// conta o tempo dele desde a sua última leitura, e não só desde o fim do turno do Claude (sem
    /// isto, a conversa que você acabou de ler sumia do dock ao passar para outra). Só em memória.
    pub lida_em: BTreeMap<String, Ms>,
    /// O ▶ armado para parar quando você sai da conversa (spec 009, `play_para_com_claude`): o
    /// timer em que o turno de uma sessão acabou e desde quando você está fora das conversas dele
    /// (`None`: numa delas). Só em memória.
    pub play_armado: BTreeMap<i64, Option<Ms>>,
    /// Os timers escondidos (spec 004): arquivados, ou de grupo ou espaço arquivado. Não recebem
    /// tempo: a regra de pasta deles não vale e a sessão ligada a um deles fica sem timer. O app
    /// passa pelo `esconder` (ao abrir e a cada arquivar, desarquivar ou apagar).
    pub escondidos: BTreeSet<i64>,
}

impl Motor {
    pub fn novo(cfg: ConfigMotor, agora: Ms) -> Motor {
        let dia = dia_de(agora, cfg.corte_min, cfg.fuso_min);
        Motor {
            cfg,
            dia,
            abertos: vec![],
            sessoes: BTreeMap::new(),
            regras_pasta: vec![],
            bloqueadas: BTreeMap::new(),
            leitura_bloqueada: BTreeMap::new(),
            encerrados: BTreeSet::new(),
            desfazer: BTreeMap::new(),
            conversa_aberta: None,
            escrevendo: false,
            escrevendo_desde: None,
            troca: None,
            neutra_desde: None,
            saiu_do_app: None,
            ultima_amostra: None,
            ausencia: None,
            atividades_padrao: BTreeMap::new(),
            voltar_para: BTreeMap::new(),
            perguntas: vec![],
            prox_pergunta: 1,
            perguntadas: BTreeSet::new(),
            ligadas_sozinhas: BTreeMap::new(),
            segundo_plano: BTreeMap::new(),
            pausado_desde: BTreeMap::new(),
            chat_trabalhando: BTreeMap::new(),
            pasta_agora: BTreeMap::new(),
            conta_do_app: None,
            janelas: janela::Janelas::default(),
            lida_em: BTreeMap::new(),
            play_armado: BTreeMap::new(),
            escondidos: BTreeSet::new(),
        }
    }

    /// A sessão espera você, mas com uma tarefa em segundo plano ainda valendo (menos de 2 h do
    /// lançamento dela): "segundo plano".
    pub fn em_segundo_plano(&self, s: &Sessao, agora: Ms) -> bool {
        s.estado == EstadoSessao::EsperandoVoce && self.segundo_plano.get(&s.id).is_some_and(|ate| agora < *ate)
    }

    /// Fecha (em `fim`, nunca antes do inicio) os abertos que satisfazem `pred`; devolve os fechamentos.
    /// A leitura que ficou sem dividir numa virada do dia (troca suave) e dividida antes de fechar
    /// depois dela; e a troca esperando confirmacao se ajusta (`troca_apos_fechar`). Pause e ■ no
    /// meio de uma saida esperando confirmacao fecham a leitura no instante da saida (voce ja tinha
    /// saido; o clique e depois).
    pub(crate) fn fechar_se(
        &mut self,
        fim: Ms,
        motivo: MotivoFim,
        pred: impl Fn(&TrechoAberto) -> bool,
    ) -> Vec<Mudanca> {
        let mut out = vec![];
        let (fim_leitura, motivo_leitura) = match self.troca.as_ref().and_then(|tr| tr.saiu) {
            Some((saiu, m)) if motivo == MotivoFim::Pausa && saiu < fim => (saiu, m),
            _ => (fim, motivo),
        };
        if motivo != MotivoFim::CorteDoDia
            && self.abertos.iter().any(|t| t.origem == Origem::Leitura && pred(t))
        {
            out.extend(self.dividir_leitura_adiada(fim_leitura));
        }
        let (fechar, manter): (Vec<_>, Vec<_>) = std::mem::take(&mut self.abertos).into_iter().partition(|t| pred(t));
        self.abertos = manter;
        if motivo != MotivoFim::CorteDoDia
            && let Some(f) = fechar.iter().filter(|t| t.origem == Origem::Leitura).map(|t| fim_leitura.max(t.inicio)).max()
        {
            self.troca_apos_fechar(f);
        }
        out.extend(fechar.into_iter().map(|t| {
            let (f, m) = if t.origem == Origem::Leitura { (fim_leitura, motivo_leitura) } else { (fim, motivo) };
            Mudanca::Fechou { fim: f.max(t.inicio), timer_id: t.timer_id, sessao_id: t.sessao_id, origem: t.origem, motivo: m }
        }));
        out
    }

    /// A atividade padrao do timer (sem timer, ou fora do mapa: Trabalho).
    pub fn atividade_padrao(&self, timer_id: Option<i64>) -> i64 {
        timer_id.and_then(|t| self.atividades_padrao.get(&t).copied()).unwrap_or(ATIVIDADE_TRABALHO)
    }

    /// O play manual aberto do timer (um por vez), se houver.
    pub fn manual_aberto(&self, timer_id: i64) -> Option<&TrechoAberto> {
        self.abertos.iter().find(|t| t.origem == Origem::Manual && t.timer_id == Some(timer_id))
    }

    /// Abre o play manual do timer na atividade; com um ja aberto (de qualquer atividade), nada:
    /// o manual e um por vez por timer (a troca e `play_atividade`).
    pub(crate) fn abrir_manual(&mut self, timer_id: i64, atividade_id: i64, inicio: Ms) -> Option<Mudanca> {
        if self.manual_aberto(timer_id).is_some() {
            return None;
        }
        self.pausado_desde.remove(&timer_id);
        self.abertos.push(TrechoAberto {
            timer_id: Some(timer_id),
            sessao_id: None,
            origem: Origem::Manual,
            inicio,
            atividade_id,
        });
        Some(Mudanca::Abriu { timer_id: Some(timer_id), sessao_id: None, origem: Origem::Manual, inicio, atividade_id })
    }

    /// AAAA-MM-DD do dia de trabalho do motor (o dos encerramentos gravados).
    pub fn dia_texto(&self) -> String {
        self.dia.format("%Y-%m-%d").to_string()
    }

    /// O timer foi encerrado hoje (■)?
    pub fn encerrado(&self, timer_id: Option<i64>) -> bool {
        timer_id.is_some_and(|t| self.encerrados.contains(&t))
    }

    /// O conjunto de encerrados, para o banco gravar.
    pub(crate) fn mudanca_encerrados(&self) -> Mudanca {
        Mudanca::Encerrados { dia: self.dia_texto(), timers: self.encerrados.iter().copied().collect() }
    }

    /// Tira o timer dos encerrados (mensagem numa sessao dele, play): tudo volta ao normal.
    pub(crate) fn desencerrar(&mut self, timer_id: Option<i64>) -> Option<Mudanca> {
        let id = timer_id?;
        self.desfazer.remove(&id);
        self.encerrados.remove(&id).then(|| self.mudanca_encerrados())
    }

    /// Ao abrir o app: os encerramentos gravados so valem no mesmo dia de trabalho.
    pub fn restaurar_encerrados(&mut self, dia: &str, timers: &[i64]) {
        if dia == self.dia_texto() {
            self.encerrados = timers.iter().copied().collect();
        }
    }

    pub(crate) fn nova_pergunta(&mut self, p: Pergunta) -> Mudanca {
        let id = self.prox_pergunta;
        self.prox_pergunta += 1;
        self.perguntas.push((id, p.clone()));
        Mudanca::Pergunta(id, p)
    }

    /// Passa o dia de trabalho. Manuais fecham no corte e nao reabrem; os demais (claude, leitura...)
    /// fecham e reabrem no mesmo instante.
    pub(crate) fn corte_do_dia(&mut self, agora: Ms) -> Vec<Mudanca> {
        let mut out = vec![];
        let hoje = dia_de(agora, self.cfg.corte_min, self.cfg.fuso_min);
        while self.dia < hoje {
            let prox = self.dia.succ_opt().expect("data fora do intervalo");
            let corte = inicio_do_dia(prox, self.cfg.corte_min, self.cfg.fuso_min);
            // No app desde antes da virada: a leitura fecha quando voce entrou nele (nao conta o
            // tempo neutro no dia velho nem deixa um trecho vazio no novo). Voltar a ela reabre.
            if let Some(desde) = self.neutra_desde.filter(|d| *d < corte) {
                out.extend(self.fechar_se(desde, MotivoFim::SaiuDaJanela, |t| t.origem == Origem::Leitura));
            }
            // Saida da leitura esperando confirmacao (troca suave) desde antes da virada: a leitura
            // nao e dividida agora. Confirmada, ela acaba antes da virada; voltando a ela, e dividida
            // entao (`dividir_leitura_adiada`).
            let adiar = self.troca.as_ref().and_then(|tr| tr.saiu).is_some_and(|(s, _)| s < corte);
            let fica = |t: &TrechoAberto| adiar && t.origem == Origem::Leitura;
            let seguem: Vec<TrechoAberto> =
                self.abertos.iter().filter(|t| t.origem != Origem::Manual && !fica(t)).cloned().collect();
            out.extend(self.fechar_se(corte, MotivoFim::CorteDoDia, |t| !fica(t)));
            if let Some(tr) = &mut self.troca {
                tr.cortes.push(corte);
                if adiar && self.abertos.iter().any(|t| t.origem == Origem::Leitura) {
                    tr.adiados.push(corte);
                }
            }
            for t in seguem {
                out.push(Mudanca::Abriu {
                    timer_id: t.timer_id,
                    sessao_id: t.sessao_id.clone(),
                    origem: t.origem,
                    inicio: corte,
                    atividade_id: t.atividade_id,
                });
                self.abertos.push(TrechoAberto { inicio: corte, ..t });
            }
            // Quem esperava voce desde o dia anterior vira parada (menos a sessao sendo lida
            // e a que terminou ha menos de `ESPERA_RECENTE_MS`).
            let lida = self.lendo_agora().map(str::to_string);
            for s in self.sessoes.values_mut() {
                if s.estado == EstadoSessao::EsperandoVoce
                    && lida.as_deref() != Some(s.id.as_str())
                    && s.terminou_em.is_none_or(|t| t + ESPERA_RECENTE_MS <= corte)
                {
                    s.estado = EstadoSessao::Parada;
                    out.push(Mudanca::Sessao(s.clone()));
                }
            }
            self.dia = prox;
            // O encerramento (■) vale so no dia em que foi feito; o pausado no dock tambem.
            self.desfazer.clear();
            self.pausado_desde.clear();
            if !self.encerrados.is_empty() {
                self.encerrados.clear();
                out.push(self.mudanca_encerrados());
            }
        }
        out
    }

    pub fn tique(&mut self, agora: Ms) -> Vec<Mudanca> {
        // Sessoes sem evento fecham no ultimo evento, antes do corte (que reabriria o trecho).
        let mut out = self.sessoes_sem_evento(agora);
        out.extend(self.corte_do_dia(agora));
        // os avisos vencidos saem
        self.ligadas_sozinhas.retain(|_, l| agora < l.em + AVISO_LIGADA_MS);
        // as ligações de janela de sessões que saíram ou sem evento há 24 h (rodada 28)
        self.limpar_janelas(agora);
        out
    }

    pub fn responder(&mut self, pergunta_id: u64, r: Resposta, agora: Ms) -> Vec<Mudanca> {
        let Some(pos) = self.perguntas.iter().position(|(id, _)| *id == pergunta_id) else {
            return vec![];
        };
        let mut out = self.corte_do_dia(agora);
        let dia_ini = inicio_do_dia(self.dia, self.cfg.corte_min, self.cfg.fuso_min);
        match self.perguntas[pos].1.clone() {
            Pergunta::VoltaInatividade { timer_ids, de, ate, atividades }
            | Pergunta::AppFechado { timer_ids, de, ate, atividades } => {
                // A ausencia inteira antes do corte do dia (voltou ainda ontem): nada religa nem
                // conta, o manual de ontem parou na virada (regra 9) e ontem nao muda; a tela so
                // oferece "Deixar pausado".
                // A que atravessou o corte: o "Contar" conta so de 00:00 ate a volta (um trecho
                // fechado, o timer fica parado, como na virada); o "Retomar agora" nao religa.
                // No mesmo dia: o "Contar" reabre desde que parou e o "Retomar agora" a partir de
                // agora, sem o tempo fora.
                let atividade_de = |m: &Motor, id: i64| atividades.get(&id).copied().unwrap_or_else(|| m.atividade_padrao(Some(id)));
                // "Deixar pausado": o pausado fica no dock recolhido a partir da resposta
                if r == Resposta::DeixarPausado {
                    let parados: Vec<i64> = timer_ids
                        .iter()
                        .copied()
                        .filter(|id| self.manual_aberto(*id).is_none() && !self.encerrado(Some(*id)))
                        .collect();
                    for id in parados {
                        self.pausado_desde.insert(id, agora);
                    }
                }
                if ate < dia_ini {
                    // so resolve a pergunta
                } else if de < dia_ini {
                    if r == Resposta::Contar && ate > dia_ini {
                        // um ▶ por vez (spec 003): só o último ▶ da pergunta conta
                        let ids: Vec<i64> =
                            if self.cfg.um_play_por_vez { timer_ids.last().copied().into_iter().collect() } else { timer_ids };
                        for id in ids {
                            if self.manual_aberto(id).is_some() {
                                continue;
                            }
                            // com o ▶ já noutro timer, acaba no início dele (um ▶ por vez, R4)
                            let fim = match self.cfg.um_play_por_vez {
                                true => self
                                    .abertos
                                    .iter()
                                    .filter(|t| t.origem == Origem::Manual && t.timer_id != Some(id))
                                    .map(|t| t.inicio)
                                    .min()
                                    .map_or(ate, |o| ate.min(o)),
                                false => ate,
                            };
                            if fim <= dia_ini {
                                continue;
                            }
                            let atividade_id = atividade_de(self, id);
                            out.push(Mudanca::Abriu { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, inicio: dia_ini, atividade_id });
                            out.push(Mudanca::Fechou { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, fim, motivo: MotivoFim::Pausa });
                        }
                    }
                } else {
                    let desde = match r {
                        Resposta::Contar => Some(de),
                        Resposta::RetomarAgora => Some(agora),
                        _ => None,
                    };
                    if let Some(desde) = desde {
                        // um ▶ por vez (spec 003): só o último ▶ da pergunta volta
                        let ids: Vec<i64> =
                            if self.cfg.um_play_por_vez { timer_ids.last().copied().into_iter().collect() } else { timer_ids };
                        for id in ids {
                            // na atividade em que estava (a reuniao volta como reuniao)
                            let atividade = atividade_de(self, id);
                            let outro_aberto = self.cfg.um_play_por_vez
                                && self.abertos.iter().any(|t| t.origem == Origem::Manual && t.timer_id != Some(id));
                            if outro_aberto && r == Resposta::Contar {
                                // o ▶ já está noutro timer: o tempo fora conta aqui, num trecho
                                // fechado que acaba quando o outro ▶ começou (nunca os dois juntos),
                                // e o ▶ de agora continua o outro
                                let outro_desde = self
                                    .abertos
                                    .iter()
                                    .filter(|t| t.origem == Origem::Manual && t.timer_id != Some(id))
                                    .map(|t| t.inicio)
                                    .min()
                                    .unwrap_or(ate);
                                let fim = ate.min(outro_desde);
                                if self.manual_aberto(id).is_none() && fim > de {
                                    out.extend(self.desencerrar(Some(id)));
                                    out.push(Mudanca::Abriu { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, inicio: de, atividade_id: atividade });
                                    out.push(Mudanca::Fechou { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, fim, motivo: MotivoFim::Pausa });
                                }
                                self.pausado_desde.insert(id, agora);
                                continue;
                            }
                            out.extend(self.desencerrar(Some(id)));
                            out.extend(self.pausar_outros_manuais(id, agora));
                            out.extend(self.abrir_manual(id, atividade, desde));
                        }
                    }
                }
            }
            // `criar_regra` e do app (grava a regra no banco); o motor so liga a sessao ao timer.
            // `NaoContar`: a sessao fica sem timer, por decisao sua (a heranca nao a liga depois).
            // a conversa nova: "Certo" (e sem resposta) fica no timer da regra; outro timer a
            // move desde o inicio e a ligacao passa a "mao"
            Pergunta::ConversaNova { sessao_id, .. } => {
                if let Resposta::LigarA { timer_id, .. } = r {
                    // so os trechos de hoje (revisao da rodada 15): ontem pode ja ter sido lancado
                    out.extend(self.ligar_sessao_hoje(&sessao_id, timer_id, agora));
                }
            }
            Pergunta::SessaoNova { sessao_id, .. } => {
                if let Resposta::LigarA { timer_id, .. } = r {
                    out.extend(self.ligar_sessao(&sessao_id, timer_id, true, agora));
                } else if let Some(s) =
                    self.sessoes.get_mut(&sessao_id).filter(|s| s.timer_id.is_none() && s.ligacao != Some(Ligacao::Mao))
                {
                    s.ligacao = Some(Ligacao::Mao);
                    out.push(Mudanca::Sessao(s.clone()));
                }
            }
        }
        // a pergunta pode ter saido no caminho (ligar a sessao tira a da conversa nova)
        if let Some(pos) = self.perguntas.iter().position(|(id, _)| *id == pergunta_id) {
            self.perguntas.remove(pos);
            out.push(Mudanca::PerguntaResolvida(pergunta_id));
        }
        out
    }

    /// Chamado ao abrir o app: tudo que ficou aberto no banco fecha no ultimo tique.
    /// Excecao: fechado por menos de `RETOMADA_MS` no mesmo dia, os manuais seguem abertos, sem pergunta.
    /// `saida_leitura`: o app fechou com uma saida da conversa lida esperando confirmacao (troca
    /// suave, `saida_esperando`): a leitura fecha nela, nao no ultimo tique.
    pub fn ao_reabrir(
        &mut self,
        abertos_no_banco: Vec<TrechoAberto>,
        ultimo_tique: Ms,
        saida_leitura: Option<Ms>,
        agora: Ms,
    ) -> Vec<Mudanca> {
        let mut timers: Vec<i64> = vec![];
        let mut atividades: BTreeMap<i64, i64> = BTreeMap::new();
        let mut out = vec![];
        let retoma = agora - ultimo_tique < RETOMADA_MS
            && dia_de(ultimo_tique, self.cfg.corte_min, self.cfg.fuso_min) == self.dia;
        // um ▶ por vez (spec 003): na retomada, só o manual mais recente segue; os outros fecham
        // no último tique e entram na pergunta, na ordem em que abriram
        let mut abertos_no_banco = abertos_no_banco;
        abertos_no_banco.sort_by_key(|t| t.inicio);
        let mais_recente = abertos_no_banco.iter().rposition(|t| t.origem == Origem::Manual);
        for (i, t) in abertos_no_banco.into_iter().enumerate() {
            let segue = !self.cfg.um_play_por_vez || Some(i) == mais_recente;
            if retoma && t.origem == Origem::Manual && segue {
                if !self.abertos.iter().any(|a| a.origem == Origem::Manual && a.timer_id == t.timer_id) {
                    self.abertos.push(t);
                }
                continue;
            }
            if t.origem == Origem::Manual {
                if let Some(id) = t.timer_id {
                    if !timers.contains(&id) {
                        timers.push(id);
                    }
                    atividades.insert(id, t.atividade_id);
                }
            }
            // Aberto de antes de uma virada (a leitura de uma troca suave que ficou sem dividir):
            // cada dia com o seu pedaco.
            let fim = match saida_leitura {
                Some(s) if t.origem == Origem::Leitura && s >= t.inicio => s.min(ultimo_tique),
                _ => ultimo_tique,
            }
            .max(t.inicio);
            let mut corte = proximo_corte(t.inicio, self.cfg.corte_min, self.cfg.fuso_min);
            while corte < fim {
                out.push(Mudanca::Fechou {
                    fim: corte,
                    timer_id: t.timer_id,
                    sessao_id: t.sessao_id.clone(),
                    origem: t.origem,
                    motivo: MotivoFim::CorteDoDia,
                });
                out.push(Mudanca::Abriu {
                    timer_id: t.timer_id,
                    sessao_id: t.sessao_id.clone(),
                    origem: t.origem,
                    inicio: corte,
                    atividade_id: t.atividade_id,
                });
                corte = proximo_corte(corte, self.cfg.corte_min, self.cfg.fuso_min);
            }
            out.push(Mudanca::Fechou {
                fim,
                timer_id: t.timer_id,
                sessao_id: t.sessao_id,
                origem: t.origem,
                motivo: MotivoFim::AppFechado,
            });
        }
        if !timers.is_empty() {
            out.push(self.nova_pergunta(Pergunta::AppFechado { timer_ids: timers, de: ultimo_tique, ate: agora, atividades }));
        }
        out
    }
}
