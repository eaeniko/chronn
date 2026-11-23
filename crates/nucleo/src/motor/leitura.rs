// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Tempo de leitura (spec §6, regra 3): o tempo que voce passa no Claude com a conversa de uma
//! sessao aberta, enquanto ela nao trabalha. A conversa aberta e reconhecida pelo titulo: no app
//! do Claude, pelo `Document` da UI Automation; nos outros programas, pelo titulo da janela.
//! Privacidade: titulos so ficam em memoria, para comparar.
use super::chat::{e_sessao_do_chat, sessao_do_chat};
use super::manual::Amostra;
use super::Motor;
use crate::modelo::{EstadoSessao, MotivoFim, Mudanca, Origem, Sessao, TrechoAberto};
use crate::tempo::{Ms, MIN};

/// Nome de sessao mais curto que isto nao e procurado no titulo da janela (falso positivo).
pub const NOME_MIN_NO_TITULO: usize = 6;
/// Sessao que terminou ha menos que isto ainda aparece "esperando voce" (tela e virada do dia).
pub const ESPERA_RECENTE_MS: Ms = 60 * MIN;
/// As janelas do app sao neutras por ate isto seguido; passou, a leitura fecha quando voce
/// entrou no app.
pub const NEUTRA_MAX_MS: Ms = 2 * MIN;
/// Amostras seguidas com evidencia de que voce saiu da conversa para liberar a leitura pausada.
pub const SAIDAS_PARA_LIBERAR: u32 = 2;

/// De onde vem a conversa aberta na janela em uso.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Conversa {
    /// Outro programa do Claude (terminal, VS Code): o titulo da janela deve conter o nome da sessao.
    #[default]
    PeloTitulo,
    /// App do Claude: o titulo da conversa aberta, lido pela UI Automation. `None` e "nao sei":
    /// a consulta falhou, passou do limite ou nao achou conversa.
    App(Option<String>),
    /// App do Claude, uma conversa do Chat (rodada 27): o id (do endereco) e o titulo. A sessao e
    /// achada pelo id (`chat:<id>`), nao pelo nome.
    Chat { id: String, titulo: String },
}

/// O que a leitura usa da janela em uso.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct JanelaEmUso {
    pub processo: Option<String>,
    pub titulo: Option<String>,
    pub conversa: Conversa,
    /// A janela (o HWND; rodada 28), só em memória: a conversa aberta no terminal sem o nome no título.
    pub janela: Option<isize>,
}

impl JanelaEmUso {
    fn de(a: &Amostra) -> JanelaEmUso {
        JanelaEmUso { processo: a.processo.clone(), titulo: a.titulo.clone(), conversa: a.conversa.clone(), janela: a.janela }
    }
}

/// A conversa aberta numa janela do Claude.
#[derive(Clone, Debug, PartialEq)]
pub enum Aberta {
    Sessao(String),
    /// Uma conversa lida pela UI Automation que nao e de nenhuma sessao conhecida.
    Desconhecida,
    /// Nao da para saber: a UI Automation nao respondeu ou nao achou conversa, o titulo da janela
    /// nao traz nome de sessao, ou o nome e de sessoes de timers diferentes.
    NaoSei,
}

/// A situacao da leitura (qual conversa conta, "escrevendo") so muda depois que a nova se mantem
/// por isto: a tela nao pisca (nem pausa o timer) a cada olhada para outro lugar. Com a amostra a
/// cada 5 s, sao duas amostras seguidas mostrando a situacao nova: uma so nunca troca nada. A
/// troca confirmada vale desde o instante em que aconteceu (a contagem fica exata); voltar a
/// mesma conversa antes disso nao deixa buraco.
pub const TROCA_SUAVE_MS: Ms = 5_000;

/// Uma troca da leitura esperando `TROCA_SUAVE_MS` para valer.
#[derive(Clone, Debug, PartialEq)]
pub struct Troca {
    /// A sessao que passa a ser lida, ou `None`: nenhuma (voce saiu da conversa).
    pub alvo: Option<String>,
    /// Desde quando a situacao nova se mantem (a amostra em que apareceu).
    pub visto: Ms,
    /// Onde a leitura nova comeca, se a troca valer: `visto`, ou depois de um fechamento no meio.
    pub inicio: Ms,
    /// Quando (e por que) voce saiu da leitura aberta: onde ela acaba, se a troca valer.
    pub saiu: Option<(Ms, MotivoFim)>,
    /// Viradas do dia no meio da espera.
    pub cortes: Vec<Ms>,
    /// Viradas em que a leitura aberta ficou sem dividir (a saida comecou antes delas).
    pub adiados: Vec<Ms>,
    /// Amostras desde que a espera comecou (a que a mostrou e as seguintes, inclusive as do
    /// app): voltar a conversa lida so faz a saida valer com duas ou mais (nunca uma so).
    pub amostras: u32,
}

/// Onde voce esta, para a leitura.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Onde {
    /// Janela do app ha ate `NEUTRA_MAX_MS`: nada muda.
    Neutra,
    /// Janela do app ha mais tempo: conta como ter saido do Claude no instante em que entrou.
    NeutraVencida(Ms),
    /// Programa que nao e do Claude nem do app.
    Fora { em_foco: bool },
    /// Um programa do Claude.
    Claude(JanelaEmUso),
}

fn nome_de(s: &Sessao) -> Option<&str> {
    s.nome.as_deref().map(str::trim).filter(|n| !n.is_empty())
}

/// Sessoes com o mesmo nome: a de evento mais recente, se todas sao do mesmo timer; de timers
/// diferentes, nao da para saber para qual vai o tempo.
fn uma_so(candidatas: &[&Sessao]) -> Aberta {
    let Some(primeira) = candidatas.first() else { return Aberta::NaoSei };
    if candidatas.iter().any(|s| s.timer_id != primeira.timer_id) {
        return Aberta::NaoSei;
    }
    let s = candidatas.iter().max_by_key(|s| s.ultimo_evento).expect("nao vazia");
    Aberta::Sessao(s.id.clone())
}

// IMPL-INICIO
impl Motor {
    /// Sessao sendo lida agora (trecho `Leitura` aberto).
    pub fn lendo_agora(&self) -> Option<&str> {
        self.abertos.iter().find(|t| t.origem == Origem::Leitura).and_then(|t| t.sessao_id.as_deref())
    }

    /// O fim de turno mais recente de uma sessão que espera você, para o tempo do lembrete
    /// (0.6.1): a lida agora, `agora`; as outras, o mais novo entre o fim do turno do Claude e a
    /// sua última leitura dela depois dele (`lida_em`).
    pub fn espera_ultima(&self, s: &Sessao, agora: Ms) -> Option<Ms> {
        if self.lendo_agora() == Some(s.id.as_str()) {
            return Some(agora);
        }
        let fim = s.terminou_em?;
        Some(self.lida_em.get(&s.id).copied().filter(|l| *l > fim).unwrap_or(fim))
    }

    /// A sessao ainda espera voce de verdade: e a que esta sendo lida ou terminou ha menos de
    /// `ESPERA_RECENTE_MS`. A que terminou ha horas continua `EsperandoVoce` no estado.
    /// Com o timer encerrado (■), o lembrete some.
    pub fn espera_recente(&self, s: &Sessao, agora: Ms) -> bool {
        s.estado == EstadoSessao::EsperandoVoce
            && !self.encerrado(s.timer_id)
            && (self.lendo_agora() == Some(s.id.as_str()) || s.terminou_em.is_some_and(|t| agora < t + ESPERA_RECENTE_MS))
    }

    fn no_claude(&self, j: &JanelaEmUso) -> bool {
        j.processo.as_deref().is_some_and(|p| {
            let p = p.to_lowercase();
            self.cfg.programas_claude.iter().any(|c| c.to_lowercase() == p)
        })
    }

    /// Conversa aberta numa janela do Claude. No app, o titulo lido e igual ao nome da sessao (sem
    /// diferenciar maiusculas, sem espacos nas pontas); nos outros programas, o titulo da janela
    /// contem o nome (de pelo menos `NOME_MIN_NO_TITULO` letras), o mais longo vencendo. Nome de
    /// sessoes de timers diferentes e ambiguo: "nao sei" (melhor faltar do que sobrar). A conversa
    /// do Chat e achada pelo id; as sessoes do Chat nao entram na busca pelo nome.
    pub fn sessao_aberta(&self, j: &JanelaEmUso) -> Aberta {
        let do_code = |s: &&Sessao| !e_sessao_do_chat(&s.id);
        match &j.conversa {
            Conversa::App(None) => Aberta::NaoSei,
            Conversa::Chat { id, .. } => {
                let id = sessao_do_chat(id);
                if self.sessoes.contains_key(&id) { Aberta::Sessao(id) } else { Aberta::Desconhecida }
            }
            Conversa::App(Some(titulo)) => {
                let t = titulo.trim().to_lowercase();
                let iguais: Vec<&Sessao> = self
                    .sessoes
                    .values()
                    .filter(do_code)
                    .filter(|s| nome_de(s).is_some_and(|n| n.to_lowercase() == t))
                    .collect();
                if iguais.is_empty() { Aberta::Desconhecida } else { uma_so(&iguais) }
            }
            // rodada 28: sem o nome no título (o Claude Code no terminal não põe), a sessão ligada à
            // janela que terminou por último (`sessao_pela_janela`)
            Conversa::PeloTitulo => match self.pelo_titulo(j) {
                Aberta::Sessao(id) => Aberta::Sessao(id),
                _ => self.sessao_pela_janela(j.janela).map_or(Aberta::NaoSei, Aberta::Sessao),
            },
        }
    }

    /// A conversa aberta pelo título da janela (o nome da sessão contido nele, o mais longo vencendo).
    fn pelo_titulo(&self, j: &JanelaEmUso) -> Aberta {
        let do_code = |s: &&Sessao| !e_sessao_do_chat(&s.id);
        let Some(titulo) = j.titulo.as_deref().map(str::to_lowercase) else { return Aberta::NaoSei };
        let contidas: Vec<(usize, &Sessao)> = self
            .sessoes
            .values()
            .filter(do_code)
            .filter_map(|s| {
                let n = nome_de(s)?;
                let letras = n.chars().count();
                (letras >= NOME_MIN_NO_TITULO && titulo.contains(&n.to_lowercase())).then_some((letras, s))
            })
            .collect();
        let maior = contidas.iter().map(|(l, _)| *l).max().unwrap_or(0);
        let mais_longas: Vec<&Sessao> = contidas.iter().filter(|(l, _)| *l == maior).map(|(_, s)| *s).collect();
        uma_so(&mais_longas)
    }

    /// Ligada a um timer nao encerrado, sem trabalhar (esperando voce ou parada, ate de dias atras)
    /// e sem a leitura pausada.
    fn pode_ser_lida(&self, id: &str) -> bool {
        !self.leitura_bloqueada.contains_key(id)
            && self.sessoes.get(id).is_some_and(|s| {
                s.timer_id.is_some() && !self.encerrado(s.timer_id) && s.estado != EstadoSessao::Trabalhando
            })
    }

    fn abrir_leitura(&mut self, sessao_id: &str, inicio: Ms) -> Option<Mudanca> {
        let timer_id = self.sessoes.get(sessao_id)?.timer_id;
        // leitura e escrita: a atividade padrao do timer
        let atividade_id = self.atividade_padrao(timer_id);
        self.abertos.push(TrechoAberto {
            timer_id,
            sessao_id: Some(sessao_id.to_string()),
            origem: Origem::Leitura,
            inicio,
            atividade_id,
        });
        Some(Mudanca::Abriu { timer_id, sessao_id: Some(sessao_id.to_string()), origem: Origem::Leitura, inicio, atividade_id })
    }

    /// Fecha a leitura aberta da sessao (se houver).
    pub(crate) fn fechar_leitura_de(&mut self, sessao_id: &str, fim: Ms, motivo: MotivoFim) -> Vec<Mudanca> {
        self.fechar_se(fim, motivo, |t| t.origem == Origem::Leitura && t.sessao_id.as_deref() == Some(sessao_id))
    }

    /// Onde voce esta nesta amostra. As janelas do app (principal e dock) sao neutras por ate
    /// `NEUTRA_MAX_MS` seguidos.
    pub(crate) fn onde_da_amostra(&mut self, a: &Amostra) -> Onde {
        if a.neutra {
            self.saiu_do_app = None;
            let desde = *self.neutra_desde.get_or_insert(a.t);
            return if a.t - desde > NEUTRA_MAX_MS { Onde::NeutraVencida(desde) } else { Onde::Neutra };
        }
        self.saiu_do_app = self.neutra_desde.take();
        let j = JanelaEmUso::de(a);
        if self.no_claude(&j) { Onde::Claude(j) } else { Onde::Fora { em_foco: a.em_foco } }
    }

    fn aberta_em(&self, onde: &Onde) -> Option<Aberta> {
        match onde {
            Onde::Claude(j) => Some(self.sessao_aberta(j)),
            _ => None,
        }
    }

    /// Leitura pausada (regra 8): so volta com evidencia de que voce saiu da conversa, por
    /// `SAIDAS_PARA_LIBERAR` amostras seguidas: outra conversa reconhecida, ou outro programa em
    /// foco (debaixo do mouse nao basta). "Nao sei" e a propria conversa zeram a conta; as janelas
    /// do app nao mudam nada. Escrever na conversa tambem libera (no `evento`).
    fn conferir_bloqueios(&mut self, onde: &Onde, aberta: Option<&Aberta>) {
        let saiu = |id: &str| -> Option<bool> {
            match onde {
                Onde::Neutra | Onde::NeutraVencida(_) => None,
                Onde::Fora { em_foco } => Some(*em_foco),
                Onde::Claude(_) => Some(match aberta {
                    Some(Aberta::Sessao(s)) => s != id,
                    Some(Aberta::Desconhecida) => true,
                    Some(Aberta::NaoSei) | None => false,
                }),
            }
        };
        let mut liberar = vec![];
        for (id, saidas) in self.leitura_bloqueada.iter_mut() {
            match saiu(id) {
                Some(true) => *saidas += 1,
                Some(false) => *saidas = 0,
                None => {}
            }
            if *saidas >= SAIDAS_PARA_LIBERAR {
                liberar.push(id.clone());
            }
        }
        for id in liberar {
            self.leitura_bloqueada.remove(&id);
        }
    }

    /// Fecha a leitura aberta em `fim`; antes, a virada do dia que caiu antes de `fim` (para a
    /// leitura nao atravessar o dia).
    fn fechar_lida(&mut self, fim: Ms, motivo: MotivoFim) -> Vec<Mudanca> {
        let Some(id) = self.lendo_agora().map(str::to_string) else { return vec![] };
        let mut out = self.corte_do_dia(fim);
        out.extend(self.fechar_leitura_de(&id, fim, motivo));
        out
    }

    /// Fins que nao esperam a troca suave (ja passaram do prazo deles): sem uso por mais que o
    /// tempo de uso (no ultimo uso), no app por mais de `NEUTRA_MAX_MS` (quando entrou nele) e
    /// a conversa lida que deixou de poder ser lida (agora). Com uma saida esperando confirmacao,
    /// a leitura acaba nela, se foi antes.
    fn fim_imediato(&self, a: &Amostra, onde: &Onde) -> Option<(Ms, MotivoFim)> {
        let id = self.lendo_agora()?;
        let mut fins: Vec<(Ms, MotivoFim)> = vec![];
        if a.ocioso_ms > self.cfg.uso_ms {
            fins.push((a.t - a.ocioso_ms, MotivoFim::SemUso));
        }
        match onde {
            Onde::NeutraVencida(desde) => fins.push((*desde, MotivoFim::SaiuDaJanela)),
            Onde::Neutra => {}
            _ if !self.pode_ser_lida(id) => fins.push((a.t, MotivoFim::OutraSessao)),
            _ => {}
        }
        if !fins.is_empty() {
            fins.extend(self.troca.as_ref().and_then(|tr| tr.saiu));
        }
        // o mais cedo; no empate, o primeiro
        fins.into_iter().reduce(|x, y| if y.0 < x.0 { y } else { x })
    }

    /// A situacao que a amostra mostra: `Some(Some(id))`, a conversa `id` aberta e podendo ser
    /// lida, com uso; `Some(None)`, nenhuma (outro programa, outra conversa, "nao sei", sem uso);
    /// `None`, nada a dizer (janela do app: neutra).
    fn situacao_da_amostra(&self, a: &Amostra, onde: &Onde, aberta: Option<&Aberta>) -> Option<Option<String>> {
        match onde {
            Onde::Neutra | Onde::NeutraVencida(_) => None,
            Onde::Fora { .. } => Some(None),
            Onde::Claude(_) => Some(match aberta {
                Some(Aberta::Sessao(id)) if a.ocioso_ms <= self.cfg.uso_ms && self.pode_ser_lida(id) => Some(id.clone()),
                _ => None,
            }),
        }
    }

    /// Divide a leitura aberta nas viradas em que ela ficou sem dividir, antes de `ate`.
    pub(crate) fn dividir_leitura_adiada(&mut self, ate: Ms) -> Vec<Mudanca> {
        let adiados = match &mut self.troca {
            Some(tr) => std::mem::take(&mut tr.adiados),
            None => return vec![],
        };
        let mut out = vec![];
        let Some(pos) = self.abertos.iter().position(|t| t.origem == Origem::Leitura) else { return out };
        for corte in adiados {
            let t = &mut self.abertos[pos];
            if corte <= t.inicio || corte >= ate {
                continue;
            }
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
            t.inicio = corte;
        }
        out
    }

    /// A leitura aberta fechou fora da troca (pause, mensagem, sem uso...): a saida esperando
    /// confirmacao perde o sentido; a conversa que ia passar a ser lida comeca depois do fechamento
    /// (nada sobreposto).
    pub(crate) fn troca_apos_fechar(&mut self, fim: Ms) {
        if let Some(tr) = &mut self.troca {
            if tr.alvo.is_none() {
                self.troca = None;
            } else {
                tr.saiu = None;
                tr.adiados.clear();
                tr.inicio = tr.inicio.max(fim);
            }
        }
    }

    /// Aplica a troca confirmada: a leitura aberta acaba quando voce saiu dela; a nova comeca quando
    /// apareceu (dividida nas viradas do dia que passaram na espera).
    fn confirmar_troca(&mut self) -> Vec<Mudanca> {
        let Some(tr) = self.troca.take() else { return vec![] };
        let mut out = vec![];
        if let Some((saiu, motivo)) = tr.saiu {
            out.extend(self.fechar_lida(saiu, motivo));
        }
        if let Some(id) = tr.alvo.filter(|id| self.pode_ser_lida(id) && self.lendo_agora().is_none()) {
            let timer_id = self.sessoes.get(&id).and_then(|s| s.timer_id);
            let atividade_id = self.atividade_padrao(timer_id);
            let mut inicio = tr.inicio;
            for corte in tr.cortes.into_iter().filter(|c| *c > tr.inicio) {
                let sessao_id = Some(id.clone());
                out.push(Mudanca::Abriu { timer_id, sessao_id: sessao_id.clone(), origem: Origem::Leitura, inicio, atividade_id });
                out.push(Mudanca::Fechou { fim: corte, timer_id, sessao_id, origem: Origem::Leitura, motivo: MotivoFim::CorteDoDia });
                inicio = corte;
            }
            out.extend(self.corte_do_dia(inicio));
            out.extend(self.abrir_leitura(&id, inicio));
        }
        out
    }

    /// Chamado pelo `amostra`, depois da inatividade e antes do corte do dia (que divide a leitura
    /// que segue). Troca suave: a leitura so muda (abre, fecha ou passa para outra conversa) quando
    /// a situacao nova se mantem por `TROCA_SUAVE_MS`, e entao vale desde o instante em que
    /// aconteceu. Saindo da conversa lida por menos que isso e voltando a ela, nada muda: um trecho
    /// so, sem buraco; voltando depois do prazo (a espera passou so pelo app), a saida vale e
    /// a volta e outra leitura. As janelas do app sao neutras: nao mudam a situacao nem
    /// desfazem a troca, mas a espera anda com o tempo nelas (confirma `TROCA_SUAVE_MS` depois).
    pub(crate) fn leitura_na_amostra(&mut self, a: &Amostra, onde: &Onde) -> Vec<Mudanca> {
        let aberta = self.aberta_em(onde);
        self.conferir_bloqueios(onde, aberta.as_ref());
        // a conversa aberta (para o pause); com "nao sei" ou no app, fica a que era
        match (onde, &aberta) {
            (Onde::Claude(_), Some(Aberta::Sessao(id))) => self.conversa_aberta = Some(id.clone()),
            (Onde::Claude(_), Some(Aberta::Desconhecida)) | (Onde::Fora { .. } | Onde::NeutraVencida(_), _) => {
                self.conversa_aberta = None
            }
            _ => {}
        }
        let mut out = vec![];
        if let Some((fim, motivo)) = self.fim_imediato(a, onde) {
            out.extend(self.fechar_lida(fim, motivo));
        }
        let nova = self.situacao_da_amostra(a, onde, aberta.as_ref());
        if let Some(nova) = &nova {
            let lida = self.lendo_agora().map(str::to_string);
            let saida_vencida = self
                .troca
                .as_ref()
                .filter(|tr| tr.amostras >= 2)
                .and_then(|tr| tr.saiu)
                .filter(|(s, _)| a.t - s >= TROCA_SUAVE_MS);
            if let (true, Some((saiu, motivo)), Some(id)) = (*nova == lida, saida_vencida, lida.clone()) {
                // de volta a conversa lida depois do prazo (fora so pelo app, ou amostras
                // espacadas): a saida vale, no instante dela; a volta e outra leitura, desde agora
                self.troca = None;
                out.extend(self.fechar_lida(saiu, motivo));
                out.extend(self.corte_do_dia(a.t));
                out.extend(self.abrir_leitura(&id, a.t));
            } else if *nova == lida {
                // de volta a conversa lida antes do prazo (ou nada lido e nada para ler): a troca
                // nao vale, um trecho so
                out.extend(self.dividir_leitura_adiada(a.t));
                self.troca = None;
            } else {
                // saindo do app (neutro) para outra coisa, a saida foi quando entrou nele
                let saiu = lida.as_ref().map(|_| {
                    let m = if matches!(onde, Onde::Fora { .. }) { MotivoFim::SaiuDaJanela } else { MotivoFim::OutraSessao };
                    (self.saiu_do_app.unwrap_or(a.t), m)
                });
                match &mut self.troca {
                    Some(tr) if tr.alvo == *nova => {}
                    Some(tr) => {
                        tr.alvo = nova.clone();
                        tr.visto = a.t;
                        tr.inicio = a.t;
                        tr.saiu = tr.saiu.or(saiu);
                    }
                    None => {
                        self.troca = Some(Troca {
                            alvo: nova.clone(),
                            visto: a.t,
                            inicio: a.t,
                            saiu,
                            cortes: vec![],
                            adiados: vec![],
                            amostras: 0,
                        })
                    }
                }
            }
        }
        if let Some(tr) = &mut self.troca {
            tr.amostras += 1;
        }
        // a espera anda com o tempo tambem nas janelas do app (que so nao mostram o contrario);
        // como `visto` e de uma amostra anterior, uma amostra so nunca confirma
        if self.troca.as_ref().is_some_and(|tr| a.t - tr.visto >= TROCA_SUAVE_MS) {
            out.extend(self.confirmar_troca());
        }
        self.etiqueta_na_amostra(a, nova.as_ref());
        out
    }

    /// "escrevendo": o foco na caixa de mensagem da conversa que passa a ser lida, no app do Claude
    /// (so a etiqueta). Muda como a leitura: depois de `TROCA_SUAVE_MS` mostrando o contrario; as
    /// janelas do app nao mudam nada.
    fn etiqueta_na_amostra(&mut self, a: &Amostra, nova: Option<&Option<String>>) {
        if let Some(nova) = nova {
            let escreve = a.escrevendo && matches!(a.conversa, Conversa::App(_) | Conversa::Chat { .. }) && nova.is_some();
            if escreve == self.escrevendo {
                self.escrevendo_desde = None;
            } else if self.escrevendo_desde.is_none() {
                self.escrevendo_desde = Some(a.t);
            }
        }
        if self.escrevendo_desde.is_some_and(|d| a.t - d >= TROCA_SUAVE_MS) {
            self.escrevendo = !self.escrevendo;
            self.escrevendo_desde = None;
        }
        // sem leitura, nada de "escrevendo" sobrando para a proxima
        if self.lendo_agora().is_none() && self.escrevendo {
            self.escrevendo = false;
            self.escrevendo_desde = None;
        }
    }

    /// Instante da saida da conversa lida que espera confirmacao, se ha uma: e onde a leitura
    /// acaba se o app fechar agora (o app grava a cada ciclo; `ao_reabrir`).
    pub fn saida_esperando(&self) -> Option<Ms> {
        self.lendo_agora()?;
        self.troca.as_ref()?.saiu.map(|(s, _)| s)
    }

    /// Voce esta escrevendo na conversa lida agora (a leitura segue aberta).
    pub fn escrevendo_agora(&self) -> bool {
        self.escrevendo && self.lendo_agora().is_some()
    }
}
// IMPL-FIM

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::{ConfigApp, ConfigMotor};
    use crate::modelo::RegraPasta;
    use crate::tempo::ms_local;
    use fila::EventoHook;

    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms { ms_local(2026, 11, 17, hh, mm, F) }
    fn motor() -> Motor {
        let app = ConfigApp {
            programas_claude: ["claude", "code", "windowsterminal"].iter().map(|s| s.to_string()).collect(),
            ..ConfigApp::default()
        };
        let mut m = Motor::novo(ConfigMotor::de(&app, F), h(8, 0));
        m.regras_pasta = vec![
            RegraPasta { id: 1, pasta: "G:\\A".into(), timer_id: 1 },
            RegraPasta { id: 2, pasta: "G:\\B".into(), timer_id: 2 },
        ];
        m
    }
    fn ev(evento: &str, sessao: &str, t: Ms) -> EventoHook {
        let cwd = format!("G:\\{sessao}");
        EventoHook { t, evento: evento.into(), sessao: sessao.into(), cwd, transcript: None, notificacao: None, origem: None, conta: None }
    }
    /// Nome das sessoes de teste (o custom-title do transcript).
    fn nome(s: &str) -> String { format!("Conversa {s}") }
    fn nomear(m: &mut Motor) {
        for s in m.sessoes.values_mut() {
            s.nome = Some(nome(&s.id));
        }
    }
    /// Amostra com o app do Claude em uso e a conversa `conversa` aberta (`None`: "nao sei").
    fn app(t: Ms, ocioso: Ms, conversa: Option<&str>) -> Amostra {
        Amostra {
            t,
            ocioso_ms: ocioso,
            processo: Some("claude".into()),
            titulo: Some("Claude".into()),
            conversa: Conversa::App(conversa.map(nome)),
            em_foco: true,
            ..Default::default()
        }
    }
    fn le(t: Ms, s: &str) -> Amostra { app(t, 1_000, Some(s)) }
    /// A UI Automation nao respondeu (falhou, passou do limite ou nao achou conversa).
    fn nao_sei(t: Ms) -> Amostra { app(t, 1_000, None) }
    /// Uma conversa que o app nao conhece, aberta no app.
    fn desconhecida(t: Ms) -> Amostra {
        Amostra { conversa: Conversa::App(Some("Receita de bolo".into())), ..nao_sei(t) }
    }
    /// O dock ou a janela principal do app em uso.
    fn dock(t: Ms) -> Amostra {
        Amostra { t, ocioso_ms: 1_000, processo: Some("work-timer".into()), titulo: Some("Chronn — dock".into()), neutra: true, em_foco: true, ..Default::default() }
    }
    /// Outro programa em foco.
    fn outro_programa(t: Ms) -> Amostra {
        Amostra { t, ocioso_ms: 1_000, processo: Some("msedge".into()), titulo: Some("Notícias".into()), em_foco: true, ..Default::default() }
    }
    /// Outro programa so debaixo do mouse (o foco esta noutro lugar).
    fn sob_mouse(t: Ms) -> Amostra { Amostra { em_foco: false, ..outro_programa(t) } }
    fn terminal(t: Ms, titulo: &str) -> Amostra {
        Amostra { t, ocioso_ms: 1_000, processo: Some("WindowsTerminal".into()), titulo: Some(titulo.into()), em_foco: true, ..Default::default() }
    }
    fn trechos(v: &[Mudanca]) -> Vec<Mudanca> {
        v.iter().filter(|m| matches!(m, Mudanca::Abriu { .. } | Mudanca::Fechou { .. })).cloned().collect()
    }
    fn abriu_l(timer: i64, s: &str, t: Ms) -> Mudanca {
        Mudanca::Abriu { timer_id: Some(timer), sessao_id: Some(s.into()), origem: Origem::Leitura, inicio: t, atividade_id: 1 }
    }
    fn fechou_l(timer: i64, s: &str, t: Ms, motivo: MotivoFim) -> Mudanca {
        Mudanca::Fechou { timer_id: Some(timer), sessao_id: Some(s.into()), origem: Origem::Leitura, fim: t, motivo }
    }
    /// Amostras de minuto em minuto, como o laco (sem buraco maior que o tempo de uso), de `de` a `ate`.
    fn amostras(m: &mut Motor, de: Ms, ate: Ms, a: impl Fn(Ms) -> Amostra) -> Vec<Mudanca> {
        let mut v = vec![];
        let mut t = de;
        while t <= ate {
            v.extend(trechos(&m.amostra(&a(t))));
            t += MIN;
        }
        v
    }
    /// A situacao da amostra se mantem pela troca suave: a amostra e a mesma `TROCA_SUAVE_MS` depois.
    /// Devolve os trechos das duas.
    fn firme(m: &mut Motor, a: Amostra) -> Vec<Mudanca> {
        let mut v = trechos(&m.amostra(&a));
        v.extend(trechos(&m.amostra(&Amostra { t: a.t + TROCA_SUAVE_MS, ..a })));
        v
    }
    const S5: Ms = 5_000;
    /// B (timer 2) termina 10:00, A (timer 1) termina 10:02: A terminou por ultimo.
    fn dois() -> Motor {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "A", h(9, 55)));
        m.evento(&ev("UserPromptSubmit", "B", h(9, 57)));
        m.evento(&ev("Stop", "B", h(10, 0)));
        m.evento(&ev("Stop", "A", h(10, 2)));
        nomear(&mut m);
        m
    }
    /// Lendo B desde 10:03 (confirmado as 10:03:10).
    fn lendo_b() -> Motor {
        let mut m = dois();
        assert_eq!(firme(&mut m, le(h(10, 3), "B")), vec![abriu_l(2, "B", h(10, 3))]);
        assert_eq!(m.lendo_agora(), Some("B"));
        m
    }
    /// A leitura (e a escrita) usa a atividade padrao do timer da sessao lida (rodada 9), e corre
    /// junto com a reuniao manual do mesmo timer.
    #[test] fn leitura_usa_a_atividade_padrao_do_timer() {
        let mut m = dois();
        m.atividades_padrao.insert(2, crate::modelo::ATIVIDADE_ESTUDO);
        m.play_atividade(2, crate::modelo::ATIVIDADE_REUNIAO, h(10, 2));
        let v = firme(&mut m, le(h(10, 3), "B"));
        assert_eq!(
            v,
            vec![Mudanca::Abriu {
                timer_id: Some(2),
                sessao_id: Some("B".into()),
                origem: Origem::Leitura,
                inicio: h(10, 3),
                atividade_id: crate::modelo::ATIVIDADE_ESTUDO
            }]
        );
        assert_eq!(m.abertos.len(), 2, "a reuniao manual segue junto");
    }

    /// Lendo B, o timer 2 pausado as 10:05 com B aberta.
    fn pausado_lendo_b() -> Motor {
        let mut m = lendo_b();
        m.amostra(&dock(h(10, 4)));
        let v = m.pausar(2, h(10, 5));
        assert_eq!(trechos(&v), vec![fechou_l(2, "B", h(10, 5), MotivoFim::Pausa)]);
        assert!(m.leitura_bloqueada.contains_key("B"));
        m
    }

    #[test] fn leitura_vai_para_a_conversa_aberta_e_nao_para_a_que_terminou_por_ultimo() {
        let mut m = dois();
        // a primeira amostra so comeca a espera: a tela nao muda ainda
        assert_eq!(trechos(&m.amostra(&le(h(10, 3), "B"))), vec![]);
        assert_eq!(m.lendo_agora(), None);
        assert_eq!(m.conversa_aberta.as_deref(), Some("B"), "o pause ja sabe qual conversa esta aberta");
        // 10 s depois vale, desde quando apareceu
        assert_eq!(trechos(&m.amostra(&le(h(10, 3) + TROCA_SUAVE_MS, "B"))), vec![abriu_l(2, "B", h(10, 3))]);
        assert_eq!(m.lendo_agora(), Some("B"));
    }

    // ---- troca suave ----
    #[test] fn troca_de_a_para_b_confirmada_termina_a_e_abre_b_no_instante_da_troca() {
        let mut m = lendo_b();
        let t = h(10, 5);
        // uma amostra em A: a tela segue em B (nem pisca apagada, o timer nao pausa)
        assert_eq!(trechos(&m.amostra(&le(t, "A"))), vec![]);
        assert_eq!(m.lendo_agora(), Some("B"));
        // a segunda, 5 s depois: vale, no instante da troca
        let v = trechos(&m.amostra(&le(t + S5, "A")));
        assert_eq!(v, vec![fechou_l(2, "B", t, MotivoFim::OutraSessao), abriu_l(1, "A", t)]);
        assert_eq!(m.lendo_agora(), Some("A"));
        assert!(m.troca.is_none());
    }
    #[test] fn uma_amostra_diferente_sozinha_nao_troca_nada() {
        let t = h(10, 5);
        let saidas: [fn(Ms) -> Amostra; 5] = [outro_programa, sob_mouse, |t| le(t, "A"), nao_sei, desconhecida];
        for saida in saidas {
            let mut m = lendo_b();
            assert_eq!(trechos(&m.amostra(&saida(t))), vec![]);
            assert_eq!(m.lendo_agora(), Some("B"));
            assert_eq!(trechos(&m.amostra(&le(t + S5, "B"))), vec![]);
            assert!(m.troca.is_none());
            assert_eq!((m.abertos.len(), m.abertos[0].inicio), (1, h(10, 3)), "um trecho so, sem buraco");
        }
        // nem para abrir: uma amostra so numa conversa nao abre a leitura dela
        let mut m = dois();
        assert_eq!(trechos(&m.amostra(&le(t, "B"))), vec![]);
        assert_eq!(trechos(&m.amostra(&outro_programa(t + S5))), vec![]);
        assert_eq!(m.lendo_agora(), None);
        assert!(m.troca.is_none());
    }
    #[test] fn saida_de_3_s_e_volta_a_mesma_conversa_e_um_trecho_so() {
        let t = h(10, 5);
        // outro programa, outra conversa ou "nao sei" por 3 s: nada muda, e os segundos contam
        let saidas: [fn(Ms) -> Amostra; 3] = [outro_programa, |t| le(t, "A"), nao_sei];
        for saida in saidas {
            let mut m = lendo_b();
            assert_eq!(trechos(&m.amostra(&saida(t))), vec![]);
            assert_eq!(trechos(&m.amostra(&le(t + 3_000, "B"))), vec![]);
            assert!(m.troca.is_none());
            assert_eq!(amostras(&mut m, h(10, 6), h(10, 8), |t| le(t, "B")), vec![]);
            assert_eq!(m.abertos.len(), 1);
            assert_eq!(m.abertos[0].inicio, h(10, 3), "um trecho so, sem buraco");
        }
    }
    #[test] fn saida_de_8_s_termina_no_instante_da_saida() {
        let mut m = lendo_b();
        let t = h(10, 5);
        assert_eq!(trechos(&m.amostra(&outro_programa(t))), vec![]);
        assert_eq!(trechos(&m.amostra(&outro_programa(t + S5))), vec![fechou_l(2, "B", t, MotivoFim::SaiuDaJanela)]);
        assert_eq!(m.lendo_agora(), None);
        // de volta aos 8 s: conta de novo desde a volta (quando se mantiver)
        assert_eq!(trechos(&m.amostra(&le(t + 8_000, "B"))), vec![]);
        assert_eq!(trechos(&m.amostra(&le(t + 13_000, "B"))), vec![abriu_l(2, "B", t + 8_000)]);
    }
    #[test] fn passando_por_outra_conversa_no_caminho_a_saida_vale_quando_saiu() {
        // B -> A (uma amostra) -> outro programa: B acaba quando saiu dela; A nao chegou a valer
        let mut m = lendo_b();
        let t = h(10, 5);
        m.amostra(&le(t, "A"));
        assert_eq!(trechos(&m.amostra(&outro_programa(t + S5))), vec![]);
        assert_eq!(trechos(&m.amostra(&outro_programa(t + 2 * S5))), vec![fechou_l(2, "B", t, MotivoFim::OutraSessao)]);
        assert_eq!(m.lendo_agora(), None);
        // B -> A (uma amostra) -> C: C vale desde que apareceu
        let mut m = lendo_b();
        m.sessoes.get_mut("A").unwrap().nome = Some("Outra".into());
        m.evento(&ev("UserPromptSubmit", "C", h(10, 0)));
        m.evento(&ev("Stop", "C", h(10, 1)));
        m.sessoes.get_mut("C").unwrap().nome = Some(nome("C"));
        m.sessoes.get_mut("C").unwrap().timer_id = Some(3);
        let mut a = le(t, "A");
        a.conversa = Conversa::App(Some("Outra".into()));
        m.amostra(&a);
        assert_eq!(trechos(&m.amostra(&le(t + S5, "C"))), vec![]);
        let v = trechos(&m.amostra(&le(t + 2 * S5, "C")));
        assert_eq!(v, vec![fechou_l(2, "B", t, MotivoFim::OutraSessao), abriu_l(3, "C", t + S5)]);
    }
    #[test] fn a_espera_anda_com_o_tempo_nas_janelas_do_app() {
        // B -> A e depois o dock: a troca vale 5 s depois, numa amostra do app
        let mut m = lendo_b();
        let t = h(10, 5);
        m.amostra(&le(t, "A"));
        assert_eq!(trechos(&m.amostra(&dock(t + 3_000))), vec![], "antes do prazo, nada");
        let v = trechos(&m.amostra(&dock(t + S5)));
        assert_eq!(v, vec![fechou_l(2, "B", t, MotivoFim::OutraSessao), abriu_l(1, "A", t)]);
        // fora e uma amostra no app 5 s depois: a saida vale, no instante dela
        let mut m = lendo_b();
        m.amostra(&outro_programa(t));
        assert_eq!(trechos(&m.amostra(&dock(t + S5))), vec![fechou_l(2, "B", t, MotivoFim::SaiuDaJanela)]);
        // o app sozinho (sem amostra de fora antes) segue neutro: nada muda
        let mut m = lendo_b();
        assert_eq!(amostras(&mut m, h(10, 5), h(10, 6), dock), vec![]);
        assert_eq!(m.lendo_agora(), Some("B"));
    }
    /// Caso do revisor (I1): Chrome as 10:00, o dock por 60 s, de volta a conversa as 10:01:05.
    #[test] fn fora_e_o_dock_por_60_s_e_a_volta_e_outra_leitura() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "B", h(9, 50)));
        m.evento(&ev("Stop", "B", h(9, 55)));
        nomear(&mut m);
        firme(&mut m, le(h(9, 58), "B"));
        m.amostra(&le(h(9, 59), "B"));
        let t = h(10, 0);
        assert_eq!(trechos(&m.amostra(&outro_programa(t))), vec![]);
        let mut v = vec![];
        let mut x = t + S5;
        while x <= t + MIN {
            v.extend(trechos(&m.amostra(&dock(x))));
            x += S5;
        }
        assert_eq!(v, vec![fechou_l(2, "B", t, MotivoFim::SaiuDaJanela)], "a leitura vai ate 10:00");
        let volta = t + MIN + S5;
        assert_eq!(firme(&mut m, le(volta, "B")), vec![abriu_l(2, "B", volta)], "a nova, desde 10:01:05");
        // com as amostras espacadas (a espera nao chegou a valer no app), a volta depois do
        // prazo faz o mesmo: a leitura acaba na saida e a volta e outra
        let mut m = lendo_b();
        let t = h(10, 5);
        m.amostra(&outro_programa(t));
        m.amostra(&dock(t + 3_000));
        let v = trechos(&m.amostra(&le(t + 7_000, "B")));
        assert_eq!(v, vec![fechou_l(2, "B", t, MotivoFim::SaiuDaJanela), abriu_l(2, "B", t + 7_000)]);
        // uma amostra fora so, e a volta na seguinte (5 s depois): nada muda
        let mut m = lendo_b();
        m.amostra(&outro_programa(t));
        assert_eq!(trechos(&m.amostra(&le(t + S5, "B"))), vec![]);
        assert_eq!((m.abertos.len(), m.abertos[0].inicio), (1, h(10, 3)));
    }
    #[test] fn abrir_uma_leitura_nova_espera_e_conta_desde_quando_apareceu() {
        let mut m = dois();
        let t = h(10, 5);
        assert_eq!(trechos(&m.amostra(&le(t, "B"))), vec![]);
        // olhou outra coisa antes de valer: nada
        assert_eq!(trechos(&m.amostra(&outro_programa(t + S5))), vec![]);
        assert!(m.troca.is_none());
        assert_eq!(trechos(&m.amostra(&le(t + 2 * S5, "B"))), vec![]);
        assert_eq!(trechos(&m.amostra(&le(t + 3 * S5, "B"))), vec![abriu_l(2, "B", t + 2 * S5)]);
    }
    #[test] fn claude_trabalhando_na_conversa_que_ia_ser_lida_nada_sobreposto() {
        let mut m = lendo_b();
        let t = h(10, 5);
        m.amostra(&le(t, "A"));
        // A trabalha e termina dentro da espera: a leitura de A so comeca depois do trecho do Claude
        m.evento(&ev("PostToolUse", "A", t + 2_000));
        m.evento(&ev("Stop", "A", t + 4_000));
        let v = trechos(&m.amostra(&le(t + S5, "A")));
        assert_eq!(v, vec![fechou_l(2, "B", t, MotivoFim::OutraSessao), abriu_l(1, "A", t + 4_000)]);
        // escrever na conversa lida fecha na hora (o Claude e imediato)
        let mut m = lendo_b();
        m.amostra(&le(t - 2_000, "B"));
        let v = trechos(&m.evento(&ev("UserPromptSubmit", "B", t)));
        assert!(v.contains(&fechou_l(2, "B", t, MotivoFim::NovaMensagem)));
    }
    /// Caso do revisor (I1): pause ou ■ no meio da espera fecham a leitura na saida, nao no clique.
    #[test] fn pause_e_encerrar_no_meio_da_espera_fecham_no_instante_da_saida() {
        let t = h(10, 5);
        let mut m = lendo_b();
        m.amostra(&outro_programa(t));
        m.amostra(&dock(t + 2_000));
        assert_eq!(trechos(&m.pausar(2, t + 3_000)), vec![fechou_l(2, "B", t, MotivoFim::SaiuDaJanela)]);
        assert!(m.troca.is_none());
        let mut m = lendo_b();
        m.amostra(&outro_programa(t));
        assert_eq!(trechos(&m.encerrar(2, t + 4_000)), vec![fechou_l(2, "B", t, MotivoFim::SaiuDaJanela)]);
        let mut m = lendo_b();
        m.amostra(&outro_programa(t));
        assert_eq!(trechos(&m.pausar_tudo(t + 4_000)), vec![fechou_l(2, "B", t, MotivoFim::SaiuDaJanela)]);
        // trocando de conversa: B acaba na saida, e A (outro timer) segue esperando, desde que apareceu
        let mut m = lendo_b();
        m.amostra(&le(t, "A"));
        assert_eq!(trechos(&m.pausar(2, t + 3_000)), vec![fechou_l(2, "B", t, MotivoFim::OutraSessao)]);
        assert_eq!(trechos(&m.amostra(&le(t + S5, "A"))), vec![abriu_l(1, "A", t)]);
        // sem saida esperando, o pause fecha no clique, como sempre
        let mut m = lendo_b();
        assert_eq!(trechos(&m.pausar(2, t)), vec![fechou_l(2, "B", t, MotivoFim::Pausa)]);
    }
    /// Caso do revisor (I1): o app fecha no meio da espera; ao reabrir, a leitura fecha na saida.
    #[test] fn app_fechado_no_meio_da_espera_fecha_a_leitura_na_saida() {
        let t = h(10, 5);
        let mut m = lendo_b();
        assert_eq!(m.saida_esperando(), None);
        m.amostra(&outro_programa(t));
        assert_eq!(m.saida_esperando(), Some(t));
        m.amostra(&dock(t + 3_000));
        // o app grava a saida a cada ciclo; reaberto as 11:00, o ultimo tique foi 10:05:03
        let mut novo = Motor::novo(m.cfg.clone(), h(11, 0));
        let v = trechos(&novo.ao_reabrir(m.abertos.clone(), t + 3_000, Some(t), h(11, 0)));
        assert_eq!(v, vec![fechou_l(2, "B", t, MotivoFim::AppFechado)]);
        // uma saida gravada de antes do trecho (de outra leitura) nao vale: fecha no ultimo tique
        let mut novo = Motor::novo(m.cfg.clone(), h(11, 0));
        let v = trechos(&novo.ao_reabrir(m.abertos.clone(), t + 3_000, Some(h(9, 0)), h(11, 0)));
        assert_eq!(v, vec![fechou_l(2, "B", t + 3_000, MotivoFim::AppFechado)]);
        // de volta a conversa antes do prazo: nada esperando
        m.amostra(&le(t + 4_000, "B"));
        assert_eq!(m.saida_esperando(), None);
    }
    #[test] fn virada_do_dia_no_meio_da_espera() {
        let corte = ms_local(2026, 11, 18, 0, 0, F);
        let lendo_b_tarde = || {
            let mut m = motor();
            m.evento(&ev("UserPromptSubmit", "B", h(23, 50)));
            m.evento(&ev("Stop", "B", h(23, 55)));
            nomear(&mut m);
            firme(&mut m, le(h(23, 58), "B"));
            m.amostra(&le(h(23, 59), "B"));
            m
        };
        let saiu = corte - 2_000;
        // a amostra seguinte, ja no dia novo, confirma a saida: a leitura acaba antes da virada
        let mut m = lendo_b_tarde();
        assert_eq!(trechos(&m.amostra(&outro_programa(saiu))), vec![]);
        assert_eq!(trechos(&m.amostra(&outro_programa(saiu + S5))), vec![fechou_l(2, "B", saiu, MotivoFim::SaiuDaJanela)]);
        assert!(m.abertos.is_empty());
        // a virada chega antes da confirmacao (o tique ou um evento dos hooks): a leitura espera
        // a troca, sem dividir; a saida confirmada acaba antes da virada, sem pedaco no dia novo
        let mut m = lendo_b_tarde();
        m.amostra(&outro_programa(saiu));
        assert_eq!(trechos(&m.tique(corte + 1_000)), vec![], "a virada espera a troca");
        assert_eq!(m.abertos[0].inicio, h(23, 58));
        assert_eq!(trechos(&m.amostra(&outro_programa(saiu + S5))), vec![fechou_l(2, "B", saiu, MotivoFim::SaiuDaJanela)]);
        assert!(m.abertos.is_empty());
        // voltou a tempo: a leitura segue, dividida na virada
        let mut m = lendo_b_tarde();
        m.amostra(&outro_programa(saiu));
        m.tique(corte + 1_000);
        let v = trechos(&m.amostra(&le(saiu + 4_000, "B")));
        assert_eq!(v, vec![fechou_l(2, "B", corte, MotivoFim::CorteDoDia), abriu_l(2, "B", corte)]);
        assert_eq!((m.abertos.len(), m.abertos[0].inicio), (1, corte));
        // pausou no meio da espera, depois da virada: a leitura acaba na saida, antes da virada
        let mut m = lendo_b_tarde();
        m.amostra(&outro_programa(saiu));
        m.tique(corte + 1_000);
        assert_eq!(trechos(&m.pausar(2, corte + 2_000)), vec![fechou_l(2, "B", saiu, MotivoFim::SaiuDaJanela)]);
        // fechou depois da virada por outra coisa (mensagem nela): cada dia com o seu pedaco
        let mut m = lendo_b_tarde();
        m.amostra(&outro_programa(saiu));
        m.tique(corte + 1_000);
        let v = trechos(&m.evento(&ev("UserPromptSubmit", "B", corte + 2_000)));
        assert_eq!(v, vec![
            fechou_l(2, "B", corte, MotivoFim::CorteDoDia),
            abriu_l(2, "B", corte),
            fechou_l(2, "B", corte + 2_000, MotivoFim::NovaMensagem),
            Mudanca::Abriu { timer_id: Some(2), sessao_id: Some("B".into()), origem: Origem::Claude, inicio: corte + 2_000, atividade_id: 1 },
        ]);
        // uma leitura nova que apareceu antes da virada e valeu depois: um pedaco em cada dia
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "B", h(23, 50)));
        m.evento(&ev("Stop", "B", h(23, 55)));
        nomear(&mut m);
        assert_eq!(trechos(&m.amostra(&le(saiu, "B"))), vec![]);
        assert_eq!(trechos(&m.tique(corte + 1_000)), vec![]);
        let v = trechos(&m.amostra(&le(saiu + S5, "B")));
        assert_eq!(v, vec![abriu_l(2, "B", saiu), fechou_l(2, "B", corte, MotivoFim::CorteDoDia), abriu_l(2, "B", corte)]);
        assert_eq!(m.abertos[0].inicio, corte);
        // troca de B para A atravessando a virada: B acaba antes dela, A tem um pedaco em cada dia
        let mut m = lendo_b_tarde();
        m.evento(&ev("UserPromptSubmit", "A", h(23, 50)));
        m.evento(&ev("Stop", "A", h(23, 51)));
        nomear(&mut m);
        m.amostra(&le(saiu, "A"));
        assert_eq!(trechos(&m.tique(corte + 1_000)), vec![]);
        let v = trechos(&m.amostra(&le(saiu + S5, "A")));
        assert_eq!(v, vec![
            fechou_l(2, "B", saiu, MotivoFim::OutraSessao),
            abriu_l(1, "A", saiu),
            fechou_l(1, "A", corte, MotivoFim::CorteDoDia),
            abriu_l(1, "A", corte),
        ]);
    }
    #[test] fn app_fechado_no_meio_da_espera_da_virada_fecha_cada_dia_com_o_seu_pedaco() {
        let corte = ms_local(2026, 11, 18, 0, 0, F);
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "B", h(23, 50)));
        m.evento(&ev("Stop", "B", h(23, 55)));
        nomear(&mut m);
        firme(&mut m, le(h(23, 58), "B"));
        m.amostra(&outro_programa(corte - 2_000));
        m.tique(corte + 1_000);
        // o banco guardou a leitura aberta desde 23:58; o app reabre as 08:00
        let mut novo = Motor::novo(m.cfg.clone(), ms_local(2026, 11, 18, 8, 0, F));
        let v = trechos(&novo.ao_reabrir(m.abertos.clone(), corte + 1_000, None, ms_local(2026, 11, 18, 8, 0, F)));
        assert_eq!(v, vec![
            fechou_l(2, "B", corte, MotivoFim::CorteDoDia),
            abriu_l(2, "B", corte),
            fechou_l(2, "B", corte + 1_000, MotivoFim::AppFechado),
        ]);
    }
    /// Dia inteiro de amostras de 5 s, sorteadas (outra conversa, outro programa, "nao sei", o
    /// app, sem uso, mensagens, pause), passando da meia-noite: nenhum trecho fecha antes de
    /// abrir, nenhum atravessa o dia, as leituras nao se sobrepoem nem ao trecho do Claude da
    /// mesma sessao.
    #[test] fn nada_com_fim_antes_do_inicio_nem_sobreposto() {
        let mut m = dois();
        m.evento(&ev("UserPromptSubmit", "C", h(9, 0)));
        m.evento(&ev("Stop", "C", h(9, 1)));
        m.sessoes.get_mut("C").unwrap().timer_id = Some(1);
        nomear(&mut m);
        let mut semente: u64 = 0x5eed;
        let mut sorteia = |n: u64| {
            semente = semente.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (semente >> 33) % n
        };
        let mut todas: Vec<Mudanca> = vec![];
        let mut t = h(22, 0);
        let fim = ms_local(2026, 11, 18, 2, 0, F);
        let mut atual = 0;
        while t < fim {
            // a situacao dura de 1 a 8 amostras (5 a 40 s)
            if sorteia(4) == 0 {
                atual = sorteia(9);
            }
            let s = ["A", "B", "C"][sorteia(3) as usize];
            match sorteia(60) {
                0 => todas.extend(m.evento(&ev("UserPromptSubmit", s, t - 1_000))),
                1 => todas.extend(m.evento(&ev("Stop", s, t - 500))),
                2 => todas.extend(m.pausar(1 + sorteia(2) as i64, t - 200)),
                _ => {}
            }
            let a = match atual {
                0 | 1 => le(t, "A"),
                2 | 3 => le(t, "B"),
                4 => le(t, "C"),
                5 => outro_programa(t),
                6 => nao_sei(t),
                7 => dock(t),
                _ => app(t, 4 * MIN, Some("A")),
            };
            todas.extend(m.amostra(&a));
            todas.extend(m.tique(t));
            t += S5 + sorteia(2) as Ms * 1_000;
        }
        // monta os trechos fechados, por (timer, sessao, origem)
        let corte_min = m.cfg.corte_min;
        let mut abertos: Vec<(Option<i64>, Option<String>, Origem, Ms)> = vec![];
        let mut fechados: Vec<(Option<String>, Origem, Ms, Ms)> = vec![];
        for x in &todas {
            match x {
                Mudanca::Abriu { timer_id, sessao_id, origem, inicio, .. } => {
                    assert!(
                        !abertos.iter().any(|a| a.0 == *timer_id && a.1 == *sessao_id && a.2 == *origem),
                        "abriu duas vezes: {x:?}"
                    );
                    abertos.push((*timer_id, sessao_id.clone(), *origem, *inicio));
                }
                Mudanca::Fechou { timer_id, sessao_id, origem, fim, .. } => {
                    let pos = abertos
                        .iter()
                        .position(|a| a.0 == *timer_id && a.1 == *sessao_id && a.2 == *origem)
                        .unwrap_or_else(|| panic!("fechou sem abrir: {x:?}"));
                    let (_, s, o, inicio) = abertos.remove(pos);
                    assert!(*fim >= inicio, "fim antes do inicio: {x:?} (inicio {inicio})");
                    let corte = crate::tempo::proximo_corte(inicio, corte_min, F);
                    assert!(*fim <= corte, "atravessou o dia: {x:?} (inicio {inicio})");
                    fechados.push((s, o, inicio, *fim));
                }
                _ => {}
            }
        }
        let leituras: Vec<_> = fechados.iter().filter(|f| f.1 == Origem::Leitura && f.3 > f.2).collect();
        assert!(leituras.len() > 20, "o sorteio precisa ter leituras: {}", leituras.len());
        for (i, a) in leituras.iter().enumerate() {
            for b in &leituras[i + 1..] {
                assert!(a.3 <= b.2 || b.3 <= a.2, "leituras sobrepostas: {a:?} e {b:?}");
            }
            for c in fechados.iter().filter(|c| c.1 == Origem::Claude && c.0 == a.0 && c.3 > c.2) {
                assert!(a.3 <= c.2 || c.3 <= a.2, "leitura sobre o Claude: {a:?} e {c:?}");
            }
        }
    }

    /// Escrevendo na conversa aberta: a mesma leitura (um trecho so, mesma regra), so a etiqueta
    /// muda, e so depois de se manter por `TROCA_SUAVE_MS`.
    #[test] fn escrever_na_conversa_aberta_conta_como_leitura_e_so_muda_a_etiqueta() {
        let escreve = |t: Ms, s: &str| Amostra { escrevendo: true, ..le(t, s) };
        let mut m = dois();
        // escrever ja abre a leitura, como ler
        assert_eq!(firme(&mut m, escreve(h(10, 3), "B")), vec![abriu_l(2, "B", h(10, 3))]);
        assert!(m.escrevendo_agora());
        // sai do campo por uma amostra e volta: a etiqueta nem muda
        m.amostra(&le(h(10, 4), "B"));
        assert!(m.escrevendo_agora());
        m.amostra(&escreve(h(10, 4) + S5, "B"));
        assert!(m.escrevendo_agora());
        // le a resposta por 5 s: "lendo"
        assert_eq!(firme(&mut m, le(h(10, 4) + 2 * S5, "B")), vec![]);
        assert!(!m.escrevendo_agora());
        assert_eq!(firme(&mut m, escreve(h(10, 5), "B")), vec![]);
        assert!(m.escrevendo_agora());
        assert_eq!((m.abertos.len(), m.abertos[0].inicio), (1, h(10, 3)));
        // sem uso por mais que o tempo de uso, fecha como a leitura (digitar ja e uso), na hora
        let mut parado = escreve(h(10, 8), "B");
        parado.ocioso_ms = 3 * MIN + 20_000;
        assert_eq!(trechos(&m.amostra(&parado)), vec![fechou_l(2, "B", h(10, 4) + 40_000, MotivoFim::SemUso)]);
        assert!(!m.escrevendo_agora());
        // escrevendo numa conversa que nao pode ser lida (Claude trabalhando nela): nada
        let mut m = dois();
        m.evento(&ev("UserPromptSubmit", "B", h(10, 3)));
        firme(&mut m, escreve(h(10, 4), "B"));
        assert!(!m.escrevendo_agora());
        // o "escrevendo" de outro programa (terminal, pelo titulo) nao existe: so no app
        let mut m = dois();
        firme(&mut m, Amostra { escrevendo: true, ..terminal(h(10, 3), "Conversa B") });
        assert_eq!(m.lendo_agora(), Some("B"));
        assert!(!m.escrevendo_agora());
    }
    #[test] fn conversa_nao_reconhecida_nao_conta_leitura() {
        let mut m = dois();
        assert_eq!(firme(&mut m, nao_sei(h(10, 3))), vec![]);
        assert_eq!(firme(&mut m, desconhecida(h(10, 4))), vec![]);
        assert_eq!(m.lendo_agora(), None);
    }
    #[test] fn nao_sei_curto_nao_fecha_a_leitura_e_longo_fecha_quando_comecou() {
        let mut m = lendo_b();
        assert_eq!(trechos(&m.amostra(&nao_sei(h(10, 4)))), vec![]);
        assert_eq!(trechos(&m.amostra(&le(h(10, 4) + S5, "B"))), vec![]);
        assert_eq!(m.abertos[0].inicio, h(10, 3), "um trecho so, sem buraco");
        assert_eq!(trechos(&m.amostra(&nao_sei(h(10, 6)))), vec![]);
        let v = m.amostra(&nao_sei(h(10, 6) + S5));
        assert_eq!(trechos(&v), vec![fechou_l(2, "B", h(10, 6), MotivoFim::OutraSessao)]);
        // conversa lida e de nenhuma sessao conhecida: fecha quando apareceu
        let mut m = lendo_b();
        assert_eq!(firme(&mut m, desconhecida(h(10, 4))), vec![fechou_l(2, "B", h(10, 4), MotivoFim::OutraSessao)]);
    }
    #[test] fn titulo_compara_sem_maiusculas_nem_espacos() {
        let mut m = dois();
        let mut a = le(h(10, 3), "B");
        a.conversa = Conversa::App(Some("  CONVERSA b ".into()));
        assert_eq!(firme(&mut m, a), vec![abriu_l(2, "B", h(10, 3))]);
    }
    #[test] fn leitura_de_25_min_com_uso_continuo_conta_inteira() {
        let mut m = lendo_b();
        assert_eq!(amostras(&mut m, h(10, 4), h(10, 28), |t| le(t, "B")), vec![]);
        let v = firme(&mut m, outro_programa(h(10, 28) + S5));
        assert_eq!(v, vec![fechou_l(2, "B", h(10, 28) + S5, MotivoFim::SaiuDaJanela)]);
    }
    #[test] fn tres_min_sem_uso_encerra_no_ultimo_uso() {
        let mut m = lendo_b();
        assert_eq!(amostras(&mut m, h(10, 4), h(10, 9), |t| le(t, "B")), vec![]);
        // o fim por falta de uso nao espera a troca suave (ja passou do prazo dele)
        let v = m.amostra(&app(h(10, 10), 3 * MIN + 1_000, Some("B")));
        assert_eq!(trechos(&v), vec![fechou_l(2, "B", h(10, 10) - 3 * MIN - 1_000, MotivoFim::SemUso)]);
        // voltou a mexer com a conversa aberta: conta de novo a partir de agora
        assert_eq!(firme(&mut m, le(h(10, 11), "B")), vec![abriu_l(2, "B", h(10, 11))]);
    }
    #[test] fn dock_por_menos_de_2_min_e_voltar_segue_sem_buraco() {
        let mut m = lendo_b();
        assert_eq!(amostras(&mut m, h(10, 4), h(10, 6), dock), vec![]);
        assert_eq!(m.lendo_agora(), Some("B"));
        assert_eq!(trechos(&m.amostra(&le(h(10, 7), "B"))), vec![]);
        assert_eq!(m.abertos[0].inicio, h(10, 3));
        // e o prazo recomeca a cada entrada no app
        assert_eq!(amostras(&mut m, h(10, 8), h(10, 10), dock), vec![]);
        assert_eq!(m.lendo_agora(), Some("B"));
    }
    #[test] fn app_por_mais_de_2_min_fecha_quando_entrou() {
        let mut m = lendo_b();
        assert_eq!(amostras(&mut m, h(10, 4), h(10, 6), dock), vec![]);
        let v = m.amostra(&dock(h(10, 7)));
        assert_eq!(trechos(&v), vec![fechou_l(2, "B", h(10, 4), MotivoFim::SaiuDaJanela)]);
        // trabalhando no app, ativo: nada reabre
        assert_eq!(amostras(&mut m, h(10, 8), h(10, 20), dock), vec![]);
        // de volta a conversa: conta de novo a partir de agora
        assert_eq!(firme(&mut m, le(h(10, 21), "B")), vec![abriu_l(2, "B", h(10, 21))]);
    }
    #[test] fn do_app_para_outro_programa_fecha_quando_entrou_no_app() {
        let mut m = lendo_b();
        m.amostra(&le(h(10, 4), "B"));
        assert_eq!(amostras(&mut m, h(10, 5), h(10, 6), dock), vec![]);
        let v = firme(&mut m, outro_programa(h(10, 7)));
        assert_eq!(v, vec![fechou_l(2, "B", h(10, 5), MotivoFim::SaiuDaJanela)]);
        // para outra conversa no Claude (ou "nao sei"), o mesmo
        let mut m = lendo_b();
        m.amostra(&dock(h(10, 4)));
        let v = firme(&mut m, le(h(10, 5), "A"));
        assert_eq!(v, vec![fechou_l(2, "B", h(10, 4), MotivoFim::OutraSessao), abriu_l(1, "A", h(10, 5))]);
        let mut m = lendo_b();
        m.amostra(&dock(h(10, 4)));
        assert_eq!(firme(&mut m, nao_sei(h(10, 5))), vec![fechou_l(2, "B", h(10, 4), MotivoFim::OutraSessao)]);
        // so a mesma conversa segue sem buraco (e o "sair do app" vale so para aquela amostra)
        let mut m = lendo_b();
        m.amostra(&dock(h(10, 4)));
        assert_eq!(trechos(&m.amostra(&le(h(10, 5), "B"))), vec![]);
        assert_eq!(firme(&mut m, outro_programa(h(10, 6))), vec![fechou_l(2, "B", h(10, 6), MotivoFim::SaiuDaJanela)]);
    }
    #[test] fn virada_do_dia_no_app_fecha_quando_entrou_sem_trecho_vazio() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "B", h(23, 50)));
        m.evento(&ev("Stop", "B", h(23, 55)));
        nomear(&mut m);
        firme(&mut m, le(h(23, 56), "B"));
        m.amostra(&le(h(23, 58), "B"));
        m.amostra(&dock(h(23, 59)));
        let corte = ms_local(2026, 11, 18, 0, 0, F);
        let v = m.amostra(&dock(corte + 30_000));
        assert_eq!(trechos(&v), vec![fechou_l(2, "B", h(23, 59), MotivoFim::SaiuDaJanela)]);
        assert!(m.abertos.is_empty());
        // passa dos 2 min no app: nada mais (nenhum trecho de 0 s)
        assert_eq!(trechos(&m.amostra(&dock(corte + 2 * MIN))), vec![]);
        // de volta a conversa: conta a partir de agora, no dia novo
        let t = corte + 3 * MIN;
        assert_eq!(firme(&mut m, le(t, "B")), vec![abriu_l(2, "B", t)]);
    }
    #[test] fn sem_uso_no_dock_encerra_no_ultimo_uso() {
        let mut m = lendo_b();
        amostras(&mut m, h(10, 4), h(10, 6), |t| le(t, "B"));
        m.amostra(&dock(h(10, 7)));
        let mut a = dock(h(10, 8));
        a.ocioso_ms = 3 * MIN + 30_000;
        assert_eq!(trechos(&m.amostra(&a)), vec![fechou_l(2, "B", h(10, 4) + 30_000, MotivoFim::SemUso)]);
    }
    #[test] fn dock_nao_abre_leitura() {
        let mut m = dois();
        assert_eq!(firme(&mut m, dock(h(10, 3))), vec![]);
        // a leitura que fechou por falta de uso nao reabre mexendo no dock
        let mut m = lendo_b();
        m.amostra(&app(h(10, 6), 2 * MIN + 30_000, Some("B")));
        m.amostra(&app(h(10, 8), 4 * MIN, Some("B")));
        assert_eq!(m.lendo_agora(), None);
        assert_eq!(firme(&mut m, dock(h(10, 9))), vec![]);
    }
    #[test] fn outro_programa_encerra_mesmo_so_debaixo_do_mouse() {
        let mut m = lendo_b();
        m.amostra(&le(h(10, 4), "B"));
        assert_eq!(firme(&mut m, sob_mouse(h(10, 5))), vec![fechou_l(2, "B", h(10, 5), MotivoFim::SaiuDaJanela)]);
        assert_eq!(firme(&mut m, dock(h(10, 6))), vec![]);
        // vindo do dock, fecha quando entrou nele
        let mut m = lendo_b();
        m.amostra(&dock(h(10, 4)));
        assert_eq!(firme(&mut m, sob_mouse(h(10, 5))), vec![fechou_l(2, "B", h(10, 4), MotivoFim::SaiuDaJanela)]);
    }
    #[test] fn trocar_de_conversa_troca_a_leitura() {
        let mut m = lendo_b();
        let v = firme(&mut m, le(h(10, 6), "A"));
        assert_eq!(v, vec![fechou_l(2, "B", h(10, 6), MotivoFim::OutraSessao), abriu_l(1, "A", h(10, 6))]);
    }
    #[test] fn pausar_durante_a_leitura_so_religa_saindo_e_voltando() {
        let mut m = pausado_lendo_b();
        // B segue aberta (no dock e de volta no Claude): nao religa
        assert_eq!(amostras(&mut m, h(10, 6), h(10, 7), dock), vec![]);
        assert_eq!(amostras(&mut m, h(10, 8), h(10, 12), |t| le(t, "B")), vec![]);
        // sai para outra conversa por duas amostras e volta
        assert_eq!(trechos(&m.amostra(&desconhecida(h(10, 13)))), vec![]);
        assert!(m.leitura_bloqueada.contains_key("B"), "uma amostra so nao libera");
        assert_eq!(trechos(&m.amostra(&desconhecida(h(10, 14)))), vec![]);
        assert!(m.leitura_bloqueada.is_empty());
        assert_eq!(firme(&mut m, le(h(10, 15), "B")), vec![abriu_l(2, "B", h(10, 15))]);
    }
    #[test] fn pausar_e_passar_o_mouse_por_outra_janela_continua_bloqueado() {
        let mut m = pausado_lendo_b();
        assert_eq!(amostras(&mut m, h(10, 6), h(10, 9), sob_mouse), vec![]);
        assert!(m.leitura_bloqueada.contains_key("B"));
        assert_eq!(firme(&mut m, le(h(10, 10), "B")), vec![]);
    }
    #[test] fn pausar_e_a_consulta_falhar_continua_bloqueado() {
        let mut m = pausado_lendo_b();
        assert_eq!(amostras(&mut m, h(10, 6), h(10, 9), nao_sei), vec![]);
        assert!(m.leitura_bloqueada.contains_key("B"));
        assert_eq!(firme(&mut m, le(h(10, 10), "B")), vec![]);
        // "nao sei" no meio zera a conta da saida
        m.amostra(&outro_programa(h(10, 11)));
        m.amostra(&nao_sei(h(10, 12)));
        m.amostra(&outro_programa(h(10, 13)));
        assert!(m.leitura_bloqueada.contains_key("B"));
    }
    #[test] fn pausar_e_outro_programa_em_foco_por_duas_amostras_libera() {
        let mut m = pausado_lendo_b();
        m.amostra(&outro_programa(h(10, 6)));
        assert!(m.leitura_bloqueada.contains_key("B"));
        // o app no meio nao conta nem zera
        m.amostra(&dock(h(10, 7)));
        m.amostra(&outro_programa(h(10, 8)));
        assert!(m.leitura_bloqueada.is_empty());
        assert_eq!(firme(&mut m, le(h(10, 9), "B")), vec![abriu_l(2, "B", h(10, 9))]);
    }
    #[test] fn pausar_com_nao_sei_na_ultima_amostra_bloqueia_a_que_esta_sendo_lida() {
        let mut m = lendo_b();
        m.amostra(&nao_sei(h(10, 4)));
        assert_eq!(m.lendo_agora(), Some("B"));
        m.pausar(2, h(10, 5));
        assert!(m.leitura_bloqueada.contains_key("B"));
        assert!(m.troca.is_none(), "a saida esperando confirmacao acabou com o pause");
    }
    #[test] fn pausar_e_escrever_na_conversa_libera_a_leitura() {
        let mut m = pausado_lendo_b();
        m.evento(&ev("UserPromptSubmit", "B", h(10, 6)));
        m.evento(&ev("Stop", "B", h(10, 8)));
        assert_eq!(firme(&mut m, le(h(10, 9), "B")), vec![abriu_l(2, "B", h(10, 9))]);
    }
    #[test] fn pausar_outro_timer_nao_bloqueia_a_conversa_aberta() {
        let mut m = lendo_b();
        m.pausar(1, h(10, 5));
        assert!(m.leitura_bloqueada.is_empty());
        assert_eq!(m.lendo_agora(), Some("B"));
    }
    #[test] fn sessao_parada_ha_horas_aberta_conta_leitura() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "B", h(9, 0)));
        m.evento(&ev("Stop", "B", h(9, 5)));
        m.evento(&ev("SessionEnd", "B", h(9, 6)));
        nomear(&mut m);
        assert_eq!(m.sessoes["B"].estado, EstadoSessao::Parada);
        assert_eq!(firme(&mut m, le(h(15, 0), "B")), vec![abriu_l(2, "B", h(15, 0))]);
        // esperando voce desde cedo tambem conta
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "A", h(8, 0)));
        m.evento(&ev("Stop", "A", h(8, 5)));
        nomear(&mut m);
        assert_eq!(firme(&mut m, le(h(16, 0), "A")), vec![abriu_l(1, "A", h(16, 0))]);
    }
    #[test] fn sessao_trabalhando_aberta_nao_conta_leitura() {
        let mut m = dois();
        m.evento(&ev("UserPromptSubmit", "B", h(10, 3)));
        assert_eq!(firme(&mut m, le(h(10, 4), "B")), vec![]);
        assert_eq!(m.lendo_agora(), None);
    }
    #[test] fn responder_outra_sessao_nao_encerra_leitura() {
        let mut m = lendo_b();
        let v = m.evento(&ev("UserPromptSubmit", "A", h(10, 4)));
        assert!(!v.iter().any(|x| matches!(x, Mudanca::Fechou { origem: Origem::Leitura, .. })));
        assert_eq!(m.lendo_agora(), Some("B"));
    }
    #[test] fn escrever_na_sessao_lida_encerra_leitura() {
        let mut m = lendo_b();
        let v = m.evento(&ev("UserPromptSubmit", "B", h(10, 15)));
        assert!(trechos(&v).contains(&fechou_l(2, "B", h(10, 15), MotivoFim::NovaMensagem)));
        assert_eq!(m.lendo_agora(), None);
        // trabalhando: a conversa segue aberta, mas nao conta leitura
        assert_eq!(firme(&mut m, le(h(10, 16), "B")), vec![]);
    }
    #[test] fn posttooluse_na_sessao_lida_fecha_leitura() {
        let mut m = lendo_b();
        let v = trechos(&m.evento(&ev("PostToolUse", "B", h(10, 8))));
        let claude_b = Mudanca::Abriu { timer_id: Some(2), sessao_id: Some("B".into()), origem: Origem::Claude, inicio: h(10, 8), atividade_id: 1 };
        assert_eq!(v, vec![fechou_l(2, "B", h(10, 8), MotivoFim::NovaMensagem), claude_b]);
        assert_eq!(m.lendo_agora(), None);
    }
    #[test] fn terminal_reconhece_pelo_titulo_da_janela() {
        let mut m = dois();
        let v = firme(&mut m, terminal(h(10, 3), "✳ conversa b — PowerShell"));
        assert_eq!(v, vec![abriu_l(2, "B", h(10, 3))]);
        // titulo sem o nome de nenhuma sessao: nada
        let mut m = dois();
        assert_eq!(firme(&mut m, terminal(h(10, 3), "PowerShell")), vec![]);
        // o app do Claude nao usa o titulo da janela ("Claude")
        let mut m = dois();
        m.sessoes.get_mut("B").unwrap().nome = Some("Claude".into());
        assert_eq!(firme(&mut m, nao_sei(h(10, 3))), vec![]);
    }
    #[test] fn terminal_ignora_nome_curto_e_prefere_o_mais_longo() {
        let mut m = dois();
        m.sessoes.get_mut("A").unwrap().nome = Some("Flow".into());
        m.sessoes.get_mut("B").unwrap().nome = Some("Ajustes".into());
        assert_eq!(firme(&mut m, terminal(h(10, 3), "Flow")), vec![]);
        m.sessoes.get_mut("A").unwrap().nome = Some("Ajustes do dock".into());
        assert_eq!(firme(&mut m, terminal(h(10, 4), "Ajustes do dock - VS Code")), vec![abriu_l(1, "A", h(10, 4))]);
    }
    #[test] fn nome_repetido_em_timers_diferentes_nao_conta() {
        let mut m = dois();
        m.sessoes.get_mut("A").unwrap().nome = Some(nome("B"));
        assert_eq!(m.sessao_aberta(&JanelaEmUso::de(&le(h(10, 3), "B"))), Aberta::NaoSei);
        assert_eq!(firme(&mut m, le(h(10, 3), "B")), vec![]);
        assert_eq!(firme(&mut m, terminal(h(10, 4), "Conversa B")), vec![]);
        // no mesmo timer tanto faz: fica com a de evento mais recente (A, 10:02)
        m.sessoes.get_mut("A").unwrap().timer_id = Some(2);
        assert_eq!(firme(&mut m, le(h(10, 5), "B")), vec![abriu_l(2, "A", h(10, 5))]);
    }
    #[test] fn programa_do_claude_compara_sem_maiusculas_nem_acento() {
        let mut m = dois();
        m.cfg.programas_claude = vec!["Edição".into()];
        let mut a = le(h(10, 3), "B");
        a.processo = Some("EDIÇÃO".into());
        assert_eq!(firme(&mut m, a), vec![abriu_l(2, "B", h(10, 3))]);
    }
    #[test] fn sessao_sem_timer_nao_e_lida() {
        let mut m = motor();
        let mut e = ev("UserPromptSubmit", "X", h(9, 0));
        e.cwd = "D:\\sem-regra".into();
        m.evento(&e);
        e.evento = "Stop".into();
        e.t = h(9, 5);
        m.evento(&e);
        nomear(&mut m);
        assert_eq!(firme(&mut m, le(h(9, 6), "X")), vec![]);
        assert_eq!(m.lendo_agora(), None);
    }
    // I2: o PC dormiu no meio da leitura.
    #[test] fn sono_do_pc_fecha_a_leitura_na_amostra_anterior() {
        let mut m = lendo_b();
        amostras(&mut m, h(10, 4), h(10, 6), |t| le(t, "B"));
        let v = firme(&mut m, le(h(13, 0), "B"));
        // o buraco nao conta; de volta com a conversa aberta, conta a partir de agora
        assert_eq!(v, vec![fechou_l(2, "B", h(10, 6), MotivoFim::SemUso), abriu_l(2, "B", h(13, 0))]);
        // dormiu no meio de uma troca: ela nao atravessa o buraco
        let mut m = lendo_b();
        m.amostra(&outro_programa(h(10, 4)));
        let v = trechos(&m.amostra(&le(h(13, 0), "B")));
        assert_eq!(v, vec![fechou_l(2, "B", h(10, 4), MotivoFim::SemUso)]);
    }
    #[test] fn leitura_que_abre_depois_da_virada_abre_no_dia_novo() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "B", h(23, 50)));
        m.evento(&ev("Stop", "B", h(23, 55)));
        nomear(&mut m);
        let t = ms_local(2026, 11, 18, 0, 2, F);
        assert_eq!(firme(&mut m, le(t, "B")), vec![abriu_l(2, "B", t)]);
        assert_eq!(m.abertos[0].inicio, t);
    }
    #[test] fn leitura_em_curso_e_dividida_na_virada() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "B", h(23, 50)));
        m.evento(&ev("Stop", "B", h(23, 55)));
        nomear(&mut m);
        firme(&mut m, le(h(23, 58), "B"));
        let corte = ms_local(2026, 11, 18, 0, 0, F);
        let v = m.amostra(&le(corte + MIN, "B"));
        assert_eq!(trechos(&v), vec![fechou_l(2, "B", corte, MotivoFim::CorteDoDia), abriu_l(2, "B", corte)]);
    }
    #[test] fn leitura_que_acabou_antes_da_virada_nao_e_esticada_ate_o_corte() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "B", h(23, 50)));
        m.evento(&ev("Stop", "B", h(23, 55)));
        nomear(&mut m);
        firme(&mut m, le(h(23, 58), "B"));
        m.amostra(&le(h(23, 59), "B"));
        // parou de mexer as 23:58:10; a amostra seguinte ja e do dia novo
        let t = ms_local(2026, 11, 18, 0, 1, F) + 30_000;
        let v = m.amostra(&app(t, 140_000 + 60_000, Some("B")));
        assert_eq!(trechos(&v), vec![fechou_l(2, "B", h(23, 58) + 10_000, MotivoFim::SemUso)]);
        assert!(m.abertos.is_empty());
    }
    #[test] fn nao_sei_antes_da_virada_fecha_no_dia_dele() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "B", h(23, 50)));
        m.evento(&ev("Stop", "B", h(23, 55)));
        nomear(&mut m);
        firme(&mut m, le(h(23, 58), "B"));
        m.amostra(&nao_sei(h(23, 59)));
        let v = m.amostra(&nao_sei(ms_local(2026, 11, 18, 0, 0, F) + 30_000));
        assert_eq!(trechos(&v), vec![fechou_l(2, "B", h(23, 59), MotivoFim::OutraSessao)]);
        assert!(m.abertos.is_empty());
    }
    /// 0.6.1, o dock que trocava um timer pelo outro: A e B esperam você desde 10:00 e 10:02;
    /// você lê B das 10:40 às 10:50 e passa para A. B não é mais lida, mas o tempo do lembrete
    /// dela conta da sua última leitura (10:50), não do fim do turno (10:00): as duas ficam no dock.
    #[test] fn o_lembrete_conta_da_ultima_leitura_e_nao_so_do_fim_do_turno() {
        let mut m = dois();
        firme(&mut m, le(h(10, 40), "B"));
        assert_eq!(m.espera_ultima(&m.sessoes["B"], h(10, 45)), Some(h(10, 45)), "lida agora");
        for k in 1..=110 {
            m.amostra(&le(h(10, 40) + TROCA_SUAVE_MS + k * S5, "B"));
        }
        firme(&mut m, le(h(10, 50), "A"));
        assert_eq!(m.lendo_agora(), Some("A"));
        // a amostra das 10:50 ainda via B (a troca suave confirma depois): a saída foi ali
        assert_eq!(m.espera_ultima(&m.sessoes["B"], h(10, 51)), Some(h(10, 50)));
        assert_eq!(m.sessoes["B"].terminou_em, Some(h(10, 0)), "o fim do turno não muda");
        // a A, nunca lida antes, conta do fim do turno dela quando você sai dela
        assert_eq!(m.espera_ultima(&m.sessoes["A"], h(10, 51)), Some(h(10, 51)), "lida agora");
        // um turno novo de B depois da leitura: vale o fim dele
        m.evento(&ev("UserPromptSubmit", "B", h(11, 0)));
        m.evento(&ev("Stop", "B", h(11, 5)));
        assert_eq!(m.espera_ultima(&m.sessoes["B"], h(11, 6)), Some(h(11, 5)));
    }
    #[test] fn espera_recente_vale_por_uma_hora_ou_enquanto_lida() {
        let m = lendo_b();
        assert!(m.espera_recente(&m.sessoes["A"], h(11, 1)));
        assert!(!m.espera_recente(&m.sessoes["A"], h(11, 2)));
        assert!(m.espera_recente(&m.sessoes["B"], h(15, 0)), "sendo lida");
    }

    // ---- encerrar (■) ----
    fn encerrados(v: &[Mudanca]) -> Vec<Mudanca> {
        v.iter().filter(|m| matches!(m, Mudanca::Encerrados { .. })).cloned().collect()
    }
    fn gravou(timers: &[i64]) -> Mudanca {
        Mudanca::Encerrados { dia: "2026-11-17".into(), timers: timers.to_vec() }
    }
    /// B (timer 2) esperando voce desde 10:00, encerrado as 10:03 sem estar sendo lida.
    fn encerrado_b() -> Motor {
        let mut m = dois();
        assert!(m.espera_recente(&m.sessoes["B"], h(10, 3)));
        let v = m.encerrar(2, h(10, 3));
        assert_eq!(encerrados(&v), vec![gravou(&[2])]);
        assert!(m.encerrado(Some(2)));
        m
    }
    #[test] fn encerrar_tira_o_lembrete_e_a_leitura_nao_religa_nem_abrindo_a_conversa() {
        let mut m = encerrado_b();
        assert!(!m.espera_recente(&m.sessoes["B"], h(10, 4)), "sem o lembrete");
        assert!(m.espera_recente(&m.sessoes["A"], h(10, 4)), "o outro timer segue igual");
        assert_eq!(amostras(&mut m, h(10, 4), h(10, 8), |t| le(t, "B")), vec![]);
        // sair e voltar (o que libera o pause) tambem nao religa
        assert_eq!(amostras(&mut m, h(10, 9), h(10, 11), outro_programa), vec![]);
        assert_eq!(amostras(&mut m, h(10, 12), h(10, 14), |t| le(t, "B")), vec![]);
        // a conversa do outro timer conta normalmente
        assert_eq!(firme(&mut m, le(h(10, 15), "A")), vec![abriu_l(1, "A", h(10, 15))]);
        // encerrar de novo nao grava nada a mais
        assert_eq!(encerrados(&m.encerrar(2, h(10, 16))), vec![]);
    }
    #[test] fn encerrar_durante_a_leitura_fecha_e_nao_religa_saindo_e_voltando() {
        let mut m = lendo_b();
        let v = m.encerrar(2, h(10, 5));
        assert_eq!(trechos(&v), vec![fechou_l(2, "B", h(10, 5), MotivoFim::Pausa)]);
        m.amostra(&outro_programa(h(10, 6)));
        m.amostra(&outro_programa(h(10, 7)));
        assert!(m.leitura_bloqueada.is_empty(), "o pause ja teria liberado");
        assert_eq!(amostras(&mut m, h(10, 8), h(10, 12), |t| le(t, "B")), vec![]);
    }
    #[test] fn encerrado_a_sessao_nao_religa_sem_mensagem_nova() {
        let mut m = encerrado_b();
        // PostToolUse sem prompt (permissao respondida) nao religa, mesmo sem a trava do pause
        // (que nao sobrevive a reabrir o app)
        m.bloqueadas.clear();
        assert_eq!(trechos(&m.evento(&ev("PostToolUse", "B", h(10, 10)))), vec![]);
        assert!(m.encerrado(Some(2)));
    }
    #[test] fn mensagem_numa_sessao_do_timer_volta_tudo_ao_normal() {
        let mut m = encerrado_b();
        let v = m.evento(&ev("UserPromptSubmit", "B", h(10, 10)));
        assert_eq!(encerrados(&v), vec![gravou(&[])]);
        assert!(trechos(&v).contains(&Mudanca::Abriu { timer_id: Some(2), sessao_id: Some("B".into()), origem: Origem::Claude, inicio: h(10, 10), atividade_id: 1 }));
        assert!(!m.encerrado(Some(2)));
        m.evento(&ev("Stop", "B", h(10, 12)));
        assert!(m.espera_recente(&m.sessoes["B"], h(10, 13)), "o lembrete volta");
        assert_eq!(firme(&mut m, le(h(10, 13), "B")), vec![abriu_l(2, "B", h(10, 13))]);
    }
    #[test] fn mensagem_em_sessao_de_outro_timer_nao_desfaz_o_encerrar() {
        let mut m = encerrado_b();
        assert_eq!(encerrados(&m.evento(&ev("UserPromptSubmit", "A", h(10, 10)))), vec![]);
        assert!(m.encerrado(Some(2)));
    }
    #[test] fn play_volta_ao_normal() {
        let mut m = encerrado_b();
        let v = m.play(2, h(10, 10));
        assert_eq!(encerrados(&v), vec![gravou(&[])]);
        assert!(!m.encerrado(Some(2)));
        assert!(m.espera_recente(&m.sessoes["B"], h(10, 11)), "o lembrete volta (ainda dentro de 1 h)");
        // pausar depois do play e so pausar: abrir a conversa conta leitura
        m.pausar(2, h(10, 12));
        assert_eq!(firme(&mut m, le(h(10, 13), "B")), vec![abriu_l(2, "B", h(10, 13))]);
    }
    #[test] fn reabrir_o_app_no_mesmo_dia_continua_encerrado() {
        let antes = encerrado_b();
        let Mudanca::Encerrados { dia, timers } = antes.mudanca_encerrados() else { unreachable!() };
        // app reaberto: motor novo com as sessoes do banco e o que foi gravado
        let mut m = motor();
        m.sessoes = antes.sessoes.clone();
        m.restaurar_encerrados(&dia, &timers);
        assert!(m.encerrado(Some(2)));
        assert!(!m.espera_recente(&m.sessoes["B"], h(10, 20)));
        assert_eq!(amostras(&mut m, h(10, 20), h(10, 24), |t| le(t, "B")), vec![]);
        assert_eq!(trechos(&m.evento(&ev("PostToolUse", "B", h(10, 25)))), vec![]);
        // gravado noutro dia: nao vale
        let mut m = motor();
        m.restaurar_encerrados("2026-11-16", &[2]);
        assert!(m.encerrados.is_empty());
    }
    #[test] fn virada_do_dia_limpa_o_encerrar() {
        let mut m = motor();
        m.evento(&ev("UserPromptSubmit", "B", h(23, 40)));
        m.evento(&ev("Stop", "B", h(23, 45)));
        nomear(&mut m);
        m.encerrar(2, h(23, 50));
        assert_eq!(trechos(&m.amostra(&le(h(23, 55), "B"))), vec![]);
        let t = ms_local(2026, 11, 18, 0, 1, F);
        let v = m.tique(t);
        assert_eq!(encerrados(&v), vec![Mudanca::Encerrados { dia: "2026-11-18".into(), timers: vec![] }]);
        assert!(m.encerrados.is_empty());
        // B terminou ha menos de 1 h: o lembrete volta no dia novo
        assert!(m.espera_recente(&m.sessoes["B"], t));
        // e o que estava gravado no dia anterior nao volta ao reabrir o app no dia novo
        let mut reaberto = Motor::novo(m.cfg.clone(), t);
        reaberto.restaurar_encerrados("2026-11-17", &[2]);
        assert!(reaberto.encerrados.is_empty());
    }
    fn manual(timer: i64, t: Ms) -> Mudanca {
        Mudanca::Abriu { timer_id: Some(timer), sessao_id: None, origem: Origem::Manual, inicio: t, atividade_id: 1 }
    }
    #[test] fn desfazer_encerrar_reabre_o_manual_sem_buraco_e_a_leitura_volta_na_proxima_amostra() {
        let mut m = lendo_b();
        m.play(2, h(10, 4));
        let v = m.encerrar(2, h(10, 5));
        assert_eq!(trechos(&v), vec![
            fechou_l(2, "B", h(10, 5), MotivoFim::Pausa),
            Mudanca::Fechou { timer_id: Some(2), sessao_id: None, origem: Origem::Manual, fim: h(10, 5), motivo: MotivoFim::Pausa },
        ]);
        let v = m.desfazer_encerrar(2, h(10, 5) + 4_000);
        // o manual reabre no instante do ■ (sem buraco); a leitura nao reabre aqui
        assert_eq!(trechos(&v), vec![manual(2, h(10, 5))]);
        assert_eq!(encerrados(&v), vec![gravou(&[])]);
        assert!(!m.encerrado(Some(2)));
        assert!(m.leitura_bloqueada.is_empty() && m.bloqueadas.is_empty(), "as travas voltam a como estavam");
        assert!(m.espera_recente(&m.sessoes["B"], h(10, 6)), "o lembrete volta");
        assert_eq!(firme(&mut m, le(h(10, 6), "B")), vec![abriu_l(2, "B", h(10, 6))]);
        // desfazer de novo nao faz nada
        assert_eq!(m.desfazer_encerrar(2, h(10, 6)), vec![]);
    }
    #[test] fn desfazer_encerrar_reabre_o_claude_que_ainda_trabalha() {
        let mut m = dois();
        m.evento(&ev("UserPromptSubmit", "B", h(10, 10)));
        let claude = |t| Mudanca::Abriu { timer_id: Some(2), sessao_id: Some("B".into()), origem: Origem::Claude, inicio: t, atividade_id: 1 };
        m.encerrar(2, h(10, 12));
        assert_eq!(trechos(&m.desfazer_encerrar(2, h(10, 12) + 2_000)), vec![claude(h(10, 12))]);
        assert!(!m.bloqueadas.contains_key("B"));
        // se o Claude terminou entre o ■ e o desfazer, nada reabre
        let mut m = dois();
        m.evento(&ev("UserPromptSubmit", "B", h(10, 10)));
        m.encerrar(2, h(10, 12));
        m.evento(&ev("Stop", "B", h(10, 12) + 1_000));
        assert_eq!(trechos(&m.desfazer_encerrar(2, h(10, 12) + 2_000)), vec![]);
        assert!(!m.encerrado(Some(2)));
    }
    #[test] fn desfazer_encerrar_vencido_ou_ja_desfeito_nao_faz_nada() {
        let mut m = encerrado_b();
        assert_eq!(m.desfazer_encerrar(2, h(10, 3) + crate::motor::DESFAZER_ENCERRAR_MS + 1), vec![]);
        assert!(m.encerrado(Some(2)));
        let mut m = encerrado_b();
        m.play(2, h(10, 3) + 1_000);
        assert_eq!(m.desfazer_encerrar(2, h(10, 3) + 2_000), vec![]);
        assert_eq!(m.abertos.len(), 1, "so o play");
        // virada do dia entre o ■ e o desfazer
        let mut m = motor();
        m.play(2, h(23, 59));
        m.encerrar(2, h(23, 59) + 30_000);
        let t = ms_local(2026, 11, 18, 0, 0, F) + 10_000;
        assert_eq!(trechos(&m.desfazer_encerrar(2, t)), vec![]);
        assert!(m.abertos.is_empty());
    }
    // M1: a fila lida depois do clique traz uma mensagem de antes dele.
    #[test] fn mensagem_de_antes_do_encerrar_lida_depois_nao_desfaz() {
        let mut m = encerrado_b();
        let v = m.evento(&ev("UserPromptSubmit", "B", h(10, 3) - 3_000));
        assert_eq!(trechos(&v), vec![]);
        assert_eq!(encerrados(&v), vec![]);
        assert!(m.encerrado(Some(2)));
        assert_eq!(m.sessoes["B"].estado, EstadoSessao::Trabalhando);
        // a mensagem seguinte, depois do clique, volta ao normal
        let v = m.evento(&ev("UserPromptSubmit", "B", h(10, 8)));
        assert_eq!(encerrados(&v), vec![gravou(&[])]);
        assert!(trechos(&v).contains(&Mudanca::Abriu { timer_id: Some(2), sessao_id: Some("B".into()), origem: Origem::Claude, inicio: h(10, 8), atividade_id: 1 }));
    }
    #[test] fn mensagem_de_antes_do_pause_lida_depois_nao_religa() {
        let mut m = dois();
        m.play(2, h(10, 4));
        m.pausar(2, h(10, 5));
        assert_eq!(trechos(&m.evento(&ev("UserPromptSubmit", "B", h(10, 5) - 2_000))), vec![]);
        assert!(m.bloqueadas.contains_key("B"));
        assert_eq!(trechos(&m.evento(&ev("PostToolUse", "B", h(10, 6)))), vec![]);
        assert_eq!(
            trechos(&m.evento(&ev("UserPromptSubmit", "B", h(10, 7)))),
            vec![Mudanca::Abriu { timer_id: Some(2), sessao_id: Some("B".into()), origem: Origem::Claude, inicio: h(10, 7), atividade_id: 1 }]
        );
    }
}
