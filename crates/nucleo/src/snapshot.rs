// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Snapshot: tudo o que a interface precisa num objeto só (Serialize).
use serde::Serialize;

use crate::banco::Banco;
use crate::config::{texto_de_corte, ConfigApp};
use crate::modelo::{Atividade, EstadoSessao, Grupo, Ligacao, MotivoFim, Origem, Pergunta, Sessao, Timer, Trecho, ATIVIDADE_REUNIAO};
use crate::motor::{raiz_do_projeto, sugestao_de_pasta, Motor, AVISO_LIGADA_MS};
use crate::tempo::{dia_de, inicio_do_dia, proximo_corte, Ms, MIN};
use crate::APP_NOME;

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Snapshot {
    /// AAAA-MM-DD do dia de trabalho.
    pub dia: String,
    pub dia_inicio: Ms,
    pub agora: Ms,
    /// Quantos timers estao rodando.
    pub rodando: usize,
    pub grupos: Vec<GrupoDto>,
    pub perguntas: Vec<PerguntaDto>,
    /// Id da sessao sendo lida agora (a conversa aberta no Claude).
    pub lendo: Option<String>,
    /// Voce esta escrevendo na conversa lida agora (o foco na caixa de mensagem dela, no app do
    /// Claude): o dock diz "escrevendo" em vez de "lendo". Conta como leitura.
    pub escrevendo: bool,
    /// Sessões que uma regra de pasta (ou a herança da conversa anterior) acabou de ligar sozinha:
    /// o dock mostra "Ligada a X pela pasta · Trocar" até `ate`.
    pub avisos: Vec<AvisoDto>,
    /// Todas as atividades (Trabalho, Reunião, Estudo...), as arquivadas também (trechos antigos
    /// apontam para elas), por ordem.
    pub atividades: Vec<Atividade>,
    /// Todos os espaços (rodada 20), os arquivados também (os Ajustes desarquivam), por ordem.
    /// Os grupos e timers de um espaço arquivado ficam fora de `grupos`.
    pub espacos: Vec<EspacoDto>,
    pub config: ConfigApp,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct EspacoDto {
    pub id: i64,
    pub nome: String,
    pub cor: String,
    pub ordem: i64,
    pub arquivado: bool,
    /// O tempo real (a união) dos timers do espaço nos dias fechados, cada um desde o seu zerar
    /// (`Acumulado::real_por_espaco`): a tela soma o de hoje para a dica do cabeçalho do espaço.
    pub real_antes_ms: Ms,
    /// Grupos e timers sem grupo não arquivados do espaço (revisão da rodada 20): com algum, o
    /// "Apagar" dos Ajustes fica desabilitado, e o núcleo recusa do mesmo jeito.
    pub ativos: i64,
    /// Grupos e timers sem grupo arquivados: ao apagar o espaço, vão para outro.
    pub arquivados: i64,
}

/// A chave do grupo de um timer no acumulado (rodada 20): o id do grupo, ou o negativo do espaço
/// para o "Sem grupo" dele (cada espaço tem o seu; os ids dos grupos são positivos).
pub fn chave_do_grupo(grupo_id: Option<i64>, espaco_id: i64) -> Option<i64> {
    grupo_id.or(Some(-espaco_id))
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AvisoDto {
    pub sessao_id: String,
    pub sessao_nome: String,
    pub cwd: String,
    pub timer_id: i64,
    /// "Ligada a Flow pela pasta" ou "Ligada a Cliente B pela conversa anterior".
    pub texto: String,
    /// Até quando o aviso fica (ms desde 1970).
    pub ate: Ms,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GrupoDto {
    /// `None` = "Sem grupo" (o do espaço `espaco_id`).
    pub id: Option<i64>,
    /// O espaço do grupo (rodada 20).
    pub espaco_id: i64,
    pub nome: String,
    pub cor: String,
    pub link_projeto: Option<String>,
    pub card_padrao: Option<String>,
    pub link_projeto_nome: Option<String>,
    pub card_padrao_nome: Option<String>,
    pub ordem: i64,
    pub timers: Vec<TimerDto>,
    /// O tempo real (a união) dos timers do grupo nos dias fechados, cada um desde o seu zerar: a
    /// tela soma o de hoje para a dica do cabeçalho.
    pub real_antes_ms: Ms,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TimerDto {
    pub id: i64,
    pub grupo_id: Option<i64>,
    pub nome: String,
    pub cor: String,
    pub link_card: Option<String>,
    pub link_card_nome: Option<String>,
    pub ordem: i64,
    /// A atividade do Claude trabalhando, da leitura e do ▶ manual deste timer.
    pub atividade_padrao_id: i64,
    /// A atividade do play manual aberto (▶ ou 👥), se houver: o 👥 aparece apertado com a reunião.
    pub manual_atividade_id: Option<i64>,
    pub rodando: bool,
    pub motivo: Option<String>,
    /// O que o timer está fazendo, para a etiqueta: `"reuniao"` (a reunião manual, 👥),
    /// `"trabalhando"` (Claude trabalhando), `"lendo"` (você lendo a conversa), `"escrevendo"`
    /// (você escrevendo nela; conta como leitura), `"manual"` (o ▶ rodando; rodada 13),
    /// `"esperando"` (o Claude terminou tudo e a sessão espera você) ou `"segundo_plano"` (a
    /// sessão terminou o turno com uma tarefa em segundo plano ainda rodando; rodada 13). Os dois
    /// últimos valem até a virada do dia; cada lugar da tela corta pelo tempo dele
    /// (`espera_ultima` e os Ajustes "Esperando você").
    pub situacao: Option<String>,
    /// Desde quando uma sessão do timer espera você (o `terminou_em` mais antigo entre as que
    /// esperam): ordena os lembretes do dock.
    pub espera_desde: Option<Ms>,
    /// O fim de turno mais recente entre as sessões que esperam você (agora, para a sendo lida):
    /// a tela some com o lembrete depois do tempo configurado a partir dele.
    pub espera_ultima: Option<Ms>,
    /// O que o Claude está fazendo no timer, sem olhar o ▶ e a reunião manuais (rodada 14):
    /// `"trabalhando"`, `"lendo"`, `"escrevendo"`, `"esperando"` ou `"segundo_plano"`. Com o
    /// manual rodando, a tela junta os dois ("manual + esperando você").
    pub situacao_claude: Option<String>,
    /// O ⏸ (ou a inatividade) parou o play manual do timer neste instante, e ele segue parado
    /// (rodada 14): o dock recolhido o mostra por um tempo, com o ▶. Nunca depois do ■.
    pub pausado_em: Option<Ms>,
    /// Encerrado hoje (■): parado, fora do dock recolhido e sem o lembrete "esperando você".
    pub encerrado: bool,
    pub trechos: Vec<TrechoDto>,
    pub sessoes: Vec<SessaoDto>,
    /// Soma dos ajustes à mão de hoje (com sinal). O total do dia do timer é a soma das
    /// atividades, cada uma a união dos trechos dela mais os ajustes nela, nunca abaixo de zero
    /// (`relatorio`).
    pub ajuste_ms: Ms,
    pub ajustes: Vec<AjusteDto>,
    /// O último "Zerar" (rodada 11): o número do timer é o acumulado desde ele. Hoje, contam os
    /// trechos a partir dele e os ajustes criados depois dele; os dias fechados vêm em
    /// `acumulado_antes`.
    pub zerado_em: Ms,
    /// O zerar só de uma atividade (rodada 19), das que foram zeradas depois do timer todo: a
    /// atividade conta desde ele (o mais recente entre os dois). As outras contam desde `zerado_em`.
    pub zerados_atividade: Vec<ZeradoAtividadeDto>,
    /// O espaço do timer (rodada 20): o do grupo, ou o do timer sem grupo.
    pub espaco_id: i64,
    /// O nome do grupo do timer quando ele está arquivado (o timer rodando aparece no "Sem
    /// grupo"; re-revisão da rodada 20): o editar timer mostra esse grupo, e salvar sem mexer o
    /// mantém. Nulo com o grupo à vista ou sem grupo.
    pub grupo_arquivado: Option<String>,
    /// As atividades que o painel do timer não mostra (rodada 20; uma oculta com tempo ou rodando
    /// aparece mesmo assim).
    pub atividades_ocultas: Vec<i64>,
    /// O acumulado dos dias fechados (antes de `dia_inicio`) desde o zerar, por atividade
    /// (`relatorio::acumulado`); vazio com o timer zerado hoje.
    pub acumulado_antes: Vec<AtividadeMsDto>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ZeradoAtividadeDto {
    pub atividade_id: i64,
    pub zerado_em: Ms,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AtividadeMsDto {
    pub atividade_id: i64,
    pub ms: Ms,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AjusteDto {
    pub id: i64,
    pub delta_ms: Ms,
    pub motivo: Option<String>,
    pub atividade_id: i64,
    /// Quando foi feito: só os feitos depois do zerar entram no número do timer.
    pub criado_em: Ms,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TrechoDto {
    pub inicio: Ms,
    pub fim: Option<Ms>,
    pub origem: String,
    pub atividade_id: i64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct SessaoDto {
    pub id: String,
    /// Titulo da sessao ou o ultimo nome da pasta.
    pub nome: String,
    pub cwd: String,
    pub estado: String,
    pub timer_id: Option<i64>,
    /// Quando a sessão começou: ao mover uma sessão que começou há pouco, o padrão é "desde o início".
    pub criada_em: Ms,
    /// Como ela entrou no timer (rodada 15, o ✎ da sessão): "regra", "mao", "herdada", "deduzida".
    pub ligacao: Option<String>,
    /// Ligada pela regra de pasta: a regra (a que vale para a pasta dela, do mesmo timer).
    pub regra_id: Option<i64>,
    pub regra_pasta: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PerguntaDto {
    pub id: u64,
    pub tipo: String,
    pub texto: String,
    /// (valor para `responder`, rotulo do botao).
    pub opcoes: Vec<(String, String)>,
    /// Sessao da pergunta `sessao_nova` (a UI acha a pergunta ao ligar a sessao por outro caminho).
    pub sessao_id: Option<String>,
    /// Pasta da pergunta `sessao_nova` (a do projeto, num worktree): o dock mostra a curta
    /// (`pasta_curta`, com reticências se não couber) e o caminho inteiro na dica.
    pub pasta: Option<String>,
    pub pasta_curta: Option<String>,
}

const COR_SEM_GRUPO: &str = "#8a8f98";

/// Valor da opção "+ Novo timer…" da pergunta de sessão nova (a página trata; não é resposta).
pub const OPCAO_NOVO_TIMER: &str = "novo";

/// Motivo de o timer estar rodando: Claude vence leitura (ou escrita na conversa lida), que vence
/// o play manual. A reunião manual (👥) vem na frente: "reunião", ou "reunião · Claude
/// trabalhando" com o Claude trabalhando junto (a sobreposição permitida).
fn motivo(origens: &[Origem], escrevendo: bool, reuniao: bool) -> Option<String> {
    let m = if origens.contains(&Origem::Claude) {
        Some("Claude trabalhando")
    } else if origens.contains(&Origem::Leitura) {
        Some(if escrevendo { "você escrevendo" } else { "você lendo" })
    } else if origens.contains(&Origem::Manual) && !reuniao {
        Some("play manual")
    } else {
        None
    };
    match (reuniao, m) {
        (true, Some(m)) => Some(format!("reunião · {m}")),
        (true, None) => Some("reunião".to_string()),
        (false, m) => m.map(str::to_string),
    }
}

/// A sessão espera você, para a tela (rodada 13): terminou hoje (ou é a lida agora) e o timer não
/// está encerrado. Vale até a virada do dia; o dock recolhido e o resto da tela cortam antes, cada
/// um pelo seu tempo (Ajustes → "Esperando você"). `ESPERA_RECENTE_MS` segue nas regras do motor.
fn espera_na_tela(motor: &Motor, s: &Sessao, dia_inicio: Ms) -> bool {
    s.estado == EstadoSessao::EsperandoVoce
        && !motor.encerrado(s.timer_id)
        && (motor.lendo_agora() == Some(s.id.as_str()) || s.terminou_em.is_some_and(|t| t >= dia_inicio))
}

/// O próximo instante em que alguma etiqueta "esperando você" (ou "segundo plano") muda sem
/// evento nenhum (revisão da rodada 13): o fim do tempo do dock recolhido (`espera_dock_min`) e do
/// dock expandido e do app (`espera_app_min`) de cada sessão que espera você hoje, o vencimento de
/// cada tarefa em segundo plano, o fim do tempo de cada timer pausado no dock recolhido
/// (`pausados`, os instantes do ⏸; `pausado_dock_min`; rodada 14) e a próxima virada do dia. O laço pede um snapshot novo quando ele
/// passa, para a tela tirar a etiqueta na hora.
pub fn proximo_vencimento(motor: &Motor, cfg: &ConfigApp, pausados: &[Ms], agora: Ms) -> Ms {
    let (corte, fuso) = (motor.cfg.corte_min, motor.cfg.fuso_min);
    let dia_inicio = inicio_do_dia(dia_de(agora, corte, fuso), corte, fuso);
    let limites: Vec<Ms> =
        [cfg.espera_dock_min, cfg.espera_app_min].iter().filter(|m| **m > 0).map(|m| *m as Ms * MIN).collect();
    let esperas = motor
        .sessoes
        .values()
        .filter(|s| espera_na_tela(motor, s, dia_inicio))
        .filter_map(|s| s.terminou_em)
        .flat_map(|t| limites.iter().map(move |l| t + l));
    let tarefas = motor.segundo_plano.values().copied();
    // o pausado sai do dock recolhido (rodada 14)
    let pausa = cfg.pausado_dock_min as Ms * MIN;
    let pausados = pausados.iter().filter(|_| pausa > 0).map(|p| p + pausa);
    esperas.chain(tarefas).chain(pausados).filter(|v| *v > agora).fold(proximo_corte(agora, corte, fuso), Ms::min)
}

/// O que o timer está fazendo (a etiqueta). A reunião manual (👥) vem na frente: é o que você
/// está fazendo e o que precisa lembrar de parar. Depois, Claude trabalhando, a leitura (ou a
/// escrita), o ▶ manual ("manual", rodada 13), e por fim as sessões que esperam você (o
/// lembrete, com o timer parado): "esperando" quando alguma terminou tudo, "segundo_plano" quando
/// todas as que esperam têm uma tarefa em segundo plano rodando (`Motor::em_segundo_plano`).
fn situacao(motor: &Motor, timer_id: i64, origens: &[Origem], reuniao: bool, dia_inicio: Ms, agora: Ms) -> Option<String> {
    let claude = situacao_claude(motor, timer_id, origens, dia_inicio, agora);
    let s = if reuniao {
        "reuniao"
    } else if matches!(claude.as_deref(), Some("trabalhando" | "lendo" | "escrevendo")) {
        return claude;
    } else if origens.contains(&Origem::Manual) {
        "manual"
    } else {
        return claude;
    };
    Some(s.to_string())
}

/// O que o Claude está fazendo no timer, sem os manuais (rodada 14): trabalhando, a leitura (ou a
/// escrita) e, com o timer sem nada disso, as sessões que esperam você ("esperando" vence
/// "segundo_plano").
fn situacao_claude(motor: &Motor, timer_id: i64, origens: &[Origem], dia_inicio: Ms, agora: Ms) -> Option<String> {
    let esperam: Vec<&Sessao> =
        motor.sessoes.values().filter(|s| s.timer_id == Some(timer_id) && espera_na_tela(motor, s, dia_inicio)).collect();
    let s = if origens.contains(&Origem::Claude) {
        "trabalhando"
    } else if origens.contains(&Origem::Leitura) {
        // só há uma leitura aberta por vez: a escrita é nela
        if motor.escrevendo_agora() { "escrevendo" } else { "lendo" }
    } else if esperam.iter().any(|s| !motor.em_segundo_plano(s, agora)) {
        "esperando"
    } else if !esperam.is_empty() {
        "segundo_plano"
    } else {
        return None;
    };
    Some(s.to_string())
}

/// Desde quando o timer está pausado no dock recolhido, se ele segue parado (rodada 14): o
/// instante da ação guardado no motor (`Motor::pausado_desde`: o clique do ⏸, a volta da
/// inatividade ou o "Deixar pausado"; revisão da rodada 14). Ainda ausente (a inatividade antes da
/// volta), nada. Sem o instante em memória (o app reaberto), o fim do último trecho manual do dia,
/// se ele fechou com `pausa` ou `inatividade`. Nada do timer aberto e sem ■. Uma troca de
/// atividade (▶ ↔ 👥) vai como `pausa`, mas abre o outro no mesmo instante: aí o timer roda.
fn pausado_em(motor: &Motor, timer_id: i64, trechos: &[Trecho]) -> Option<Ms> {
    if motor.encerrado(Some(timer_id)) || motor.abertos.iter().any(|a| a.timer_id == Some(timer_id)) {
        return None;
    }
    if motor.ausencia.as_ref().is_some_and(|(_, ids, _)| ids.contains(&timer_id)) {
        return None;
    }
    if let Some(em) = motor.pausado_desde.get(&timer_id) {
        return Some(*em);
    }
    let ultimo = trechos
        .iter()
        .filter(|t| t.timer_id == Some(timer_id) && t.origem == Origem::Manual && !t.descartado)
        .max_by_key(|t| (t.inicio, t.id))?;
    match (ultimo.fim, ultimo.motivo_fim) {
        (Some(fim), Some(MotivoFim::Pausa | MotivoFim::Inatividade)) => Some(fim),
        _ => None,
    }
}

/// Os timers pausados de hoje e o instante do ⏸ (`pausado_em`), para o laço saber quando vence o
/// tempo deles no dock (`proximo_vencimento`).
pub fn pausados_de_hoje(banco: &Banco, motor: &Motor, agora: Ms) -> rusqlite::Result<Vec<Ms>> {
    let (corte, fuso) = (motor.cfg.corte_min, motor.cfg.fuso_min);
    let dia_inicio = inicio_do_dia(dia_de(agora, corte, fuso), corte, fuso);
    let trechos = banco.trechos_entre(dia_inicio, proximo_corte(agora, corte, fuso))?;
    let timers: std::collections::BTreeSet<i64> = trechos.iter().filter_map(|t| t.timer_id).collect();
    Ok(timers.into_iter().filter_map(|t| pausado_em(motor, t, &trechos)).collect())
}

/// Ultimas duas partes da pasta, com `\` (ex.: `Clientes\Cliente D`).
fn pasta_curta(cwd: &str) -> String {
    let partes: Vec<&str> = cwd.split(['\\', '/']).filter(|p| !p.is_empty()).collect();
    partes[partes.len().saturating_sub(2)..].join("\\")
}

/// "A", "A e B", "A, B e C".
fn lista_de_nomes(nomes: &[String]) -> String {
    match nomes {
        [] => String::new(),
        [um] => um.clone(),
        [inicio @ .., ultimo] => format!("{} e {ultimo}", inicio.join(", ")),
    }
}

/// "25 min" ou "8h 00min", como o total do dia.
fn duracao(de: Ms, ate: Ms) -> String {
    let min = ((ate - de).max(0) + MIN / 2) / MIN;
    if min < 60 {
        format!("{min} min")
    } else {
        format!("{}h {:02}min", min / 60, min % 60)
    }
}

pub fn nome_da_sessao(s: &Sessao) -> String {
    match s.nome.as_deref().map(str::trim) {
        Some(n) if !n.is_empty() => n.to_string(),
        _ => sugestao_de_pasta(&s.cwd),
    }
}

/// A sessão para a tela; `regras`: as regras de pasta, para dizer qual ligou a sessão.
pub fn sessao_dto(s: &Sessao, regras: &[crate::modelo::RegraPasta]) -> SessaoDto {
    sessao_dto_com(s, s.estado, regras)
}

fn sessao_dto_com(s: &Sessao, estado: EstadoSessao, regras: &[crate::modelo::RegraPasta]) -> SessaoDto {
    let regra = (s.ligacao == Some(Ligacao::Regra))
        .then(|| crate::motor::regra_da_pasta(regras, &s.cwd))
        .flatten()
        .filter(|r| Some(r.timer_id) == s.timer_id);
    SessaoDto {
        id: s.id.clone(),
        nome: nome_da_sessao(s),
        cwd: s.cwd.clone(),
        estado: estado.as_str().to_string(),
        timer_id: s.timer_id,
        criada_em: s.criada_em,
        ligacao: s.ligacao.map(|l| l.as_str().to_string()),
        regra_id: regra.map(|r| r.id),
        regra_pasta: regra.map(|r| r.pasta.clone()),
    }
}

/// Os avisos de ligada sozinha que valem agora, com o nome do timer (timer fora da lista, como
/// um arquivado: sem aviso).
fn avisos(motor: &Motor, timers: &[Timer], agora: Ms) -> Vec<AvisoDto> {
    motor
        .avisos_de_ligacao(agora)
        .into_iter()
        .filter_map(|(s, l)| {
            let t = timers.iter().find(|t| t.id == l.timer_id)?;
            let por = if l.como == Ligacao::Herdada { "pela conversa anterior" } else { "pela pasta" };
            Some(AvisoDto {
                sessao_id: s.id.clone(),
                sessao_nome: nome_da_sessao(s),
                cwd: s.cwd.clone(),
                timer_id: t.id,
                texto: format!("Ligada a {} {por}", t.nome),
                ate: l.em + AVISO_LIGADA_MS,
            })
        })
        .collect()
}

/// Estado para a tela: "esperando você" só vale para a sessão que teve evento hoje, a que está
/// sendo lida ou a que terminou há menos de `ESPERA_RECENTE_MS`. A que espera há dias aparece parada,
/// como a do timer encerrado (■).
fn estado_na_tela(motor: &Motor, s: &Sessao, dia_inicio: Ms, agora: Ms) -> EstadoSessao {
    if s.estado == EstadoSessao::EsperandoVoce
        && (motor.encerrado(s.timer_id) || (s.ultimo_evento < dia_inicio && !motor.espera_recente(s, agora)))
    {
        EstadoSessao::Parada
    } else {
        s.estado
    }
}

/// Ausência que começou antes do corte do dia: " desde 00:00 (8h 12min)", que é o que o "Contar" faz.
fn desde_o_corte(de: Ms, ate: Ms, dia_inicio: Ms, corte_min: u32) -> String {
    if de >= dia_inicio {
        return String::new();
    }
    format!(" desde {} ({})", texto_de_corte(corte_min), duracao(dia_inicio, ate))
}

/// `dia_inicio` e `corte_min`: o "Contar" nunca atravessa o corte do dia, e o texto diz isso.
/// `atividades`: o timer cujo play manual estava noutra atividade que não a padrão dele aparece
/// com ela ("Cliente A (reunião)"): o "Contar" reabre nela.
#[allow(clippy::too_many_arguments)]
fn pergunta_dto(
    id: u64,
    p: &Pergunta,
    timers: &[Timer],
    rotulos: &std::collections::BTreeMap<i64, String>,
    atividades: &[Atividade],
    dia_inicio: Ms,
    corte_min: u32,
    motor: &Motor,
) -> PerguntaDto {
    let nomes = |ids: &[i64], de: &std::collections::BTreeMap<i64, i64>| {
        let v: Vec<String> = ids
            .iter()
            .filter_map(|i| timers.iter().find(|t| t.id == *i))
            .map(|t| match de.get(&t.id).filter(|a| **a != t.atividade_padrao_id) {
                Some(a) => match atividades.iter().find(|x| x.id == *a) {
                    Some(x) => format!("{} ({})", t.nome, x.nome.to_lowercase()),
                    None => t.nome.clone(),
                },
                None => t.nome.clone(),
            })
            .collect();
        lista_de_nomes(&v)
    };
    // Na volta do mesmo dia, "Retomar agora" na frente (o botão destacado): religa os mesmos timers
    // sem contar o tempo fora. A ausência que atravessou o corte do dia não retoma (o manual para na
    // virada, regra 9): "Deixar pausado" na frente e "Contar desde 00:00" (de 00:00 até a volta).
    // A ausência inteira antes do corte (a pergunta de ontem respondida hoje): só "Deixar pausado";
    // ontem não muda e nada religa.
    let contar_ou_nao = |de: Ms, ate: Ms| {
        if ate < dia_inicio {
            vec![("pausado".to_string(), "Deixar pausado".to_string())]
        } else if de < dia_inicio {
            vec![
                ("pausado".to_string(), "Deixar pausado".to_string()),
                ("contar".to_string(), format!("Contar desde {}", texto_de_corte(corte_min))),
            ]
        } else {
            vec![
                ("retomar".to_string(), "Retomar agora".to_string()),
                ("contar".to_string(), "Contar o tempo fora".to_string()),
                ("pausado".to_string(), "Deixar pausado".to_string()),
            ]
        }
    };
    let (sessao_id, pasta) = match p {
        Pergunta::SessaoNova { sessao_id, cwd, .. } | Pergunta::ConversaNova { sessao_id, cwd, .. } => {
            (Some(sessao_id.clone()), Some(raiz_do_projeto(cwd).to_string()))
        }
        _ => (None, None),
    };
    let curta = pasta.as_deref().map(pasta_curta);
    // o nome do timer no botão (com o espaço, se o nome se repete noutro espaço)
    let rotulo = |t: &Timer| rotulos.get(&t.id).cloned().unwrap_or_else(|| t.nome.clone());
    let (tipo, texto, opcoes) = match p {
        Pergunta::SessaoNova { cwd, sugestao, pasta_mae, .. } => {
            let mut opcoes = vec![];
            let sug = sugestao.trim();
            let sug_min = sug.to_lowercase();
            let sugerido = timers.iter().find(|t| !sug.is_empty() && t.nome.trim().to_lowercase() == sug_min);
            // "outro" abre a lista de timers na página: com sugestão, é outro que não ela
            let escolher = match sugerido {
                // numa pasta-mãe (vários projetos dentro; rodada 15), o botão liga só esta conversa:
                // a regra continua possível pelo "Outro timer…" (a caixa vem desmarcada, com o aviso)
                Some(t) if *pasta_mae => {
                    opcoes.push((format!("ligar:{}", t.id), rotulo(t)));
                    "Outro timer…"
                }
                Some(t) => {
                    // o botão cria a regra de pasta: o rótulo diz isso
                    opcoes.push((format!("ligar:{}:regra", t.id), format!("{} · sempre nesta pasta", rotulo(t))));
                    "Outro timer…"
                }
                None => "Escolher timer…",
            };
            opcoes.push(("outro".to_string(), escolher.to_string()));
            // "novo" abre a janela principal com o "Novo timer" (nome pela pasta) e liga a sessão
            // ao timer criado; também nunca vai ao núcleo como resposta
            opcoes.push((OPCAO_NOVO_TIMER.to_string(), "+ Novo timer…".to_string()));
            opcoes.push(("nao".to_string(), "Não contar".to_string()));
            // num worktree, a pasta do projeto (é onde a regra fica)
            let texto =
                format!("Sessão nova do Claude na pasta {}. Em qual timer contar?", pasta_curta(raiz_do_projeto(cwd)));
            ("sessao_nova", texto, opcoes)
        }
        // a conversa nova numa pasta com regra (rodada 15): já conta no timer da regra
        Pergunta::ConversaNova { sessao_id, cwd, timer_id } => {
            let nome = motor.sessoes.get(sessao_id).map_or_else(|| sugestao_de_pasta(raiz_do_projeto(cwd)), nome_da_sessao);
            let timer = timers.iter().find(|t| t.id == *timer_id).map_or("o timer da regra", |t| t.nome.as_str());
            (
                "conversa_nova",
                format!("Conversa nova: {nome} · contando em {timer} pela pasta"),
                vec![
                    ("certo".to_string(), "Certo".to_string()),
                    ("outro".to_string(), "Outro timer…".to_string()),
                    (OPCAO_NOVO_TIMER.to_string(), "+ Novo timer…".to_string()),
                ],
            )
        }
        // o texto combina com o botão destacado: "Retomar …?" no mesmo dia, "Contar … desde 00:00?"
        // depois da virada; a ausência inteira de antes da virada só avisa
        Pergunta::VoltaInatividade { timer_ids, de, ate, atividades } => (
            "volta_inatividade",
            if *ate < dia_inicio {
                format!(
                    "Antes da virada do dia, você ficou {} sem mexer no PC com {} {}. O tempo daquele dia fica como está.",
                    duracao(*de, *ate),
                    nomes(timer_ids, atividades),
                    if timer_ids.len() > 1 { "ligados" } else { "ligado" }
                )
            } else if *de < dia_inicio {
                format!(
                    "Você ficou {} sem mexer no PC. Contar em {}{}?",
                    duracao(*de, *ate),
                    nomes(timer_ids, atividades),
                    desde_o_corte(*de, *ate, dia_inicio, corte_min)
                )
            } else {
                format!("Você ficou {} sem mexer no PC. Retomar {}?", duracao(*de, *ate), nomes(timer_ids, atividades))
            },
            contar_ou_nao(*de, *ate),
        ),
        Pergunta::AppFechado { timer_ids, de, ate, atividades } => (
            "app_fechado",
            if *ate < dia_inicio {
                format!(
                    "Antes da virada do dia, o {APP_NOME} ficou fechado {} com {} {}. O tempo daquele dia fica como está.",
                    duracao(*de, *ate),
                    nomes(timer_ids, atividades),
                    if timer_ids.len() > 1 { "ligados" } else { "ligado" }
                )
            } else {
                format!(
                    "O {APP_NOME} ficou fechado {} com {} {}. {}?",
                    duracao(*de, *ate),
                    nomes(timer_ids, atividades),
                    if timer_ids.len() > 1 { "ligados" } else { "ligado" },
                    if *de < dia_inicio { format!("Contar{}", desde_o_corte(*de, *ate, dia_inicio, corte_min)) } else { "Retomar".to_string() }
                )
            },
            contar_ou_nao(*de, *ate),
        ),
    };
    PerguntaDto { id, tipo: tipo.to_string(), texto, opcoes, sessao_id, pasta, pasta_curta: curta }
}

pub fn montar(banco: &Banco, motor: &Motor, agora: Ms) -> rusqlite::Result<Snapshot> {
    let (corte, fuso) = (motor.cfg.corte_min, motor.cfg.fuso_min);
    let dia = dia_de(agora, corte, fuso);
    let dia_inicio = inicio_do_dia(dia, corte, fuso);
    let dia_fim = proximo_corte(agora, corte, fuso);

    // Os espaços arquivados ficam fora, com os grupos e os timers deles (rodada 20).
    let espacos = banco.espacos()?;
    let ativos: Vec<i64> = espacos.iter().filter(|e| !e.arquivado).map(|e| e.id).collect();
    let grupos: Vec<Grupo> = banco.grupos()?.into_iter().filter(|g| ativos.contains(&g.espaco_id)).collect();
    // o espaço de cada grupo, os arquivados também (o timer rodando num deles)
    let espaco_do_grupo = banco.espacos_dos_grupos()?;
    let nome_do_grupo = banco.nomes_dos_grupos()?;
    // o timer com algo aberto aparece sempre (revisão da rodada 20: rodando nunca fica invisível)
    let rodando: std::collections::BTreeSet<i64> = motor.abertos.iter().filter_map(|a| a.timer_id).collect();
    let no_grupo_a_vista = |t: &Timer| t.grupo_id.is_some_and(|g| grupos.iter().any(|x| x.id == g));
    // Timers de grupo arquivado ficam fora, como o proprio grupo (menos os que estão rodando).
    let timers: Vec<Timer> = banco
        .timers()?
        .into_iter()
        .filter(|t| {
            rodando.contains(&t.id)
                || match t.grupo_id {
                    Some(_) => no_grupo_a_vista(t),
                    None => ativos.contains(&t.espaco_id),
                }
        })
        .collect();
    let espaco_de = |t: &Timer| t.grupo_id.and_then(|g| espaco_do_grupo.get(&g).copied()).unwrap_or(t.espaco_id);
    // na pergunta "Em qual timer contar?", com 2 ou mais espaços, o nome repetido leva o espaço
    // ("Estudos · Pessoal")
    let rotulos: std::collections::BTreeMap<i64, String> = if ativos.len() < 2 {
        Default::default()
    } else {
        let chave = |t: &Timer| t.nome.trim().to_lowercase();
        timers
            .iter()
            .filter(|t| timers.iter().filter(|x| chave(x) == chave(t)).count() > 1)
            .filter_map(|t| espacos.iter().find(|e| e.id == espaco_de(t)).map(|e| (t.id, format!("{} · {}", t.nome, e.nome))))
            .collect()
    };
    let ocultas = banco.atividades_ocultas()?;
    // Trechos de timer arquivado ou desconhecido sao ignorados.
    let trechos = banco.trechos_entre(dia_inicio, dia_fim)?;
    let dia_texto = dia.format("%Y-%m-%d").to_string();
    let ajustes = banco.ajustes_do_dia(&dia_texto)?;
    let atividades = banco.atividades()?;
    // o acumulado dos dias fechados dos timers da tela (guardado no banco: não varre o histórico a
    // cada ciclo)
    let zerados = banco.todos_os_zerados()?;
    let na_tela: std::collections::BTreeMap<i64, Option<i64>> =
        timers.iter().map(|t| (t.id, chave_do_grupo(t.grupo_id, espaco_de(t)))).collect();
    let espaco_na_tela: std::collections::BTreeMap<i64, i64> = timers.iter().map(|t| (t.id, espaco_de(t))).collect();
    let antes = banco.acumulado_antes(&na_tela, &espaco_na_tela, dia_inicio, corte, fuso)?;

    let timer_dto = |t: &Timer| {
        let meus: Vec<AjusteDto> = ajustes
            .iter()
            .filter(|a| a.timer_id == t.id)
            .map(|a| AjusteDto {
                id: a.id,
                delta_ms: a.delta_ms,
                motivo: a.motivo.clone(),
                atividade_id: a.atividade_id,
                criado_em: a.criado_em,
            })
            .collect();
        let origens: Vec<Origem> =
            motor.abertos.iter().filter(|a| a.timer_id == Some(t.id)).map(|a| a.origem).collect();
        let manual_atividade_id = motor.manual_aberto(t.id).map(|a| a.atividade_id);
        let reuniao = manual_atividade_id == Some(ATIVIDADE_REUNIAO);
        let mut sessoes: Vec<(&Sessao, EstadoSessao)> = motor
            .sessoes
            .values()
            .filter(|s| s.timer_id == Some(t.id))
            .map(|s| (s, estado_na_tela(motor, s, dia_inicio, agora)))
            // a sessão lida agora aparece mesmo parada há dias (o dock mostra o nome dela)
            .filter(|(s, estado)| {
                s.ultimo_evento >= dia_inicio || *estado != EstadoSessao::Parada || motor.lendo_agora() == Some(s.id.as_str())
            })
            .collect();
        sessoes.sort_by(|(a, _), (b, _)| b.ultimo_evento.cmp(&a.ultimo_evento).then(a.id.cmp(&b.id)));
        TimerDto {
            id: t.id,
            grupo_id: t.grupo_id,
            nome: t.nome.clone(),
            cor: t.cor.clone(),
            link_card: t.link_card.clone(),
            link_card_nome: t.link_card_nome.clone(),
            ordem: t.ordem,
            atividade_padrao_id: t.atividade_padrao_id,
            manual_atividade_id,
            rodando: !origens.is_empty(),
            motivo: motivo(&origens, motor.escrevendo_agora(), reuniao),
            situacao: situacao(motor, t.id, &origens, reuniao, dia_inicio, agora),
            espera_desde: motor
                .sessoes
                .values()
                .filter(|s| s.timer_id == Some(t.id) && espera_na_tela(motor, s, dia_inicio))
                .filter_map(|s| s.terminou_em)
                .min(),
            espera_ultima: motor
                .sessoes
                .values()
                .filter(|s| s.timer_id == Some(t.id) && espera_na_tela(motor, s, dia_inicio))
                .filter_map(|s| if motor.lendo_agora() == Some(s.id.as_str()) { Some(agora) } else { s.terminou_em })
                .max(),
            situacao_claude: situacao_claude(motor, t.id, &origens, dia_inicio, agora),
            pausado_em: pausado_em(motor, t.id, &trechos),
            encerrado: motor.encerrado(Some(t.id)),
            trechos: trechos
                .iter()
                .filter(|x| x.timer_id == Some(t.id) && !x.descartado)
                .map(|x| TrechoDto {
                    inicio: x.inicio.max(dia_inicio),
                    fim: x.fim.map(|f| f.min(dia_fim)),
                    origem: x.origem.as_str().to_string(),
                    atividade_id: x.atividade_id,
                })
                .collect(),
            sessoes: sessoes.into_iter().map(|(s, estado)| sessao_dto_com(s, estado, &motor.regras_pasta)).collect(),
            ajuste_ms: meus.iter().map(|a| a.delta_ms).sum(),
            ajustes: meus,
            zerado_em: zerados.do_timer(t.id),
            zerados_atividade: zerados
                .atividades
                .range((t.id, i64::MIN)..=(t.id, i64::MAX))
                .filter(|(_, z)| **z > zerados.do_timer(t.id))
                .map(|((_, a), z)| ZeradoAtividadeDto { atividade_id: *a, zerado_em: *z })
                .collect(),
            espaco_id: espaco_de(t),
            grupo_arquivado: t
                .grupo_id
                .filter(|_| !no_grupo_a_vista(t))
                .and_then(|g| nome_do_grupo.get(&g).cloned()),
            atividades_ocultas: ocultas.get(&t.id).cloned().unwrap_or_default(),
            acumulado_antes: antes
                .por_atividade
                .range((t.id, i64::MIN)..=(t.id, i64::MAX))
                .map(|((_, a), ms)| AtividadeMsDto { atividade_id: *a, ms: *ms })
                .collect(),
        }
    };

    // por espaço (na ordem dos espaços): os grupos dele e, no fim, o "Sem grupo" dele, com o timer
    // rodando de um grupo arquivado; um espaço arquivado só entra com um timer rodando
    let mut grupos_dto: Vec<GrupoDto> = vec![];
    for e in &espacos {
        grupos_dto.extend(grupos.iter().filter(|g| g.espaco_id == e.id).map(|g| GrupoDto {
            id: Some(g.id),
            espaco_id: e.id,
            nome: g.nome.clone(),
            cor: g.cor.clone(),
            link_projeto: g.link_projeto.clone(),
            card_padrao: g.card_padrao.clone(),
            link_projeto_nome: g.link_projeto_nome.clone(),
            card_padrao_nome: g.card_padrao_nome.clone(),
            ordem: g.ordem,
            timers: timers.iter().filter(|t| t.grupo_id == Some(g.id)).map(&timer_dto).collect(),
            real_antes_ms: antes.real_por_grupo.get(&Some(g.id)).copied().unwrap_or(0),
        }));
        let soltos: Vec<TimerDto> =
            timers.iter().filter(|t| !no_grupo_a_vista(t) && espaco_de(t) == e.id).map(&timer_dto).collect();
        if soltos.is_empty() {
            continue;
        }
        grupos_dto.push(GrupoDto {
            id: None,
            espaco_id: e.id,
            nome: "Sem grupo".to_string(),
            cor: COR_SEM_GRUPO.to_string(),
            link_projeto: None,
            card_padrao: None,
            link_projeto_nome: None,
            card_padrao_nome: None,
            ordem: i64::MAX,
            timers: soltos,
            real_antes_ms: antes.real_por_grupo.get(&chave_do_grupo(None, e.id)).copied().unwrap_or(0),
        });
    }
    let espacos_dto: Vec<EspacoDto> = espacos
        .iter()
        .map(|e| EspacoDto {
            id: e.id,
            nome: e.nome.clone(),
            cor: e.cor.clone(),
            ordem: e.ordem,
            arquivado: e.arquivado,
            real_antes_ms: antes.real_por_espaco.get(&e.id).copied().unwrap_or(0),
            ativos: 0,
            arquivados: 0,
        })
        .collect();
    let mut espacos_dto = espacos_dto;
    for e in &mut espacos_dto {
        let c = banco.conteudo_do_espaco(e.id)?;
        (e.ativos, e.arquivados) = (c.ativos, c.arquivados);
    }

    Ok(Snapshot {
        dia: dia_texto,
        dia_inicio,
        agora,
        rodando: grupos_dto.iter().flat_map(|g| &g.timers).filter(|t| t.rodando).count(),
        grupos: grupos_dto,
        perguntas: motor
            .perguntas
            .iter()
            .map(|(id, p)| pergunta_dto(*id, p, &timers, &rotulos, &atividades, dia_inicio, corte, motor))
            .collect(),
        lendo: motor.lendo_agora().map(str::to_string),
        escrevendo: motor.escrevendo_agora(),
        avisos: avisos(motor, &timers, agora),
        atividades,
        espacos: espacos_dto,
        config: banco.config_app()?,
    })
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::{ConfigApp, ConfigMotor};
    use crate::modelo::*;
    use crate::tempo::*;
    use fila::EventoHook;

    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms { ms_local(2026, 11, 17, hh, mm, F) }
    fn motor(agora: Ms) -> Motor { Motor::novo(ConfigMotor::de(&ConfigApp::default(), F), agora) }

    #[test]
    fn snapshot_agrupa_timers_e_sem_grupo_por_ultimo() {
        let b = Banco::abrir_memoria().unwrap();
        let g1 = b.criar_grupo("Empresa", "#111").unwrap();
        let g2 = b.criar_grupo("Clientes", "#222").unwrap();
        b.criar_timer(Some(g2), "CLIENTE_D", "#a00").unwrap();
        b.criar_timer(None, "Solto", "#b00").unwrap();
        b.criar_timer(Some(g1), "Projeto A", "#c00").unwrap();
        b.criar_timer(Some(g1), "Cliente F", "#d00").unwrap();
        let m = motor(h(9, 0));
        let s = montar(&b, &m, h(9, 0)).unwrap();
        assert_eq!(s.dia, "2026-11-17");
        assert_eq!(s.dia_inicio, h(0, 0));
        assert_eq!(s.agora, h(9, 0));
        assert_eq!(s.rodando, 0);
        assert_eq!(s.grupos.iter().map(|g| g.id).collect::<Vec<_>>(), [Some(g1), Some(g2), None]);
        assert_eq!(s.grupos[2].nome, "Sem grupo");
        let nomes = |i: usize| s.grupos[i].timers.iter().map(|t| t.nome.as_str()).collect::<Vec<_>>();
        assert_eq!(nomes(0), ["Projeto A", "Cliente F"]);
        assert_eq!(nomes(1), ["CLIENTE_D"]);
        assert_eq!(nomes(2), ["Solto"]);
        assert!(s.grupos.iter().flat_map(|g| &g.timers).all(|t| !t.rodando && t.motivo.is_none()));
        assert_eq!(s.config, ConfigApp::default());
    }

    /// Rodada 20: os grupos por espaço, o "Sem grupo" de cada espaço, o espaço arquivado fora, o
    /// tempo real de cada espaço nos dias fechados e as atividades ocultas.
    #[test]
    fn snapshot_por_espaco_com_o_sem_grupo_de_cada_um() {
        let b = Banco::abrir_memoria().unwrap();
        let pessoal = b.criar_espaco("Pessoal", "#e86bd0").unwrap();
        let velho = b.criar_espaco("Velho", "#888888").unwrap();
        let casa = b.criar_grupo_em("Casa", "#111", pessoal).unwrap();
        let empresa = b.criar_grupo_em("Empresa", "#222", ESPACO_PRIMEIRO).unwrap();
        let antigo = b.criar_grupo_em("Antigo", "#333", velho).unwrap();
        let obra = b.criar_timer(Some(casa), "Obra", "#a00").unwrap();
        let leitura = b.criar_timer_no(None, pessoal, "Leitura", "#b00", h(0, 0), F).unwrap();
        let flow = b.criar_timer(Some(empresa), "Flow", "#c00").unwrap();
        let solto = b.criar_timer(None, "Solto", "#d00").unwrap();
        b.criar_timer(Some(antigo), "Sumido", "#e00").unwrap();
        b.criar_timer_no(None, velho, "Sumido solto", "#f00", h(0, 0), F).unwrap();
        let mut e = b.espacos().unwrap().into_iter().find(|e| e.id == velho).unwrap();
        e.arquivado = true;
        b.editar_espaco(&e).unwrap();
        b.guardar_atividades_ocultas(flow, &[ATIVIDADE_ESTUDO]).unwrap();
        // ontem: Obra 9:00–10:00 e Leitura 9:30–10:30 (Pessoal); Flow 9:00–9:20 (Geral)
        b.zerar(None, 0).unwrap();
        let ontem = |hh, mm| ms_local(2026, 11, 16, hh, mm, F);
        for (t, i, f) in [(obra, ontem(9, 0), ontem(10, 0)), (leitura, ontem(9, 30), ontem(10, 30)), (flow, ontem(9, 0), ontem(9, 20))] {
            b.aplicar(&[Mudanca::Abriu { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, inicio: i, atividade_id: 1 }]).unwrap();
            b.aplicar(&[Mudanca::Fechou { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, fim: f, motivo: MotivoFim::Pausa }]).unwrap();
        }
        let s = montar(&b, &motor(h(9, 0)), h(9, 0)).unwrap();
        // na ordem dos espaços: Geral (Empresa e o Sem grupo dele), Pessoal (Casa e o Sem grupo dele)
        assert_eq!(
            s.grupos.iter().map(|g| (g.id, g.espaco_id, g.nome.as_str())).collect::<Vec<_>>(),
            [
                (Some(empresa), ESPACO_PRIMEIRO, "Empresa"),
                (None, ESPACO_PRIMEIRO, "Sem grupo"),
                (Some(casa), pessoal, "Casa"),
                (None, pessoal, "Sem grupo"),
            ]
        );
        let timer = |id: i64| s.grupos.iter().flat_map(|g| &g.timers).find(|t| t.id == id).unwrap();
        assert_eq!((timer(obra).espaco_id, timer(leitura).espaco_id, timer(solto).espaco_id), (pessoal, pessoal, ESPACO_PRIMEIRO));
        assert_eq!(s.grupos.iter().flat_map(|g| &g.timers).count(), 4, "o espaço arquivado fica fora");
        assert_eq!(timer(flow).atividades_ocultas, [ATIVIDADE_ESTUDO]);
        assert!(timer(obra).atividades_ocultas.is_empty());
        // os espaços (o arquivado também, para os Ajustes) e o tempo real de cada um ontem
        assert_eq!(
            s.espacos.iter().map(|e| (e.id, e.arquivado, e.real_antes_ms)).collect::<Vec<_>>(),
            [(ESPACO_PRIMEIRO, false, 20 * MIN), (pessoal, false, 90 * MIN), (velho, true, 0)]
        );
        // o Sem grupo de cada espaço tem o seu tempo real
        assert_eq!((s.grupos[1].real_antes_ms, s.grupos[3].real_antes_ms), (0, 60 * MIN));
    }

    #[test]
    fn snapshot_sem_grupo_some_quando_vazio() {
        let b = Banco::abrir_memoria().unwrap();
        let g = b.criar_grupo("Empresa", "#111").unwrap();
        b.criar_timer(Some(g), "A", "#a00").unwrap();
        let s = montar(&b, &motor(h(9, 0)), h(9, 0)).unwrap();
        assert_eq!(s.grupos.iter().map(|g| g.id).collect::<Vec<_>>(), [Some(g)]);
    }

    #[test]
    fn snapshot_marca_rodando_com_motivo() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Projeto A", "#c00").unwrap();
        let outro = b.criar_timer(None, "Outro", "#d00").unwrap();
        let mut m = motor(h(9, 0));
        m.regras_pasta = vec![RegraPasta { id: 1, pasta: "G:\\Projeto A".into(), timer_id: t }];
        let ev = EventoHook {
            t: h(9, 0),
            evento: "UserPromptSubmit".into(),
            sessao: "s1".into(),
            cwd: "G:\\Projeto A".into(),
            transcript: None,
            notificacao: None,
        };
        let mud = m.evento(&ev);
        b.aplicar(&mud).unwrap();
        let s = montar(&b, &m, h(9, 30)).unwrap();
        assert_eq!(s.rodando, 1);
        let timers: Vec<&TimerDto> = s.grupos.iter().flat_map(|g| &g.timers).collect();
        let projeto_a = timers.iter().find(|x| x.id == t).unwrap();
        assert!(projeto_a.rodando);
        assert_eq!(projeto_a.motivo.as_deref(), Some("Claude trabalhando"));
        assert_eq!(projeto_a.trechos, vec![TrechoDto { inicio: h(9, 0), fim: None, origem: "claude".into(), atividade_id: 1 }]);
        assert_eq!(projeto_a.sessoes.len(), 1);
        assert_eq!(projeto_a.sessoes[0].id, "s1");
        assert_eq!(projeto_a.sessoes[0].nome, "Projeto A");
        assert_eq!(projeto_a.sessoes[0].estado, "trabalhando");
        let o = timers.iter().find(|x| x.id == outro).unwrap();
        assert!(!o.rodando);
        assert_eq!(o.motivo, None);
    }

    #[test]
    fn snapshot_motivos_de_manual_e_leitura() {
        let b = Banco::abrir_memoria().unwrap();
        let t1 = b.criar_timer(None, "A", "#a00").unwrap();
        let t2 = b.criar_timer(None, "B", "#b00").unwrap();
        let mut m = motor(h(9, 0));
        let mud = m.play(t1, h(9, 0));
        b.aplicar(&mud).unwrap();
        m.abertos.push(TrechoAberto { timer_id: Some(t2), sessao_id: Some("s2".into()), origem: Origem::Leitura, inicio: h(9, 5), atividade_id: 1 });
        let s = montar(&b, &m, h(9, 10)).unwrap();
        let ts: Vec<&TimerDto> = s.grupos.iter().flat_map(|g| &g.timers).collect();
        assert_eq!(ts[0].motivo.as_deref(), Some("play manual"));
        assert_eq!(ts[1].motivo.as_deref(), Some("você lendo"));
        assert_eq!(s.rodando, 2);
        assert_eq!(s.lendo.as_deref(), Some("s2"));
    }

    #[test]
    fn snapshot_ignora_trechos_de_timer_arquivado_e_descartados() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#a00").unwrap();
        let arq = b.criar_timer(None, "Velho", "#b00").unwrap();
        for id in [t, arq] {
            b.aplicar(&[
                Mudanca::Abriu { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, inicio: h(8, 0), atividade_id: 1 },
                Mudanca::Fechou { timer_id: Some(id), sessao_id: None, origem: Origem::Manual, fim: h(8, 30), motivo: MotivoFim::Pausa },
            ]).unwrap();
        }
        // trecho de ontem nao entra
        b.aplicar(&[
            Mudanca::Abriu { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, inicio: h(0, 0) - 120 * MIN, atividade_id: 1 },
            Mudanca::Fechou { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, fim: h(0, 0) - 60 * MIN, motivo: MotivoFim::Pausa },
        ]).unwrap();
        let mut velho = b.timers().unwrap().into_iter().find(|x| x.id == arq).unwrap();
        velho.arquivado = true;
        b.editar_timer(&velho).unwrap();
        let s = montar(&b, &motor(h(9, 0)), h(9, 0)).unwrap();
        let ts: Vec<&TimerDto> = s.grupos.iter().flat_map(|g| &g.timers).collect();
        assert_eq!(ts.len(), 1);
        assert_eq!(ts[0].trechos.len(), 1);
        b.descartar(t, h(0, 0), h(23, 0)).unwrap();
        let s = montar(&b, &motor(h(9, 0)), h(9, 0)).unwrap();
        assert!(s.grupos[0].timers[0].trechos.is_empty());
    }

    #[test]
    fn snapshot_texto_da_pergunta_de_inatividade() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Cliente F", "#a00").unwrap();
        let mut m = motor(h(9, 0));
        m.perguntas.push((7, Pergunta::VoltaInatividade { timer_ids: vec![t], de: h(9, 9), ate: h(9, 35), atividades: Default::default() }));
        let s = montar(&b, &m, h(9, 35)).unwrap();
        assert_eq!(s.perguntas.len(), 1);
        let p = &s.perguntas[0];
        assert_eq!(p.id, 7);
        assert_eq!(p.tipo, "volta_inatividade");
        assert_eq!(p.sessao_id, None);
        assert_eq!(p.texto, "Você ficou 26 min sem mexer no PC. Retomar Cliente F?");
        assert_eq!(
            p.opcoes,
            vec![
                ("retomar".to_string(), "Retomar agora".to_string()),
                ("contar".to_string(), "Contar o tempo fora".to_string()),
                ("pausado".to_string(), "Deixar pausado".to_string()),
            ]
        );
    }

    #[test]
    fn pergunta_diz_a_atividade_que_nao_e_a_padrao() {
        let b = Banco::abrir_memoria().unwrap();
        let cliente_a = b.criar_timer(None, "Cliente A", "#a00").unwrap();
        let projeto_a = b.criar_timer(None, "Projeto A", "#b00").unwrap();
        let curso = b.criar_timer(None, "Curso", "#c00").unwrap();
        let mut c = b.timers().unwrap()[2].clone();
        c.atividade_padrao_id = ATIVIDADE_ESTUDO;
        b.editar_timer(&c).unwrap();
        let mut m = motor(h(9, 0));
        let atividades = [(cliente_a, ATIVIDADE_REUNIAO), (projeto_a, ATIVIDADE_TRABALHO), (curso, ATIVIDADE_ESTUDO)].into_iter().collect();
        m.perguntas.push((1, Pergunta::VoltaInatividade { timer_ids: vec![cliente_a, projeto_a, curso], de: h(9, 9), ate: h(9, 35), atividades }));
        let s = montar(&b, &m, h(9, 35)).unwrap();
        assert_eq!(s.perguntas[0].texto, "Você ficou 26 min sem mexer no PC. Retomar Cliente A (reunião), Projeto A e Curso?");
    }

    #[test]
    fn reuniao_manual_tem_etiqueta_motivo_e_atividade_nos_trechos() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Cliente A", "#a00").unwrap();
        let outro = b.criar_timer(None, "Projeto A", "#b00").unwrap();
        let mut m = motor(h(9, 0));
        let mud = m.play_atividade(t, ATIVIDADE_REUNIAO, h(9, 0));
        b.aplicar(&mud).unwrap();
        b.aplicar(&m.play(outro, h(9, 0))).unwrap();
        b.criar_ajuste(t, ATIVIDADE_REUNIAO, "2026-11-17", 15 * MIN, Some("antes do PC"), h(9, 1)).unwrap();
        let s = montar(&b, &m, h(9, 30)).unwrap();
        let ts: Vec<&TimerDto> = s.grupos.iter().flat_map(|g| &g.timers).collect();
        let cliente_a = ts.iter().find(|x| x.id == t).unwrap();
        assert_eq!(cliente_a.situacao.as_deref(), Some("reuniao"));
        assert_eq!(cliente_a.motivo.as_deref(), Some("reunião"));
        assert_eq!(cliente_a.manual_atividade_id, Some(ATIVIDADE_REUNIAO));
        assert_eq!(cliente_a.atividade_padrao_id, ATIVIDADE_TRABALHO);
        assert_eq!(cliente_a.trechos, vec![TrechoDto { inicio: h(9, 0), fim: None, origem: "manual".into(), atividade_id: ATIVIDADE_REUNIAO }]);
        assert_eq!(cliente_a.ajustes[0].atividade_id, ATIVIDADE_REUNIAO);
        let projeto_a = ts.iter().find(|x| x.id == outro).unwrap();
        assert_eq!((projeto_a.situacao.as_deref(), projeto_a.motivo.as_deref()), (Some("manual"), Some("play manual")), "o ▶ tem a etiqueta \"manual\" (rodada 13)");
        assert_eq!(s.atividades.iter().map(|a| a.nome.as_str()).collect::<Vec<_>>(), ["Trabalho", "Reunião", "Estudo"]);
        // o Claude trabalhando junto: a reunião segue na frente
        m.regras_pasta = vec![RegraPasta { id: 1, pasta: "G:\\Cliente A".into(), timer_id: t }];
        let ev = EventoHook { t: h(9, 40), evento: "UserPromptSubmit".into(), sessao: "s1".into(), cwd: "G:\\Cliente A".into(), transcript: None, notificacao: None };
        b.aplicar(&m.evento(&ev)).unwrap();
        let s = montar(&b, &m, h(9, 45)).unwrap();
        let cliente_a = s.grupos.iter().flat_map(|g| &g.timers).find(|x| x.id == t).unwrap();
        assert_eq!((cliente_a.situacao.as_deref(), cliente_a.motivo.as_deref()), (Some("reuniao"), Some("reunião · Claude trabalhando")));
    }

    #[test]
    fn snapshot_texto_da_pergunta_de_app_fechado() {
        let b = Banco::abrir_memoria().unwrap();
        let t1 = b.criar_timer(None, "A", "#a00").unwrap();
        let t2 = b.criar_timer(None, "B", "#b00").unwrap();
        let mut m = motor(h(9, 0));
        m.perguntas.push((1, Pergunta::AppFechado { timer_ids: vec![t1, t2], de: h(8, 0), ate: h(8, 40), atividades: Default::default() }));
        let p = &montar(&b, &m, h(9, 0)).unwrap().perguntas[0];
        assert_eq!(p.tipo, "app_fechado");
        assert_eq!(p.texto, format!("O {APP_NOME} ficou fechado 40 min com A e B ligados. Retomar?"));
        let valores: Vec<&str> = p.opcoes.iter().map(|o| o.0.as_str()).collect();
        assert_eq!(valores, ["retomar", "contar", "pausado"]);
    }

    #[test]
    fn snapshot_duracao_longa_e_contar_desde_o_corte() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Cliente F", "#a00").unwrap();
        let mut m = motor(h(9, 0));
        m.perguntas.push((1, Pergunta::VoltaInatividade { timer_ids: vec![t], de: h(9, 0), ate: h(17, 0), atividades: Default::default() }));
        // saiu ontem às 23:12, voltou hoje às 08:12
        m.perguntas.push((2, Pergunta::VoltaInatividade { timer_ids: vec![t], de: h(0, 0) - 48 * MIN, ate: h(8, 12), atividades: Default::default() }));
        m.perguntas.push((3, Pergunta::AppFechado { timer_ids: vec![t], de: h(0, 0) - 48 * MIN, ate: h(8, 12), atividades: Default::default() }));
        let s = montar(&b, &m, h(17, 0)).unwrap();
        let textos: Vec<&str> = s.perguntas.iter().map(|p| p.texto.as_str()).collect();
        assert_eq!(
            textos,
            [
                "Você ficou 8h 00min sem mexer no PC. Retomar Cliente F?",
                "Você ficou 9h 00min sem mexer no PC. Contar em Cliente F desde 00:00 (8h 12min)?",
                &format!("O {APP_NOME} ficou fechado 9h 00min com Cliente F ligado. Contar desde 00:00 (8h 12min)?"),
            ]
        );
        // depois da virada não há "Retomar agora" (o manual para na virada): "Deixar pausado" na
        // frente e "Contar desde 00:00"
        let valores = |p: &PerguntaDto| p.opcoes.iter().map(|o| (o.0.clone(), o.1.clone())).collect::<Vec<_>>();
        let virada = vec![("pausado".to_string(), "Deixar pausado".to_string()), ("contar".to_string(), "Contar desde 00:00".to_string())];
        assert_eq!(valores(&s.perguntas[1]), virada);
        assert_eq!(valores(&s.perguntas[2]), virada);
        assert_eq!(s.perguntas[0].opcoes[0].0, "retomar");
        // a ausência inteira de ontem (23:20–23:50) respondida hoje: só "Deixar pausado", sem "Contar"
        let mut m = motor(h(9, 0));
        m.perguntas.push((4, Pergunta::VoltaInatividade { timer_ids: vec![t], de: h(0, 0) - 40 * MIN, ate: h(0, 0) - 10 * MIN, atividades: Default::default() }));
        m.perguntas.push((5, Pergunta::AppFechado { timer_ids: vec![t], de: h(0, 0) - 40 * MIN, ate: h(0, 0) - 10 * MIN, atividades: Default::default() }));
        let s = montar(&b, &m, h(8, 10)).unwrap();
        let so = vec![("pausado".to_string(), "Deixar pausado".to_string())];
        assert_eq!((valores(&s.perguntas[0]), valores(&s.perguntas[1])), (so.clone(), so));
        assert_eq!(s.perguntas[0].texto, "Antes da virada do dia, você ficou 30 min sem mexer no PC com Cliente F ligado. O tempo daquele dia fica como está.");
        assert_eq!(s.perguntas[1].texto, format!("Antes da virada do dia, o {APP_NOME} ficou fechado 30 min com Cliente F ligado. O tempo daquele dia fica como está."));
    }

    #[test]
    fn snapshot_sessao_que_espera_ha_dias_aparece_parada_ou_some() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#a00").unwrap();
        let mut m = motor(h(9, 0));
        let sessao = |id: &str, terminou: Ms| Sessao {
            id: id.into(),
            cwd: "C:\\a".into(),
            nome: None,
            transcript: None,
            timer_id: Some(t),
            estado: EstadoSessao::EsperandoVoce,
            ultimo_evento: terminou,
            terminou_em: Some(terminou),
            criada_em: terminou,
            ligacao: None,
            desligada_em: None,
        };
        // de anteontem e de hoje cedo
        m.sessoes.insert("velha".into(), sessao("velha", h(0, 0) - 30 * 60 * MIN));
        m.sessoes.insert("hoje".into(), sessao("hoje", h(8, 0)));
        let s = montar(&b, &m, h(9, 0)).unwrap();
        let v: Vec<(&str, &str)> =
            s.grupos[0].timers[0].sessoes.iter().map(|x| (x.id.as_str(), x.estado.as_str())).collect();
        assert_eq!(v, [("hoje", "esperando_voce")]);
        // terminou ontem às 23:55: às 00:30 ainda espera você (menos de 1 h)
        m.sessoes.remove("hoje");
        m.sessoes.insert("ontem".into(), sessao("ontem", h(0, 0) - 5 * MIN));
        let s = montar(&b, &m, h(0, 30)).unwrap();
        let v: Vec<(&str, &str)> =
            s.grupos[0].timers[0].sessoes.iter().map(|x| (x.id.as_str(), x.estado.as_str())).collect();
        assert_eq!(v, [("ontem", "esperando_voce")]);
        assert!(montar(&b, &m, h(0, 55)).unwrap().grupos[0].timers[0].sessoes.is_empty());
        // a de anteontem, aberta e sendo lida agora, aparece (o dock mostra o nome)
        m.abertos.push(TrechoAberto { timer_id: Some(t), sessao_id: Some("velha".into()), origem: Origem::Leitura, inicio: h(9, 0), atividade_id: 1 });
        let s = montar(&b, &m, h(9, 1)).unwrap();
        let v: Vec<(&str, &str)> =
            s.grupos[0].timers[0].sessoes.iter().map(|x| (x.id.as_str(), x.estado.as_str())).collect();
        assert_eq!(v, [("velha", "esperando_voce")]);
        assert_eq!(s.lendo.as_deref(), Some("velha"));
    }

    fn ev(t: Ms, evento: &str, sessao: &str) -> EventoHook {
        EventoHook { t, evento: evento.into(), sessao: sessao.into(), cwd: "G:\\Projeto A".into(), transcript: None, notificacao: None }
    }
    fn situacao_de(s: &Snapshot, id: i64) -> Option<String> {
        s.grupos.iter().flat_map(|g| &g.timers).find(|t| t.id == id).unwrap().situacao.clone()
    }

    #[test]
    fn situacao_trabalhando_esperando_e_de_novo_trabalhando() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Projeto A", "#c00").unwrap();
        let mut m = motor(h(9, 0));
        m.regras_pasta = vec![RegraPasta { id: 1, pasta: "G:\\Projeto A".into(), timer_id: t }];
        b.aplicar(&m.evento(&ev(h(9, 0), "UserPromptSubmit", "s1"))).unwrap();
        assert_eq!(situacao_de(&montar(&b, &m, h(9, 5)).unwrap(), t).as_deref(), Some("trabalhando"));
        // terminou: o timer para, mas a etiqueta lembra que a sessão espera você
        b.aplicar(&m.evento(&ev(h(9, 8), "Stop", "s1"))).unwrap();
        let s = montar(&b, &m, h(9, 11)).unwrap();
        let timer = &s.grupos[0].timers[0];
        assert!(!timer.rodando);
        assert_eq!(timer.situacao.as_deref(), Some("esperando"));
        assert_eq!((timer.espera_desde, timer.espera_ultima), (Some(h(9, 8)), Some(h(9, 8))));
        // até a virada do dia (cada lugar da tela corta pelo seu tempo; rodada 13)
        assert_eq!(situacao_de(&montar(&b, &m, h(23, 59)).unwrap(), t).as_deref(), Some("esperando"));
        assert_eq!(situacao_de(&montar(&b, &m, ms_local(2026, 11, 18, 0, 1, F)).unwrap(), t), None, "depois da virada");
        // com uma tarefa em segundo plano pendente: "segundo plano", por até 2 h do fim do turno
        // a tarefa lançada às 9:08 vale até as 11:08
        m.segundo_plano.insert("s1".into(), h(11, 8));
        assert_eq!(situacao_de(&montar(&b, &m, h(11, 7)).unwrap(), t).as_deref(), Some("segundo_plano"));
        assert_eq!(situacao_de(&montar(&b, &m, h(11, 8)).unwrap(), t).as_deref(), Some("esperando"));
        m.segundo_plano.clear();
        // escrever nela: trabalhando de novo
        b.aplicar(&m.evento(&ev(h(9, 30), "UserPromptSubmit", "s1"))).unwrap();
        assert_eq!(situacao_de(&montar(&b, &m, h(9, 31)).unwrap(), t).as_deref(), Some("trabalhando"));
    }

    #[test]
    fn encerrado_tira_o_lembrete_e_a_sessao_aparece_parada() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Projeto A", "#c00").unwrap();
        let mut m = motor(h(9, 0));
        m.regras_pasta = vec![RegraPasta { id: 1, pasta: "G:\\Projeto A".into(), timer_id: t }];
        b.aplicar(&m.evento(&ev(h(9, 0), "UserPromptSubmit", "s1"))).unwrap();
        b.aplicar(&m.evento(&ev(h(9, 8), "Stop", "s1"))).unwrap();
        b.aplicar(&m.encerrar(t, h(9, 10))).unwrap();
        let s = montar(&b, &m, h(9, 11)).unwrap();
        let timer = &s.grupos[0].timers[0];
        assert!(timer.encerrado);
        assert!(!timer.rodando);
        assert_eq!(timer.situacao, None);
        assert_eq!(timer.espera_desde, None);
        let v: Vec<(&str, &str)> = timer.sessoes.iter().map(|x| (x.id.as_str(), x.estado.as_str())).collect();
        assert_eq!(v, [("s1", "parada")]);
        // o play volta ao normal
        b.aplicar(&m.play(t, h(9, 12))).unwrap();
        let s = montar(&b, &m, h(9, 13)).unwrap();
        assert!(!s.grupos[0].timers[0].encerrado);
        assert_eq!(s.grupos[0].timers[0].sessoes[0].estado, "esperando_voce");
    }

    #[test]
    fn situacao_lendo_vence_esperando_e_manual_tem_a_dele() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Projeto A", "#c00").unwrap();
        let manual = b.criar_timer(None, "Manual", "#d00").unwrap();
        let mut m = motor(h(9, 0));
        m.regras_pasta = vec![RegraPasta { id: 1, pasta: "G:\\Projeto A".into(), timer_id: t }];
        b.aplicar(&m.evento(&ev(h(9, 0), "UserPromptSubmit", "s1"))).unwrap();
        b.aplicar(&m.evento(&ev(h(9, 8), "Stop", "s1"))).unwrap();
        m.abertos.push(TrechoAberto { timer_id: Some(t), sessao_id: Some("s1".into()), origem: Origem::Leitura, inicio: h(9, 12), atividade_id: 1 });
        b.aplicar(&m.play(manual, h(9, 0))).unwrap();
        let s = montar(&b, &m, h(9, 15)).unwrap();
        assert_eq!(situacao_de(&s, t).as_deref(), Some("lendo"));
        assert_eq!(situacao_de(&s, manual).as_deref(), Some("manual"), "o ▶ tem etiqueta (rodada 13)");
        // lida há horas: segue "lendo" (a sessão lida espera você enquanto é lida)
        assert_eq!(situacao_de(&montar(&b, &m, h(13, 0)).unwrap(), t).as_deref(), Some("lendo"));
        assert!(!s.escrevendo);
        // com o foco na caixa de mensagem da conversa lida: "escrevendo", no timer e no topo
        m.escrevendo = true;
        let s = montar(&b, &m, h(9, 16)).unwrap();
        assert_eq!(situacao_de(&s, t).as_deref(), Some("escrevendo"));
        let timer = s.grupos.iter().flat_map(|g| &g.timers).find(|x| x.id == t).unwrap();
        assert_eq!(timer.motivo.as_deref(), Some("você escrevendo"));
        assert!(s.escrevendo);
        assert_eq!(situacao_de(&s, manual).as_deref(), Some("manual"));
        // a leitura fechou (você mandou a mensagem): nada de "escrevendo" sobrando
        m.abertos.retain(|a| a.origem != Origem::Leitura);
        let s = montar(&b, &m, h(9, 17)).unwrap();
        assert!(!s.escrevendo);
        assert_eq!(situacao_de(&s, t).as_deref(), Some("esperando"));
    }

    #[test]
    fn snapshot_traz_os_ajustes_do_dia_de_cada_timer_e_os_nomes_dos_links() {
        let b = Banco::abrir_memoria().unwrap();
        let g = b.criar_grupo("INV", "#111").unwrap();
        let t = b.criar_timer(Some(g), "Painel", "#a00").unwrap();
        let outro = b.criar_timer(Some(g), "Outro", "#b00").unwrap();
        b.criar_ajuste(t, 1, "2026-11-17", 15 * MIN, Some("reunião sem timer"), h(10, 0)).unwrap();
        b.criar_ajuste(t, 1, "2026-11-17", -5 * MIN, None, h(11, 0)).unwrap();
        b.criar_ajuste(t, 1, "2026-11-16", 60 * MIN, None, h(0, 0) - MIN).unwrap();
        let mut gr = b.grupos().unwrap()[0].clone();
        gr.card_padrao = Some("https://sprints.zoho.com/workspace/exemplo#P146/itemdetails/I7".into());
        gr.card_padrao_nome = Some("INV - Inovação".into());
        b.editar_grupo(&gr).unwrap();
        let mut tm = b.timers().unwrap()[0].clone();
        tm.link_card_nome = Some("Card do painel".into());
        b.editar_timer(&tm).unwrap();
        let s = montar(&b, &motor(h(12, 0)), h(12, 0)).unwrap();
        let painel = &s.grupos[0].timers[0];
        assert_eq!(painel.ajuste_ms, 10 * MIN, "só os de hoje");
        assert_eq!(
            painel.ajustes.iter().map(|a| (a.delta_ms, a.motivo.as_deref())).collect::<Vec<_>>(),
            [(15 * MIN, Some("reunião sem timer")), (-5 * MIN, None)]
        );
        assert_eq!(painel.link_card_nome.as_deref(), Some("Card do painel"));
        assert_eq!(s.grupos[0].card_padrao_nome.as_deref(), Some("INV - Inovação"));
        let o = s.grupos[0].timers.iter().find(|x| x.id == outro).unwrap();
        assert_eq!((o.ajuste_ms, o.ajustes.len()), (0, 0));
    }

    #[test]
    fn snapshot_sugere_ligar_ao_timer_com_nome_da_pasta() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Projeto A", "#a00").unwrap();
        let mut m = motor(h(9, 0));
        m.perguntas.push((3, Pergunta::SessaoNova {
            sessao_id: "s1".into(),
            cwd: "G:\\Empresa\\projeto a\\".into(),
            sugestao: "projeto a".into(),
            pasta_mae: false,
        }));
        m.perguntas.push((4, Pergunta::SessaoNova {
            sessao_id: "s2".into(),
            cwd: "D:/Clientes/Cliente D".into(),
            sugestao: "CLIENTE_D".into(),
            pasta_mae: false,
        }));
        let s = montar(&b, &m, h(9, 0)).unwrap();
        let p = &s.perguntas[0];
        assert_eq!(p.tipo, "sessao_nova");
        assert_eq!(p.sessao_id.as_deref(), Some("s1"));
        assert_eq!(p.texto, "Sessão nova do Claude na pasta Empresa\\projeto a. Em qual timer contar?");
        assert_eq!(
            p.opcoes,
            vec![
                (format!("ligar:{t}:regra"), "Projeto A · sempre nesta pasta".to_string()),
                ("outro".to_string(), "Outro timer…".to_string()),
                ("novo".to_string(), "+ Novo timer…".to_string()),
                ("nao".to_string(), "Não contar".to_string()),
            ]
        );
        let p = &s.perguntas[1];
        assert_eq!(p.texto, "Sessão nova do Claude na pasta Clientes\\Cliente D. Em qual timer contar?");
        let mut m = motor(h(9, 0));
        m.perguntas.push((5, Pergunta::SessaoNova {
            sessao_id: "s3".into(),
            cwd: "G:\\Empresa\\Flow\\.claude\\worktrees\\feat-029".into(),
            sugestao: "Flow".into(),
            pasta_mae: false,
        }));
        let s = montar(&b, &m, h(9, 0)).unwrap();
        assert_eq!(s.perguntas[0].texto, "Sessão nova do Claude na pasta Empresa\\Flow. Em qual timer contar?");
        // a pasta inteira (a do projeto) vai para a dica do dock; a curta, para o texto
        assert_eq!(s.perguntas[0].pasta.as_deref(), Some("G:\\Empresa\\Flow"));
        assert_eq!(s.perguntas[0].pasta_curta.as_deref(), Some("Empresa\\Flow"));
        // nenhum timer com o nome da pasta: sem sugestão
        assert_eq!(
            p.opcoes,
            vec![
                ("outro".to_string(), "Escolher timer…".to_string()),
                ("novo".to_string(), "+ Novo timer…".to_string()),
                ("nao".to_string(), "Não contar".to_string()),
            ]
        );
    }

    #[test]
    fn snapshot_leva_o_zerar_o_acumulado_de_antes_e_o_dia_continua_o_de_hoje() {
        let b = Banco::abrir_memoria().unwrap();
        let g = b.criar_grupo("Empresa", "#111").unwrap();
        let cliente_a = b.criar_timer(Some(g), "Cliente A", "#a00").unwrap();
        let flow = b.criar_timer(Some(g), "Flow", "#b00").unwrap();
        b.zerar(None, 0).unwrap();
        let ontem = |hh, mm| ms_local(2026, 11, 16, hh, mm, F);
        let manual = |t: i64, at: i64, i: Ms, f: Ms| {
            vec![
                Mudanca::Abriu { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, inicio: i, atividade_id: at },
                Mudanca::Fechou { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, fim: f, motivo: MotivoFim::Pausa },
            ]
        };
        // ontem: Cliente A 9:00–11:00 em Trabalho e 10:00–10:30 em Reunião; Flow 9:00–10:00
        b.aplicar(&manual(cliente_a, 1, ontem(9, 0), ontem(11, 0))).unwrap();
        b.aplicar(&manual(cliente_a, ATIVIDADE_REUNIAO, ontem(10, 0), ontem(10, 30))).unwrap();
        b.aplicar(&manual(flow, 1, ontem(9, 0), ontem(10, 0))).unwrap();
        // hoje: Cliente A 8:00–9:00 e um ajuste
        b.aplicar(&manual(cliente_a, 1, h(8, 0), h(9, 0))).unwrap();
        b.criar_ajuste(cliente_a, 1, "2026-11-17", 10 * MIN, None, h(9, 5)).unwrap();
        let s = montar(&b, &motor(h(9, 10)), h(9, 10)).unwrap();
        let t = |s: &Snapshot, id: i64| s.grupos[0].timers.iter().find(|x| x.id == id).unwrap().clone();
        let at = |t: &TimerDto| t.acumulado_antes.iter().map(|a| (a.atividade_id, a.ms)).collect::<Vec<_>>();
        assert_eq!(at(&t(&s, cliente_a)), [(1, 120 * MIN), (ATIVIDADE_REUNIAO, 30 * MIN)]);
        assert_eq!(at(&t(&s, flow)), [(1, 60 * MIN)]);
        assert_eq!(s.grupos[0].real_antes_ms, 120 * MIN, "ontem, a união: 9:00 às 11:00");
        assert_eq!(t(&s, cliente_a).zerado_em, 0);
        assert_eq!(t(&s, cliente_a).ajustes[0].criado_em, h(9, 5));
        // os trechos e os ajustes de hoje continuam os de hoje (o total do dia, na tela, é o de hoje)
        assert_eq!(t(&s, cliente_a).trechos.len(), 1);
        assert_eq!(t(&s, cliente_a).ajuste_ms, 10 * MIN);
        // zerado hoje às 8:30: nada de antes, e os trechos de hoje seguem todos no snapshot
        b.zerar(Some(&[cliente_a]), h(8, 30)).unwrap();
        let s = montar(&b, &motor(h(9, 10)), h(9, 10)).unwrap();
        assert!(t(&s, cliente_a).acumulado_antes.is_empty());
        assert_eq!((t(&s, cliente_a).zerado_em, t(&s, cliente_a).trechos.len()), (h(8, 30), 1));
        assert_eq!(s.grupos[0].real_antes_ms, 60 * MIN, "só o Flow");
    }

    /// Medida do snapshot com 30 dias de exemplo (12 timers em 3 grupos, ~60 trechos por dia,
    /// ajustes): sem o acumulado guardado (o primeiro, ou depois de mexer no passado) e com ele.
    /// `cargo test --release -p nucleo medir_snapshot -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn medir_snapshot_com_30_dias() {
        let d = tempfile::tempdir().unwrap();
        let b = Banco::abrir(&d.path().join("medida.db")).unwrap();
        let grupos: Vec<i64> = (0..3).map(|g| b.criar_grupo(&format!("G{g}"), "#111").unwrap()).collect();
        let timers: Vec<i64> =
            (0..12).map(|i| b.criar_timer(Some(grupos[i % 3]), &format!("T{i}"), "#a00").unwrap()).collect();
        b.zerar(None, 0).unwrap();
        let hoje = h(0, 0);
        let mut mud = vec![];
        let mut n = 0;
        for dia in 1..=30i64 {
            let ini = hoje - dia * 24 * 60 * MIN;
            for k in 0..60i64 {
                let t = timers[(k % 12) as usize];
                let i = ini + 8 * 60 * MIN + k * 9 * MIN;
                let (origem, sessao) = if k % 3 == 0 { (Origem::Manual, None) } else { (Origem::Claude, Some(format!("s{dia}-{k}"))) };
                let at = if k % 7 == 0 { ATIVIDADE_REUNIAO } else { 1 };
                mud.push(Mudanca::Abriu { timer_id: Some(t), sessao_id: sessao.clone(), origem, inicio: i, atividade_id: at });
                mud.push(Mudanca::Fechou { timer_id: Some(t), sessao_id: sessao, origem, fim: i + 25 * MIN, motivo: MotivoFim::Pausa });
                n += 1;
            }
            let dia_txt = dia_de(ini, 0, F).format("%Y-%m-%d").to_string();
            b.criar_ajuste(timers[(dia % 12) as usize], 1, &dia_txt, 15 * MIN, None, ini + 20 * 60 * MIN).unwrap();
        }
        b.aplicar(&mud).unwrap();
        let m = motor(h(10, 0));
        let medir = |rotulo: &str, vezes: u32, limpar: bool| {
            let mut total = std::time::Duration::ZERO;
            for _ in 0..vezes {
                if limpar {
                    b.restaurar(&[]).unwrap();
                }
                let t0 = std::time::Instant::now();
                let s = montar(&b, &m, h(10, 0)).unwrap();
                total += t0.elapsed();
                assert!(s.grupos.iter().flat_map(|g| &g.timers).all(|t| !t.acumulado_antes.is_empty()));
            }
            println!("{rotulo}: {:.2} ms por snapshot ({vezes} vezes)", total.as_secs_f64() * 1000.0 / vezes as f64);
        };
        println!("30 dias, {n} trechos, 30 ajustes, 12 timers em 3 grupos, zerados há mais de 30 dias");
        medir("sem o acumulado guardado", 50, true);
        medir("com o acumulado guardado", 500, false);
    }

    #[test]
    fn proximo_vencimento_do_esperando_voce_das_tarefas_e_da_virada() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Projeto A", "#c00").unwrap();
        let mut m = motor(h(9, 0));
        m.regras_pasta = vec![RegraPasta { id: 1, pasta: "G:\\Projeto A".into(), timer_id: t }];
        let cfg = ConfigApp::default(); // 30 min no recolhido, até a virada no app
        let virada = ms_local(2026, 11, 18, 0, 0, F);
        // sem ninguém esperando: a virada
        assert_eq!(proximo_vencimento(&m, &cfg, &[], h(9, 0)), virada);
        m.evento(&ev(h(9, 0), "UserPromptSubmit", "s1"));
        m.evento(&ev(h(9, 8), "Stop", "s1"));
        assert_eq!(proximo_vencimento(&m, &cfg, &[], h(9, 10)), h(9, 38), "30 min do recolhido");
        assert_eq!(proximo_vencimento(&m, &cfg, &[], h(9, 38)), virada, "venceu: o próximo é a virada");
        let cfg2 = ConfigApp { espera_dock_min: 15, espera_app_min: 120, ..ConfigApp::default() };
        assert_eq!(proximo_vencimento(&m, &cfg2, &[], h(9, 10)), h(9, 23));
        assert_eq!(proximo_vencimento(&m, &cfg2, &[], h(9, 30)), h(11, 8), "2 h do app");
        // a tarefa em segundo plano que vence antes
        m.segundo_plano.insert("s1".into(), h(10, 0));
        assert_eq!(proximo_vencimento(&m, &cfg2, &[], h(9, 30)), h(10, 0));
        // encerrado (■): nada da sessão
        m.encerrar(t, h(9, 31));
        m.segundo_plano.clear();
        assert_eq!(proximo_vencimento(&m, &cfg2, &[], h(9, 32)), virada);
    }

    #[test]
    fn pausado_fica_no_dock_recolhido_e_o_vencimento_dele() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Flow", "#a00").unwrap();
        let outro = b.criar_timer(None, "Cliente C", "#b00").unwrap();
        let mut m = motor(h(9, 0));
        let pausado = |b: &Banco, m: &Motor, agora: Ms, id: i64| {
            montar(b, m, agora).unwrap().grupos[0].timers.iter().find(|x| x.id == id).unwrap().pausado_em
        };
        b.aplicar(&m.play(t, h(9, 0))).unwrap();
        assert_eq!(pausado(&b, &m, h(9, 5), t), None, "rodando");
        b.aplicar(&m.pausar(t, h(9, 10))).unwrap();
        assert_eq!(pausado(&b, &m, h(9, 11), t), Some(h(9, 10)), "o ⏸");
        assert_eq!(pausados_de_hoje(&b, &m, h(9, 11)).unwrap(), [h(9, 10)]);
        // o vencimento: 10 min depois do ⏸ (o padrão); com 0, nada
        let cfg = ConfigApp::default();
        assert_eq!(proximo_vencimento(&m, &cfg, &[h(9, 10)], h(9, 11)), h(9, 20));
        let sai = ConfigApp { pausado_dock_min: 0, ..ConfigApp::default() };
        assert_eq!(proximo_vencimento(&m, &sai, &[h(9, 10)], h(9, 11)), ms_local(2026, 11, 18, 0, 0, F));
        // o ▶ de novo: não está mais pausado
        b.aplicar(&m.play(t, h(9, 12))).unwrap();
        assert_eq!(pausado(&b, &m, h(9, 13), t), None);
        // a troca de atividade (▶ ↔ 👥) vai como "pausa", mas o timer segue rodando
        b.aplicar(&m.play_atividade(t, ATIVIDADE_REUNIAO, h(9, 14))).unwrap();
        assert_eq!(pausado(&b, &m, h(9, 15), t), None);
        // o ■ não deixa pausado
        b.aplicar(&m.encerrar(t, h(9, 16))).unwrap();
        assert_eq!(pausado(&b, &m, h(9, 17), t), None, "encerrado");
        // a inatividade também deixa
        b.aplicar(&m.play(outro, h(9, 20))).unwrap();
        let a = crate::motor::Amostra { t: h(9, 31), ocioso_ms: 11 * MIN, ..Default::default() };
        b.aplicar(&m.amostra(&a)).unwrap();
        assert!(m.abertos.iter().all(|x| x.timer_id != Some(outro)), "pausou por inatividade");
        // ainda fora: nada no dock (revisão da rodada 14)
        assert_eq!(pausado(&b, &m, h(9, 32), outro), None);
        // sem o instante em memória (o app reaberto): o fim do trecho
        let mut sem = motor(h(9, 33));
        sem.encerrados.clear();
        assert_eq!(pausado(&b, &sem, h(9, 33), outro), Some(h(9, 20)));
    }

    #[test]
    fn pausado_por_inatividade_conta_da_volta() {
        // 25 min sem mexer com o prazo de 10 min: o pausado aparece na volta e vence 10 min depois
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Flow", "#a00").unwrap();
        let mut m = motor(h(9, 0));
        b.aplicar(&m.play(t, h(9, 0))).unwrap();
        let am = |t: Ms, ocioso: Ms| crate::motor::Amostra { t, ocioso_ms: ocioso, ..Default::default() };
        b.aplicar(&m.amostra(&am(h(9, 30), 10 * MIN))).unwrap(); // parado desde 9:20
        b.aplicar(&m.amostra(&am(h(9, 45), 25 * MIN))).unwrap();
        let pausado = |m: &Motor, agora: Ms| montar(&b, m, agora).unwrap().grupos[0].timers[0].pausado_em;
        assert_eq!(pausado(&m, h(9, 45)), None, "ainda fora");
        b.aplicar(&m.amostra(&am(h(9, 46), 0))).unwrap(); // voltou às 9:46
        assert_eq!(pausado(&m, h(9, 46)), Some(h(9, 46)), "a partir da volta, e não das 9:20");
        let cfg = ConfigApp::default();
        let p = pausados_de_hoje(&b, &m, h(9, 47)).unwrap();
        assert_eq!(p, [h(9, 46)]);
        assert_eq!(proximo_vencimento(&m, &cfg, &p, h(9, 47)), h(9, 56), "10 min depois da volta");
        // "Deixar pausado" às 9:50: o prazo recomeça da resposta
        let id = m.perguntas[0].0;
        b.aplicar(&m.responder(id, Resposta::DeixarPausado, h(9, 50))).unwrap();
        assert_eq!(pausado(&m, h(9, 51)), Some(h(9, 50)));
        // o ▶ tira; o ⏸ conta do clique
        b.aplicar(&m.play(t, h(10, 0))).unwrap();
        assert_eq!(pausado(&m, h(10, 1)), None);
        b.aplicar(&m.pausar(t, h(10, 5))).unwrap();
        assert_eq!(pausado(&m, h(10, 6)), Some(h(10, 5)));
        // o ■ tira
        b.aplicar(&m.play(t, h(10, 7))).unwrap();
        b.aplicar(&m.encerrar(t, h(10, 8))).unwrap();
        assert!(m.pausado_desde.is_empty());
    }

    #[test]
    fn situacao_do_claude_junto_do_manual() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Projeto A", "#c00").unwrap();
        let mut m = motor(h(9, 0));
        m.regras_pasta = vec![RegraPasta { id: 1, pasta: "G:\\Projeto A".into(), timer_id: t }];
        b.aplicar(&m.play(t, h(9, 0))).unwrap();
        let t0 = |m: &Motor, agora: Ms| {
            let s = montar(&b, m, agora).unwrap();
            let x = s.grupos[0].timers[0].clone();
            (x.situacao, x.situacao_claude)
        };
        assert_eq!(t0(&m, h(9, 1)), (Some("manual".into()), None));
        b.aplicar(&m.evento(&ev(h(9, 2), "UserPromptSubmit", "s1"))).unwrap();
        assert_eq!(t0(&m, h(9, 3)), (Some("trabalhando".into()), Some("trabalhando".into())));
        // o fim do Claude não pausa o ▶: "manual" e, do Claude, "esperando"
        b.aplicar(&m.evento(&ev(h(9, 5), "Stop", "s1"))).unwrap();
        assert_eq!(t0(&m, h(9, 6)), (Some("manual".into()), Some("esperando".into())));
        m.segundo_plano.insert("s1".into(), h(11, 0));
        assert_eq!(t0(&m, h(9, 7)).1.as_deref(), Some("segundo_plano"));
        // com a reunião manual
        b.aplicar(&m.play_atividade(t, ATIVIDADE_REUNIAO, h(9, 8))).unwrap();
        assert_eq!(t0(&m, h(9, 9)), (Some("reuniao".into()), Some("segundo_plano".into())));
    }
}
