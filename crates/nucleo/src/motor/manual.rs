// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use super::chat::SinaisDoChat;
use super::leitura::{Conversa, Onde, TROCA_SUAVE_MS};
use super::Motor;
use crate::modelo::{EstadoSessao, MotivoFim, Mudanca, Origem, Pergunta, TrechoAberto, ATIVIDADE_REUNIAO};
use crate::tempo::Ms;

/// O "Desfazer" do ■ vale por isto (a faixa da tela fica 10 s).
pub const DESFAZER_ENCERRAR_MS: Ms = 60_000;

/// O que um ■ mudou, para desfazer: quando, os trechos manual e do Claude que ele fechou e como
/// estavam as travas das sessoes do timer antes dele.
#[derive(Clone, Debug, PartialEq)]
pub struct DesfazerEncerrar {
    pub em: Ms,
    pub fechados: Vec<TrechoAberto>,
    pub bloqueadas: Vec<(String, Option<Ms>)>,
    pub leitura: Vec<(String, Option<u32>)>,
}

/// Uma amostra do PC (a cada 5 s): ociosidade e a janela em uso.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Amostra {
    pub t: Ms,
    pub ocioso_ms: Ms,
    pub processo: Option<String>,
    pub titulo: Option<String>,
    /// Conversa aberta na janela em uso (app do Claude: lida pela UI Automation).
    pub conversa: Conversa,
    /// No app do Claude, o foco do teclado está na caixa de mensagem da conversa aberta: so muda a
    /// etiqueta ("escrevendo" em vez de "lendo"); a contagem e a mesma da leitura.
    pub escrevendo: bool,
    /// A janela em uso e do proprio app (principal ou dock): neutra para a leitura.
    pub neutra: bool,
    /// A janela em uso e a que tem o foco (e nao so a que esta debaixo do mouse).
    pub em_foco: bool,
    /// So sera ligado no Plano 2.
    pub reuniao_ativa: bool,
    /// Os sinais do Chat da janela do app do Claude, em uso ou nao (rodada 27); `None`: nao sei
    /// (sem a janela, sem resposta no teto ou com falha).
    pub chat: Option<SinaisDoChat>,
    /// A janela em uso (o HWND da `JanelaInfo`; rodada 28): liga a sessão do terminal à janela em que
    /// você escreveu. Só em memória: nunca vai para o banco, para log, para arquivo nem para erro.
    pub janela: Option<isize>,
}

impl Motor {
    /// ▶: o play manual na atividade padrao do timer.
    pub fn play(&mut self, timer_id: i64, agora: Ms) -> Vec<Mudanca> {
        let atividade = self.atividade_padrao(Some(timer_id));
        self.play_atividade(timer_id, atividade, agora)
    }

    /// O play manual numa atividade (👥 e a reuniao manual; o ▶ e a padrao). O manual e um por vez
    /// por timer: com o de outra atividade aberto, ele fecha e o novo abre no mesmo instante (e o
    /// substituido fica guardado para voltar em `parar_atividade`). O Claude trabalhando e a
    /// leitura do timer seguem: podem se sobrepor ao manual. Com um ▶ por vez (spec 003), o play
    /// manual dos outros timers fecha no mesmo instante (`pausar_outros_manuais`).
    pub fn play_atividade(&mut self, timer_id: i64, atividade_id: i64, agora: Ms) -> Vec<Mudanca> {
        // o escondido (spec 004) nunca recebe tempo
        if self.escondido(Some(timer_id)) {
            return vec![];
        }
        let mut out = self.corte_do_dia(agora);
        self.tirar_da_ausencia(timer_id);
        out.extend(self.tirar_das_perguntas(Some(timer_id)));
        out.extend(self.desencerrar(Some(timer_id)));
        out.extend(self.pausar_outros_manuais(timer_id, agora));
        self.play_armado.remove(&timer_id);
        match self.manual_aberto(timer_id).map(|t| t.atividade_id) {
            Some(a) if a == atividade_id => return out,
            Some(antes) => {
                out.extend(self.fechar_se(agora, MotivoFim::TrocouAtividade, |t| {
                    t.origem == Origem::Manual && t.timer_id == Some(timer_id)
                }));
                self.voltar_para.insert(timer_id, antes);
            }
            None => {
                self.voltar_para.remove(&timer_id);
            }
        }
        out.extend(self.abrir_manual(timer_id, atividade_id, agora));
        out
    }

    /// 👥 apertado de novo (fim da reuniao): o play manual dessa atividade para. Se ele tinha
    /// substituido outro play manual do timer (▶ → 👥), o outro volta no mesmo instante (👥 → ▶);
    /// senao o manual so fecha. Nada mais do timer muda: o Claude trabalhando e a leitura seguem,
    /// sem a trava do pause (regra 8).
    pub fn parar_atividade(&mut self, timer_id: i64, atividade_id: i64, agora: Ms) -> Vec<Mudanca> {
        let mut out = self.corte_do_dia(agora);
        if self.manual_aberto(timer_id).is_none_or(|t| t.atividade_id != atividade_id) {
            return out;
        }
        match self.voltar_para.remove(&timer_id).filter(|a| *a != atividade_id) {
            Some(antes) => {
                out.extend(self.fechar_se(agora, MotivoFim::TrocouAtividade, |t| {
                    t.origem == Origem::Manual && t.timer_id == Some(timer_id)
                }));
                out.extend(self.abrir_manual(timer_id, antes, agora));
            }
            None => {
                out.extend(self.fechar_se(agora, MotivoFim::Pausa, |t| {
                    t.origem == Origem::Manual && t.timer_id == Some(timer_id)
                }));
                // a reunião parada pelo 👥 aceso, sem ▶ para voltar: pausado desde o clique
                self.pausado_desde.insert(timer_id, agora);
                self.tirar_da_ausencia(timer_id);
                out.extend(self.tirar_das_perguntas(Some(timer_id)));
            }
        }
        out
    }

    /// Encerrar (■, "terminei isso por agora"): pausa como a regra 8 e marca o timer como
    /// encerrado hoje. Encerrado, ele sai do dock recolhido, as sessoes dele nao mostram mais o
    /// lembrete "esperando voce" e a leitura delas nao religa, nem abrindo a conversa. A proxima
    /// mensagem numa sessao dele, o play ou a virada do dia voltam tudo ao normal.
    pub fn encerrar(&mut self, timer_id: i64, agora: Ms) -> Vec<Mudanca> {
        let mut out = self.corte_do_dia(agora);
        // como estava, para o "Desfazer"
        let fechados: Vec<TrechoAberto> = self
            .abertos
            .iter()
            .filter(|t| t.timer_id == Some(timer_id) && matches!(t.origem, Origem::Manual | Origem::Claude))
            .cloned()
            .collect();
        let ids: Vec<String> =
            self.sessoes.values().filter(|s| s.timer_id == Some(timer_id)).map(|s| s.id.clone()).collect();
        let bloqueadas = ids.iter().map(|id| (id.clone(), self.bloqueadas.get(id).copied())).collect();
        let leitura = ids.iter().map(|id| (id.clone(), self.leitura_bloqueada.get(id).copied())).collect();
        out.extend(self.pausar(timer_id, agora));
        // o ■ não deixa pausado no dock
        self.pausado_desde.remove(&timer_id);
        if self.encerrados.insert(timer_id) {
            self.desfazer.insert(timer_id, DesfazerEncerrar { em: agora, fechados, bloqueadas, leitura });
            out.push(self.mudanca_encerrados());
        }
        out
    }

    /// "Desfazer" do ■ (ate `DESFAZER_ENCERRAR_MS` depois): tira a marca, devolve as travas das
    /// sessoes como estavam (a leitura volta na proxima amostra) e reabre, no instante do ■, o
    /// play manual e o Claude que ainda trabalha: sem buraco. Ja desfeito por mensagem, play ou
    /// virada do dia, nao faz nada.
    pub fn desfazer_encerrar(&mut self, timer_id: i64, agora: Ms) -> Vec<Mudanca> {
        let mut out = self.corte_do_dia(agora);
        let Some(d) = self.desfazer.remove(&timer_id) else { return out };
        if agora - d.em > DESFAZER_ENCERRAR_MS || !self.encerrados.remove(&timer_id) {
            return out;
        }
        out.push(self.mudanca_encerrados());
        for (id, antes) in d.bloqueadas {
            match antes {
                Some(em) => self.bloqueadas.insert(id, em),
                None => self.bloqueadas.remove(&id),
            };
        }
        for (id, antes) in d.leitura {
            match antes {
                Some(n) => self.leitura_bloqueada.insert(id, n),
                None => self.leitura_bloqueada.remove(&id),
            };
        }
        for t in d.fechados {
            match (t.origem, t.sessao_id) {
                // na atividade em que estava (a reuniao volta como reuniao)
                // um ▶ por vez (spec 003): com o ▶ noutro timer desde o ■, este não volta
                (Origem::Manual, _)
                    if self.cfg.um_play_por_vez
                        && self.abertos.iter().any(|a| a.origem == Origem::Manual && a.timer_id != Some(timer_id)) =>
                {
                    self.pausado_desde.insert(timer_id, d.em);
                }
                (Origem::Manual, _) => out.extend(self.abrir_manual(timer_id, t.atividade_id, d.em)),
                (Origem::Claude, Some(sessao)) => {
                    let trabalha = self
                        .sessoes
                        .get(&sessao)
                        .is_some_and(|s| s.estado == EstadoSessao::Trabalhando && s.timer_id == Some(timer_id));
                    if trabalha {
                        self.bloqueadas.remove(&sessao);
                        out.extend(self.abrir_claude(&sessao, Some(timer_id), d.em));
                    }
                }
                _ => {}
            }
        }
        out
    }

    pub fn pausar(&mut self, timer_id: i64, agora: Ms) -> Vec<Mudanca> {
        let mut out = self.corte_do_dia(agora);
        self.play_armado.remove(&timer_id);
        // o ⏸ do play manual: o pausado fica no dock recolhido a partir do clique
        if self.manual_aberto(timer_id).is_some() {
            self.pausado_desde.insert(timer_id, agora);
        }
        out.extend(self.pausar_trechos(timer_id, None, agora));
        self.tirar_da_ausencia(timer_id);
        out.extend(self.tirar_das_perguntas(Some(timer_id)));
        out
    }

    /// Um ▶ por vez (spec 003, `um_play_por_vez`): fecha em `agora` o play manual dos outros
    /// timers, de qualquer atividade (a reunião também: é o mesmo cronômetro seu). Eles ficam
    /// pausados no dock e saem das perguntas de "contar"; o Claude e a leitura deles seguem, sem as
    /// travas do ⏸.
    pub(crate) fn pausar_outros_manuais(&mut self, timer_id: i64, agora: Ms) -> Vec<Mudanca> {
        if !self.cfg.um_play_por_vez {
            return vec![];
        }
        let outro = |t: &TrechoAberto| t.origem == Origem::Manual && t.timer_id.is_some_and(|id| id != timer_id);
        let ids: Vec<i64> = self.abertos.iter().filter(|t| outro(t)).filter_map(|t| t.timer_id).collect();
        if ids.is_empty() {
            return vec![];
        }
        let mut out = self.fechar_se(agora, MotivoFim::Pausa, outro);
        for id in ids {
            self.pausado_desde.insert(id, agora);
            self.voltar_para.remove(&id);
            self.play_armado.remove(&id);
            self.tirar_da_ausencia(id);
            out.extend(self.tirar_das_perguntas(Some(id)));
        }
        out
    }

    /// O fim do turno de `sessao_id` em `em` (spec 009, `play_para_com_claude`; o `Stop`, o
    /// `Notification` ou o stop na conversa): o ▶ do timer dela (não a reunião) fica armado e para
    /// quando você sai das conversas do timer (`play_armado_na_amostra`). Com uma conversa do
    /// timer aberta agora, você está nela; sem (ou sem saber: no terminal, a janela diz "não sei"
    /// enquanto o Claude trabalha), a saída conta desde o fim do turno e a amostra seguinte decide:
    /// de volta numa conversa do timer, segue; fora dela, o ▶ fecha no instante do fim.
    pub(crate) fn play_no_fim_do_turno(&mut self, sessao_id: &str, em: Ms) -> Vec<Mudanca> {
        if !self.cfg.play_para_com_claude {
            return vec![];
        }
        let Some(timer_id) = self.sessoes.get(sessao_id).and_then(|s| s.timer_id) else { return vec![] };
        if self.manual_aberto(timer_id).is_none_or(|t| t.atividade_id == ATIVIDADE_REUNIAO) {
            return vec![];
        }
        // já armado (outra sessão do mesmo timer): fica como está
        if self.play_armado.contains_key(&timer_id) {
            return vec![];
        }
        let saiu = (!self.no_timer_agora(timer_id)).then_some(em);
        self.play_armado.insert(timer_id, saiu);
        vec![]
    }

    /// Você está numa conversa do timer: lendo uma sessão dele, com a conversa aberta de uma sessão
    /// dele, ou com a troca suave entrando numa.
    fn no_timer_agora(&self, timer_id: i64) -> bool {
        let do_timer = |id: &str| self.sessoes.get(id).is_some_and(|s| s.timer_id == Some(timer_id));
        self.lendo_agora().is_some_and(do_timer)
            || self.conversa_aberta.as_deref().is_some_and(do_timer)
            || self.troca.as_ref().and_then(|tr| tr.alvo.as_deref()).is_some_and(do_timer)
    }

    /// Fecha em `fim` o ▶ do timer pelo fim do Claude (motivo `Terminou`): fica pausado no dock.
    /// Antes, a virada do dia que caiu antes de `fim` (o ▶ não atravessa o dia).
    fn fechar_play_pelo_claude(&mut self, timer_id: i64, fim: Ms) -> Vec<Mudanca> {
        self.play_armado.remove(&timer_id);
        self.voltar_para.remove(&timer_id);
        let mut out = self.corte_do_dia(fim);
        let fechou = self.fechar_se(fim, MotivoFim::Terminou, |t| t.origem == Origem::Manual && t.timer_id == Some(timer_id));
        if !fechou.is_empty() {
            self.pausado_desde.insert(timer_id, fim);
        }
        out.extend(fechou);
        out
    }

    /// Na amostra, depois da leitura (spec 009): o ▶ armado fecha quando você saiu das conversas do
    /// timer, com a troca suave. Só as amostras que dizem onde você está contam (`conclusiva`: a
    /// janela do app e o "não sei" não mudam nada).
    /// - A leitura de uma sessão do timer fechou nesta amostra (a saída da troca suave já
    ///   confirmada) e você não está noutra do timer: fecha no instante dela. A leitura que fecha por
    ///   falta de uso (`SemUso`: lendo parado) não é saída.
    /// - Fora das conversas do timer sem a leitura: a saída fica anotada e, mantida por
    ///   `TROCA_SUAVE_MS`, o ▶ fecha no instante dela; de volta antes, nada.
    /// Uma mensagem nova (ou o turno que continua) numa sessão do timer desarma (`evento`).
    fn play_armado_na_amostra(&mut self, fechadas: &[Mudanca], conclusiva: bool, saida: Ms, agora: Ms) -> Vec<Mudanca> {
        if !self.cfg.play_para_com_claude {
            self.play_armado.clear();
            return vec![];
        }
        let mut out = vec![];
        for (timer_id, saiu) in self.play_armado.clone() {
            if self.manual_aberto(timer_id).is_none_or(|t| t.atividade_id == ATIVIDADE_REUNIAO) {
                self.play_armado.remove(&timer_id);
                continue;
            }
            // outra sessão do timer ainda trabalha: o Claude não terminou neste timer (M3)
            let trabalha = self.sessoes.values().any(|s| s.timer_id == Some(timer_id) && s.estado == EstadoSessao::Trabalhando);
            if !conclusiva || trabalha {
                continue;
            }
            if self.no_timer_agora(timer_id) {
                self.play_armado.insert(timer_id, None);
                continue;
            }
            let leitura_fechou = fechadas.iter().find_map(|m| match m {
                Mudanca::Fechou { timer_id: Some(t), origem: Origem::Leitura, fim, motivo, .. }
                    if *t == timer_id && !matches!(motivo, MotivoFim::CorteDoDia | MotivoFim::SemUso) =>
                {
                    Some(*fim)
                }
                _ => None,
            });
            if let Some(f) = leitura_fechou {
                out.extend(self.fechar_play_pelo_claude(timer_id, f));
                continue;
            }
            // a janela do app há mais de `NEUTRA_MAX_MS`: a saída foi quando você entrou nela (R3)
            let desde = saiu.unwrap_or(saida);
            if agora - desde >= TROCA_SUAVE_MS {
                out.extend(self.fechar_play_pelo_claude(timer_id, desde));
            } else {
                self.play_armado.insert(timer_id, Some(desde));
            }
        }
        out
    }

    /// A re-revisão (R1): antes de a inatividade ou o sono fecharem os manuais em `ate`, o ▶ armado
    /// com a saída anotada até lá (fora das conversas do timer, ou sem saber, desde o fim do turno)
    /// fecha no instante dela, como o fim do Claude, e não entra na pergunta de "contar".
    fn fechar_armados_fora(&mut self, ate: Ms) -> Vec<Mudanca> {
        if !self.cfg.play_para_com_claude {
            return vec![];
        }
        let fora: Vec<(i64, Ms)> =
            self.play_armado.iter().filter_map(|(t, s)| s.filter(|d| *d <= ate).map(|d| (*t, d))).collect();
        let mut out = vec![];
        for (timer_id, desde) in fora {
            out.extend(self.fechar_play_pelo_claude(timer_id, desde));
        }
        out
    }

    /// O ⏸ da linha de uma atividade (painel do timer): para so essa atividade do timer.
    /// - Atividade que nao e a padrao (a reuniao): e o `parar_atividade` (o 👥 aceso; o ▶ que ela
    ///   substituiu volta). Se a padrao do timer mudou com o Claude ou a leitura abertos na antiga,
    ///   eles fecham tambem (o ⏸ sempre faz efeito), sem trava: o Claude volta na padrao nova.
    /// - A padrao sem play manual de outra atividade aberto: e o ⏸ do timer (`pausar`).
    /// - A padrao com a reuniao aberta: fecham o Claude, a leitura e o manual dela, com as travas
    ///   do pause (regra 8), e a reuniao segue. O ▶ que a reuniao substituiu nao volta mais.
    pub fn pausar_atividade(&mut self, timer_id: i64, atividade_id: i64, agora: Ms) -> Vec<Mudanca> {
        if atividade_id != self.atividade_padrao(Some(timer_id)) {
            let mut out = self.parar_atividade(timer_id, atividade_id, agora);
            out.extend(self.fechar_se(agora, MotivoFim::Pausa, |t| {
                t.timer_id == Some(timer_id) && t.atividade_id == atividade_id && t.origem != Origem::Manual
            }));
            return out;
        }
        if self.manual_aberto(timer_id).is_none_or(|t| t.atividade_id == atividade_id) {
            return self.pausar(timer_id, agora);
        }
        let mut out = self.corte_do_dia(agora);
        out.extend(self.pausar_trechos(timer_id, Some(atividade_id), agora));
        out
    }

    /// Fecha os trechos do timer (`so_atividade`: so os dessa atividade) e poe as travas do pause:
    /// as sessoes ligadas so voltam com uma mensagem sua, e a conversa lida so volta a contar
    /// saindo dela e voltando.
    fn pausar_trechos(&mut self, timer_id: i64, so_atividade: Option<i64>, agora: Ms) -> Vec<Mudanca> {
        // A sessao lida e a conversa aberta, se sao deste timer, so voltam a contar leitura saindo
        // da conversa e voltando (ou escrevendo nela).
        let lidas: Vec<String> = self
            .lendo_agora()
            .map(str::to_string)
            .into_iter()
            .chain(self.conversa_aberta.clone())
            .filter(|id| self.sessoes.get(id).is_some_and(|s| s.timer_id == Some(timer_id)))
            .collect();
        for id in lidas {
            self.leitura_bloqueada.insert(id, 0);
        }
        let out = self.fechar_se(agora, MotivoFim::Pausa, |t| {
            t.timer_id == Some(timer_id) && so_atividade.is_none_or(|a| t.atividade_id == a)
        });
        self.voltar_para.remove(&timer_id);
        let ligadas: Vec<String> = self
            .sessoes
            .values()
            .filter(|s| s.timer_id == Some(timer_id))
            .map(|s| s.id.clone())
            .collect();
        self.bloqueadas.extend(ligadas.into_iter().map(|id| (id, agora)));
        out
    }

    pub fn pausar_tudo(&mut self, agora: Ms) -> Vec<Mudanca> {
        let mut out = self.corte_do_dia(agora);
        let mut timers: Vec<i64> = vec![];
        for id in self.abertos.iter().filter_map(|t| t.timer_id) {
            if !timers.contains(&id) {
                timers.push(id);
            }
        }
        for id in timers {
            out.extend(self.pausar(id, agora));
        }
        out.extend(self.fechar_se(agora, MotivoFim::Pausa, |_| true));
        self.ausencia = None;
        // "Pausar tudo" vale tambem para quem esperava a resposta de "contar o tempo fora".
        out.extend(self.tirar_das_perguntas(None));
        out
    }

    /// O "Pausar tudo" de um espaço (rodada 20): o ⏸ de cada timer dado que tem algo aberto (o
    /// manual, o Claude, a leitura). Os timers de outros espaços seguem rodando.
    pub fn pausar_timers(&mut self, ids: &[i64], agora: Ms) -> Vec<Mudanca> {
        let mut out = self.corte_do_dia(agora);
        let mut timers: Vec<i64> = vec![];
        for id in self.abertos.iter().filter_map(|t| t.timer_id) {
            if ids.contains(&id) && !timers.contains(&id) {
                timers.push(id);
            }
        }
        for id in timers {
            out.extend(self.pausar(id, agora));
        }
        out
    }

    pub(super) fn tirar_da_ausencia(&mut self, timer_id: i64) {
        if let Some((_, ids, _)) = &mut self.ausencia {
            ids.retain(|i| *i != timer_id);
            if ids.is_empty() {
                self.ausencia = None;
            }
        }
    }

    /// Mexer no timer (play ou pause) responde por ele: sai das perguntas de "contar o tempo fora"
    /// (volta da inatividade, app fechado). `None` tira todos. Pergunta que fica vazia e resolvida.
    pub(super) fn tirar_das_perguntas(&mut self, timer_id: Option<i64>) -> Vec<Mudanca> {
        let mut out = vec![];
        self.perguntas.retain_mut(|(id, p)| match p {
            Pergunta::VoltaInatividade { timer_ids, .. } | Pergunta::AppFechado { timer_ids, .. } => {
                timer_ids.retain(|i| timer_id.is_some_and(|t| t != *i));
                if timer_ids.is_empty() {
                    out.push(Mudanca::PerguntaResolvida(*id));
                }
                !timer_ids.is_empty()
            }
            Pergunta::SessaoNova { .. } | Pergunta::ConversaNova { .. } => true,
        });
        out
    }

    /// Fecha em `de` os manuais abertos (com `so_anteriores`, so os que ja rodavam em `de`)
    /// e guarda a ausencia para perguntar na volta.
    fn pausar_manuais_por_inatividade(&mut self, de: Ms, so_anteriores: bool) -> Vec<Mudanca> {
        let alvo = |t: &TrechoAberto| t.origem == Origem::Manual && (!so_anteriores || t.inicio <= de);
        let ids: Vec<i64> = self.abertos.iter().filter(|t| alvo(t)).filter_map(|t| t.timer_id).collect();
        if ids.is_empty() {
            return vec![];
        }
        let atividades =
            self.abertos.iter().filter(|t| alvo(t)).filter_map(|t| Some((t.timer_id?, t.atividade_id))).collect();
        self.ausencia = Some((de, ids, atividades));
        self.fechar_se(de, MotivoFim::Inatividade, alvo)
    }

    pub fn amostra(&mut self, a: &Amostra) -> Vec<Mudanca> {
        // O Chat (rodada 27): os eventos sinteticos dos sinais vem antes, como os dos hooks (lidos
        // da fila antes da amostra); a conversa do Chat aberta passa a ser conhecida antes da leitura.
        // sem os sinais ("não sei": a janela fechada, a consulta atrasada), nada muda, mas a
        // lacuna fica anotada
        let mut out = match &a.chat {
            Some(s) => self.sinais_do_chat(s, a.t),
            None => {
                self.sem_sinais_do_chat();
                vec![]
            }
        };
        if let Conversa::Chat { id, titulo } = &a.conversa {
            out.extend(self.conhecer_chat(id, titulo, a.t));
        }
        // a amostra fica guardada (uns 45 s) para ligar a sessão do terminal à janela (rodada 28)
        self.guardar_amostra_da_janela(a);
        let onde = self.onde_da_amostra(a);
        let anterior = self.ultima_amostra;
        self.ultima_amostra = Some(a.t);
        let inat = self.cfg.inatividade_ms;
        let ultimo_uso = a.t - a.ocioso_ms;

        // Buraco entre amostras maior que o tempo de uso (PC dormiu, laco travou): a leitura aberta
        // acaba na amostra anterior, em vez de contar o buraco.
        // A troca suave esperando confirmacao nao atravessa o buraco (a saida dela vale, se foi antes).
        if let Some(p) = anterior.filter(|p| a.t - p > self.cfg.uso_ms) {
            let fim = self.troca.as_ref().and_then(|tr| tr.saiu).map_or(p, |(s, _)| s.min(p));
            out.extend(self.fechar_se(fim, MotivoFim::SemUso, |t| t.origem == Origem::Leitura));
            self.troca = None;
            self.escrevendo_desde = None;
        }
        // Sono do PC: houve uso muito depois da ultima amostra, sem amostras no meio.
        // So os manuais que ja rodavam na amostra anterior: timer ligado logo depois de acordar fica.
        if self.ausencia.is_none() && !a.reuniao_ativa {
            if let Some(p) = anterior {
                if ultimo_uso - p > inat {
                    // o ▶ armado com a saída anotada fecha nela, e não vira tempo fora (spec 009)
                    out.extend(self.fechar_armados_fora(p));
                    out.extend(self.pausar_manuais_por_inatividade(p, true));
                }
            }
        }
        // Inatividade comum.
        if a.ocioso_ms >= inat {
            if self.ausencia.is_none() && !a.reuniao_ativa {
                out.extend(self.fechar_armados_fora(ultimo_uso));
                out.extend(self.pausar_manuais_por_inatividade(ultimo_uso, false));
            }
        } else if let Some((de, timer_ids, atividades)) = self.ausencia.take() {
            // Voltou: o pausado fica no dock recolhido a partir da volta (revisão da rodada 14)
            for id in &timer_ids {
                self.pausado_desde.insert(*id, ultimo_uso);
            }
            out.push(self.nova_pergunta(Pergunta::VoltaInatividade { timer_ids, de, ate: ultimo_uso, atividades }));
        }
        // Ordem: inatividade/sono -> leitura -> corte do dia.
        // A leitura fecha e abre nos instantes dela (a troca suave vale desde quando aconteceu), cada
        // um com o corte que veio antes dele; o que segue aberto e dividido no corte depois.
        let conclusiva = self.amostra_conclusiva(&onde);
        let leitura = self.leitura_na_amostra(a, &onde);
        // o ▶ armado para parar com o Claude (spec 009): fecha quando você saiu da conversa
        let saida = match onde {
            Onde::NeutraVencida(desde) => desde,
            _ => a.t,
        };
        let play = self.play_armado_na_amostra(&leitura, conclusiva, saida, a.t);
        out.extend(leitura);
        out.extend(play);
        // a sessão lida nesta amostra (0.6.1): o lembrete dela conta desde aqui
        if let Some(id) = self.lendo_agora().map(str::to_string) {
            self.lida_em.insert(id, a.t);
        }
        // O corte vem depois do sono e da inatividade: os que atravessam a meia-noite fecham no inicio da ausencia.
        out.extend(self.corte_do_dia(a.t));
        out
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::{ConfigApp, ConfigMotor};
    use crate::modelo::Resposta;
    use crate::tempo::{ms_local, MIN};

    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms { ms_local(2026, 11, 17, hh, mm, F) }
    fn cfg_teste() -> ConfigMotor {
        let app = ConfigApp {
            programas_claude: ["claude", "code", "windowsterminal"].iter().map(|s| s.to_string()).collect(),
            pastas_ignoradas: vec!["G:\\pessoal".into()],
            ..ConfigApp::default()
        };
        ConfigMotor::de(&app, F)
    }
    fn am(t: Ms, ocioso: Ms) -> Amostra {
        Amostra { t, ocioso_ms: ocioso, processo: Some("msedge".into()), ..Default::default() }
    }
    fn trechos(v: &[Mudanca]) -> Vec<Mudanca> {
        v.iter().filter(|m| matches!(m, Mudanca::Abriu { .. } | Mudanca::Fechou { .. })).cloned().collect()
    }
    fn abriu_manual(id: i64, t: Ms) -> Mudanca {
        Mudanca::Abriu { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, inicio: t, atividade_id: 1 }
    }
    fn pergunta_de(v: &[Mudanca]) -> u64 {
        v.iter().find_map(|m| if let Mudanca::Pergunta(id, _) = m { Some(*id) } else { None }).unwrap()
    }
    fn volta(m: &mut Motor) -> Vec<Mudanca> {
        m.play(1, h(9, 0));
        m.amostra(&am(h(9, 5), 0));
        m.amostra(&am(h(9, 20), 11 * MIN));
        m.amostra(&am(h(9, 35), 0))
    }

    #[test] fn play_abre_e_pausar_fecha_manual() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        assert_eq!(trechos(&m.play(1, h(9, 0))), vec![abriu_manual(1, h(9, 0))]);
        assert_eq!(trechos(&m.pausar(1, h(9, 30))), vec![Mudanca::Fechou { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, fim: h(9, 30), motivo: MotivoFim::Pausa }]);
    }
    #[test] fn play_duas_vezes_nao_duplica() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.play(1, h(9, 0));
        assert_eq!(m.play(1, h(9, 10)), vec![]);
    }
    #[test] fn inatividade_pausa_no_ultimo_uso_e_pergunta_ao_voltar() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.play(1, h(9, 0));
        m.amostra(&am(h(9, 5), 0));
        let v = m.amostra(&am(h(9, 20), 11 * MIN));
        assert!(v.contains(&Mudanca::Fechou { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, fim: h(9, 9), motivo: MotivoFim::Inatividade }));
        let v = m.amostra(&am(h(9, 35), 0));
        assert!(v.iter().any(|x| matches!(x, Mudanca::Pergunta(_, Pergunta::VoltaInatividade { timer_ids, de, ate, .. }) if timer_ids == &vec![1] && *de == h(9, 9) && *ate == h(9, 35))));
    }
    #[test] fn responder_contar_reabre_desde_a_saida() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        let v = volta(&mut m);
        let id = pergunta_de(&v);
        let r = m.responder(id, Resposta::Contar, h(9, 36));
        assert_eq!(trechos(&r), vec![abriu_manual(1, h(9, 9))]);
        assert!(r.contains(&Mudanca::PerguntaResolvida(id)));
        assert!(m.perguntas.is_empty());
    }
    #[test] fn deixar_pausado_nao_reabre() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        let v = volta(&mut m);
        let id = pergunta_de(&v);
        assert_eq!(m.responder(id, Resposta::DeixarPausado, h(9, 36)), vec![Mudanca::PerguntaResolvida(id)]);
        assert!(m.abertos.is_empty());
    }
    #[test] fn reuniao_ativa_impede_pausa_por_inatividade() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.play(1, h(9, 0));
        m.amostra(&am(h(9, 5), 0));
        let mut a = am(h(9, 20), 15 * MIN);
        a.reuniao_ativa = true;
        let v = m.amostra(&a);
        assert!(!v.iter().any(|x| matches!(x, Mudanca::Fechou { .. })));
        assert_eq!(m.abertos.len(), 1);
    }
    #[test] fn sono_do_pc_conta_como_inatividade() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.play(1, h(9, 0));
        m.amostra(&am(h(9, 5), 0));
        let v = m.amostra(&am(h(11, 0), 0));
        assert!(v.contains(&Mudanca::Fechou { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, fim: h(9, 5), motivo: MotivoFim::Inatividade }));
        assert!(v.iter().any(|x| matches!(x, Mudanca::Pergunta(_, Pergunta::VoltaInatividade { timer_ids, de, ate, .. }) if timer_ids == &vec![1] && *de == h(9, 5) && *ate == h(11, 0))));
    }
    // I2: timer ligado logo depois de o PC acordar nao e fechado pela regra do sono.
    #[test] fn sono_nao_fecha_timer_ligado_depois_de_acordar() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        // com "Vários ▶ ao mesmo tempo" ligado (spec 003)
        m.cfg.um_play_por_vez = false;
        m.play(1, h(9, 0));
        m.amostra(&am(h(9, 5), 0));
        m.play(2, h(11, 0) - 2_000);
        let v = m.amostra(&am(h(11, 0), 0));
        assert_eq!(trechos(&v), vec![Mudanca::Fechou { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, fim: h(9, 5), motivo: MotivoFim::Inatividade }]);
        assert!(v.iter().any(|x| matches!(x, Mudanca::Pergunta(_, Pergunta::VoltaInatividade { timer_ids, .. }) if timer_ids == &vec![1])));
        assert_eq!(m.abertos.iter().map(|t| t.timer_id).collect::<Vec<_>>(), [Some(2)]);
        // so o timer novo estava ligado: nada fecha, nada pergunta
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.amostra(&am(h(9, 5), 0));
        m.play(2, h(11, 0) - 2_000);
        assert_eq!(m.amostra(&am(h(11, 0), 0)), vec![]);
    }
    // I5: mexer no timer responde por ele na pergunta de "contar o tempo fora".
    #[test] fn play_tira_o_timer_da_pergunta_e_contar_nao_inventa_tempo() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        let v = volta(&mut m);
        let id = pergunta_de(&v);
        let v = m.play(1, h(9, 40));
        assert_eq!(v, vec![Mudanca::PerguntaResolvida(id), abriu_manual(1, h(9, 40))]);
        assert!(m.perguntas.is_empty());
        // a resposta atrasada nao faz mais nada
        assert_eq!(m.responder(id, Resposta::Contar, h(9, 41)), vec![]);
        assert_eq!(m.abertos.len(), 1);
    }
    #[test] fn pausar_um_dos_timers_deixa_a_pergunta_so_com_o_outro() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        // com "Vários ▶ ao mesmo tempo" ligado (spec 003)
        m.cfg.um_play_por_vez = false;
        m.play(1, h(9, 0));
        m.play(2, h(9, 0));
        m.amostra(&am(h(9, 5), 0));
        m.amostra(&am(h(9, 20), 11 * MIN));
        let v = m.amostra(&am(h(9, 35), 0));
        let id = pergunta_de(&v);
        assert_eq!(m.pausar(2, h(9, 36)), vec![]);
        assert!(matches!(&m.perguntas[0].1, Pergunta::VoltaInatividade { timer_ids, .. } if timer_ids == &vec![1]));
        let r = m.responder(id, Resposta::Contar, h(9, 37));
        assert_eq!(trechos(&r), vec![abriu_manual(1, h(9, 9))]);
    }
    #[test] fn pausar_tudo_resolve_as_perguntas_de_contar() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        let v = volta(&mut m);
        let id = pergunta_de(&v);
        m.perguntas.push((99, Pergunta::SessaoNova { sessao_id: "s".into(), cwd: "D:\\x".into(), sugestao: "x".into(), pasta_mae: false }));
        assert_eq!(m.pausar_tudo(h(9, 40)), vec![Mudanca::PerguntaResolvida(id)]);
        assert_eq!(m.perguntas.iter().map(|p| p.0).collect::<Vec<_>>(), [99]);
    }
    // C8: reinicio rapido do app (atualizacao) nao pergunta.
    #[test] fn ao_reabrir_em_menos_de_2_min_retoma_manual_sem_perguntar() {
        let mut m = Motor::novo(cfg_teste(), h(10, 1));
        let abertos = vec![
            TrechoAberto { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, inicio: h(9, 0), atividade_id: 1 },
            TrechoAberto { timer_id: Some(2), sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(9, 30), atividade_id: 1 },
        ];
        let v = m.ao_reabrir(abertos.clone(), h(10, 0), None, h(10, 1));
        assert_eq!(v, vec![Mudanca::Fechou { timer_id: Some(2), sessao_id: Some("s1".into()), origem: Origem::Claude, fim: h(10, 0), motivo: MotivoFim::AppFechado }]);
        assert_eq!(m.abertos, vec![abertos[0].clone()]);
        assert!(m.perguntas.is_empty());
        // 2 min ou mais: fecha e pergunta
        let mut m = Motor::novo(cfg_teste(), h(10, 2));
        let v = m.ao_reabrir(abertos, h(10, 0), None, h(10, 2));
        assert!(m.abertos.is_empty());
        assert!(v.iter().any(|x| matches!(x, Mudanca::Pergunta(_, Pergunta::AppFechado { .. }))));
    }
    #[test] fn ao_reabrir_depois_da_virada_nao_retoma() {
        let agora = ms_local(2026, 11, 18, 0, 0, F) + 30_000;
        let mut m = Motor::novo(cfg_teste(), agora);
        let abertos = vec![TrechoAberto { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, inicio: h(23, 0), atividade_id: 1 }];
        let v = m.ao_reabrir(abertos, h(23, 59), None, agora);
        assert!(m.abertos.is_empty());
        assert!(v.iter().any(|x| matches!(x, Mudanca::Fechou { fim, .. } if *fim == h(23, 59))));
    }
    #[test] fn corte_do_dia_para_manual() {
        let mut m = Motor::novo(cfg_teste(), h(23, 0));
        m.play(1, h(23, 50));
        let v = m.tique(ms_local(2026, 11, 18, 0, 1, F));
        assert_eq!(trechos(&v), vec![Mudanca::Fechou { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, fim: ms_local(2026, 11, 18, 0, 0, F), motivo: MotivoFim::CorteDoDia }]);
        assert_eq!(m.dia, chrono::NaiveDate::from_ymd_opt(2026, 11, 18).unwrap());
    }
    #[test] fn corte_do_dia_reabre_claude_no_mesmo_instante() {
        let mut m = Motor::novo(cfg_teste(), h(23, 0));
        m.abertos.push(TrechoAberto { timer_id: Some(2), sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(23, 30), atividade_id: 1 });
        let corte = ms_local(2026, 11, 18, 0, 0, F);
        let v = trechos(&m.tique(corte + MIN));
        assert_eq!(v.len(), 2);
        assert!(matches!(&v[1], Mudanca::Abriu { origem: Origem::Claude, inicio, .. } if *inicio == corte));
        assert_eq!(m.abertos[0].inicio, corte);
    }
    #[test] fn pausar_bloqueia_sessoes_do_timer() {
        use crate::modelo::{EstadoSessao, Sessao};
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.sessoes.insert("s1".into(), Sessao { id: "s1".into(), cwd: "x".into(), nome: None, transcript: None, timer_id: Some(1), estado: EstadoSessao::Trabalhando, ultimo_evento: h(8, 0), terminou_em: None, criada_em: h(8, 0), ligacao: None, desligada_em: None, origem: None, conta: None });
        m.pausar(1, h(9, 0));
        assert!(m.bloqueadas.contains_key("s1"));
    }
    #[test] fn ao_reabrir_fecha_no_ultimo_tique_e_pergunta_manual() {
        let mut m = Motor::novo(cfg_teste(), h(12, 0));
        let abertos = vec![
            TrechoAberto { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, inicio: h(9, 0), atividade_id: 1 },
            TrechoAberto { timer_id: Some(2), sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(9, 30), atividade_id: 1 },
        ];
        let v = m.ao_reabrir(abertos, h(10, 0), None, h(12, 0));
        assert_eq!(trechos(&v), vec![
            Mudanca::Fechou { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, fim: h(10, 0), motivo: MotivoFim::AppFechado },
            Mudanca::Fechou { timer_id: Some(2), sessao_id: Some("s1".into()), origem: Origem::Claude, fim: h(10, 0), motivo: MotivoFim::AppFechado },
        ]);
        assert!(v.iter().any(|x| matches!(x, Mudanca::Pergunta(_, Pergunta::AppFechado { timer_ids, de, ate, .. }) if timer_ids == &vec![1] && *de == h(10, 0) && *ate == h(12, 0))));
    }
    #[test] fn sono_atravessando_meia_noite_pergunta() {
        let mut m = Motor::novo(cfg_teste(), h(22, 0));
        m.play(1, h(23, 0));
        m.amostra(&am(h(23, 5), 0));
        let t = ms_local(2026, 11, 18, 8, 0, F);
        let v = m.amostra(&am(t, 0));
        assert!(v.contains(&Mudanca::Fechou { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, fim: h(23, 5), motivo: MotivoFim::Inatividade }));
        assert!(!v.iter().any(|x| matches!(x, Mudanca::Fechou { motivo: MotivoFim::CorteDoDia, .. })));
        assert!(v.iter().any(|x| matches!(x, Mudanca::Pergunta(_, Pergunta::VoltaInatividade { de, ate, .. }) if *de == h(23, 5) && *ate == t)));
    }
    #[test] fn inatividade_detectada_depois_da_meia_noite() {
        let mut m = Motor::novo(cfg_teste(), h(22, 0));
        m.play(1, h(23, 0));
        m.amostra(&am(h(23, 55), 5 * MIN));
        let v = m.amostra(&am(ms_local(2026, 11, 18, 0, 11, F), 21 * MIN));
        assert!(v.contains(&Mudanca::Fechou { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, fim: h(23, 50), motivo: MotivoFim::Inatividade }));
        assert!(!v.iter().any(|x| matches!(x, Mudanca::Fechou { motivo: MotivoFim::CorteDoDia, .. })));
    }
    // ---- atividades (rodada 9) ----

    use crate::modelo::{EstadoSessao, Sessao, ATIVIDADE_ESTUDO, ATIVIDADE_REUNIAO, ATIVIDADE_TRABALHO};
    use fila::EventoHook;
    use std::collections::BTreeMap;

    const R: i64 = ATIVIDADE_REUNIAO;
    const W: i64 = ATIVIDADE_TRABALHO;
    fn abriu_em(id: i64, t: Ms, atividade_id: i64) -> Mudanca {
        Mudanca::Abriu { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, inicio: t, atividade_id }
    }
    fn fechou_manual(id: i64, t: Ms, motivo: MotivoFim) -> Mudanca {
        Mudanca::Fechou { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, fim: t, motivo }
    }
    fn manuais(m: &Motor, id: i64) -> Vec<i64> {
        m.abertos.iter().filter(|t| t.origem == Origem::Manual && t.timer_id == Some(id)).map(|t| t.atividade_id).collect()
    }
    fn com_sessao(m: &mut Motor, timer: i64) {
        m.sessoes.insert("s1".into(), Sessao { id: "s1".into(), cwd: "G:\\Cliente A".into(), nome: None, transcript: None, timer_id: Some(timer), estado: EstadoSessao::EsperandoVoce, ultimo_evento: h(8, 0), terminou_em: Some(h(8, 0)), criada_em: h(8, 0), ligacao: None, desligada_em: None, origem: None, conta: None });
    }
    fn prompt(m: &mut Motor, t: Ms) -> Vec<Mudanca> {
        m.evento(&EventoHook { t, evento: "UserPromptSubmit".into(), sessao: "s1".into(), cwd: "G:\\Cliente A".into(), transcript: None, notificacao: None, origem: None, conta: None })
    }

    #[test] fn play_usa_a_atividade_padrao_do_timer() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        // com "Vários ▶ ao mesmo tempo" ligado (spec 003)
        m.cfg.um_play_por_vez = false;
        m.atividades_padrao.insert(2, ATIVIDADE_ESTUDO);
        assert_eq!(trechos(&m.play(1, h(9, 0))), vec![abriu_em(1, h(9, 0), W)]);
        assert_eq!(trechos(&m.play(2, h(9, 0))), vec![abriu_em(2, h(9, 0), ATIVIDADE_ESTUDO)]);
    }
    #[test] fn reuniao_com_o_play_ligado_troca_no_mesmo_instante_e_volta() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.play(1, h(9, 0));
        // 👥 com o ▶ ligado: o trabalho fecha e a reunião abre no mesmo instante
        assert_eq!(trechos(&m.play_atividade(1, R, h(10, 0))), vec![fechou_manual(1, h(10, 0), MotivoFim::TrocouAtividade), abriu_em(1, h(10, 0), R)]);
        assert_eq!(manuais(&m, 1), [R]);
        // 👥 de novo: nada
        assert_eq!(m.play_atividade(1, R, h(10, 5)), vec![]);
        // fim da reunião (👥 apertado): o trabalho volta no mesmo instante
        assert_eq!(trechos(&m.parar_atividade(1, R, h(11, 0))), vec![fechou_manual(1, h(11, 0), MotivoFim::TrocouAtividade), abriu_em(1, h(11, 0), W)]);
        assert_eq!(manuais(&m, 1), [W]);
        // e o contrário: ▶ com a reunião ligada troca a reunião pelo trabalho
        m.play_atividade(1, R, h(12, 0));
        assert_eq!(trechos(&m.play(1, h(12, 30))), vec![fechou_manual(1, h(12, 30), MotivoFim::TrocouAtividade), abriu_em(1, h(12, 30), W)]);
    }
    #[test] fn reuniao_do_zero_so_fecha_no_fim_sem_travar_o_claude() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        com_sessao(&mut m, 1);
        prompt(&mut m, h(9, 0));
        assert_eq!(trechos(&m.play_atividade(1, R, h(9, 30))), vec![abriu_em(1, h(9, 30), R)]);
        assert_eq!(trechos(&m.parar_atividade(1, R, h(10, 0))), vec![fechou_manual(1, h(10, 0), MotivoFim::Pausa)]);
        assert!(m.abertos.iter().any(|t| t.origem == Origem::Claude), "o Claude segue contando");
        assert!(!m.bloqueadas.contains_key("s1"), "sem a trava do pause");
        // parar a atividade que não está ligada não faz nada
        assert_eq!(m.parar_atividade(1, R, h(10, 5)), vec![]);
        m.play(1, h(10, 10));
        assert_eq!(m.parar_atividade(1, R, h(10, 20)), vec![]);
        assert_eq!(manuais(&m, 1), [W]);
    }
    #[test] fn claude_trabalhando_corre_junto_com_a_reuniao_manual() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.atividades_padrao.insert(1, W);
        com_sessao(&mut m, 1);
        m.play_atividade(1, R, h(9, 0));
        let v = trechos(&prompt(&mut m, h(9, 10)));
        assert_eq!(v, vec![Mudanca::Abriu { timer_id: Some(1), sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(9, 10), atividade_id: W }]);
        let mut at: Vec<(Origem, i64)> = m.abertos.iter().map(|t| (t.origem, t.atividade_id)).collect();
        at.sort_by_key(|x| x.1);
        assert_eq!(at, [(Origem::Claude, W), (Origem::Manual, R)]);
    }
    #[test] fn pause_encerrar_e_pausar_tudo_valem_para_os_dois() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        com_sessao(&mut m, 1);
        prompt(&mut m, h(9, 0));
        m.play_atividade(1, R, h(9, 10));
        let v = trechos(&m.pausar(1, h(9, 30)));
        assert_eq!(v.len(), 2);
        assert!(m.abertos.is_empty());
        // ■ e o desfazer: a reunião volta como reunião, no instante do ■
        prompt(&mut m, h(10, 0));
        m.play_atividade(1, R, h(10, 5));
        m.encerrar(1, h(10, 30));
        assert!(m.abertos.is_empty());
        let v = trechos(&m.desfazer_encerrar(1, h(10, 31)));
        assert!(v.contains(&abriu_em(1, h(10, 30), R)), "{v:?}");
        assert_eq!(manuais(&m, 1), [R]);
        m.play_atividade(2, R, h(11, 0));
        m.pausar_tudo(h(11, 30));
        assert!(m.abertos.is_empty());
        assert!(m.voltar_para.is_empty());
    }
    #[test] fn inatividade_pergunta_e_contar_reabre_a_reuniao() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        // com "Vários ▶ ao mesmo tempo" ligado (spec 003)
        m.cfg.um_play_por_vez = false;
        m.play(2, h(9, 0));
        m.play_atividade(1, R, h(9, 0));
        m.amostra(&am(h(9, 5), 0));
        let v = m.amostra(&am(h(9, 20), 11 * MIN));
        assert!(v.contains(&fechou_manual(1, h(9, 9), MotivoFim::Inatividade)));
        let v = m.amostra(&am(h(9, 35), 0));
        let Some(Mudanca::Pergunta(id, Pergunta::VoltaInatividade { atividades, .. })) = v.iter().find(|x| matches!(x, Mudanca::Pergunta(..))) else { panic!() };
        assert_eq!(atividades.get(&1), Some(&R));
        assert_eq!(atividades.get(&2), Some(&W));
        let r = trechos(&m.responder(*id, Resposta::Contar, h(9, 36)));
        assert!(r.contains(&abriu_em(1, h(9, 9), R)) && r.contains(&abriu_em(2, h(9, 9), W)), "{r:?}");
    }
    #[test] fn app_fechado_pergunta_e_contar_reabre_na_atividade_e_a_retomada_mantem() {
        let ab = |inicio| TrechoAberto { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, inicio, atividade_id: R };
        let mut m = Motor::novo(cfg_teste(), h(12, 0));
        let v = m.ao_reabrir(vec![ab(h(9, 0))], h(10, 0), None, h(12, 0));
        let Some(Mudanca::Pergunta(id, Pergunta::AppFechado { atividades, .. })) = v.iter().find(|x| matches!(x, Mudanca::Pergunta(..))) else { panic!() };
        assert_eq!(atividades.get(&1), Some(&R));
        assert_eq!(trechos(&m.responder(*id, Resposta::Contar, h(12, 1))), vec![abriu_em(1, h(10, 0), R)]);
        // reabrir em menos de 2 min: a reunião segue aberta, como reunião
        let mut m = Motor::novo(cfg_teste(), h(10, 1));
        m.ao_reabrir(vec![ab(h(9, 0))], h(10, 0), None, h(10, 1));
        assert_eq!(manuais(&m, 1), [R]);
    }
    #[test] fn corte_do_dia_reabre_o_claude_na_atividade_dele() {
        let mut m = Motor::novo(cfg_teste(), h(23, 0));
        m.atividades_padrao.insert(1, ATIVIDADE_ESTUDO);
        com_sessao(&mut m, 1);
        prompt(&mut m, h(23, 55));
        m.play_atividade(1, R, h(23, 56));
        let corte = ms_local(2026, 11, 18, 0, 0, F);
        let v = trechos(&m.tique(corte + MIN));
        assert!(v.contains(&Mudanca::Abriu { timer_id: Some(1), sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: corte, atividade_id: ATIVIDADE_ESTUDO }));
        assert!(v.contains(&fechou_manual(1, corte, MotivoFim::CorteDoDia)), "a reunião manual para na virada");
        assert_eq!(manuais(&m, 1), Vec::<i64>::new());
    }
    #[test] fn ligar_a_um_timer_de_estudo_leva_a_atividade_dele() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.atividades_padrao.insert(2, ATIVIDADE_ESTUDO);
        com_sessao(&mut m, 1);
        prompt(&mut m, h(9, 0));
        let v = trechos(&m.ligar_sessao("s1", 2, false, h(9, 30)));
        assert!(v.contains(&Mudanca::Abriu { timer_id: Some(2), sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(9, 30), atividade_id: ATIVIDADE_ESTUDO }));
        m.ligar_sessao("s1", 1, true, h(9, 40));
        assert_eq!(m.abertos.iter().map(|t| (t.timer_id, t.atividade_id)).collect::<Vec<_>>(), [(Some(1), W)]);
    }
    /// Sorteio de cliques (▶, 👥, fim da reunião, pause, ■, desfazer, "Pausar tudo", inatividade e
    /// o "Contar") em dois timers ao longo do dia: nunca há dois manuais abertos no mesmo timer, e
    /// os trechos manuais de um timer nunca se sobrepõem nem fecham antes de abrir.
    #[test] fn nunca_dois_manuais_do_mesmo_timer() {
        let mut m = Motor::novo(cfg_teste(), h(7, 0));
        com_sessao(&mut m, 1);
        let mut semente: u64 = 0x9e37;
        let mut sorteia = |n: u64| {
            semente = semente.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (semente >> 33) % n
        };
        let mut todas: Vec<Mudanca> = vec![];
        let mut t = h(8, 0);
        // o último clique ou mensagem: a ociosidade de uma amostra nunca passa dele
        let mut uso = t;
        while t < h(20, 0) {
            t += (5 + sorteia(400) as i64) * 1000;
            let op = sorteia(12);
            let clique = op < 11;
            let id = 1 + sorteia(2) as i64;
            let at = [W, R, ATIVIDADE_ESTUDO][sorteia(3) as usize];
            let v = match op {
                0..=2 => m.play(id, t),
                3..=5 => m.play_atividade(id, at, t),
                6 if sorteia(2) == 0 => m.parar_atividade(id, at, t),
                6 => m.pausar_atividade(id, at, t),
                7 => m.pausar(id, t),
                8 => m.encerrar(id, t),
                9 => m.desfazer_encerrar(id, t),
                10 => m.pausar_tudo(t),
                _ => {
                    // amostra: às vezes longe do PC desde o último uso (a inatividade pausa os
                    // manuais), às vezes de volta (a pergunta e o "Contar")
                    let ocioso = if sorteia(2) == 0 {
                        t += 12 * MIN;
                        t - uso
                    } else {
                        uso = t;
                        0
                    };
                    let mut v = m.amostra(&am(t, ocioso));
                    if let Some(p) = m.perguntas.first().map(|p| p.0) {
                        match sorteia(3) {
                            0 => v.extend(m.responder(p, Resposta::Contar, t)),
                            1 => v.extend(m.responder(p, Resposta::RetomarAgora, t)),
                            _ => {}
                        }
                    }
                    v
                }
            };
            if sorteia(5) == 0 {
                todas.extend(prompt(&mut m, t));
                uso = t;
            }
            if clique {
                uso = t;
            }
            todas.extend(v);
            for id in [1, 2] {
                assert!(manuais(&m, id).len() <= 1, "dois manuais no timer {id} em {t}");
            }
        }
        // replay: cada timer com no máximo um manual aberto, e os fechados em ordem
        let mut aberto: BTreeMap<i64, Ms> = BTreeMap::new();
        let mut fechados: Vec<(i64, Ms, Ms)> = vec![];
        for x in &todas {
            match x {
                Mudanca::Abriu { timer_id: Some(id), origem: Origem::Manual, inicio, .. } => {
                    assert!(aberto.insert(*id, *inicio).is_none(), "abriu um segundo manual no timer {id}");
                }
                Mudanca::Fechou { timer_id: Some(id), origem: Origem::Manual, fim, .. } => {
                    let ini = aberto.remove(id).expect("fechou sem abrir");
                    assert!(*fim >= ini);
                    fechados.push((*id, ini, *fim));
                }
                _ => {}
            }
        }
        for id in [1, 2] {
            let mut v: Vec<(Ms, Ms)> = fechados.iter().filter(|x| x.0 == id).map(|x| (x.1, x.2)).collect();
            v.sort();
            assert!(v.windows(2).all(|w| w[0].1 <= w[1].0), "manuais sobrepostos no timer {id}");
        }
        assert!(fechados.len() > 20);
    }

    // ---- ⏸ da linha de uma atividade (rodada 10) ----

    #[test] fn pausar_o_trabalho_com_a_reuniao_aberta_so_para_o_trabalho() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        com_sessao(&mut m, 1);
        prompt(&mut m, h(9, 0));
        m.play_atividade(1, R, h(10, 0));
        let v = trechos(&m.pausar_atividade(1, W, h(10, 30)));
        assert_eq!(v, vec![Mudanca::Fechou { timer_id: Some(1), sessao_id: Some("s1".into()), origem: Origem::Claude, fim: h(10, 30), motivo: MotivoFim::Pausa }]);
        assert_eq!(manuais(&m, 1), [R], "a reunião segue");
        assert!(m.bloqueadas.contains_key("s1"), "a trava do pause: o Claude só volta com uma mensagem sua");
        // o ▶ substituído pela reunião não volta mais no fim dela
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.play(1, h(9, 0));
        m.play_atividade(1, R, h(10, 0));
        m.pausar_atividade(1, W, h(10, 30));
        assert_eq!(trechos(&m.parar_atividade(1, R, h(11, 0))), vec![fechou_manual(1, h(11, 0), MotivoFim::Pausa)]);
        assert!(m.abertos.is_empty());
    }
    #[test] fn pausar_a_reuniao_e_o_trabalho_sem_reuniao() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        com_sessao(&mut m, 1);
        prompt(&mut m, h(9, 0));
        // ⏸ da Reunião: é o 👥 aceso (o Claude segue, sem trava)
        m.play_atividade(1, R, h(9, 30));
        assert_eq!(trechos(&m.pausar_atividade(1, R, h(10, 0))), vec![fechou_manual(1, h(10, 0), MotivoFim::Pausa)]);
        assert!(m.abertos.iter().any(|t| t.origem == Origem::Claude));
        assert!(!m.bloqueadas.contains_key("s1"));
        // ⏸ do Trabalho sem reunião: é o ⏸ do timer
        m.play(1, h(10, 10));
        let v = trechos(&m.pausar_atividade(1, W, h(10, 20)));
        assert_eq!(v.len(), 2, "o ▶ e o Claude: {v:?}");
        assert!(m.abertos.is_empty());
        assert!(m.bloqueadas.contains_key("s1"));
        // a atividade padrão de outro timer (Estudo): o ⏸ dela é o do timer
        m.atividades_padrao.insert(2, ATIVIDADE_ESTUDO);
        m.play(2, h(11, 0));
        assert_eq!(trechos(&m.pausar_atividade(2, ATIVIDADE_ESTUDO, h(11, 10))), vec![fechou_manual(2, h(11, 10), MotivoFim::Pausa)]);
    }

    #[test] fn pausar_a_antiga_padrao_fecha_o_claude_que_ficou_nela() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        com_sessao(&mut m, 1);
        prompt(&mut m, h(9, 0));
        // a padrão do timer passou a Estudo com o Claude aberto em Trabalho
        m.atividades_padrao.insert(1, ATIVIDADE_ESTUDO);
        let v = trechos(&m.pausar_atividade(1, W, h(9, 30)));
        assert_eq!(v, vec![Mudanca::Fechou { timer_id: Some(1), sessao_id: Some("s1".into()), origem: Origem::Claude, fim: h(9, 30), motivo: MotivoFim::Pausa }]);
        assert!(!m.bloqueadas.contains_key("s1"), "sem trava");
        // o Claude volta na padrão nova
        let v = trechos(&prompt(&mut m, h(9, 40)));
        assert!(v.iter().any(|x| matches!(x, Mudanca::Abriu { origem: Origem::Claude, atividade_id, .. } if *atividade_id == ATIVIDADE_ESTUDO)), "{v:?}");
    }

    // ---- "Retomar agora" (rodada 10) ----

    #[test] fn retomar_agora_religa_os_mesmos_timers_e_atividades_sem_o_tempo_fora() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        // com "Vários ▶ ao mesmo tempo" ligado (spec 003)
        m.cfg.um_play_por_vez = false;
        m.play(2, h(9, 0));
        m.play_atividade(1, R, h(9, 0));
        m.amostra(&am(h(9, 5), 0));
        m.amostra(&am(h(9, 20), 11 * MIN));
        let v = m.amostra(&am(h(9, 35), 0));
        let id = pergunta_de(&v);
        let r = m.responder(id, Resposta::RetomarAgora, h(9, 36));
        let t = trechos(&r);
        assert_eq!(t.len(), 2, "{t:?}");
        assert!(t.contains(&abriu_em(1, h(9, 36), R)) && t.contains(&abriu_em(2, h(9, 36), W)), "{t:?}");
        assert!(r.contains(&Mudanca::PerguntaResolvida(id)));
        assert!(m.perguntas.is_empty());
        assert_eq!((manuais(&m, 1), manuais(&m, 2)), (vec![R], vec![W]));
    }
    #[test] fn retomar_agora_no_app_fechado_e_depois_da_virada() {
        let ab = |inicio| TrechoAberto { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, inicio, atividade_id: R };
        let mut m = Motor::novo(cfg_teste(), h(12, 0));
        let v = m.ao_reabrir(vec![ab(h(9, 0))], h(10, 0), None, h(12, 0));
        let id = pergunta_de(&v);
        assert_eq!(trechos(&m.responder(id, Resposta::RetomarAgora, h(12, 1))), vec![abriu_em(1, h(12, 1), R)]);
        // a volta no dia seguinte: o manual parou na virada (regra 9), o "Retomar agora" não religa
        // o timer da noite anterior; só resolve a pergunta
        let mut m = Motor::novo(cfg_teste(), h(22, 0));
        m.play(1, h(23, 0));
        m.amostra(&am(h(23, 20), 0));
        m.amostra(&am(h(23, 40), 10 * MIN));
        let v = m.amostra(&am(ms_local(2026, 11, 18, 8, 0, F), 0));
        let id = pergunta_de(&v);
        let r = m.responder(id, Resposta::RetomarAgora, ms_local(2026, 11, 18, 8, 10, F));
        assert_eq!(trechos(&r), vec![]);
        assert!(r.contains(&Mudanca::PerguntaResolvida(id)));
        assert!(m.abertos.is_empty() && m.perguntas.is_empty());
        // a pergunta de ontem respondida hoje, mesmo sem a ausência ter atravessado quando foi
        // feita: também não religa
        let mut m = Motor::novo(cfg_teste(), h(22, 0));
        m.play(1, h(23, 0));
        m.amostra(&am(h(23, 20), 0));
        m.amostra(&am(h(23, 40), 10 * MIN));
        let id = pergunta_de(&m.amostra(&am(h(23, 50), 0)));
        assert_eq!(trechos(&m.responder(id, Resposta::RetomarAgora, ms_local(2026, 11, 18, 0, 5, F))), vec![]);
        assert!(m.abertos.is_empty());
    }
    #[test] fn retomar_agora_atrasado_nao_faz_nada_e_respeita_quem_ja_mexeu() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        // com "Vários ▶ ao mesmo tempo" ligado (spec 003)
        m.cfg.um_play_por_vez = false;
        m.play(1, h(9, 0));
        m.play(2, h(9, 0));
        m.amostra(&am(h(9, 5), 0));
        m.amostra(&am(h(9, 20), 11 * MIN));
        let id = pergunta_de(&m.amostra(&am(h(9, 35), 0)));
        // o 2 foi religado à mão: sai da pergunta, e o "Retomar agora" só religa o 1
        m.play(2, h(9, 36));
        let r = trechos(&m.responder(id, Resposta::RetomarAgora, h(9, 37)));
        assert_eq!(r, vec![abriu_manual(1, h(9, 37))]);
        assert_eq!(m.responder(id, Resposta::RetomarAgora, h(9, 38)), vec![]);
    }

    #[test] fn contar_depois_da_virada_nao_atravessa_o_corte() {
        let mut m = Motor::novo(cfg_teste(), h(22, 0));
        m.play(1, h(23, 0));
        m.amostra(&am(h(23, 20), 0));
        m.amostra(&am(h(23, 40), 10 * MIN));
        let v = m.amostra(&am(ms_local(2026, 11, 18, 8, 0, F), 0));
        let id = pergunta_de(&v);
        let r = m.responder(id, Resposta::Contar, ms_local(2026, 11, 18, 8, 10, F));
        // só de 00:00 até a volta (08:00), fechado: nunca até a resposta, e o timer fica parado
        let zero = ms_local(2026, 11, 18, 0, 0, F);
        assert_eq!(trechos(&r), vec![abriu_manual(1, zero), fechou_manual(1, ms_local(2026, 11, 18, 8, 0, F), MotivoFim::Pausa)]);
        assert!(m.abertos.is_empty());
    }

    /// A revisão (I2): a ausência inteira de ontem (23:20–23:50), com a pergunta respondida hoje às
    /// 08:10. Nem "Contar" nem "Retomar agora" abrem ou contam nada hoje; só resolvem a pergunta.
    #[test] fn pergunta_de_ontem_respondida_hoje_nao_abre_nem_conta() {
        for r in [Resposta::Contar, Resposta::RetomarAgora, Resposta::DeixarPausado] {
            let mut m = Motor::novo(cfg_teste(), h(22, 0));
            m.play(1, h(23, 0));
            m.amostra(&am(h(23, 10), 0));
            m.amostra(&am(h(23, 35), 15 * MIN));
            let id = pergunta_de(&m.amostra(&am(h(23, 50), 0)));
            assert!(matches!(&m.perguntas[0].1, Pergunta::VoltaInatividade { de, ate, .. } if *de == h(23, 20) && *ate == h(23, 50)));
            let v = m.responder(id, r.clone(), ms_local(2026, 11, 18, 8, 10, F));
            assert_eq!(trechos(&v), vec![], "{r:?}");
            assert!(v.contains(&Mudanca::PerguntaResolvida(id)));
            assert!(m.abertos.is_empty() && m.perguntas.is_empty());
        }
        // o app fechado ontem e reaberto ontem, respondido hoje: o mesmo
        let ab = TrechoAberto { timer_id: Some(1), sessao_id: None, origem: Origem::Manual, inicio: h(22, 0), atividade_id: 1 };
        let mut m = Motor::novo(cfg_teste(), h(23, 30));
        let id = pergunta_de(&m.ao_reabrir(vec![ab], h(23, 0), None, h(23, 30)));
        assert_eq!(trechos(&m.responder(id, Resposta::Contar, ms_local(2026, 11, 18, 8, 10, F))), vec![]);
        assert!(m.abertos.is_empty());
    }
    // ---- um ▶ por vez (spec 003) ----

    #[test] fn um_play_por_vez_pausa_o_play_dos_outros_no_mesmo_instante() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        assert!(m.cfg.um_play_por_vez, "o padrão é um ▶ por vez");
        m.play(1, h(9, 0));
        assert_eq!(trechos(&m.play(2, h(10, 0))), vec![fechou_manual(1, h(10, 0), MotivoFim::Pausa), abriu_manual(2, h(10, 0))]);
        assert_eq!(m.pausado_desde.get(&1), Some(&h(10, 0)), "o outro fica pausado no dock");
        // a reunião de outro timer também é o seu cronômetro
        assert_eq!(trechos(&m.play_atividade(1, R, h(10, 30))), vec![fechou_manual(2, h(10, 30), MotivoFim::Pausa), abriu_em(1, h(10, 30), R)]);
        assert_eq!(trechos(&m.play(2, h(11, 0))), vec![fechou_manual(1, h(11, 0), MotivoFim::Pausa), abriu_manual(2, h(11, 0))]);
        // o ▶ de novo no mesmo timer não mexe em nada
        assert_eq!(m.play(2, h(11, 5)), vec![]);
    }
    #[test] fn um_play_por_vez_deixa_o_claude_do_outro_timer_rodando() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        com_sessao(&mut m, 1);
        prompt(&mut m, h(9, 0));
        m.play(1, h(9, 0));
        let v = trechos(&m.play(2, h(9, 30)));
        assert_eq!(v, vec![fechou_manual(1, h(9, 30), MotivoFim::Pausa), abriu_manual(2, h(9, 30))]);
        assert!(m.abertos.iter().any(|t| t.origem == Origem::Claude && t.timer_id == Some(1)), "o Claude segue no 1");
        assert!(!m.bloqueadas.contains_key("s1"), "sem a trava do ⏸");
    }
    #[test] fn um_play_por_vez_com_varios_play_ligado_nao_pausa() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.cfg.um_play_por_vez = false;
        m.play(1, h(9, 0));
        assert_eq!(trechos(&m.play(2, h(10, 0))), vec![abriu_manual(2, h(10, 0))]);
    }
    #[test] fn a_volta_da_inatividade_religa_so_o_ultimo_play() {
        // dois ▶ de antes (com "Vários ▶", ou da versão de antes) e a opção agora em um por vez
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.cfg.um_play_por_vez = false;
        m.play(1, h(9, 0));
        m.play(2, h(9, 2));
        m.amostra(&am(h(9, 5), 0));
        m.amostra(&am(h(9, 20), 11 * MIN));
        let id = pergunta_de(&m.amostra(&am(h(9, 35), 0)));
        m.cfg.um_play_por_vez = true;
        assert_eq!(trechos(&m.responder(id, Resposta::Contar, h(9, 36))), vec![abriu_manual(2, h(9, 9))]);
        assert_eq!(m.abertos.iter().map(|t| t.timer_id).collect::<Vec<_>>(), [Some(2)]);
    }
    #[test] fn contar_com_o_play_ja_noutro_timer_conta_o_tempo_fora_fechado() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        let id = pergunta_de(&volta(&mut m));
        // voltou e deu ▶ noutro timer antes de responder
        m.play(2, h(9, 36));
        let r = trechos(&m.responder(id, Resposta::Contar, h(9, 37)));
        assert_eq!(r, vec![abriu_manual(1, h(9, 9)), fechou_manual(1, h(9, 35), MotivoFim::Pausa)]);
        assert_eq!(m.abertos.iter().map(|t| t.timer_id).collect::<Vec<_>>(), [Some(2)]);
        // o "Retomar agora" é como o ▶: o outro pausa
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        let id = pergunta_de(&volta(&mut m));
        m.play(2, h(9, 36));
        let r = trechos(&m.responder(id, Resposta::RetomarAgora, h(9, 37)));
        assert_eq!(r, vec![fechou_manual(2, h(9, 37), MotivoFim::Pausa), abriu_manual(1, h(9, 37))]);
    }
    #[test] fn ao_reabrir_em_menos_de_2_min_retoma_so_o_play_mais_recente() {
        let ab = |id, inicio| TrechoAberto { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, inicio, atividade_id: W };
        let mut m = Motor::novo(cfg_teste(), h(10, 1));
        let v = m.ao_reabrir(vec![ab(2, h(9, 30)), ab(1, h(9, 0))], h(10, 0), None, h(10, 1));
        assert_eq!(trechos(&v), vec![fechou_manual(1, h(10, 0), MotivoFim::AppFechado)]);
        assert!(v.iter().any(|x| matches!(x, Mudanca::Pergunta(_, Pergunta::AppFechado { timer_ids, .. }) if timer_ids == &vec![1])));
        assert_eq!(m.abertos.iter().map(|t| t.timer_id).collect::<Vec<_>>(), [Some(2)]);
    }
    #[test] fn desfazer_o_encerrar_nao_religa_o_play_com_outro_ja_rodando() {
        let mut m = Motor::novo(cfg_teste(), h(8, 0));
        m.play(1, h(9, 0));
        m.encerrar(1, h(9, 30));
        m.play(2, h(9, 31));
        let v = trechos(&m.desfazer_encerrar(1, h(9, 32)));
        assert_eq!(v, vec![]);
        assert_eq!(m.abertos.iter().map(|t| t.timer_id).collect::<Vec<_>>(), [Some(2)]);
    }
    /// A revisão (M4): o app reaberto em menos de 2 min com dois ▶ (de antes da 0.6.2): o "Contar"
    /// do que fechou não conta junto com o que seguiu.
    #[test] fn contar_no_app_reaberto_nao_conta_junto_com_o_play_que_seguiu() {
        let ab = |id, inicio| TrechoAberto { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, inicio, atividade_id: W };
        let mut m = Motor::novo(cfg_teste(), h(10, 1));
        let v = m.ao_reabrir(vec![ab(1, h(9, 0)), ab(2, h(9, 30))], h(10, 0), None, h(10, 1));
        let id = pergunta_de(&v);
        assert_eq!(trechos(&m.responder(id, Resposta::Contar, h(10, 2))), vec![]);
        assert_eq!(m.abertos.iter().map(|t| t.timer_id).collect::<Vec<_>>(), [Some(2)]);
        assert!(m.pausado_desde.contains_key(&1));
    }
    /// A revisão (M5): a ausência que atravessa a virada, com dois ▶ de antes: o "Contar" de 00:00
    /// em diante vale só para o último ▶.
    #[test] fn contar_depois_da_virada_com_um_play_por_vez_conta_so_o_ultimo() {
        let mut m = Motor::novo(cfg_teste(), h(22, 0));
        m.cfg.um_play_por_vez = false;
        m.play(1, h(23, 0));
        m.play(2, h(23, 1));
        m.amostra(&am(h(23, 20), 0));
        m.amostra(&am(h(23, 40), 10 * MIN));
        let id = pergunta_de(&m.amostra(&am(ms_local(2026, 11, 18, 0, 30, F), 0)));
        m.cfg.um_play_por_vez = true;
        let zero = ms_local(2026, 11, 18, 0, 0, F);
        let r = trechos(&m.responder(id, Resposta::Contar, ms_local(2026, 11, 18, 0, 31, F)));
        assert_eq!(r, vec![abriu_manual(2, zero), fechou_manual(2, ms_local(2026, 11, 18, 0, 30, F), MotivoFim::Pausa)]);
    }
    /// A re-revisão (R4): depois da virada, o "Contar" com o ▶ já noutro timer acaba no início dele.
    #[test] fn contar_depois_da_virada_acaba_no_inicio_do_outro_play() {
        let mut m = Motor::novo(cfg_teste(), h(22, 0));
        m.play(1, h(23, 0));
        m.amostra(&am(h(23, 20), 0));
        m.amostra(&am(h(23, 40), 10 * MIN));
        let id = pergunta_de(&m.amostra(&am(ms_local(2026, 11, 18, 0, 30, F), 0)));
        let zero = ms_local(2026, 11, 18, 0, 0, F);
        m.play(2, zero + 10 * MIN);
        let r = trechos(&m.responder(id, Resposta::Contar, ms_local(2026, 11, 18, 0, 31, F)));
        assert_eq!(r, vec![abriu_manual(1, zero), fechou_manual(1, zero + 10 * MIN, MotivoFim::Pausa)]);
    }
}
