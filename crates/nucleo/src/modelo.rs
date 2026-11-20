// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::tempo::Ms;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Origem {
    Manual,
    Claude,
    Leitura,
    Janela,
    Reuniao,
}

impl Origem {
    pub fn as_str(&self) -> &'static str {
        match self {
            Origem::Manual => "manual",
            Origem::Claude => "claude",
            Origem::Leitura => "leitura",
            Origem::Janela => "janela",
            Origem::Reuniao => "reuniao",
        }
    }

    pub fn de_str(s: &str) -> Option<Origem> {
        match s {
            "manual" => Some(Origem::Manual),
            "claude" => Some(Origem::Claude),
            "leitura" => Some(Origem::Leitura),
            "janela" => Some(Origem::Janela),
            "reuniao" => Some(Origem::Reuniao),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MotivoFim {
    Pausa,
    Inatividade,
    Terminou,
    SessaoParou,
    NovaMensagem,
    OutraSessao,
    SemUso,
    SaiuDaJanela,
    TetoLeitura,
    CorteDoDia,
    AppFechado,
    Desligada,
    /// O play manual trocou de atividade (▶ ↔ 👥): o trecho fecha e o da outra atividade abre no
    /// mesmo instante. No banco vai como `pausa` (`no_banco`): uma versão anterior do app não
    /// conhece o texto novo e não leria o trecho. A troca continua visível no banco: o manual
    /// seguinte do mesmo timer, noutra atividade, começa no mesmo instante.
    TrocouAtividade,
    /// Você apertou o stop na conversa (rodada 13): o Claude Code não chama o hook `Stop` nessa
    /// hora, e o app acha a interrupção no fim do transcript (`[Request interrupted by user…]`).
    /// O trecho do Claude fecha no instante dela.
    Interrompida,
    /// Um motivo gravado por uma versão mais nova do app, que esta não conhece (só na leitura:
    /// o trecho segue valendo, com o motivo genérico).
    Outro,
}

impl MotivoFim {
    pub fn as_str(&self) -> &'static str {
        match self {
            MotivoFim::Pausa => "pausa",
            MotivoFim::Inatividade => "inatividade",
            MotivoFim::Terminou => "terminou",
            MotivoFim::SessaoParou => "sessao_parou",
            MotivoFim::NovaMensagem => "nova_mensagem",
            MotivoFim::OutraSessao => "outra_sessao",
            MotivoFim::SemUso => "sem_uso",
            MotivoFim::SaiuDaJanela => "saiu_da_janela",
            MotivoFim::TetoLeitura => "teto_leitura",
            MotivoFim::CorteDoDia => "corte_do_dia",
            MotivoFim::AppFechado => "app_fechado",
            MotivoFim::Desligada => "desligada",
            MotivoFim::TrocouAtividade => "trocou_atividade",
            MotivoFim::Interrompida => "interrompida",
            MotivoFim::Outro => "outro",
        }
    }

    /// O texto gravado no banco: o de `as_str`, menos os motivos novos que uma versão anterior do
    /// app não leria (`TrocouAtividade` vai como `pausa`; `Interrompida`, como `terminou`: o turno
    /// acabou, e nada decide pelo motivo; revisão da rodada 13).
    pub fn no_banco(&self) -> &'static str {
        match self {
            MotivoFim::Interrompida => MotivoFim::Terminou.as_str(),
            MotivoFim::TrocouAtividade | MotivoFim::Outro => MotivoFim::Pausa.as_str(),
            m => m.as_str(),
        }
    }

    /// A leitura do banco: um texto desconhecido (de uma versão mais nova) vira `Outro`, em vez de
    /// impedir a leitura dos trechos.
    pub fn do_banco(s: &str) -> MotivoFim {
        MotivoFim::de_str(s).unwrap_or(MotivoFim::Outro)
    }

    pub fn de_str(s: &str) -> Option<MotivoFim> {
        match s {
            "pausa" => Some(MotivoFim::Pausa),
            "inatividade" => Some(MotivoFim::Inatividade),
            "terminou" => Some(MotivoFim::Terminou),
            "sessao_parou" => Some(MotivoFim::SessaoParou),
            "nova_mensagem" => Some(MotivoFim::NovaMensagem),
            "outra_sessao" => Some(MotivoFim::OutraSessao),
            "sem_uso" => Some(MotivoFim::SemUso),
            "saiu_da_janela" => Some(MotivoFim::SaiuDaJanela),
            "teto_leitura" => Some(MotivoFim::TetoLeitura),
            "corte_do_dia" => Some(MotivoFim::CorteDoDia),
            "app_fechado" => Some(MotivoFim::AppFechado),
            "desligada" => Some(MotivoFim::Desligada),
            "trocou_atividade" => Some(MotivoFim::TrocouAtividade),
            "interrompida" => Some(MotivoFim::Interrompida),
            "outro" => Some(MotivoFim::Outro),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EstadoSessao {
    Trabalhando,
    EsperandoVoce,
    Parada,
}

impl EstadoSessao {
    pub fn as_str(&self) -> &'static str {
        match self {
            EstadoSessao::Trabalhando => "trabalhando",
            EstadoSessao::EsperandoVoce => "esperando_voce",
            EstadoSessao::Parada => "parada",
        }
    }

    pub fn de_str(s: &str) -> Option<EstadoSessao> {
        match s {
            "trabalhando" => Some(EstadoSessao::Trabalhando),
            "esperando_voce" => Some(EstadoSessao::EsperandoVoce),
            "parada" => Some(EstadoSessao::Parada),
            _ => None,
        }
    }
}

/// Como a sessão ficou ligada ao timer: pela regra de pasta, à mão (ligar, mover, "Não contar",
/// desligar: a pessoa decidiu, e a decisão vence a regra), herdada da conversa anterior (a
/// mesma conversa continuada numa sessão nova, `Motor::herdar`) ou deduzida (sessão de antes da
/// versão 4 do banco cujo timer não é o da regra de agora, sem prova de que foi movida à mão:
/// pode ter sido uma regra trocada depois). A deduzida não passa o timer adiante nem herda.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Ligacao {
    Regra,
    Mao,
    Herdada,
    Deduzida,
}

impl Ligacao {
    pub fn as_str(&self) -> &'static str {
        match self {
            Ligacao::Regra => "regra",
            Ligacao::Mao => "mao",
            Ligacao::Herdada => "herdada",
            Ligacao::Deduzida => "deduzida",
        }
    }

    pub fn de_str(s: &str) -> Option<Ligacao> {
        match s {
            "regra" => Some(Ligacao::Regra),
            "mao" => Some(Ligacao::Mao),
            "herdada" => Some(Ligacao::Herdada),
            "deduzida" => Some(Ligacao::Deduzida),
            _ => None,
        }
    }
}

/// Trabalho: a atividade dos trechos de antes da versão 5 do banco e o padrão de todo timer.
pub const ATIVIDADE_TRABALHO: i64 = 1;
/// Reunião: a do botão 👥 (reunião manual), pelo id (renomear não muda o botão).
pub const ATIVIDADE_REUNIAO: i64 = 2;
/// Estudo (um timer de curso pode ter Estudo como padrão).
pub const ATIVIDADE_ESTUDO: i64 = 3;

const fn trabalho() -> i64 {
    ATIVIDADE_TRABALHO
}

/// O espaço da migração e do banco novo (rodada 20): "Trabalho" num banco com dados, "Geral" num
/// novo. É o padrão das colunas `espaco_id`.
pub const ESPACO_PRIMEIRO: i64 = 1;

const fn espaco_primeiro() -> i64 {
    ESPACO_PRIMEIRO
}

/// Um espaço (rodada 20): um nível acima dos grupos ("Trabalho", "Pessoal"). Cada espaço tem os
/// seus grupos e o seu "Sem grupo".
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Espaco {
    pub id: i64,
    pub nome: String,
    pub cor: String,
    pub ordem: i64,
    pub arquivado: bool,
}

/// O tipo de hora de um trecho (Trabalho, Reunião, Estudo...): cada trecho contado tem uma. A
/// lista é editável em Ajustes (renomear, acrescentar, cor, arquivar); nunca se apaga uma
/// atividade, porque trechos e ajustes apontam para ela.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Atividade {
    pub id: i64,
    pub nome: String,
    pub cor: String,
    pub ordem: i64,
    pub arquivada: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Grupo {
    pub id: i64,
    pub nome: String,
    pub cor: String,
    pub link_projeto: Option<String>,
    pub card_padrao: Option<String>,
    pub ordem: i64,
    pub arquivado: bool,
    /// Nome dado pela pessoa ao projeto do link (o link do Zoho Sprints só traz o código, ex.: `P146`).
    #[serde(default)]
    pub link_projeto_nome: Option<String>,
    /// Nome dado ao card padrão (vale para os timers do grupo que herdam o card).
    #[serde(default)]
    pub card_padrao_nome: Option<String>,
    /// O espaço do grupo (rodada 20).
    #[serde(default = "espaco_primeiro")]
    pub espaco_id: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Timer {
    pub id: i64,
    pub grupo_id: Option<i64>,
    pub nome: String,
    pub cor: String,
    pub link_card: Option<String>,
    pub ordem: i64,
    pub arquivado: bool,
    /// Nome dado pela pessoa ao card do link (o link do Zoho Sprints só traz o código, ex.: `I7`).
    #[serde(default)]
    pub link_card_nome: Option<String>,
    /// A atividade do Claude trabalhando, da leitura, da escrita e do ▶ manual deste timer
    /// (Trabalho por padrão; um timer de curso pode ter Estudo).
    #[serde(default = "trabalho")]
    pub atividade_padrao_id: i64,
    /// O espaço do timer sem grupo (o "Sem grupo" de cada espaço, rodada 20). Com grupo, vale o
    /// espaço do grupo.
    #[serde(default = "espaco_primeiro")]
    pub espaco_id: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trecho {
    pub id: i64,
    pub timer_id: Option<i64>,
    pub sessao_id: Option<String>,
    pub origem: Origem,
    pub inicio: Ms,
    pub fim: Option<Ms>,
    pub motivo_fim: Option<MotivoFim>,
    pub descartado: bool,
    pub atividade_id: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sessao {
    pub id: String,
    pub cwd: String,
    pub nome: Option<String>,
    pub transcript: Option<String>,
    pub timer_id: Option<i64>,
    pub estado: EstadoSessao,
    pub ultimo_evento: Ms,
    pub terminou_em: Option<Ms>,
    pub criada_em: Ms,
    /// Como o timer foi ligado (`None`: sem timer e sem decisão da pessoa, ou sessão de antes da
    /// versão 4 do banco sem timer).
    #[serde(default)]
    pub ligacao: Option<Ligacao>,
    /// Quando a sessão foi desligada à mão pela última vez (rodada 15): ligar de novo "desde o
    /// início" move só o que veio depois (o tempo de antes fica onde estava).
    #[serde(default)]
    pub desligada_em: Option<Ms>,
}

/// Ajuste à mão do tempo de um timer num dia de trabalho (`dia` = AAAA-MM-DD): soma ao total do
/// timer e do dia (nunca abaixo de zero).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Ajuste {
    pub id: i64,
    pub timer_id: i64,
    pub dia: String,
    /// Com sinal: positivo soma, negativo tira.
    pub delta_ms: Ms,
    pub motivo: Option<String>,
    pub criado_em: Ms,
    /// Soma na atividade e no timer (Trabalho nos ajustes de antes da versão 5 do banco).
    #[serde(default = "trabalho")]
    pub atividade_id: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegraPasta {
    pub id: i64,
    pub pasta: String,
    pub timer_id: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TrechoAberto {
    pub timer_id: Option<i64>,
    pub sessao_id: Option<String>,
    pub origem: Origem,
    pub inicio: Ms,
    pub atividade_id: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Pergunta {
    /// `pasta_mae`: a pasta (a raiz do projeto) tem vários projetos dentro, conferido uma vez,
    /// quando a pergunta nasce (revisão da rodada 15: o snapshot não lê o disco).
    SessaoNova {
        sessao_id: String,
        cwd: String,
        sugestao: String,
        #[serde(default)]
        pasta_mae: bool,
    },
    /// Conversa nova numa pasta com regra (rodada 15): ela já conta no timer da regra, e o dock
    /// pergunta se está certo ("Certo", "Outro timer…", "+ Novo timer…").
    ConversaNova { sessao_id: String, cwd: String, timer_id: i64 },
    /// `atividades`: a atividade do play manual de cada timer quando ele pausou (o "Contar"
    /// reabre nela); timer fora do mapa usa a padrão dele.
    VoltaInatividade { timer_ids: Vec<i64>, de: Ms, ate: Ms, #[serde(default)] atividades: BTreeMap<i64, i64> },
    AppFechado { timer_ids: Vec<i64>, de: Ms, ate: Ms, #[serde(default)] atividades: BTreeMap<i64, i64> },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Resposta {
    /// Volta da inatividade ou do app fechado: reabre os timers desde que pararam (o tempo fora conta).
    Contar,
    /// Volta da inatividade ou do app fechado: religa os mesmos timers, nas mesmas atividades, a
    /// partir de agora (o tempo fora não conta).
    RetomarAgora,
    DeixarPausado,
    NaoContar,
    LigarA { timer_id: i64, criar_regra: bool },
    /// "Certo" da conversa nova: fica no timer da regra.
    Certo,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Mudanca {
    Abriu { timer_id: Option<i64>, sessao_id: Option<String>, origem: Origem, inicio: Ms, atividade_id: i64 },
    Fechou { timer_id: Option<i64>, sessao_id: Option<String>, origem: Origem, fim: Ms, motivo: MotivoFim },
    Reatribuir { sessao_id: String, timer_id: Option<i64> },
    /// Como `Reatribuir`, mas só os trechos que começam em `desde` ou depois (a herança: os dias
    /// anteriores nunca mudam de timer sozinhos, podem já ter sido lançados).
    ReatribuirDesde { sessao_id: String, timer_id: i64, desde: Ms },
    Sessao(Sessao),
    Pergunta(u64, Pergunta),
    PerguntaResolvida(u64),
    /// Os timers encerrados (■) no dia de trabalho `dia` (AAAA-MM-DD), o conjunto inteiro: o banco
    /// grava para valer depois de reabrir o app no mesmo dia.
    Encerrados { dia: String, timers: Vec<i64> },
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn strings_das_enums_fazem_roundtrip() {
        for o in [Origem::Manual, Origem::Claude, Origem::Leitura, Origem::Janela, Origem::Reuniao] {
            assert_eq!(Origem::de_str(o.as_str()), Some(o));
        }
        for m in [
            MotivoFim::Pausa, MotivoFim::Inatividade, MotivoFim::Terminou, MotivoFim::SessaoParou,
            MotivoFim::NovaMensagem, MotivoFim::OutraSessao, MotivoFim::SemUso, MotivoFim::SaiuDaJanela,
            MotivoFim::TetoLeitura, MotivoFim::CorteDoDia, MotivoFim::AppFechado, MotivoFim::Desligada, MotivoFim::TrocouAtividade,
            MotivoFim::Interrompida, MotivoFim::Outro,
        ] {
            assert_eq!(MotivoFim::de_str(m.as_str()), Some(m));
        }
        // o que vai para o banco, uma versão anterior do app (a 4) sabe ler
        let v4 = ["pausa", "inatividade", "terminou", "sessao_parou", "nova_mensagem", "outra_sessao", "sem_uso",
            "saiu_da_janela", "teto_leitura", "corte_do_dia", "app_fechado", "desligada"];
        for m in [MotivoFim::Pausa, MotivoFim::Desligada, MotivoFim::CorteDoDia, MotivoFim::TrocouAtividade, MotivoFim::Interrompida, MotivoFim::Outro] {
            assert!(v4.contains(&m.no_banco()), "{m:?}");
        }
        assert_eq!(MotivoFim::do_banco("motivo_do_futuro"), MotivoFim::Outro);
        for e in [EstadoSessao::Trabalhando, EstadoSessao::EsperandoVoce, EstadoSessao::Parada] {
            assert_eq!(EstadoSessao::de_str(e.as_str()), Some(e));
        }
        for l in [Ligacao::Regra, Ligacao::Mao, Ligacao::Herdada, Ligacao::Deduzida] {
            assert_eq!(Ligacao::de_str(l.as_str()), Some(l));
        }
    }
}
