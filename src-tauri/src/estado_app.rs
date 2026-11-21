// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Estado do app: banco e motor atrás de um único `Mutex` no `tauri::State`.
//! A lógica dos comandos mora aqui (sem Tauri) para ser testável.
use nucleo::idioma::{ta, tfa};
use std::collections::{BTreeSet, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

use chrono::{NaiveDate, NaiveDateTime};
use nucleo::banco::Banco;
use nucleo::config::{
    ConfigApp, ConfigMotor, AO_FECHAR_BANDEJA, AO_FECHAR_SAIR, AO_MINIMIZAR_BANDEJA, AO_MINIMIZAR_BARRA, AO_MINIMIZAR_DOCK,
    INATIVIDADE_MAX_S, TEMPO_MAX_S, TEMPO_MIN_S,
};
use nucleo::dados;
use nucleo::fim_do_transcript::{LeitorDoTranscript, TAREFA_MAX_MS};
use nucleo::modelo::{Atividade, Espaco, EstadoSessao, Grupo, Ligacao, Mudanca, Pergunta, Resposta, Timer};
use nucleo::motor::{chave_de_pasta, raiz_do_projeto, sugestao_de_pasta, Motor};
use nucleo::snapshot::{self, SessaoDto, Snapshot};
use nucleo::tempo::{dia_de, inicio_do_dia, proximo_corte, Ms, MIN};
use serde::Serialize;

pub const DIA_MS: Ms = 24 * 60 * MIN;
/// Sessões dos últimos dias carregadas no motor e listadas em "Ligar uma sessão".
pub const DIAS_RECENTES: Ms = 7;
/// O título de uma sessão ativa é conferido no máximo uma vez por minuto.
pub const RELER_TITULO_MS: Ms = 60_000;
/// Sessão "ativa" para o título: trabalhando, ou com evento há menos que isto.
pub const TITULO_ATIVA_MS: Ms = 5 * MIN;
/// Sessão trabalhando sem evento há isto: o app confere o fim do transcript, atrás de uma
/// interrupção (o stop na conversa não chama o hook `Stop`; rodada 13).
pub const CONFERIR_INTERRUPCAO_MS: Ms = 10_000;
/// O título de uma sessão parada (renomeada no app depois, por exemplo) é conferido com esta folga.
pub const RELER_TITULO_PARADA_MS: Ms = 10 * MIN;
/// Sem nada rodando, o `ultimo_tique` é gravado só de minuto em minuto.
/// A saída da conversa lida esperando confirmação (troca suave), gravada a cada mudança.
pub const CHAVE_SAIDA_LEITURA: &str = "saida_leitura";
pub const GRAVAR_TIQUE_MS: Ms = 60_000;
/// O motivo do ajuste à mão é curto.
pub const MOTIVO_MAX: usize = 80;
/// Nome de uma atividade (Ajustes → Atividades): até isto de letras.
pub const ATIVIDADE_NOME_MAX: usize = 40;
/// Nome de um grupo (revisão da rodada 24): até isto de letras, o mesmo limite do Renomear, do
/// "Novo grupo" e do "Editar grupo" (`GRUPO_NOME_MAX` no `editar.js`).
pub const GRUPO_NOME_MAX: usize = 60;

/// Tamanho e data de modificação do transcript: se não mudaram, o título também não.
type Assinatura = (u64, Option<std::time::SystemTime>);

pub struct Nucleo {
    pub banco: Banco,
    pub motor: Motor,
    pub fila_offset: u64,
    pub fuso_min: i32,
    /// Dia local em que o fuso foi lido; ao virar o dia o fuso é relido.
    pub fuso_dia: NaiveDate,
    /// Quando o título de cada sessão foi conferido no transcript (cache em memória).
    pub titulo_lido: HashMap<String, Ms>,
    /// Como estava o transcript na última leitura do título.
    pub titulo_arquivo: HashMap<String, Assinatura>,
    /// O que já foi lido do transcript de cada sessão (o stop na conversa e as tarefas em segundo
    /// plano; rodada 13): até onde e as tarefas pendentes. Lido para a frente, só quando muda.
    pub leitores: HashMap<String, LeitorDoTranscript>,
    /// O próximo instante em que uma etiqueta "esperando você" (ou "segundo plano") vence sem
    /// evento, ou a virada do dia (`snapshot::proximo_vencimento`): passado, a tela é avisada.
    pub vence_em: Option<Ms>,
    /// Último `ultimo_tique` gravado no banco.
    pub tique_gravado: Ms,
    /// Última saída da conversa lida gravada (`CHAVE_SAIDA_LEITURA`).
    pub saida_gravada: Option<Ms>,
    /// Sessões que esperavam você (`Motor::espera_recente`) no último ciclo.
    pub esperando: BTreeSet<String>,
    /// Hora (local) do backup mais recente; `None` = a pasta ainda não foi lida.
    pub ultimo_backup: Option<Option<NaiveDateTime>>,
    /// A troca do banco falhou e nem o banco de antes reabriu: o núcleo está num banco em memória
    /// e não grava nada (o laço para e a tela mostra o erro). Só sai reiniciando o app.
    pub quebrado: Option<String>,
}

fn com_sufixo(p: &Path, sufixo: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(sufixo);
    PathBuf::from(s)
}

/// Arquivos do SQLite de um banco: ele, o WAL e o índice do WAL.
fn arquivos_do_banco(banco: &Path) -> [PathBuf; 3] {
    [banco.to_path_buf(), com_sufixo(banco, "-wal"), com_sufixo(banco, "-shm")]
}

/// Move os arquivos do banco `de` para `para` (o WAL junto, para nada ficar para trás); os que
/// não existem são pulados. Falhando no meio, desfaz o que moveu.
fn mover_banco(de: &Path, para: &Path) -> std::io::Result<()> {
    let mut movidos: Vec<(PathBuf, PathBuf)> = vec![];
    for (a, b) in arquivos_do_banco(de).into_iter().zip(arquivos_do_banco(para)) {
        if !a.exists() {
            continue;
        }
        if let Err(e) = std::fs::rename(&a, &b) {
            for (a, b) in movidos.into_iter().rev() {
                let _ = std::fs::rename(&b, &a);
            }
            return Err(e);
        }
        movidos.push((a, b));
    }
    Ok(())
}

/// Apaga os arquivos do banco que existirem.
fn apagar_banco(p: &Path) -> std::io::Result<()> {
    for a in arquivos_do_banco(p) {
        match std::fs::remove_file(&a) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(e),
            _ => {}
        }
    }
    Ok(())
}

pub type Estado = Mutex<Nucleo>;

/// O que o "Descartar o tempo de hoje" (o antigo "Zerar hoje") descartou, para o "Desfazer".
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Descartado {
    pub trechos: Vec<i64>,
    pub ajustes: Vec<i64>,
}

/// O que um "Zerar" (ou o "Zerar todos") gravou, para o "Desfazer": o instante (`em`) e o
/// `zerado_em` de antes de cada timer, `[timer, antes]`.
#[derive(Clone, Debug, Default, PartialEq, Serialize, serde::Deserialize)]
pub struct Zerado {
    pub em: Ms,
    pub antes: Vec<(i64, Ms)>,
    /// O zerar de cada atividade que o zerar do timer todo tirou, `[timer, atividade, antes]`
    /// (rodada 19).
    #[serde(default)]
    pub limpas: Vec<(i64, i64, Ms)>,
    /// O zerar só de uma atividade (rodada 19): `[timer, atividade, o zerar dela de antes]`
    /// (nulo: não tinha). Com ele, `antes` fica vazio.
    #[serde(default)]
    pub atividade: Option<(i64, i64, Option<Ms>)>,
}

/// O nome de um espaço vai até isto (rodada 20; como o das atividades).
pub const ESPACO_NOME_MAX: usize = 24;

/// A recusa de apagar um espaço com grupos ou timers à vista (revisão da rodada 20: o mesmo texto
/// da dica do "Apagar" nos Ajustes), no idioma de agora (rodada 23).
#[allow(non_snake_case)]
pub fn MOVA_OS_GRUPOS() -> &'static str {
    ta("erro.mova_os_grupos")
}

/// A recusa de apagar ou arquivar o último espaço não arquivado.
#[allow(non_snake_case)]
pub fn ULTIMO_ESPACO() -> &'static str {
    ta("erro.ultimo_espaco")
}

/// O que um "Mover" mudou (rodada 21), para o "Desfazer": o grupo ou o timer e onde ele estava.
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct Movido {
    /// "grupo" ou "timer".
    pub tipo: String,
    pub id: i64,
    /// Antes do mover: o grupo do timer (nulo: sem grupo; num grupo, nulo também).
    pub grupo_id: Option<i64>,
    /// Antes do mover: o espaço do grupo, ou o do timer.
    pub espaco_id: i64,
    /// Depois do mover: o grupo do timer (nulo: sem grupo; num grupo, nulo também). O "Desfazer" só
    /// age se ele ainda está lá (revisão da rodada 21).
    #[serde(default)]
    pub para_grupo_id: Option<i64>,
    /// Depois do mover: o espaço do grupo, ou o do timer.
    #[serde(default)]
    pub para_espaco_id: i64,
}

/// A recusa do "Desfazer" quando o grupo ou o timer já mudou de lugar depois do mover.
#[allow(non_snake_case)]
pub fn DESFAZER_JA_MUDOU() -> &'static str {
    ta("erro.desfazer_ja_mudou")
}

/// A frase com a primeira letra minúscula (para entrar no meio de outra).
fn minuscula(frase: &str) -> String {
    let mut c = frase.chars();
    match c.next() {
        Some(p) => p.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

/// A recusa de ocultar a atividade padrão do timer no painel.
#[allow(non_snake_case)]
pub fn PADRAO_NAO_OCULTA() -> &'static str {
    ta("erro.padrao_nao_oculta")
}

/// Chave da config do banco com o corte do dia em que o "Zerar sozinho na virada" zerou pela
/// última vez (ms). Vazia com a opção desligada.
pub const CHAVE_ZERADO_AUTO: &str = "zerado_auto";

/// O "+ Novo timer…" da pergunta de sessão nova: o que a janela principal precisa para abrir o
/// "Novo timer" já preenchido e, ao salvar, ligar a sessão ao timer criado.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct PedidoNovoTimer {
    pub sessao_id: String,
    /// Nome sugerido: o último nome da pasta do projeto (num worktree, a raiz), como a sugestão
    /// da pergunta.
    pub nome: String,
    /// Onde a regra "sempre nesta pasta" fica: a raiz do projeto, como a regra de pasta grava.
    pub pasta: String,
    /// A pasta tem vários projetos dentro (rodada 15): "Sempre que for nesta pasta" vem
    /// desmarcado, com o aviso.
    pub pasta_mae: bool,
    /// A regra desta pasta já é de outro timer (o nome dele): a caixa vem desmarcada e diz que
    /// marcar passa a regra para o timer novo.
    pub outro_timer_nome: Option<String>,
    /// O "Sempre que for na pasta" da pergunta de sessão nova (rodada 27): marcado (com a pasta
    /// escolhida em `pasta`) ou não. Nulo (o "+ Novo timer…" de outro lugar): como antes, marcado
    /// a não ser numa pasta-mãe ou com a regra de outro timer.
    pub sempre: Option<bool>,
    /// A `pasta` pode ter a regra do "sempre nesta pasta": é uma das pastas da sessão
    /// (`pastas_da_sessao`) e a conferência não deu erro (a pasta do usuário, a raiz de um drive,
    /// uma pasta que nunca conta, o Chat). Sem isso, o "Novo timer" não mostra a caixa e só liga a
    /// sessão (revisão da rodada 27: o timer criado não fica sem a sessão ligada).
    pub pode_regra: bool,
}

/// A conferência de uma regra de pasta antes de gravar (rodada 15), com o nome do timer que já
/// tem a regra desta pasta.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConferenciaRegra {
    #[serde(flatten)]
    pub conferencia: nucleo::pastas::Conferencia,
    pub outro_timer_nome: Option<String>,
}

/// Regra de pasta como a tela de Ajustes mostra.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RegraDto {
    pub id: i64,
    pub pasta: String,
    pub timer_id: i64,
    pub timer_nome: String,
}

/// A trava nunca dá pânico: se outra thread caiu segurando-a, segue com o estado como está.
pub fn trava(e: &Estado) -> MutexGuard<'_, Nucleo> {
    e.lock().unwrap_or_else(|p| p.into_inner())
}

pub fn agora_ms() -> Ms {
    chrono::Utc::now().timestamp_millis()
}

/// Fuso do Windows em minutos (ex.: -180) e o dia local de hoje.
pub fn fuso_local() -> (i32, NaiveDate) {
    let agora = chrono::Local::now();
    (agora.offset().local_minus_utc() / 60, agora.date_naive())
}

pub fn texto<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// A resposta de uma pergunta: "contar", "retomar", "pausado", "nao", "certo", "ligar:<timer>"
/// (só esta sessão), "ligar:<timer>:regra" (a regra na pasta da sessão) ou, desde a rodada 27,
/// "ligar:<timer>:pasta:<caminho>" (a regra numa das pastas da sessão: `pasta_pedida`).
fn resposta_de(valor: &str) -> Result<Resposta, String> {
    match valor {
        "contar" => Ok(Resposta::Contar),
        "retomar" => Ok(Resposta::RetomarAgora),
        "pausado" => Ok(Resposta::DeixarPausado),
        "nao" => Ok(Resposta::NaoContar),
        "certo" => Ok(Resposta::Certo),
        _ => {
            let desconhecida = || format!("resposta desconhecida: {valor}");
            let resto = valor.strip_prefix("ligar:").ok_or_else(desconhecida)?;
            // o caminho tem ":" (`D:\…`): só o primeiro separa o timer do resto
            let (id, cauda) = resto.split_once(':').unwrap_or((resto, ""));
            let criar_regra = match cauda {
                "" => false,
                "regra" => true,
                _ if pasta_pedida(valor).is_some() => true,
                _ => return Err(desconhecida()),
            };
            Ok(Resposta::LigarA { timer_id: id.parse().map_err(texto)?, criar_regra })
        }
    }
}

/// O caminho de "ligar:<timer>:pasta:<caminho>" (rodada 27), se for essa a resposta.
fn pasta_pedida(valor: &str) -> Option<&str> {
    let (_, cauda) = valor.strip_prefix("ligar:")?.split_once(':')?;
    cauda.strip_prefix("pasta:").map(str::trim).filter(|p| !p.is_empty())
}

/// Grava o arquivo inteiro de uma vez (temporário + rename).
fn gravar_atomico(caminho: &Path, conteudo: &[u8]) -> Result<(), String> {
    if let Some(pai) = caminho.parent() {
        std::fs::create_dir_all(pai).map_err(texto)?;
    }
    let mut tmp = caminho.as_os_str().to_owned();
    tmp.push(".tmp");
    let tmp = PathBuf::from(tmp);
    std::fs::write(&tmp, conteudo).map_err(texto)?;
    std::fs::rename(&tmp, caminho).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        texto(e)
    })
}

/// O `hook.json`: as pastas que o hook nem registra.
fn gravar_hook_json(cfg: &ConfigApp, hook_json: &Path) -> Result<(), String> {
    let mut j =
        serde_json::to_vec_pretty(&serde_json::json!({ "pastas_ignoradas": cfg.pastas_ignoradas })).map_err(texto)?;
    j.push(b'\n');
    gravar_atomico(hook_json, &j)
}

impl Nucleo {
    /// Monta o estado a partir do banco: config, regras, sessões recentes e reabertura
    /// (o que ficou aberto fecha no último tique e o motor pergunta pelos manuais).
    pub fn abrir(banco: Banco, agora: Ms, fuso_min: i32, fuso_dia: NaiveDate) -> Result<Nucleo, String> {
        let cfg = banco.config_app().map_err(texto)?;
        nucleo::idioma::usar(nucleo::idioma::da_escolha(&cfg.idioma));
        let mut motor = Motor::novo(ConfigMotor::de(&cfg, fuso_min), agora);
        motor.regras_pasta = banco.regras_pasta().map_err(texto)?;
        motor.atividades_padrao = banco.atividades_padrao().map_err(texto)?;
        // o encerrar (■) de hoje continua valendo depois de reabrir; o de outro dia, não
        if let Some((dia, timers)) = banco.encerrados().map_err(texto)? {
            motor.restaurar_encerrados(&dia, &timers);
        }
        let mut mud = vec![];
        for mut s in banco.sessoes_desde(agora - DIAS_RECENTES * DIA_MS).map_err(texto)? {
            // O trecho dela fecha no ao_reabrir; o próximo evento a religa.
            if s.estado == EstadoSessao::Trabalhando {
                s.estado = EstadoSessao::Parada;
                mud.push(Mudanca::Sessao(s.clone()));
            }
            // ligada a um timer antes de abrir: a primeira mensagem dela nao e de sessao nova (sem o
            // aviso "Ligada a X pela pasta" de novo). A sem timer por decisao sua ("Nao contar" ou
            // desligada) tambem nao pergunta de novo depois de reabrir.
            if s.timer_id.is_some() || s.ligacao == Some(Ligacao::Mao) {
                motor.perguntadas.insert(s.id.clone());
            }
            motor.sessoes.insert(s.id.clone(), s);
        }
        let fila_offset = banco.valor("fila_offset").map_err(texto)?.and_then(|v| v.parse().ok()).unwrap_or(0);
        let abertos = banco.abertos().map_err(texto)?;
        let ultimo_tique = banco
            .valor("ultimo_tique")
            .map_err(texto)?
            .and_then(|v| v.parse::<Ms>().ok())
            .or_else(|| abertos.iter().map(|t| t.inicio).max())
            .unwrap_or(agora);
        // a saida da conversa lida que esperava confirmacao quando o app fechou (troca suave)
        let saida_leitura = banco.valor(CHAVE_SAIDA_LEITURA).map_err(texto)?.and_then(|v| v.parse::<Ms>().ok());
        mud.extend(motor.ao_reabrir(abertos, ultimo_tique, saida_leitura, agora));
        // a conversa continuada que a regra de pasta ligou antes de existir a herança (ou com o app
        // fechado antes de o título chegar) herda agora o timer da anterior, ligada à mão
        let validos: BTreeSet<i64> = banco.timers().map_err(texto)?.iter().map(|t| t.id).collect();
        let pela_regra: Vec<String> = motor
            .sessoes
            .values()
            .filter(|s| s.ligacao == Some(Ligacao::Regra) && s.nome.is_some())
            .map(|s| s.id.clone())
            .collect();
        for id in pela_regra {
            mud.extend(motor.herdar(&id, agora, false, |t| validos.contains(&t)));
        }
        banco.aplicar(&mud).map_err(texto)?;
        banco.gravar_valor("ultimo_tique", &agora.to_string()).map_err(texto)?;
        banco.gravar_valor(CHAVE_SAIDA_LEITURA, "").map_err(texto)?;
        let mut n = Nucleo {
            banco,
            motor,
            fila_offset,
            fuso_min,
            fuso_dia,
            titulo_lido: HashMap::new(),
            titulo_arquivo: HashMap::new(),
            leitores: HashMap::new(),
            vence_em: None,
            tique_gravado: agora,
            saida_gravada: None,
            esperando: BTreeSet::new(),
            ultimo_backup: None,
            quebrado: None,
        };
        // com o "Zerar sozinho na virada" ligado e o app fechado num corte: vale o mais recente
        n.zerar_na_virada(agora)?;
        Ok(n)
    }

    /// Erro se o núcleo ficou sem banco (`quebrado`): nada é lido nem gravado.
    pub fn checar_banco(&self) -> Result<(), String> {
        match &self.quebrado {
            Some(e) => Err(tfa("erro.banco_nao_abriu", &[e])),
            None => Ok(()),
        }
    }

    // ---- banco: trocar e backups ----

    /// Troca o arquivo do banco (`banco`) por `novo` e reabre tudo (motor, sessões, perguntas)
    /// a partir dele, sem reiniciar o app. Nada é apagado antes de o novo abrir:
    /// 1. o novo é conferido (`PRAGMA quick_check`);
    /// 2. o atual (com o WAL) vira `worktimer.db.anterior`;
    /// 3. o novo vai para o lugar e é aberto.
    ///
    /// Se o 2 ou o 3 falhar, tudo volta e o banco de antes reabre (o erro vai para a tela). Se nem
    /// ele reabrir, o núcleo fica `quebrado`: não grava nada e a tela mostra o erro.
    pub fn trocar_banco(&mut self, novo: &Path, banco: &Path, agora: Ms, fuso_min: i32, dia: NaiveDate) -> Result<(), String> {
        self.trocar_banco_com(novo, banco, &mut |p: &Path| {
            Banco::abrir(p).map_err(texto).and_then(|b| Nucleo::abrir(b, agora, fuso_min, dia))
        })
    }

    /// `trocar_banco` com o "abrir" de fora (os testes simulam a falha).
    pub fn trocar_banco_com(
        &mut self,
        novo: &Path,
        banco: &Path,
        abrir: &mut dyn FnMut(&Path) -> Result<Nucleo, String>,
    ) -> Result<(), String> {
        self.checar_banco()?;
        dados::conferir_banco(novo)?;
        let anterior = com_sufixo(banco, ".anterior");
        // fecha a conexão com o arquivo (o SQLite passa o WAL para o banco)
        drop(std::mem::replace(&mut self.banco, Banco::abrir_memoria().map_err(texto)?));
        let erro = match apagar_banco(&anterior).and_then(|_| mover_banco(banco, &anterior)) {
            Err(e) => tfa("erro.guardar_banco", &[&e.to_string()]),
            Ok(()) => match std::fs::rename(novo, banco) {
                Err(e) => tfa("erro.por_banco", &[&e.to_string()]),
                Ok(()) => match abrir(banco) {
                    Ok(n) => {
                        *self = n;
                        return Ok(());
                    }
                    Err(e) => {
                        // o novo não abriu: volta para o lugar dele (para conferir) e sai; sem
                        // conseguir, vai para ".falhou", para o de antes poder voltar
                        let _ = apagar_banco(novo);
                        if mover_banco(banco, novo).is_err() {
                            let falhou = com_sufixo(banco, ".falhou");
                            let _ = apagar_banco(&falhou);
                            let _ = mover_banco(banco, &falhou);
                        }
                        tfa("erro.banco_novo", &[&e.to_string()])
                    }
                },
            },
        };
        // desfaz: o de antes volta para o lugar (se saiu) e reabre
        if anterior.exists() && !banco.exists() {
            let _ = mover_banco(&anterior, banco);
        }
        match abrir(banco) {
            Ok(n) => *self = n,
            Err(e) => {
                self.quebrado = Some(tfa("erro.de_antes_tambem", &[&erro, &e.to_string()]));
                return Err(tfa("erro.de_antes_tambem", &[&erro, &e.to_string()]));
            }
        }
        Err(tfa("erro.nada_mudou", &[&erro]))
    }

    /// Backup automático: na primeira abertura do dia (`abertura`) e a cada 6 h. Sem timers
    /// (primeiro uso) não há o que guardar. Mantém os 14 mais recentes.
    pub fn backup_se_devido(&mut self, dir: &Path, agora: NaiveDateTime, abertura: bool) -> Result<Option<PathBuf>, String> {
        self.checar_banco()?;
        if self.ultimo_backup.is_none() || abertura {
            self.ultimo_backup = Some(dados::listar(dir).first().map(|b| b.momento));
        }
        if !dados::backup_devido(self.ultimo_backup.flatten(), agora, abertura) || !self.banco.tem_timers().map_err(texto)? {
            return Ok(None);
        }
        // falhando, a próxima tentativa é daqui a 6 h (ou na próxima abertura de outro dia)
        self.ultimo_backup = Some(Some(agora));
        let p = dados::fazer_backup(&self.banco, dir, agora)?;
        dados::rotacionar(dir, dados::MANTER_BACKUPS, &[]);
        Ok(Some(p))
    }

    /// "Fazer backup agora" (Ajustes).
    pub fn backup_agora(&mut self, dir: &Path, agora: NaiveDateTime) -> Result<PathBuf, String> {
        self.checar_banco()?;
        let p = dados::fazer_backup(&self.banco, dir, agora)?;
        self.ultimo_backup = Some(Some(agora));
        dados::rotacionar(dir, dados::MANTER_BACKUPS, &[]);
        Ok(p)
    }

    /// "Restaurar" um backup da pasta, sem reiniciar o app:
    /// 1. copia o escolhido (antes de tudo: a rotação nunca o alcança);
    /// 2. faz um backup do estado de agora, fora da rotação;
    /// 3. troca o banco (`trocar_banco`);
    /// 4. só então roda a rotação, sem apagar o escolhido nem o de antes de restaurar.
    ///
    /// A leitura das filas continua de onde está (os eventos já contados não voltam a contar), e
    /// o "Iniciar com o Windows" fica como está agora (é o registro que manda).
    #[allow(clippy::too_many_arguments)]
    pub fn restaurar_backup(
        &mut self,
        dir: &Path,
        nome: &str,
        banco: &Path,
        local: NaiveDateTime,
        agora: Ms,
        fuso_min: i32,
        dia: NaiveDate,
    ) -> Result<(), String> {
        if dados::ler_nome(nome).is_none() {
            return Err(tfa("erro.backup_desconhecido", &[nome]));
        }
        let origem = dir.join(nome);
        if !origem.is_file() {
            return Err(ta("erro.backup_sumiu").into());
        }
        self.checar_banco()?;
        let novo = banco.parent().unwrap_or(Path::new(".")).join("restaurando.db");
        let _ = apagar_banco(&novo);
        dados::copiar_backup(&origem, &novo)?;
        let antes = match dados::fazer_backup(&self.banco, dir, local) {
            Ok(p) => p,
            Err(e) => {
                let _ = apagar_banco(&novo);
                return Err(tfa("erro.guardar_antes_restaurar", &[&e.to_string()]));
            }
        };
        let preparar = || -> Result<(), String> {
            let b = Banco::abrir(&novo).map_err(texto)?;
            b.gravar_valor("fila_offset", &self.fila_offset.to_string()).map_err(texto)?;
            let mut cfg = b.config_app().map_err(texto)?;
            cfg.iniciar_com_windows = self.banco.config_app().map_err(texto)?.iniciar_com_windows;
            b.salvar_config_app(&cfg).map_err(texto)
        };
        if let Err(e) = preparar() {
            let _ = apagar_banco(&novo);
            return Err(e);
        }
        self.trocar_banco(&novo, banco, agora, fuso_min, dia)?;
        self.ultimo_backup = Some(Some(local));
        let antes = antes.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        dados::rotacionar(dir, dados::MANTER_BACKUPS, &[nome, &antes]);
        Ok(())
    }

    /// Grava o `hook.json` (as pastas ignoradas que o hook lê) com a config do banco.
    pub fn gravar_hook_json(&self, hook_json: &Path) -> Result<(), String> {
        let cfg = self.banco.config_app().map_err(texto)?;
        gravar_hook_json(&cfg, hook_json)
    }

    /// As sessões que esperam você mudaram desde o último ciclo? O "esperando você" do dock vence
    /// 1 h depois de a sessão terminar, sem evento nenhum: sem isto a tela só saberia no próximo.
    pub fn esperas_mudaram(&mut self, agora: Ms) -> bool {
        // o tempo de um "esperando você" (recolhido, expandido e app) ou de uma tarefa em segundo
        // plano venceu, ou virou o dia: a tela precisa tirar a etiqueta (revisão da rodada 13)
        let venceu = self.vence_em.is_some_and(|v| v <= agora);
        let cfg = self.banco.config_app().unwrap_or_default();
        // e os timers pausados no dock recolhido (rodada 14)
        let pausados = snapshot::pausados_de_hoje(&self.banco, &self.motor, agora).unwrap_or_default();
        self.vence_em = Some(snapshot::proximo_vencimento(&self.motor, &cfg, &pausados, agora));
        let m = &self.motor;
        let agora_esperando: BTreeSet<String> = m
            .sessoes
            .values()
            .filter(|s| s.timer_id.is_some() && m.espera_recente(s, agora))
            .map(|s| s.id.clone())
            .collect();
        if agora_esperando == self.esperando {
            return venceu;
        }
        self.esperando = agora_esperando;
        true
    }

    /// Grava o `ultimo_tique` (onde os trechos abertos fecham se o app cair): a cada ciclo
    /// enquanto algo roda; parado, só de minuto em minuto. Grava também, quando muda, a saída da
    /// conversa lida que espera confirmação (troca suave): fechando o app no meio da espera, a
    /// leitura fecha nela ao reabrir, e não no último tique.
    pub fn gravar_tique(&mut self, agora: Ms) -> Result<(), String> {
        let saida = self.motor.saida_esperando();
        if saida != self.saida_gravada {
            let v = saida.map(|s| s.to_string()).unwrap_or_default();
            self.banco.gravar_valor(CHAVE_SAIDA_LEITURA, &v).map_err(texto)?;
            self.saida_gravada = saida;
        }
        if self.motor.abertos.is_empty() && (agora - self.tique_gravado).abs() < GRAVAR_TIQUE_MS {
            return Ok(());
        }
        self.banco.gravar_valor("ultimo_tique", &agora.to_string()).map_err(texto)?;
        self.tique_gravado = agora;
        Ok(())
    }

    pub fn aplicar(&self, mud: &[Mudanca]) -> Result<(), String> {
        if mud.is_empty() {
            return Ok(());
        }
        self.checar_banco()?;
        self.banco.aplicar(mud).map_err(texto)
    }

    pub fn snapshot(&self, agora: Ms) -> Result<Snapshot, String> {
        self.checar_banco()?;
        let s = snapshot::montar(&self.banco, &self.motor, agora).map_err(texto)?;
        // o idioma do estado vale para o resto do app: a bandeja, os títulos e os erros (rodada 23)
        nucleo::idioma::usar(if s.idioma == nucleo::idioma::IDIOMA_EN { nucleo::idioma::Idioma::En } else { nucleo::idioma::Idioma::Pt });
        Ok(s)
    }

    /// Timers rodando agora (distintos).
    pub fn rodando(&self) -> usize {
        let mut ids: Vec<i64> = self.motor.abertos.iter().filter_map(|t| t.timer_id).collect();
        ids.sort_unstable();
        ids.dedup();
        ids.len()
    }

    /// Relê o fuso quando o dia local muda (horário de verão etc.).
    pub fn atualizar_fuso(&mut self, fuso_min: i32, dia: NaiveDate) {
        if dia != self.fuso_dia {
            self.fuso_dia = dia;
            self.fuso_min = fuso_min;
            self.motor.cfg.fuso_min = fuso_min;
        }
    }

    pub fn checar_timer(&self, id: i64) -> Result<(), String> {
        if self.banco.timers().map_err(texto)?.iter().any(|t| t.id == id) {
            Ok(())
        } else {
            Err(ta("erro.timer_nao_encontrado").into())
        }
    }

    /// [início, fim) do dia de trabalho de `agora`.
    pub fn limites_do_dia(&self, agora: Ms) -> (Ms, Ms) {
        let (corte, fuso) = (self.motor.cfg.corte_min, self.motor.cfg.fuso_min);
        (inicio_do_dia(dia_de(agora, corte, fuso), corte, fuso), proximo_corte(agora, corte, fuso))
    }

    /// Dia de trabalho de `agora` em AAAA-MM-DD (o dos ajustes à mão).
    fn dia_texto(&self, agora: Ms) -> String {
        let (corte, fuso) = (self.motor.cfg.corte_min, self.motor.cfg.fuso_min);
        dia_de(agora, corte, fuso).format("%Y-%m-%d").to_string()
    }

    /// "Descartar o tempo de hoje…" (no "Ajustar hoje"; era o ↺ "Zerar hoje"): pausa o timer (e
    /// grava) antes de descartar o dia, para nunca descartar trecho aberto. Descarta os trechos e
    /// os ajustes à mão de hoje do timer; devolve os ids de cada um para `desfazer_descartar`.
    pub fn descartar_hoje(&mut self, timer_id: i64, agora: Ms) -> Result<Descartado, String> {
        self.checar_banco()?;
        self.checar_timer(timer_id)?;
        let mud = self.motor.pausar(timer_id, agora);
        self.aplicar(&mud)?;
        let (de, ate) = self.limites_do_dia(agora);
        let trechos = self.banco.descartar(timer_id, de, ate).map_err(texto)?;
        let ajustes = self.banco.descartar_ajustes(timer_id, &self.dia_texto(agora)).map_err(texto)?;
        Ok(Descartado { trechos, ajustes })
    }

    pub fn desfazer_descartar(&mut self, trechos: &[i64], ajustes: &[i64]) -> Result<(), String> {
        self.banco.restaurar(trechos).map_err(texto)?;
        self.banco.restaurar_ajustes(ajustes).map_err(texto)
    }

    /// ↺ "Zerar": o número do timer volta a 0:00 a partir de agora. Só grava o `zerado_em`; nada é
    /// apagado, e o timer rodando continua rodando.
    pub fn zerar(&mut self, timer_id: i64, agora: Ms) -> Result<Zerado, String> {
        self.checar_banco()?;
        self.checar_timer(timer_id)?;
        let (antes, limpas) = self.banco.zerar_com_atividades(Some(&[timer_id]), agora).map_err(texto)?;
        Ok(Zerado { em: agora, antes, limpas, atividade: None })
    }

    /// ↺ "Só <atividade>" (rodada 19): o número só daquela atividade do timer volta a 0:00 a partir
    /// de agora; as outras seguem contando, e o timer (a soma delas) cai só o dela. Nada é apagado:
    /// o total do dia e a linha do tempo ficam como estão.
    pub fn zerar_atividade(&mut self, timer_id: i64, atividade_id: i64, agora: Ms) -> Result<Zerado, String> {
        self.checar_banco()?;
        self.checar_timer(timer_id)?;
        if !self.banco.atividades().map_err(texto)?.iter().any(|a| a.id == atividade_id) {
            return Err(ta("erro.atividade_nao_encontrada").into());
        }
        let antes = self.banco.zerar_atividade(timer_id, atividade_id, agora).map_err(texto)?;
        Ok(Zerado { em: agora, antes: vec![], limpas: vec![], atividade: Some((timer_id, atividade_id, antes)) })
    }

    /// "Zerar todos" (barra do topo): o mesmo instante em todos os timers, os arquivados também.
    /// Com `espaco` (a aba de um espaço, rodada 20), só os timers dele.
    pub fn zerar_todos(&mut self, espaco: Option<i64>, agora: Ms) -> Result<Zerado, String> {
        self.checar_banco()?;
        let ids = self.timers_do_espaco(espaco)?;
        let (antes, limpas) = self.banco.zerar_com_atividades(ids.as_deref(), agora).map_err(texto)?;
        Ok(Zerado { em: agora, antes, limpas, atividade: None })
    }

    /// Os timers do espaço (os arquivados também), ou nulo sem espaço (todos).
    fn timers_do_espaco(&self, espaco: Option<i64>) -> Result<Option<Vec<i64>>, String> {
        let Some(e) = espaco else { return Ok(None) };
        self.checar_espaco(e, false)?;
        let mapa = self.banco.espacos_dos_timers().map_err(texto)?;
        Ok(Some(mapa.into_iter().filter(|(_, x)| *x == e).map(|(t, _)| t).collect()))
    }

    /// "Pausar tudo" (rodada 20: o da aba). Sem espaço, tudo, como antes; com espaço, só os timers
    /// dele.
    pub fn pausar_tudo(&mut self, espaco: Option<i64>, agora: Ms) -> Result<Vec<Mudanca>, String> {
        match self.timers_do_espaco(espaco)? {
            None => Ok(self.motor.pausar_tudo(agora)),
            Some(ids) => Ok(self.motor.pausar_timers(&ids, agora)),
        }
    }

    // ---- mover entre espaços (rodada 21) ----

    /// "Mover para outro espaço…" do grupo: o grupo e os timers dele passam a contar no espaço
    /// `espaco_id`, com todo o histórico (nenhum trecho muda; os totais dos espaços são pela
    /// posição de agora). Devolve o de antes, para o "Desfazer".
    pub fn mover_grupo(&mut self, grupo_id: i64, espaco_id: i64) -> Result<Movido, String> {
        self.checar_banco()?;
        self.checar_espaco(espaco_id, true)?;
        let g = self.banco.grupos().map_err(texto)?.into_iter().find(|g| g.id == grupo_id).ok_or(ta("erro.grupo_nao_encontrado"))?;
        self.banco.mover_grupo(grupo_id, espaco_id).map_err(texto)?;
        Ok(Movido { tipo: "grupo".into(), id: grupo_id, grupo_id: None, espaco_id: g.espaco_id, para_grupo_id: None, para_espaco_id: espaco_id })
    }

    /// "Mover para…" do timer: para o grupo `grupo_id` (de qualquer espaço) ou, sem grupo, para o
    /// "Sem grupo" do espaço `espaco_id`. O histórico vai junto. Devolve o de antes.
    pub fn mover_timer(&mut self, timer_id: i64, grupo_id: Option<i64>, espaco_id: i64) -> Result<Movido, String> {
        self.checar_banco()?;
        let t = self.banco.timers().map_err(texto)?.into_iter().find(|t| t.id == timer_id).ok_or(ta("erro.timer_nao_encontrado"))?;
        let antes = Movido {
            tipo: "timer".into(),
            id: timer_id,
            grupo_id: t.grupo_id,
            espaco_id: self.banco.espacos_dos_timers().map_err(texto)?.get(&timer_id).copied().unwrap_or(t.espaco_id),
            para_grupo_id: None,
            para_espaco_id: 0,
        };
        let espaco = match grupo_id {
            Some(g) => {
                let grupo = self.banco.grupos().map_err(texto)?.into_iter().find(|x| x.id == g).ok_or(ta("erro.grupo_nao_encontrado"))?;
                self.checar_espaco(grupo.espaco_id, true)?;
                grupo.espaco_id
            }
            None => {
                self.checar_espaco(espaco_id, true)?;
                espaco_id
            }
        };
        self.banco.mover_timer(timer_id, grupo_id, espaco).map_err(texto)?;
        Ok(Movido { para_grupo_id: grupo_id, para_espaco_id: espaco, ..antes })
    }

    /// "Desfazer" do mover: o grupo ou o timer volta para onde estava. Só se ele ainda está onde o
    /// mover o pôs e o lugar de antes não foi arquivado (revisão da rodada 21); senão, a recusa diz
    /// por quê.
    pub fn desfazer_mover(&mut self, m: &Movido) -> Result<(), String> {
        self.checar_banco()?;
        match m.tipo.as_str() {
            "grupo" => {
                let g = self.banco.grupos().map_err(texto)?.into_iter().find(|g| g.id == m.id).ok_or(DESFAZER_JA_MUDOU())?;
                if g.espaco_id != m.para_espaco_id {
                    return Err(DESFAZER_JA_MUDOU().into());
                }
                self.checar_espaco(m.espaco_id, true).map_err(|e| tfa("erro.nao_da_desfazer", &[&minuscula(&e)]))?;
                self.banco.mover_grupo(m.id, m.espaco_id).map_err(texto)?
            }
            "timer" => {
                let t = self.banco.timers().map_err(texto)?.into_iter().find(|t| t.id == m.id).ok_or(DESFAZER_JA_MUDOU())?;
                let espaco_agora = self.banco.espacos_dos_timers().map_err(texto)?.get(&m.id).copied().unwrap_or(t.espaco_id);
                if t.grupo_id != m.para_grupo_id || espaco_agora != m.para_espaco_id {
                    return Err(DESFAZER_JA_MUDOU().into());
                }
                match m.grupo_id {
                    Some(g) => {
                        let grupo = self.banco.grupos().map_err(texto)?.into_iter().find(|x| x.id == g);
                        let grupo = grupo.ok_or(ta("erro.grupo_arquivado"))?;
                        self.checar_espaco(grupo.espaco_id, true).map_err(|e| tfa("erro.nao_da_desfazer", &[&minuscula(&e)]))?;
                    }
                    None => self.checar_espaco(m.espaco_id, true).map_err(|e| tfa("erro.nao_da_desfazer", &[&minuscula(&e)]))?,
                }
                self.banco.mover_timer(m.id, m.grupo_id, m.espaco_id).map_err(texto)?
            }
            _ => return Err("Mover desconhecido".into()),
        };
        Ok(())
    }

    // ---- espaços (rodada 20) ----

    /// Erro se o espaço não existe (ou, com `ativo`, se está arquivado).
    pub fn checar_espaco(&self, id: i64, ativo: bool) -> Result<(), String> {
        match self.banco.espacos().map_err(texto)?.iter().find(|e| e.id == id) {
            Some(e) if ativo && e.arquivado => Err(tfa("erro.espaco_arquivado", &[&e.nome])),
            Some(_) => Ok(()),
            None => Err(ta("erro.espaco_nao_encontrado").into()),
        }
    }

    /// Nome de espaço: não vazio, até `ESPACO_NOME_MAX` letras, sem repetir (sem diferenciar
    /// maiúsculas).
    fn checar_nome_de_espaco(&self, nome: &str, id: Option<i64>) -> Result<(), String> {
        if nome.is_empty() {
            return Err(ta("erro.nome_vazio").into());
        }
        if nome.chars().count() > ESPACO_NOME_MAX {
            return Err(tfa("erro.nome_max", &[&ESPACO_NOME_MAX.to_string()]));
        }
        let igual = nome.to_lowercase();
        if self.banco.espacos().map_err(texto)?.iter().any(|e| Some(e.id) != id && e.nome.trim().to_lowercase() == igual) {
            return Err(tfa("erro.espaco_existe", &[nome]));
        }
        Ok(())
    }

    /// "+ Novo → Espaço" e Ajustes: acrescenta um espaço no fim da lista; devolve o id.
    pub fn criar_espaco(&mut self, nome: &str, cor: &str) -> Result<i64, String> {
        self.checar_banco()?;
        let nome = nome.trim();
        self.checar_nome_de_espaco(nome, None)?;
        self.banco.criar_espaco(nome, cor).map_err(texto)
    }

    /// Ajustes → Espaços: renomear, cor, ordem, arquivar. Nunca arquiva o último que sobra.
    /// Arquivar pausa os timers do espaço que estão rodando; a aba que era dele volta a "Todos".
    pub fn editar_espaco(&mut self, e: &Espaco, agora: Ms) -> Result<Vec<Mudanca>, String> {
        self.checar_banco()?;
        self.checar_espaco(e.id, false)?;
        let nome = e.nome.trim();
        self.checar_nome_de_espaco(nome, Some(e.id))?;
        let todos = self.banco.espacos().map_err(texto)?;
        let mut mud = vec![];
        if e.arquivado && !todos.iter().any(|x| x.id == e.id && x.arquivado) {
            if !todos.iter().any(|x| x.id != e.id && !x.arquivado) {
                return Err(ULTIMO_ESPACO().into());
            }
            let ids = self.timers_do_espaco(Some(e.id))?.unwrap_or_default();
            mud = self.motor.pausar_timers(&ids, agora);
            self.aplicar(&mud)?;
            self.largar_aba(e.id)?;
        }
        self.banco.editar_espaco(&Espaco { nome: nome.to_string(), ..e.clone() }).map_err(texto)?;
        Ok(vec![])
    }

    /// Para onde vão os itens arquivados de um espaço apagado: o primeiro outro espaço não
    /// arquivado, na ordem (nulo: não sobra nenhum).
    pub fn destino_ao_apagar(&self, id: i64) -> Result<Option<i64>, String> {
        let mut outros: Vec<Espaco> = self.banco.espacos().map_err(texto)?.into_iter().filter(|x| x.id != id && !x.arquivado).collect();
        outros.sort_by_key(|x| (x.ordem, x.id));
        Ok(outros.first().map(|x| x.id))
    }

    /// Ajustes → Espaços: apaga um espaço sem grupo nem timer sem grupo à vista (revisão da rodada
    /// 20). Os arquivados dele vão, ainda arquivados, para o primeiro outro espaço não arquivado
    /// (`destino_ao_apagar`); devolve quantos foram. Com algo à vista, recusa (`MOVA_OS_GRUPOS`).
    /// Nunca apaga o último espaço não arquivado.
    pub fn apagar_espaco(&mut self, id: i64) -> Result<i64, String> {
        self.checar_banco()?;
        self.checar_espaco(id, false)?;
        let destino = self.destino_ao_apagar(id)?.ok_or(ULTIMO_ESPACO())?;
        let foram = self.banco.apagar_espaco(id, destino).map_err(texto)?.ok_or(MOVA_OS_GRUPOS())?;
        self.largar_aba(id)?;
        Ok(foram)
    }

    /// A aba escolhida era do espaço `id` (que saiu da tela): volta a "Todos".
    fn largar_aba(&self, id: i64) -> Result<(), String> {
        if self.banco.config_app().map_err(texto)?.espaco_aba == Some(id) {
            self.guardar_dock(|c| c.espaco_aba = None)?;
        }
        Ok(())
    }

    /// "Desfazer" do zerar: cada timer volta ao valor de antes (se ninguém zerou de novo depois).
    pub fn desfazer_zerar(&mut self, z: &Zerado) -> Result<(), String> {
        self.checar_banco()?;
        if let Some((t, a, antes)) = z.atividade {
            self.banco.desfazer_zerar_atividade(t, a, antes, z.em).map_err(texto)?;
        }
        self.banco.desfazer_zerar(&z.antes, &z.limpas, z.em).map_err(texto)?;
        Ok(())
    }

    /// "Zerar sozinho na virada do dia" (desligado por padrão): ligado, cada corte do dia zera
    /// todos os timers no instante do corte, também com o app fechado (ao abrir, vale o corte mais
    /// recente). Ligar a opção não zera nada na hora: o primeiro é o próximo corte. Desligada, a
    /// marca do último corte sai. Devolve se zerou.
    pub fn zerar_na_virada(&mut self, agora: Ms) -> Result<bool, String> {
        self.checar_banco()?;
        let ligado = self.banco.config_app().map_err(texto)?.zerar_na_virada;
        let ultimo = self.banco.valor(CHAVE_ZERADO_AUTO).map_err(texto)?.and_then(|v| v.parse::<Ms>().ok());
        if !ligado {
            if ultimo.is_some() {
                self.banco.gravar_valor(CHAVE_ZERADO_AUTO, "").map_err(texto)?;
            }
            return Ok(false);
        }
        let (corte, _) = self.limites_do_dia(agora);
        match ultimo {
            Some(u) if u >= corte => Ok(false),
            Some(_) => {
                self.banco.zerar_no_corte(corte).map_err(texto)?;
                self.banco.gravar_valor(CHAVE_ZERADO_AUTO, &corte.to_string()).map_err(texto)?;
                Ok(true)
            }
            // acabou de ligar: conta a partir deste corte
            None => {
                self.banco.gravar_valor(CHAVE_ZERADO_AUTO, &corte.to_string()).map_err(texto)?;
                Ok(false)
            }
        }
    }

    /// "Ajustar hoje" do editar timer: soma (ou tira) `delta_ms` do tempo de hoje do timer, na
    /// atividade (a padrão do timer, sem escolha), com um motivo opcional curto. Devolve o id do
    /// ajuste (para desfazer).
    pub fn ajustar_hoje(
        &mut self,
        timer_id: i64,
        atividade_id: Option<i64>,
        delta_ms: Ms,
        motivo: Option<&str>,
        agora: Ms,
    ) -> Result<i64, String> {
        self.checar_banco()?;
        let atividade_id = self.checar_ajuste(timer_id, atividade_id, delta_ms, motivo)?;
        let motivo = motivo.map(str::trim).filter(|m| !m.is_empty());
        let dia = self.dia_texto(agora);
        self.banco.criar_ajuste(timer_id, atividade_id, &dia, delta_ms, motivo, agora).map_err(texto)
    }

    /// Confere o "Ajustar hoje" sem gravar; devolve a atividade (a padrão do timer, sem uma).
    pub fn checar_ajuste(&self, timer_id: i64, atividade_id: Option<i64>, delta_ms: Ms, motivo: Option<&str>) -> Result<i64, String> {
        self.checar_timer(timer_id)?;
        let atividade_id = atividade_id.unwrap_or_else(|| self.motor.atividade_padrao(Some(timer_id)));
        self.checar_atividade(atividade_id)?;
        if delta_ms == 0 {
            return Err(ta("erro.ajuste_minutos").into());
        }
        if delta_ms.abs() > DIA_MS {
            return Err(ta("erro.ajuste_24h").into());
        }
        let motivo = motivo.map(str::trim).filter(|m| !m.is_empty());
        if motivo.is_some_and(|m| m.chars().count() > MOTIVO_MAX) {
            return Err(tfa("erro.motivo_max", &[&MOTIVO_MAX.to_string()]));
        }
        Ok(atividade_id)
    }

    pub fn desfazer_ajuste(&mut self, id: i64) -> Result<(), String> {
        if self.banco.remover_ajuste(id).map_err(texto)? {
            Ok(())
        } else {
            Err(ta("erro.ajuste_nao_encontrado").into())
        }
    }

    /// O nome de um grupo cabe em `GRUPO_NOME_MAX` letras (revisão da rodada 24). Um nome de antes,
    /// mais comprido e sem mudar, segue valendo: editar a cor de um grupo assim não pede para
    /// encurtar o nome. `id`: o grupo editado (nulo: um grupo novo).
    pub fn checar_nome_de_grupo(&self, nome: &str, id: Option<i64>) -> Result<(), String> {
        if nome.trim().chars().count() <= GRUPO_NOME_MAX {
            return Ok(());
        }
        let igual_ao_de_antes = id.is_some_and(|id| {
            self.banco.grupos().map(|gs| gs.iter().any(|x| x.id == id && x.nome == nome)).unwrap_or(false)
        });
        if igual_ao_de_antes {
            Ok(())
        } else {
            Err(tfa("erro.nome_max", &[&GRUPO_NOME_MAX.to_string()]))
        }
    }

    /// Arquivar um grupo pausa os timers dele que estão rodando. O espaço (rodada 20) tem de
    /// existir e não estar arquivado.
    pub fn editar_grupo(&mut self, g: &Grupo, agora: Ms) -> Result<Vec<Mudanca>, String> {
        self.checar_nome_de_grupo(&g.nome, Some(g.id))?;
        self.checar_espaco(g.espaco_id, true)?;
        let mut mud = vec![];
        if g.arquivado {
            let ids: Vec<i64> =
                self.banco.timers().map_err(texto)?.iter().filter(|t| t.grupo_id == Some(g.id)).map(|t| t.id).collect();
            for id in ids {
                if self.motor.abertos.iter().any(|a| a.timer_id == Some(id)) {
                    mud.extend(self.motor.pausar(id, agora));
                }
            }
            self.aplicar(&mud)?;
        }
        self.banco.editar_grupo(g).map_err(texto)?;
        Ok(vec![])
    }

    /// Arquivar um timer rodando pausa ele antes. A atividade padrão nova vale para o que abrir
    /// depois (o que está aberto segue na que tinha).
    /// `editar_timer_com` sem mexer nas ocultas (testes).
    #[cfg(test)]
    pub fn editar_timer(&mut self, t: &Timer, agora: Ms) -> Result<Vec<Mudanca>, String> {
        self.editar_timer_com(t, agora, None)
    }

    /// Sem grupo, o espaço do timer (rodada 20) tem de existir e não estar arquivado; com grupo,
    /// o timer guarda o espaço do grupo. `ocultas`: as atividades que o painel do timer não mostra
    /// (nulo: as de antes); a atividade padrão nova sai delas. Tudo é conferido antes e gravado
    /// numa transação só (revisão da rodada 20).
    pub fn editar_timer_com(&mut self, t: &Timer, agora: Ms, ocultas: Option<&[i64]>) -> Result<Vec<Mudanca>, String> {
        let t = &self.checar_editar_timer(t, ocultas)?;
        let antes = self.banco.atividades_ocultas().map_err(texto)?.remove(&t.id).unwrap_or_default();
        let ocultas: Vec<i64> =
            ocultas.map_or(antes, <[i64]>::to_vec).into_iter().filter(|a| *a != t.atividade_padrao_id).collect();
        if t.arquivado && self.motor.abertos.iter().any(|a| a.timer_id == Some(t.id)) {
            let mud = self.motor.pausar(t.id, agora);
            self.aplicar(&mud)?;
        }
        self.banco.editar_timer_com_ocultas(t, &ocultas).map_err(texto)?;
        self.motor.atividades_padrao = self.banco.atividades_padrao().map_err(texto)?;
        Ok(vec![])
    }

    /// Confere o editar timer sem gravar nada (revisão da rodada 21: o diálogo confere tudo, o
    /// "Ajustar hoje" junto, antes de gravar qualquer coisa). Devolve o timer com o espaço do grupo.
    /// O espaço arquivado só recusa um destino novo: ficar onde já está (o "Sem grupo" de um
    /// espaço arquivado, ao só renomear) vale.
    pub fn checar_editar_timer(&self, t: &Timer, ocultas: Option<&[i64]>) -> Result<Timer, String> {
        self.checar_atividade(t.atividade_padrao_id)?;
        if let Some(o) = ocultas {
            if o.contains(&t.atividade_padrao_id) {
                return Err(PADRAO_NAO_OCULTA().into());
            }
            let existem = self.banco.atividades().map_err(texto)?;
            if let Some(a) = o.iter().find(|a| !existem.iter().any(|x| x.id == **a)) {
                return Err(format!("{}: {a}", ta("erro.atividade_nao_encontrada")));
            }
        }
        let mut t = t.clone();
        if let Some(g) = t.grupo_id {
            if let Some(e) = self.banco.espacos_dos_grupos().map_err(texto)?.get(&g) {
                t.espaco_id = *e;
            }
        }
        let agora_em = self.banco.espacos_dos_timers().map_err(texto)?.get(&t.id).copied();
        if agora_em != Some(t.espaco_id) {
            self.checar_destino(t.espaco_id)?;
        }
        Ok(t)
    }

    /// Erro se o espaço de um destino novo (do timer, ou do grupo novo dele) não existe ou está
    /// arquivado, com o que fazer.
    pub fn checar_destino(&self, espaco: i64) -> Result<(), String> {
        match self.banco.espacos().map_err(texto)?.iter().find(|e| e.id == espaco) {
            Some(e) if e.arquivado => Err(tfa("erro.escolha_outro_grupo", &[&e.nome])),
            Some(_) => Ok(()),
            None => Err(ta("erro.espaco_nao_encontrado").into()),
        }
    }

    /// Erro se a atividade não existe ou está arquivada.
    pub fn checar_atividade(&self, id: i64) -> Result<(), String> {
        match self.banco.atividades().map_err(texto)?.iter().find(|a| a.id == id) {
            Some(a) if !a.arquivada => Ok(()),
            Some(a) => Err(tfa("erro.atividade_arquivada", &[&a.nome])),
            None => Err(ta("erro.atividade_nao_encontrada").into()),
        }
    }

    /// Ajustes → Atividades: acrescenta uma (nome sem repetir, sem diferenciar maiúsculas).
    pub fn criar_atividade(&mut self, nome: &str, cor: &str) -> Result<i64, String> {
        self.checar_banco()?;
        let nome = nome.trim();
        self.checar_nome_de_atividade(nome, None)?;
        self.banco.criar_atividade(nome, cor).map_err(texto)
    }

    fn checar_nome_de_atividade(&self, nome: &str, id: Option<i64>) -> Result<(), String> {
        if nome.is_empty() {
            return Err(ta("erro.nome_vazio").into());
        }
        if nome.chars().count() > ATIVIDADE_NOME_MAX {
            return Err(tfa("erro.nome_max", &[&ATIVIDADE_NOME_MAX.to_string()]));
        }
        let igual = nome.to_lowercase();
        if self.banco.atividades().map_err(texto)?.iter().any(|a| Some(a.id) != id && a.nome.trim().to_lowercase() == igual) {
            return Err(tfa("erro.atividade_existe", &[nome]));
        }
        Ok(())
    }

    /// Ajustes → Atividades: renomear, cor, ordem, arquivar. Nunca arquiva a última que sobra. A
    /// que é padrão de algum timer só arquiva com `confirmar` (a pessoa respondeu à pergunta), e
    /// esses timers passam a usar a primeira atividade que sobra. Os trechos e ajustes que apontam
    /// para uma arquivada seguem contando nela.
    pub fn editar_atividade(&mut self, a: &Atividade, confirmar: bool) -> Result<(), String> {
        self.checar_banco()?;
        let todas = self.banco.atividades().map_err(texto)?;
        let antes = todas.iter().find(|x| x.id == a.id).ok_or(ta("erro.atividade_nao_encontrada"))?;
        let nome = a.nome.trim();
        self.checar_nome_de_atividade(nome, Some(a.id))?;
        if a.arquivada && !antes.arquivada {
            let sobra = todas.iter().find(|x| x.id != a.id && !x.arquivada).ok_or(ta("erro.ultima_atividade"))?;
            let usam: Vec<String> = self
                .banco
                .timers()
                .map_err(texto)?
                .into_iter()
                .filter(|t| t.atividade_padrao_id == a.id)
                .map(|t| t.nome)
                .collect();
            if !usam.is_empty() {
                if !confirmar {
                    return Err(tfa("erro.atividade_padrao_de", &[&antes.nome, &usam.join(", ")]));
                }
                self.banco.trocar_atividade_padrao(a.id, sobra.id).map_err(texto)?;
            }
        }
        self.banco.editar_atividade(&Atividade { nome: nome.to_string(), ..a.clone() }).map_err(texto)?;
        self.motor.atividades_padrao = self.banco.atividades_padrao().map_err(texto)?;
        Ok(())
    }

    /// Regra de pasta no banco e no motor (sessões novas nessa pasta já entram no timer).
    pub fn criar_regra(&mut self, pasta: &str, timer_id: i64) -> Result<(), String> {
        self.checar_banco()?;
        self.banco.criar_regra_pasta(pasta, timer_id).map_err(texto)?;
        self.motor.regras_pasta = self.banco.regras_pasta().map_err(texto)?;
        Ok(())
    }

    /// Grava a regra de `pasta` no timer (as "Pastas deste timer", rodada 15): na raiz do projeto,
    /// recusando a raiz de drive, a pasta do usuário e a pasta que nunca conta. A regra da mesma
    /// pasta de outro timer passa para este (a tela pergunta antes). `trocar`: a regra deste timer
    /// que esta substitui (o "Trocar pasta…"). Não muda as sessões já ligadas.
    pub fn salvar_regra(&mut self, pasta: &str, timer_id: i64, trocar: Option<i64>) -> Result<(), String> {
        self.checar_banco()?;
        self.checar_timer(timer_id)?;
        let c = self.conferir_regra(pasta, Some(timer_id))?.conferencia;
        if let Some(e) = c.erro {
            return Err(e);
        }
        if let Some(id) = trocar {
            if !self.motor.regras_pasta.iter().any(|r| r.id == id && r.timer_id == timer_id) {
                return Err(ta("erro.regra_nao_encontrada").into());
            }
        }
        let nova = self.banco.criar_regra_pasta(&c.pasta, timer_id).map_err(texto)?;
        if let Some(id) = trocar.filter(|id| *id != nova) {
            self.banco.remover_regra_pasta(id).map_err(texto)?;
        }
        self.motor.regras_pasta = self.banco.regras_pasta().map_err(texto)?;
        Ok(())
    }

    /// O "Desfazer" de Tirar ou Apagar uma regra (revisão da rodada 15): põe a regra de volta no
    /// timer, a não ser que, no meio-tempo, a pasta tenha virado regra de outro timer; aí não faz
    /// nada e diz de quem ela é.
    pub fn restaurar_regra(&mut self, pasta: &str, timer_id: i64) -> Result<(), String> {
        if let Some(nome) = self.conferir_regra(pasta, Some(timer_id))?.outro_timer_nome {
            return Err(tfa("erro.pasta_ja_e_de", &[&nome]));
        }
        self.salvar_regra(pasta, timer_id, None)
    }

    /// "Trocar por uma subpasta…" do ✎ da sessão (rodada 15): a regra que ligou a sessão passa a
    /// valer só em `pasta`, a pasta da regra ou uma subpasta dela que contenha a pasta da sessão.
    /// As sessões já ligadas ficam onde estão.
    pub fn trocar_por_subpasta(&mut self, regra_id: i64, pasta: &str, sessao_id: &str) -> Result<(), String> {
        let regra = self.motor.regras_pasta.iter().find(|r| r.id == regra_id).cloned().ok_or(ta("erro.regra_nao_encontrada"))?;
        let cwd = self.motor.sessoes.get(sessao_id).ok_or(ta("erro.sessao_nao_encontrada"))?.cwd.clone();
        if !nucleo::pastas::subpasta_valida(&regra.pasta, pasta, &cwd) {
            return Err(format!(
                "Escolha {} ou uma subpasta dela que tenha a pasta desta conversa.",
                regra.pasta
            ));
        }
        self.salvar_regra(pasta, regra.timer_id, Some(regra_id))
    }

    /// Regras de pasta com o nome do timer (Ajustes).
    pub fn regras(&self) -> Result<Vec<RegraDto>, String> {
        Ok(self
            .banco
            .regras_pasta_com_timer()
            .map_err(texto)?
            .into_iter()
            .map(|(r, timer_nome)| RegraDto { id: r.id, pasta: r.pasta, timer_id: r.timer_id, timer_nome })
            .collect())
    }

    /// Tira a regra do banco e do motor (sessões novas na pasta voltam a perguntar).
    pub fn remover_regra(&mut self, id: i64) -> Result<(), String> {
        self.checar_banco()?;
        if !self.banco.remover_regra_pasta(id).map_err(texto)? {
            return Err(ta("erro.regra_nao_encontrada").into());
        }
        self.motor.regras_pasta = self.banco.regras_pasta().map_err(texto)?;
        Ok(())
    }

    /// O "Ligar" de sempre (o "desde o início" leva todos os dias), nos testes.
    #[cfg(test)]
    pub fn ligar_sessao(
        &mut self,
        sessao_id: &str,
        timer_id: i64,
        desde_inicio: bool,
        criar_regra: bool,
        agora: Ms,
    ) -> Result<Vec<Mudanca>, String> {
        self.ligar_sessao_com(sessao_id, timer_id, desde_inicio, false, criar_regra, None, agora)
    }

    /// `so_hoje`: o "desde o início" leva só os trechos do dia de trabalho atual (o "Mover" do ✎
    /// e a conversa nova, rodada 15). `pasta`: onde fica a regra do "sempre nesta pasta" (rodada
    /// 27: uma das pastas da sessão, `pastas_da_sessao`); sem ela, a pasta onde a sessão começou.
    #[allow(clippy::too_many_arguments)]
    pub fn ligar_sessao_com(
        &mut self,
        sessao_id: &str,
        timer_id: i64,
        desde_inicio: bool,
        so_hoje: bool,
        criar_regra: bool,
        pasta: Option<&str>,
        agora: Ms,
    ) -> Result<Vec<Mudanca>, String> {
        self.checar_timer(timer_id)?;
        let cwd = self.motor.sessoes.get(sessao_id).ok_or(ta("erro.sessao_nao_encontrada"))?.cwd.clone();
        // a pasta pedida é conferida antes de ligar (no Chat, sem pasta, ela é erro, como no
        // `responder`); sem ela, a pasta onde a sessão começou (no Chat, nenhuma regra)
        let regra = match (criar_regra, pasta) {
            (false, _) => None,
            (true, Some(p)) => Some(self.regra_na_pasta_da_sessao(sessao_id, &cwd, p, timer_id)?),
            (true, None) => (!cwd.trim().is_empty()).then_some(cwd),
        };
        let mud = if desde_inicio && so_hoje {
            self.motor.ligar_sessao_hoje(sessao_id, timer_id, agora)
        } else {
            self.motor.ligar_sessao(sessao_id, timer_id, desde_inicio, agora)
        };
        self.aplicar(&mud)?;
        if let Some(p) = regra {
            self.criar_regra(&p, timer_id)?;
        }
        Ok(vec![])
    }

    /// A pasta da regra pedida no "Sempre que for na pasta" (rodada 27): tem de ser uma das pastas
    /// da sessão (da onde ela começou até onde está agora) e poder ter regra (`conferir_regra`).
    /// Devolve onde a regra fica.
    fn regra_na_pasta_da_sessao(&self, sessao_id: &str, cwd: &str, pasta: &str, timer_id: i64) -> Result<String, String> {
        let chave = chave_de_pasta(pasta.trim());
        // a pasta como a lista escreve (a do evento do hook), não como veio na resposta
        let pasta = self
            .motor
            .pastas_da_sessao(sessao_id, cwd)
            .into_iter()
            .find(|p| chave_de_pasta(p) == chave)
            .ok_or(ta("erro.pasta_fora_da_lista"))?;
        let c = self.conferir_regra(&pasta, Some(timer_id))?.conferencia;
        match c.erro {
            Some(e) => Err(e),
            None => Ok(c.pasta),
        }
    }

    /// O "+ Novo timer…" da sessão: nome e pasta pela pasta dela (a da pergunta, se a sessão já
    /// saiu do motor). `pasta` e `sempre`: o "Sempre que for na pasta" da pergunta de sessão nova
    /// (rodada 27), com a pasta escolhida na lista (conferida: uma das pastas da sessão).
    pub fn pedido_novo_timer(&self, sessao_id: &str, pasta: Option<&str>, sempre: Option<bool>) -> Result<PedidoNovoTimer, String> {
        let cwd = match self.motor.sessoes.get(sessao_id) {
            Some(s) => s.cwd.clone(),
            None => self
                .motor
                .perguntas
                .iter()
                .find_map(|(_, p)| match p {
                    Pergunta::SessaoNova { sessao_id: s, cwd, .. } | Pergunta::ConversaNova { sessao_id: s, cwd, .. }
                        if s == sessao_id =>
                    {
                        Some(cwd.clone())
                    }
                    _ => None,
                })
                .ok_or(ta("erro.sessao_nao_encontrada"))?,
        };
        let escolhida = match pasta.map(str::trim).filter(|p| !p.is_empty()) {
            Some(p) => {
                let chave = chave_de_pasta(p);
                Some(
                    self.motor
                        .pastas_da_sessao(sessao_id, &cwd)
                        .into_iter()
                        .find(|x| chave_de_pasta(x) == chave)
                        .ok_or(ta("erro.pasta_fora_da_lista"))?,
                )
            }
            None => None,
        };
        let raiz = escolhida.as_deref().unwrap_or_else(|| raiz_do_projeto(&cwd));
        let c = self.conferir_regra(raiz, None)?;
        // a regra só é oferecida onde o "ligar" com a pasta vai passar (`regra_na_pasta_da_sessao`)
        let chave = chave_de_pasta(raiz);
        let pode_regra = c.conferencia.erro.is_none()
            && self.motor.pastas_da_sessao(sessao_id, &cwd).iter().any(|p| chave_de_pasta(p) == chave);
        Ok(PedidoNovoTimer {
            sessao_id: sessao_id.to_string(),
            nome: sugestao_de_pasta(raiz),
            pasta: raiz.to_string(),
            pasta_mae: c.conferencia.pasta_mae,
            outro_timer_nome: c.outro_timer_nome,
            sempre: if pode_regra { sempre } else { Some(false) },
            pode_regra,
        })
    }

    /// Confere uma regra para `pasta` no timer `timer_id` (nenhum: o timer ainda não existe):
    /// onde ela fica, se a pasta é uma pasta-mãe, se a regra já é de outro timer e se pode.
    pub fn conferir_regra(&self, pasta: &str, timer_id: Option<i64>) -> Result<ConferenciaRegra, String> {
        let perfil = std::env::var("USERPROFILE").ok();
        let c = nucleo::pastas::conferir(
            pasta,
            timer_id,
            &self.motor.regras_pasta,
            &self.motor.cfg.pastas_ignoradas,
            perfil.as_deref(),
        );
        let outro_timer_nome = match c.outro_timer {
            Some(id) => self.banco.timers().map_err(texto)?.into_iter().find(|t| t.id == id).map(|t| t.nome),
            None => None,
        };
        Ok(ConferenciaRegra { conferencia: c, outro_timer_nome })
    }

    /// `valor`: "contar" | "retomar" | "pausado" | "nao" | "certo" | "ligar:{id}" | "ligar:{id}:regra".
    pub fn responder(&mut self, pergunta_id: u64, valor: &str, agora: Ms) -> Result<Vec<Mudanca>, String> {
        let r = resposta_de(valor)?;
        let pergunta = self.motor.perguntas.iter().find(|(id, _)| *id == pergunta_id).map(|(_, p)| p.clone());
        let mut regra = None;
        if let Resposta::LigarA { timer_id, criar_regra } = r {
            self.checar_timer(timer_id)?;
            // "ligar:<t>:pasta:<caminho>" (rodada 27): a regra numa das pastas da sessão nova,
            // conferida antes de ligar
            if let Some(p) = pasta_pedida(valor) {
                match &pergunta {
                    Some(Pergunta::SessaoNova { sessao_id, cwd, .. }) => {
                        regra = Some((self.regra_na_pasta_da_sessao(sessao_id, cwd, p, timer_id)?, timer_id));
                    }
                    Some(_) => return Err(ta("erro.pasta_fora_da_lista").into()),
                    None => {}
                }
            } else if let (
                true,
                Some(Pergunta::SessaoNova { cwd, sessao_id, .. } | Pergunta::ConversaNova { cwd, sessao_id, .. }),
            ) = (criar_regra, &pergunta)
            {
                let cwd = self.motor.sessoes.get(sessao_id).map_or(cwd.clone(), |s| s.cwd.clone());
                // sem pasta (o Chat), nada de "sempre nesta pasta"
                regra = (!cwd.trim().is_empty()).then_some((cwd, timer_id));
            }
        }
        let mud = self.motor.responder(pergunta_id, r, agora);
        self.aplicar(&mud)?;
        if let Some((cwd, timer_id)) = regra {
            self.criar_regra(&cwd, timer_id)?;
        }
        Ok(vec![])
    }

    /// Grava a config, regrava o `hook.json` (pastas ignoradas) e atualiza o motor.
    /// Posição, tamanho e etapa do dock ficam como estão no banco: só o dock muda esses
    /// campos (`guardar_dock`), e a config que vem da janela principal pode estar velha.
    /// Sem o modo desenvolvedor, os tempos que vierem valem os padrões (desligar o modo dev os
    /// devolve aos padrões); com ele, são conferidos.
    pub fn salvar_config(&mut self, cfg: &ConfigApp, hook_json: &Path) -> Result<(), String> {
        self.checar_banco()?;
        if cfg.modo_dev {
            // tempo zero desligaria a detecção (uso, inatividade, sessão parada)
            let curtos: Vec<&str> =
                cfg.tempos().iter().filter(|(_, s)| *s < TEMPO_MIN_S).map(|(n, _)| *n).collect();
            if !curtos.is_empty() {
                return Err(tfa("erro.tempos_minimo", &[&curtos.join(", ")]));
            }
            if cfg.inatividade_s > INATIVIDADE_MAX_S {
                return Err(ta("erro.inatividade_faixa").into());
            }
            if cfg.uso_s > TEMPO_MAX_S || cfg.sessao_parada_s > TEMPO_MAX_S {
                return Err(ta("erro.tempos_24h").into());
            }
        }
        let atual = self.banco.config_app().map_err(texto)?;
        let cfg = &ConfigApp {
            dock_x: atual.dock_x,
            dock_y: atual.dock_y,
            dock_larg: atual.dock_larg,
            dock_alt: atual.dock_alt,
            dock_expandido: atual.dock_expandido,
            // só o tour muda isto (`marcar_tour_visto`): a config que vem da página pode estar velha
            tour_visto: atual.tour_visto,
            // as dicas também: só pelos comandos delas (rodada 16)
            dicas_ligadas: atual.dicas_ligadas,
            dicas_vistas: atual.dicas_vistas.clone(),
            // só a busca de atualização muda isto (rodada 25)
            atualizacao_ultima: atual.atualizacao_ultima,
            ..cfg.clone()
        }
        .normalizada();
        self.banco.salvar_config_app(cfg).map_err(texto)?;
        gravar_hook_json(cfg, hook_json)?;
        self.motor.cfg = ConfigMotor::de(cfg, self.fuso_min);
        Ok(())
    }

    /// Muda só campos que o motor não usa (os do dock, `ao_fechar`) na config gravada.
    pub fn guardar_dock(&self, f: impl FnOnce(&mut ConfigApp)) -> Result<ConfigApp, String> {
        self.checar_banco()?;
        let mut cfg = self.banco.config_app().map_err(texto)?;
        f(&mut cfg);
        self.banco.salvar_config_app(&cfg).map_err(texto)?;
        Ok(cfg)
    }

    /// Primeiro uso ainda por fazer: nenhum timer à vista (a mesma regra do `semTimers` da
    /// página). Se o snapshot falhar, conta como primeiro uso (a janela abre).
    pub fn primeiro_uso(&self) -> bool {
        self.snapshot(agora_ms()).map_or(true, |s| s.grupos.iter().all(|g| g.timers.is_empty()))
    }

    /// "Não perguntar de novo" do "Fechar o Work Timer?": grava `"bandeja"` ou `"sair"`.
    pub fn guardar_ao_fechar(&self, valor: &str) -> Result<ConfigApp, String> {
        if valor != AO_FECHAR_BANDEJA && valor != AO_FECHAR_SAIR {
            return Err(format!("escolha desconhecida ao fechar: {valor}"));
        }
        self.guardar_dock(|c| c.ao_fechar = valor.to_string())
    }

    /// "Não perguntar de novo" do "Minimizar para a bandeja?": grava `"bandeja"` ou `"barra"`
    /// (`"dock"` também vale).
    pub fn guardar_ao_minimizar(&self, valor: &str) -> Result<ConfigApp, String> {
        if ![AO_MINIMIZAR_BANDEJA, AO_MINIMIZAR_BARRA, AO_MINIMIZAR_DOCK].contains(&valor) {
            return Err(format!("escolha desconhecida ao minimizar: {valor}"));
        }
        self.guardar_dock(|c| c.ao_minimizar = valor.to_string())
    }

    /// Posição do dock; o tamanho só vale para o expandido (o recolhido acompanha o conteúdo).
    pub fn guardar_posicao_dock(&self, x: i32, y: i32, larg: i32, alt: i32) -> Result<ConfigApp, String> {
        self.guardar_dock(|c| {
            c.dock_x = Some(x);
            c.dock_y = Some(y);
            if c.dock_expandido && larg > 0 && alt > 0 {
                c.dock_larg = Some(larg);
                c.dock_alt = Some(alt);
            }
        })
    }

    /// Sessões dos últimos 7 dias, a mais recente primeiro. O título de uma conversa do Chat só
    /// existe em memória (o banco não guarda): vem do motor.
    /// Para a lista de ligar (rodada 27): as disponíveis (sem timer) primeiro e as já ligadas por
    /// último, a mais recente primeiro em cada grupo. Uma conversa do Chat sem nome conhecido e sem
    /// timer fica de fora (não dá para reconhecer qual é).
    pub fn sessoes_recentes(&self, agora: Ms) -> Result<Vec<SessaoDto>, String> {
        let mut v = self.banco.sessoes_desde(agora - DIAS_RECENTES * DIA_MS).map_err(texto)?;
        v.reverse();
        for s in v.iter_mut().filter(|s| nucleo::motor::e_sessao_do_chat(&s.id)) {
            s.nome = self.motor.sessoes.get(&s.id).and_then(|m| m.nome.clone());
        }
        v.retain(|s| {
            !nucleo::motor::e_sessao_do_chat(&s.id) || s.timer_id.is_some() || s.nome.as_deref().is_some_and(|n| !n.trim().is_empty())
        });
        // estável: dentro de cada grupo, a ordem do banco ao contrário (a mais recente primeiro)
        v.sort_by_key(|s| s.timer_id.is_some());
        Ok(v.iter().map(|s| snapshot::sessao_dto(nucleo::idioma::atual(), s, &self.motor.regras_pasta)).collect())
    }

    /// Sessões cujo título precisa ser lido agora; marca a conferência no cache.
    /// Nunca conferida nesta execução: confere se não tem nome ou está ativa; a parada com nome só
    /// guarda como está o transcript (sem ler). Depois: ativa (trabalhando ou com evento nos últimos
    /// 5 min) 1×/min; parada a cada 10 min (pega a renomeada no app). Conferir é olhar tamanho e
    /// data do transcript; o arquivo só é lido se mudou desde a última leitura.
    pub fn titulos_a_ler(&mut self, agora: Ms) -> Vec<(String, PathBuf)> {
        let mut conferir = vec![];
        let mut so_marcar = vec![];
        for s in self.motor.sessoes.values() {
            let Some(tr) = s.transcript.as_deref() else { continue };
            let ativa = s.estado == EstadoSessao::Trabalhando || agora - s.ultimo_evento < TITULO_ATIVA_MS;
            let folga = if ativa { RELER_TITULO_MS } else { RELER_TITULO_PARADA_MS };
            match self.titulo_lido.get(&s.id) {
                None if s.nome.is_none() || ativa => conferir.push((s.id.clone(), PathBuf::from(tr))),
                None => so_marcar.push((s.id.clone(), PathBuf::from(tr))),
                Some(t) if agora - t >= folga => conferir.push((s.id.clone(), PathBuf::from(tr))),
                Some(_) => {}
            }
        }
        for (id, caminho) in so_marcar {
            self.titulo_lido.insert(id.clone(), agora);
            if let Ok(m) = std::fs::metadata(&caminho) {
                self.titulo_arquivo.insert(id, (m.len(), m.modified().ok()));
            }
        }
        let mut v = vec![];
        for (id, caminho) in conferir {
            self.titulo_lido.insert(id.clone(), agora);
            let assinatura: Option<Assinatura> =
                std::fs::metadata(&caminho).ok().map(|m| (m.len(), m.modified().ok()));
            if assinatura.is_some() && self.titulo_arquivo.get(&id) == assinatura.as_ref() {
                continue;
            }
            match assinatura {
                Some(a) => self.titulo_arquivo.insert(id.clone(), a),
                None => self.titulo_arquivo.remove(&id),
            };
            v.push((id, caminho));
        }
        v
    }

    /// Os transcripts a ler agora (rodada 13), fora da trava (`LeitorDoTranscript::ler`), com o
    /// leitor de cada um (até onde já leu e as tarefas pendentes):
    /// - os das sessões trabalhando sem evento há `CONFERIR_INTERRUPCAO_MS` (o stop na conversa);
    /// - os das sessões esperando você que terminaram há menos de `TAREFA_MAX_MS` ou ainda têm
    ///   tarefa pendente (as tarefas em segundo plano e a notificação de fim delas).
    ///
    /// Só os que cresceram (ou encolheram) desde a última leitura.
    pub fn transcripts_a_conferir(&mut self, agora: Ms) -> Vec<(String, PathBuf, LeitorDoTranscript)> {
        let mut v = vec![];
        for s in self.motor.sessoes.values() {
            let Some(tr) = s.transcript.as_deref() else { continue };
            let leitor = self.leitores.get(&s.id);
            let conferir = match s.estado {
                EstadoSessao::Trabalhando => agora - s.ultimo_evento >= CONFERIR_INTERRUPCAO_MS,
                EstadoSessao::EsperandoVoce => {
                    s.terminou_em.is_some_and(|t| agora < t + TAREFA_MAX_MS) || leitor.is_some_and(|l| !l.pendentes.is_empty())
                }
                EstadoSessao::Parada => false,
            };
            if !conferir {
                continue;
            }
            let caminho = PathBuf::from(tr);
            let Ok(tamanho) = std::fs::metadata(&caminho).map(|m| m.len()) else { continue };
            if leitor.and_then(|l| l.offset) == Some(tamanho) {
                continue;
            }
            v.push((s.id.clone(), caminho, leitor.cloned().unwrap_or_default()));
        }
        v
    }

    /// O que cada transcript lido diz (rodada 13): `lidos` traz o leitor depois da leitura e o
    /// instante da interrupção nas linhas novas. A interrupção depois do último evento de uma
    /// sessão trabalhando fecha o trecho dela (`Motor::interromper`). Depois, o "segundo plano" é
    /// refeito (`atualizar_segundo_plano`).
    pub fn aplicar_fins(&mut self, lidos: Vec<(String, LeitorDoTranscript, Option<Option<Ms>>)>, agora: Ms) -> Vec<Mudanca> {
        let mut mud = vec![];
        for (id, leitor, interrupcao) in lidos {
            self.leitores.insert(id.clone(), leitor);
            let trabalhando = self.motor.sessoes.get(&id).is_some_and(|s| s.estado == EstadoSessao::Trabalhando);
            if let (true, Some(Some(em))) = (trabalhando, interrupcao) {
                mud.extend(self.motor.interromper(&id, em));
            }
        }
        self.atualizar_segundo_plano(agora);
        mud
    }

    /// Refaz o "segundo plano" pelo que foi lido (a cada ciclo, também sem leitura nova): cada
    /// sessão com tarefa pendente ainda valendo, até quando a última vale. As tarefas lançadas há
    /// 2 h ou mais são esquecidas de vez. Não depende do `Stop`: a marca volta a cada fim de turno
    /// enquanto a tarefa valer.
    pub fn atualizar_segundo_plano(&mut self, agora: Ms) {
        self.leitores.retain(|id, _| self.motor.sessoes.contains_key(id));
        let mut m = std::collections::BTreeMap::new();
        for (id, l) in self.leitores.iter_mut() {
            l.expirar(agora);
            if let Some(ate) = l.segundo_plano_ate(agora) {
                m.insert(id.clone(), ate);
            }
        }
        self.motor.segundo_plano = m;
    }

    /// Põe os títulos lidos nas sessões; devolve as sessões que mudaram. O nome que chega pode
    /// ser o da conversa anterior, continuada nesta sessão: se ninguém mexeu nela, ela herda o
    /// timer daquela (`Motor::herdar`, com o aviso no dock).
    pub fn aplicar_titulos(&mut self, lidos: Vec<(String, Option<String>)>, agora: Ms) -> Vec<Mudanca> {
        let mut mud = vec![];
        let mut nomeadas = vec![];
        for (id, titulo) in lidos {
            let Some(titulo) = titulo else { continue };
            if let Some(s) = self.motor.sessoes.get_mut(&id) {
                if s.nome.as_deref() != Some(titulo.as_str()) {
                    s.nome = Some(titulo);
                    mud.push(Mudanca::Sessao(s.clone()));
                    nomeadas.push(id);
                }
            }
        }
        if nomeadas.is_empty() {
            return mud;
        }
        let validos: BTreeSet<i64> = match self.banco.timers() {
            Ok(v) => v.iter().map(|t| t.id).collect(),
            Err(e) => {
                log_debug!("worktimer: timers para a herança: {e}");
                return mud;
            }
        };
        for id in nomeadas {
            mud.extend(self.motor.herdar(&id, agora, true, |t| validos.contains(&t)));
        }
        mud
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use fila::EventoHook;
    use nucleo::modelo::{MotivoFim, Origem, ATIVIDADE_REUNIAO, ATIVIDADE_TRABALHO, ESPACO_PRIMEIRO};
    use nucleo::snapshot::ZeradoAtividadeDto;
    use nucleo::tempo::ms_local;
    use std::io::Write;

    const F: i32 = -180;
    pub fn h(hh: u32, mm: u32) -> Ms {
        ms_local(2026, 11, 17, hh, mm, F)
    }
    fn dia() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 11, 17).unwrap()
    }
    pub fn nucleo(agora: Ms) -> Nucleo {
        Nucleo::abrir(Banco::abrir_memoria().unwrap(), agora, F, dia()).unwrap()
    }
    pub fn ev(t: Ms, evento: &str, sessao: &str, cwd: &str) -> EventoHook {
        EventoHook { t, evento: evento.into(), sessao: sessao.into(), cwd: cwd.into(), transcript: None, notificacao: None, origem: None }
    }

    #[test]
    fn resposta_de_le_os_valores() {
        assert_eq!(resposta_de("contar"), Ok(Resposta::Contar));
        assert_eq!(resposta_de("retomar"), Ok(Resposta::RetomarAgora));
        assert_eq!(resposta_de("pausado"), Ok(Resposta::DeixarPausado));
        assert_eq!(resposta_de("nao"), Ok(Resposta::NaoContar));
        assert_eq!(resposta_de("certo"), Ok(Resposta::Certo));
        assert_eq!(resposta_de("ligar:4"), Ok(Resposta::LigarA { timer_id: 4, criar_regra: false }));
        assert_eq!(resposta_de("ligar:4:regra"), Ok(Resposta::LigarA { timer_id: 4, criar_regra: true }));
        assert!(resposta_de("outro").is_err());
        assert!(resposta_de("ligar:x").is_err());
        // a regra numa das pastas da sessão (rodada 27): o caminho tem ":"
        assert_eq!(resposta_de("ligar:4:pasta:D:\\Projetos\\app"), Ok(Resposta::LigarA { timer_id: 4, criar_regra: true }));
        assert_eq!(pasta_pedida("ligar:4:pasta:D:\\Projetos\\app"), Some("D:\\Projetos\\app"));
        assert_eq!(pasta_pedida("ligar:4:regra"), None);
        assert_eq!(pasta_pedida("ligar:4"), None);
        assert!(resposta_de("ligar:4:pasta:").is_err());
        assert!(resposta_de("ligar:4:pasta:  ").is_err());
        assert!(resposta_de("ligar:4:outra").is_err());
        assert!(resposta_de("ligar:x:pasta:D:\\a").is_err());
    }

    #[test]
    fn descartar_hoje_pausa_antes_de_descartar_e_desfaz() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "A", "#a00").unwrap();
        let mud = n.motor.play(t, h(9, 0));
        n.aplicar(&mud).unwrap();
        let z = n.descartar_hoje(t, h(10, 0)).unwrap();
        assert_eq!((z.trechos.len(), z.ajustes.len()), (1, 0));
        assert!(n.motor.abertos.is_empty());
        assert!(n.banco.abertos().unwrap().is_empty(), "nenhum trecho aberto fica para trás");
        let tr = n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert_eq!((tr[0].fim, tr[0].descartado), (Some(h(10, 0)), true));
        let s = n.snapshot(h(10, 0)).unwrap();
        assert!(s.grupos[0].timers[0].trechos.is_empty());
        n.desfazer_descartar(&z.trechos, &z.ajustes).unwrap();
        assert_eq!(n.snapshot(h(10, 0)).unwrap().grupos[0].timers[0].trechos.len(), 1);
        assert!(n.descartar_hoje(999, h(10, 0)).is_err());
    }

    #[test]
    fn descartar_hoje_descarta_os_ajustes_a_mao_e_desfazer_os_devolve() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "A", "#a00").unwrap();
        let outro = n.banco.criar_timer(None, "B", "#b00").unwrap();
        let mud = n.motor.play(t, h(9, 0));
        n.aplicar(&mud).unwrap();
        n.ajustar_hoje(t, None, 30 * MIN, Some("reunião sem timer"), h(9, 10)).unwrap();
        n.ajustar_hoje(t, None, -20 * MIN, None, h(9, 20)).unwrap();
        n.ajustar_hoje(outro, None, 5 * MIN, None, h(9, 30)).unwrap();
        let z = n.descartar_hoje(t, h(10, 0)).unwrap();
        assert_eq!((z.trechos.len(), z.ajustes.len()), (1, 2));
        let s = n.snapshot(h(10, 1)).unwrap();
        let tm = |s: &Snapshot, id: i64| s.grupos[0].timers.iter().find(|x| x.id == id).unwrap().clone();
        assert_eq!((tm(&s, t).ajuste_ms, tm(&s, t).ajustes.len()), (0, 0), "descartado: nada de ajuste");
        assert_eq!(tm(&s, outro).ajuste_ms, 5 * MIN, "o outro timer fica como estava");
        n.desfazer_descartar(&z.trechos, &z.ajustes).unwrap();
        let s = n.snapshot(h(10, 1)).unwrap();
        assert_eq!((tm(&s, t).ajuste_ms, tm(&s, t).trechos.len()), (10 * MIN, 1));
    }

    /// O tempo do timer desde o zerar, como a tela soma: os dias fechados do snapshot mais hoje (os
    /// trechos a partir do zerar e os ajustes feitos depois dele), cada atividade nunca negativa e
    /// desde o seu zerar (o do timer ou o só dela, o mais recente; rodada 19).
    fn por_atividade_na_tela(s: &Snapshot, id: i64) -> std::collections::BTreeMap<i64, Ms> {
        let t = s.grupos.iter().flat_map(|g| &g.timers).find(|t| t.id == id).unwrap();
        let zerado = |a: i64| {
            t.zerados_atividade.iter().find(|z| z.atividade_id == a).map_or(t.zerado_em, |z| z.zerado_em.max(t.zerado_em))
        };
        let mut por: std::collections::BTreeMap<i64, Vec<(Ms, Ms)>> = Default::default();
        for x in &t.trechos {
            let i = x.inicio.max(zerado(x.atividade_id));
            por.entry(x.atividade_id).or_default().push((i, x.fim.unwrap_or(s.agora).max(i)));
        }
        let mut ajuste: std::collections::BTreeMap<i64, Ms> = Default::default();
        for a in t.ajustes.iter().filter(|a| a.criado_em >= zerado(a.atividade_id)) {
            *ajuste.entry(a.atividade_id).or_default() += a.delta_ms;
        }
        let ids: BTreeSet<i64> = por.keys().chain(ajuste.keys()).chain(t.acumulado_antes.iter().map(|a| &a.atividade_id)).copied().collect();
        ids.iter()
            .map(|a| {
                let u = nucleo::relatorio::uniao_ms(por.get(a).map(Vec::as_slice).unwrap_or(&[]));
                let antes: Ms = t.acumulado_antes.iter().filter(|x| x.atividade_id == *a).map(|x| x.ms).sum();
                (*a, u + nucleo::relatorio::ajuste_efetivo(u, ajuste.get(a).copied().unwrap_or(0)) + antes)
            })
            .collect()
    }

    /// O número do timer na tela: a soma das atividades (`por_atividade_na_tela`).
    fn acumulado_na_tela(s: &Snapshot, id: i64) -> Ms {
        por_atividade_na_tela(s, id).values().sum()
    }

    /// O total do dia, como a tela faz: a união de todos os trechos de hoje (sem ajustes).
    fn total_do_dia(s: &Snapshot) -> Ms {
        let v: Vec<(Ms, Ms)> =
            s.grupos.iter().flat_map(|g| &g.timers).flat_map(|t| &t.trechos).map(|x| (x.inicio, x.fim.unwrap_or(s.agora))).collect();
        nucleo::relatorio::uniao_ms(&v)
    }

    /// Um timer com o zerar dado.
    fn timer_zerado(n: &Nucleo, nome: &str, zerado: Ms) -> i64 {
        let t = n.banco.criar_timer(None, nome, "#a00").unwrap();
        n.banco.zerar(Some(&[t]), zerado).unwrap();
        t
    }

    /// Revisão da rodada 11 (I1): a sessão que começou às 9:00, ligada "desde o início" a um
    /// timer criado às 9:30, conta no número dele desde as 9:00.
    #[test]
    fn timer_criado_depois_conta_a_sessao_ligada_desde_o_inicio() {
        let mut n = nucleo(h(9, 0));
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", "G:\\Cliente A"));
        n.aplicar(&mud).unwrap();
        // o Claude trabalhando (ferramentas a cada 8 min), ainda sem timer
        for m in [8, 16, 24] {
            let mud = n.motor.evento(&ev(h(9, m), "PostToolUse", "s1", "G:\\Cliente A"));
            n.aplicar(&mud).unwrap();
        }
        // às 9:30, "+ Novo timer…" (o que o comando `criar_timer` faz) e a ligação desde o início
        let fuso = n.fuso_min;
        let t = n.banco.criar_timer_em(None, "Cliente A", "#a00", h(9, 30), fuso).unwrap();
        assert_eq!(n.banco.zerados().unwrap()[&t], h(0, 0), "zerado no início do dia de trabalho");
        n.ligar_sessao("s1", t, true, false, h(9, 30)).unwrap();
        let mud = n.motor.evento(&ev(h(9, 32), "PostToolUse", "s1", "G:\\Cliente A"));
        n.aplicar(&mud).unwrap();
        // o Claude terminou às 9:40
        let mud = n.motor.evento(&ev(h(9, 40), "Stop", "s1", "G:\\Cliente A"));
        n.aplicar(&mud).unwrap();
        let s = n.snapshot(h(9, 50)).unwrap();
        assert_eq!(acumulado_na_tela(&s, t), 40 * MIN, "9:00 às 9:40, e não só 9:30 às 9:40");
    }

    #[test]
    fn zerar_com_o_timer_rodando_segue_rodando_e_recomeca_de_zero() {
        let mut n = nucleo(h(9, 0));
        let t = timer_zerado(&n, "A", h(8, 0));
        let mud = n.motor.play(t, h(9, 0));
        n.aplicar(&mud).unwrap();
        n.ajustar_hoje(t, None, 15 * MIN, None, h(9, 30)).unwrap();
        assert_eq!(acumulado_na_tela(&n.snapshot(h(10, 0)).unwrap(), t), 75 * MIN);
        let z = n.zerar(t, h(10, 0)).unwrap();
        assert_eq!(z, Zerado { em: h(10, 0), antes: vec![(t, h(8, 0))], ..Default::default() });
        // nada apagado e nada parou: o mesmo trecho aberto, o ajuste continua no dia
        assert_eq!(n.motor.abertos.len(), 1);
        assert_eq!(n.banco.abertos().unwrap().len(), 1);
        let s = n.snapshot(h(10, 20)).unwrap();
        assert!(s.grupos[0].timers[0].rodando);
        assert_eq!(s.grupos[0].timers[0].ajuste_ms, 15 * MIN);
        assert_eq!(acumulado_na_tela(&s, t), 20 * MIN, "de 0:00 às 10:00");
        // o total do dia continua o de hoje (9:00 às 10:20)
        assert_eq!(total_do_dia(&s), 80 * MIN);
        // um ajuste feito depois do zerar entra
        n.ajustar_hoje(t, None, 5 * MIN, None, h(10, 21)).unwrap();
        assert_eq!(acumulado_na_tela(&n.snapshot(h(10, 30)).unwrap(), t), 35 * MIN);
        // desfazer: volta ao de antes
        n.desfazer_zerar(&z).unwrap();
        assert_eq!(acumulado_na_tela(&n.snapshot(h(10, 30)).unwrap(), t), 110 * MIN);
        assert!(n.zerar(999, h(10, 0)).is_err());
    }

    #[test]
    fn zerar_todos_e_desfazer() {
        let mut n = nucleo(h(9, 0));
        let a = timer_zerado(&n, "A", h(0, 0));
        let b = timer_zerado(&n, "B", h(0, 0));
        let arq = timer_zerado(&n, "Velho", 5);
        let mut t = n.banco.timers().unwrap().into_iter().find(|t| t.id == arq).unwrap();
        t.arquivado = true;
        n.banco.editar_timer(&t).unwrap();
        for (id, i) in [(a, h(8, 0)), (b, h(8, 30))] {
            let m = n.motor.play(id, i);
            n.aplicar(&m).unwrap();
        }
        let s = n.snapshot(h(9, 0)).unwrap();
        assert_eq!((acumulado_na_tela(&s, a), acumulado_na_tela(&s, b)), (60 * MIN, 30 * MIN));
        let z = n.zerar_todos(None, h(9, 0)).unwrap();
        assert_eq!(z.antes, vec![(a, h(0, 0)), (b, h(0, 0)), (arq, 5)], "os arquivados também");
        let s = n.snapshot(h(9, 10)).unwrap();
        assert_eq!((acumulado_na_tela(&s, a), acumulado_na_tela(&s, b)), (10 * MIN, 10 * MIN));
        assert_eq!(total_do_dia(&s), 70 * MIN, "o dia não muda");
        assert!(n.banco.zerados().unwrap().values().all(|v| *v == h(9, 0)));
        n.desfazer_zerar(&z).unwrap();
        assert_eq!(n.banco.zerados().unwrap().into_iter().collect::<Vec<_>>(), [(a, h(0, 0)), (b, h(0, 0)), (arq, 5)]);
    }

    #[test]
    fn zerar_sozinho_no_corte_e_ao_abrir_depois_do_corte() {
        let d18 = |hh, mm| ms_local(2026, 11, 18, hh, mm, F);
        let mut n = nucleo(h(9, 0));
        let t = timer_zerado(&n, "A", h(0, 0));
        let m = n.motor.play(t, h(9, 0));
        n.aplicar(&m).unwrap();
        // desligado (o padrão): a virada não zera
        assert!(!n.zerar_na_virada(d18(0, 0) + 1000).unwrap());
        assert_eq!(n.banco.zerados().unwrap()[&t], h(0, 0));
        // ligar não zera na hora: conta a partir do corte de hoje
        let cfg = ConfigApp { zerar_na_virada: true, ..n.banco.config_app().unwrap() };
        let dir = tempfile::tempdir().unwrap();
        n.salvar_config(&cfg, &dir.path().join("hook.json")).unwrap();
        assert!(!n.zerar_na_virada(h(23, 0)).unwrap());
        assert_eq!(n.banco.zerados().unwrap()[&t], h(0, 0));
        assert!(!n.zerar_na_virada(h(23, 59)).unwrap(), "ainda o mesmo dia");
        // o corte do dia 18: zera no instante do corte
        assert!(n.zerar_na_virada(d18(0, 0) + 4000).unwrap());
        assert_eq!(n.banco.zerados().unwrap()[&t], d18(0, 0));
        assert!(!n.zerar_na_virada(d18(0, 0) + 9000).unwrap(), "uma vez por corte");
        // um zerar à mão depois do corte fica
        n.zerar(t, d18(10, 0)).unwrap();
        // o app fechado de 18 às 10:00 até 20 às 9:00: ao abrir, vale o corte mais recente (o 20)
        let banco = std::mem::replace(&mut n.banco, Banco::abrir_memoria().unwrap());
        let n = Nucleo::abrir(banco, ms_local(2026, 11, 20, 9, 0, F), F, dia()).unwrap();
        assert_eq!(n.banco.zerados().unwrap()[&t], ms_local(2026, 11, 20, 0, 0, F));
        // desligar tira a marca: ligar de novo conta a partir do próximo corte
        let mut n = n;
        let cfg = ConfigApp { zerar_na_virada: false, ..n.banco.config_app().unwrap() };
        n.salvar_config(&cfg, &dir.path().join("hook.json")).unwrap();
        assert!(!n.zerar_na_virada(ms_local(2026, 11, 20, 9, 1, F)).unwrap());
        assert_eq!(n.banco.valor(CHAVE_ZERADO_AUTO).unwrap().as_deref(), Some(""));
    }

    /// Rodada 19: um timer com 2h de trabalho (8:00 às 10:00) e a reunião manual desde as 10:00,
    /// aberta. Zerado no início do dia.
    fn trabalho_e_reuniao(n: &mut Nucleo) -> i64 {
        let t = timer_zerado(n, "Cliente A", h(0, 0));
        let m = n.motor.play_atividade(t, ATIVIDADE_TRABALHO, h(8, 0));
        n.aplicar(&m).unwrap();
        let m = n.motor.play_atividade(t, ATIVIDADE_REUNIAO, h(10, 0));
        n.aplicar(&m).unwrap();
        t
    }

    /// (trabalho, reunião) do timer na tela.
    fn tr_reu(s: &Snapshot, t: i64) -> (Ms, Ms) {
        let v = por_atividade_na_tela(s, t);
        (v.get(&ATIVIDADE_TRABALHO).copied().unwrap_or(0), v.get(&ATIVIDADE_REUNIAO).copied().unwrap_or(0))
    }

    #[test]
    fn zerar_so_a_reuniao_mantem_o_trabalho() {
        let mut n = nucleo(h(8, 0));
        let t = trabalho_e_reuniao(&mut n);
        let z = n.zerar_atividade(t, ATIVIDADE_REUNIAO, h(10, 30)).unwrap();
        assert_eq!(z, Zerado { em: h(10, 30), atividade: Some((t, ATIVIDADE_REUNIAO, None)), ..Default::default() });
        let s = n.snapshot(h(10, 40)).unwrap();
        assert_eq!(tr_reu(&s, t), (120 * MIN, 10 * MIN), "o trabalho fica; a reunião conta das 10:30");
        assert_eq!(acumulado_na_tela(&s, t), 130 * MIN, "o timer é a soma das atividades");
        assert_eq!(total_do_dia(&s), 160 * MIN, "o dia não muda: 8:00 às 10:40");
        let tm = s.grupos.iter().flat_map(|g| &g.timers).find(|x| x.id == t).unwrap();
        assert_eq!(tm.trechos.len(), 2, "nada apagado nem escondido");
        assert_eq!(tm.zerados_atividade, [ZeradoAtividadeDto { atividade_id: ATIVIDADE_REUNIAO, zerado_em: h(10, 30) }]);
        assert!(tm.rodando, "a reunião segue rodando");
        // o contrário: zerar só o trabalho mantém a reunião
        n.zerar_atividade(t, ATIVIDADE_TRABALHO, h(10, 45)).unwrap();
        assert_eq!(tr_reu(&n.snapshot(h(10, 50)).unwrap(), t), (0, 20 * MIN));
        // atividade ou timer que não existe
        assert!(n.zerar_atividade(t, 999, h(11, 0)).is_err());
        assert!(n.zerar_atividade(999, ATIVIDADE_REUNIAO, h(11, 0)).is_err());
    }

    #[test]
    fn zerar_o_timer_todo_depois_de_uma_atividade() {
        let mut n = nucleo(h(8, 0));
        let t = trabalho_e_reuniao(&mut n);
        n.zerar_atividade(t, ATIVIDADE_REUNIAO, h(10, 30)).unwrap();
        let z = n.zerar(t, h(10, 45)).unwrap();
        assert_eq!(z.limpas, [(t, ATIVIDADE_REUNIAO, h(10, 30))], "o zerar da reunião sai");
        assert!(n.banco.zerados_atividade().unwrap().is_empty());
        let s = n.snapshot(h(10, 50)).unwrap();
        assert_eq!(tr_reu(&s, t), (0, 5 * MIN), "tudo desde as 10:45");
        assert!(s.grupos[0].timers[0].zerados_atividade.is_empty());
        assert_eq!(total_do_dia(&s), 170 * MIN);
        // desfazer: o timer volta e o zerar da reunião também
        n.desfazer_zerar(&z).unwrap();
        assert_eq!(n.banco.zerados_atividade().unwrap()[&(t, ATIVIDADE_REUNIAO)], h(10, 30));
        assert_eq!(tr_reu(&n.snapshot(h(10, 50)).unwrap(), t), (120 * MIN, 20 * MIN));
    }

    #[test]
    fn zerar_uma_atividade_depois_do_timer_todo() {
        let mut n = nucleo(h(8, 0));
        let t = trabalho_e_reuniao(&mut n);
        n.zerar(t, h(9, 0)).unwrap();
        n.zerar_atividade(t, ATIVIDADE_TRABALHO, h(9, 30)).unwrap();
        let s = n.snapshot(h(10, 30)).unwrap();
        // o trabalho desde as 9:30 (o mais recente), a reunião desde as 9:00 (o do timer)
        assert_eq!(tr_reu(&s, t), (30 * MIN, 30 * MIN));
        assert_eq!(acumulado_na_tela(&s, t), 60 * MIN);
        // um zerar da atividade mais antigo que o do timer não vale (e não vai na tela)
        n.zerar_atividade(t, ATIVIDADE_REUNIAO, h(8, 30)).unwrap();
        let s = n.snapshot(h(10, 30)).unwrap();
        assert_eq!(tr_reu(&s, t), (30 * MIN, 30 * MIN));
        assert_eq!(s.grupos[0].timers[0].zerados_atividade.len(), 1);
    }

    #[test]
    fn desfazer_o_zerar_de_uma_atividade() {
        let mut n = nucleo(h(8, 0));
        let t = trabalho_e_reuniao(&mut n);
        let z1 = n.zerar_atividade(t, ATIVIDADE_REUNIAO, h(10, 20)).unwrap();
        let z2 = n.zerar_atividade(t, ATIVIDADE_REUNIAO, h(10, 40)).unwrap();
        assert_eq!(z2.atividade, Some((t, ATIVIDADE_REUNIAO, Some(h(10, 20)))));
        assert!(z2.antes.is_empty() && z2.limpas.is_empty(), "o timer todo não muda");
        // o "Desfazer" de um antigo não desfaz um zerar mais novo
        n.desfazer_zerar(&z1).unwrap();
        assert_eq!(n.banco.zerados_atividade().unwrap()[&(t, ATIVIDADE_REUNIAO)], h(10, 40));
        // o do mais novo volta ao de antes; o do primeiro, a nenhum
        n.desfazer_zerar(&z2).unwrap();
        assert_eq!(tr_reu(&n.snapshot(h(10, 50)).unwrap(), t), (120 * MIN, 30 * MIN));
        n.desfazer_zerar(&z1).unwrap();
        assert!(n.banco.zerados_atividade().unwrap().is_empty());
        assert_eq!(tr_reu(&n.snapshot(h(10, 50)).unwrap(), t), (120 * MIN, 50 * MIN));
        // o que vai para a tela e volta (o JSON do "Desfazer")
        let z = n.zerar_atividade(t, ATIVIDADE_REUNIAO, h(10, 50)).unwrap();
        let ida = serde_json::to_string(&z).unwrap();
        assert_eq!(serde_json::from_str::<Zerado>(&ida).unwrap(), z);
        // um Zerado de antes da rodada 19 (sem os campos novos) ainda se lê
        let velho: Zerado = serde_json::from_str(&format!("{{\"em\":5,\"antes\":[[{t},1]]}}")).unwrap();
        assert_eq!((velho.limpas.len(), velho.atividade), (0, None));
    }

    /// Rodada 20: o espaço "Pessoal" com um timer num grupo e um sem grupo; o "Geral" com um.
    fn dois_espacos(n: &mut Nucleo) -> (i64, i64, i64, i64) {
        let pessoal = n.criar_espaco("Pessoal", "#e86bd0").unwrap();
        let casa = n.banco.criar_grupo_em("Casa", "#111", pessoal).unwrap();
        let obra = timer_zerado(n, "Obra", h(0, 0));
        let mut t = n.banco.timers().unwrap().into_iter().find(|t| t.id == obra).unwrap();
        t.grupo_id = Some(casa);
        n.editar_timer(&t, h(8, 0)).unwrap();
        let leitura = n.banco.criar_timer_no(None, pessoal, "Leitura", "#222", h(8, 0), F).unwrap();
        n.banco.zerar(Some(&[leitura]), h(0, 0)).unwrap();
        let flow = timer_zerado(n, "Flow", h(0, 0));
        (pessoal, obra, leitura, flow)
    }

    #[test]
    fn zerar_todos_e_pausar_tudo_valem_para_o_espaco_da_aba() {
        let mut n = nucleo(h(8, 0));
        let (pessoal, obra, leitura, flow) = dois_espacos(&mut n);
        assert_eq!(n.banco.espacos_dos_timers().unwrap()[&obra], pessoal, "o timer do grupo segue o espaço do grupo");
        for id in [obra, leitura, flow] {
            let m = n.motor.play(id, h(8, 0));
            n.aplicar(&m).unwrap();
        }
        // "Zerar todos (Pessoal)": só os do Pessoal
        let z = n.zerar_todos(Some(pessoal), h(9, 0)).unwrap();
        assert_eq!(z.antes.iter().map(|(t, _)| *t).collect::<Vec<_>>(), [obra, leitura]);
        let s = n.snapshot(h(9, 30)).unwrap();
        assert_eq!(
            (acumulado_na_tela(&s, obra), acumulado_na_tela(&s, leitura), acumulado_na_tela(&s, flow)),
            (30 * MIN, 30 * MIN, 90 * MIN)
        );
        // o Desfazer volta só os dois
        n.desfazer_zerar(&z).unwrap();
        assert_eq!(acumulado_na_tela(&n.snapshot(h(9, 30)).unwrap(), obra), 90 * MIN);
        // "Pausar tudo (Pessoal)": o Flow segue rodando
        let m = n.pausar_tudo(Some(pessoal), h(10, 0)).unwrap();
        n.aplicar(&m).unwrap();
        let s = n.snapshot(h(10, 0)).unwrap();
        let rodando = |id: i64| s.grupos.iter().flat_map(|g| &g.timers).find(|t| t.id == id).unwrap().rodando;
        assert_eq!((rodando(obra), rodando(leitura), rodando(flow)), (false, false, true));
        // "Todos": tudo, como antes
        let m = n.pausar_tudo(None, h(10, 5)).unwrap();
        n.aplicar(&m).unwrap();
        assert!(n.motor.abertos.is_empty());
        let z = n.zerar_todos(None, h(10, 10)).unwrap();
        assert_eq!(z.antes.len(), 3);
        // espaço que não existe
        assert!(n.zerar_todos(Some(999), h(10, 20)).is_err());
        assert!(n.pausar_tudo(Some(999), h(10, 20)).is_err());
    }

    /// Rodada 21: mover leva o histórico junto; os totais dos espaços são pela posição de agora.
    #[test]
    fn mover_grupo_e_timer_entre_espacos_com_os_totais_e_o_desfazer() {
        let mut n = nucleo(h(8, 0));
        let (pessoal, obra, leitura, flow) = dois_espacos(&mut n);
        n.banco.zerar(None, 0).unwrap();
        let ontem = |hh, mm| ms_local(2026, 11, 16, hh, mm, F);
        for (t, i, f) in [(obra, ontem(9, 0), ontem(10, 0)), (leitura, ontem(11, 0), ontem(11, 30)), (flow, ontem(9, 0), ontem(9, 20))] {
            n.banco.aplicar(&[Mudanca::Abriu { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, inicio: i, atividade_id: 1 }]).unwrap();
            n.banco.aplicar(&[Mudanca::Fechou { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, fim: f, motivo: MotivoFim::Pausa }]).unwrap();
        }
        // hoje: Obra das 8:00 às 8:30
        let m = n.motor.play(obra, h(8, 0));
        n.aplicar(&m).unwrap();
        let m = n.motor.pausar(obra, h(8, 30));
        n.aplicar(&m).unwrap();
        let trechos = |n: &Nucleo| n.banco.trechos_entre(0, h(23, 0)).unwrap();
        let antes_de_tudo = trechos(&n);
        // (a soma dos timers de cada espaço, o tempo real de cada espaço nos dias fechados)
        let totais = |n: &Nucleo| {
            let s = n.snapshot(h(9, 0)).unwrap();
            let soma = |e: i64| -> Ms {
                s.grupos.iter().filter(|g| g.espaco_id == e).flat_map(|g| &g.timers).map(|t| acumulado_na_tela(&s, t.id)).sum()
            };
            let real = |e: i64| s.espacos.iter().find(|x| x.id == e).unwrap().real_antes_ms;
            ((soma(ESPACO_PRIMEIRO), soma(pessoal)), (real(ESPACO_PRIMEIRO), real(pessoal)))
        };
        assert_eq!(totais(&n), ((20 * MIN, 120 * MIN), (20 * MIN, 90 * MIN)), "antes: Flow no Geral; Obra e Leitura no Pessoal");
        // o grupo Casa (com a Obra) vai para o Geral: as horas dela, de ontem também, vão junto
        let casa = n.banco.grupos().unwrap().into_iter().find(|g| g.espaco_id == pessoal).unwrap().id;
        let m = n.mover_grupo(casa, ESPACO_PRIMEIRO).unwrap();
        assert_eq!(
            m,
            Movido { tipo: "grupo".into(), id: casa, grupo_id: None, espaco_id: pessoal, para_grupo_id: None, para_espaco_id: ESPACO_PRIMEIRO }
        );
        assert_eq!(totais(&n), ((110 * MIN, 30 * MIN), (60 * MIN, 30 * MIN)), "o tempo real do Geral: a união de Flow e Obra ontem");
        assert_eq!(trechos(&n), antes_de_tudo, "nenhum trecho muda");
        // o Desfazer volta tudo
        n.desfazer_mover(&m).unwrap();
        assert_eq!(totais(&n), ((20 * MIN, 120 * MIN), (20 * MIN, 90 * MIN)));
        // o timer Leitura vai para o "Sem grupo" do Geral; depois, para o grupo Casa (Pessoal)
        let m = n.mover_timer(leitura, None, ESPACO_PRIMEIRO).unwrap();
        assert_eq!(
            m,
            Movido { tipo: "timer".into(), id: leitura, grupo_id: None, espaco_id: pessoal, para_grupo_id: None, para_espaco_id: ESPACO_PRIMEIRO }
        );
        assert_eq!(totais(&n), ((50 * MIN, 90 * MIN), (50 * MIN, 60 * MIN)));
        assert_eq!(trechos(&n), antes_de_tudo);
        n.desfazer_mover(&m).unwrap();
        assert_eq!(totais(&n), ((20 * MIN, 120 * MIN), (20 * MIN, 90 * MIN)));
        let m = n.mover_timer(flow, Some(casa), ESPACO_PRIMEIRO).unwrap();
        assert_eq!((m.grupo_id, m.espaco_id), (None, ESPACO_PRIMEIRO));
        assert_eq!(n.banco.espacos_dos_timers().unwrap()[&flow], pessoal, "o espaço do grupo vale");
        assert_eq!(totais(&n).0, (0, 140 * MIN));
        n.desfazer_mover(&m).unwrap();
        assert_eq!(n.banco.timers().unwrap().into_iter().find(|t| t.id == flow).unwrap().grupo_id, None);
        // recusas: espaço arquivado ou que não existe, grupo ou timer que não existe
        let velho = n.criar_espaco("Velho", "#333").unwrap();
        let mut e = n.banco.espacos().unwrap().into_iter().find(|e| e.id == velho).unwrap();
        e.arquivado = true;
        n.editar_espaco(&e, h(9, 0)).unwrap();
        assert!(n.mover_grupo(casa, velho).is_err());
        assert!(n.mover_grupo(casa, 999).is_err());
        assert!(n.mover_grupo(999, ESPACO_PRIMEIRO).is_err());
        assert!(n.mover_timer(leitura, None, velho).is_err());
        assert!(n.mover_timer(leitura, Some(999), ESPACO_PRIMEIRO).is_err());
        assert!(n.mover_timer(999, None, ESPACO_PRIMEIRO).is_err());
        // o Movido vai e volta pelo JSON (o Desfazer da tela)
        let ida = serde_json::to_string(&m).unwrap();
        assert_eq!(serde_json::from_str::<Movido>(&ida).unwrap(), m);
    }

    #[test]
    fn editar_timer_num_espaco_arquivado_so_recusa_destino_novo_e_nada_fica_pela_metade() {
        let mut n = nucleo(h(8, 0));
        let (pessoal, obra, leitura, flow) = dois_espacos(&mut n);
        let mut e = n.banco.espacos().unwrap().into_iter().find(|e| e.id == pessoal).unwrap();
        e.arquivado = true;
        n.editar_espaco(&e, h(9, 0)).unwrap();
        let timer = |n: &Nucleo, id: i64| n.banco.timers().unwrap().into_iter().find(|t| t.id == id).unwrap();
        // a Leitura (Sem grupo do Pessoal, arquivado): renomear sem trocar o lugar funciona
        let mut t = timer(&n, leitura);
        t.nome = "Leitura nova".into();
        n.editar_timer(&t, h(9, 0)).unwrap();
        assert_eq!(timer(&n, leitura).nome, "Leitura nova");
        assert_eq!(n.banco.espacos_dos_timers().unwrap()[&leitura], pessoal);
        // a Obra (grupo Casa, do Pessoal) também
        let mut o = timer(&n, obra);
        o.nome = "Obra 2".into();
        n.editar_timer(&o, h(9, 0)).unwrap();
        // um destino novo num espaço arquivado: recusa, com o que fazer, e nada muda
        let mut f = timer(&n, flow);
        f.grupo_id = None;
        f.espaco_id = pessoal;
        f.nome = "Flow 2".into();
        assert_eq!(n.checar_editar_timer(&f, None).map(|_| ()), Err("Escolha outro grupo: Pessoal está arquivado.".into()));
        assert_eq!(n.editar_timer(&f, h(9, 0)), Err("Escolha outro grupo: Pessoal está arquivado.".into()));
        assert_eq!(timer(&n, flow).nome, "Flow");
        assert_eq!(n.banco.espacos_dos_timers().unwrap()[&flow], ESPACO_PRIMEIRO);
        // o ajuste se confere sem gravar
        assert!(n.checar_ajuste(leitura, None, 10 * MIN, None).is_ok());
        assert!(n.checar_ajuste(leitura, None, 0, None).is_err());
        assert!(n.checar_ajuste(999, None, 10 * MIN, None).is_err());
        assert!(n.banco.ajustes_do_dia(&n.dia_texto(h(9, 0))).unwrap().is_empty(), "conferir não grava");
        // o grupo novo num espaço arquivado também é destino novo
        assert_eq!(n.checar_destino(pessoal), Err("Escolha outro grupo: Pessoal está arquivado.".into()));
        assert!(n.checar_destino(ESPACO_PRIMEIRO).is_ok());
    }

    #[test]
    fn desfazer_mover_so_com_ele_no_lugar_e_o_de_antes_ativo() {
        let mut n = nucleo(h(8, 0));
        let (pessoal, _obra, leitura, flow) = dois_espacos(&mut n);
        let onde = |n: &Nucleo, id: i64| {
            let t = n.banco.timers().unwrap().into_iter().find(|t| t.id == id).unwrap();
            (t.grupo_id, n.banco.espacos_dos_timers().unwrap().get(&id).copied().unwrap_or(t.espaco_id))
        };
        // moveu de novo depois: o Desfazer do primeiro mover não age (nada muda) e diz por quê
        let m = n.mover_timer(leitura, None, ESPACO_PRIMEIRO).unwrap();
        assert_eq!((m.para_grupo_id, m.para_espaco_id), (None, ESPACO_PRIMEIRO));
        n.mover_timer(leitura, None, pessoal).unwrap();
        assert_eq!(n.desfazer_mover(&m), Err(DESFAZER_JA_MUDOU().into()));
        assert_eq!(onde(&n, leitura), (None, pessoal));
        // o espaço de antes foi arquivado: o Desfazer recusa e o timer fica onde está
        let velho = n.criar_espaco("Velho", "#333").unwrap();
        n.mover_timer(flow, None, velho).unwrap();
        let m = n.mover_timer(flow, None, ESPACO_PRIMEIRO).unwrap();
        assert_eq!(m.espaco_id, velho);
        let mut e = n.banco.espacos().unwrap().into_iter().find(|e| e.id == velho).unwrap();
        e.arquivado = true;
        n.editar_espaco(&e, h(9, 0)).unwrap();
        let recusa = n.desfazer_mover(&m).unwrap_err();
        assert!(recusa.starts_with("Não dá para desfazer: o espaço Velho está arquivado"), "{recusa}");
        assert_eq!(onde(&n, flow), (None, ESPACO_PRIMEIRO));
        // o grupo: movido de novo, ou o espaço de antes arquivado, também não volta
        let casa = n.banco.grupos().unwrap().into_iter().find(|g| g.espaco_id == pessoal).unwrap().id;
        let m = n.mover_grupo(casa, ESPACO_PRIMEIRO).unwrap();
        n.mover_grupo(casa, pessoal).unwrap();
        assert_eq!(n.desfazer_mover(&m), Err(DESFAZER_JA_MUDOU().into()));
        let m = n.mover_grupo(casa, ESPACO_PRIMEIRO).unwrap();
        let mut e = n.banco.espacos().unwrap().into_iter().find(|e| e.id == pessoal).unwrap();
        e.arquivado = true;
        n.editar_espaco(&e, h(9, 0)).unwrap();
        assert!(n.desfazer_mover(&m).unwrap_err().starts_with("Não dá para desfazer: o espaço"));
        assert_eq!(n.banco.espacos_dos_grupos().unwrap()[&casa], ESPACO_PRIMEIRO);
        // no lugar e com o de antes ativo: volta
        let m = n.mover_timer(leitura, None, ESPACO_PRIMEIRO).unwrap();
        e.arquivado = false;
        n.editar_espaco(&e, h(9, 0)).unwrap();
        n.desfazer_mover(&m).unwrap();
        assert_eq!(onde(&n, leitura), (None, pessoal));
        // um Movido sem o destino (de antes desta versão) não desfaz às cegas
        let velho_json = format!(r#"{{"tipo":"timer","id":{leitura},"grupo_id":null,"espaco_id":{ESPACO_PRIMEIRO}}}"#);
        let sem_destino: Movido = serde_json::from_str(&velho_json).unwrap();
        assert_eq!(n.desfazer_mover(&sem_destino), Err(DESFAZER_JA_MUDOU().into()));
    }

    #[test]
    fn apagar_espaco_so_sem_nada_a_vista_e_os_arquivados_vao_junto() {
        let mut n = nucleo(h(8, 0));
        let (pessoal, obra, leitura, _) = dois_espacos(&mut n);
        // com grupo ou timer à vista: recusa, com o texto da dica dos Ajustes
        assert_eq!(n.apagar_espaco(pessoal).unwrap_err(), MOVA_OS_GRUPOS());
        let s = n.snapshot(h(8, 0)).unwrap();
        let e = s.espacos.iter().find(|e| e.id == pessoal).unwrap();
        assert_eq!((e.ativos, e.arquivados), (2, 0), "a tela sabe o mesmo que o núcleo");
        // arquivar o grupo e o timer sem grupo: os dois vão, ainda arquivados, para o Geral
        let mut g = n.banco.grupos().unwrap().into_iter().find(|g| g.espaco_id == pessoal).unwrap();
        g.arquivado = true;
        n.editar_grupo(&g, h(9, 0)).unwrap();
        let mut t = n.banco.timers().unwrap().into_iter().find(|t| t.id == leitura).unwrap();
        t.arquivado = true;
        n.editar_timer(&t, h(9, 0)).unwrap();
        let s = n.snapshot(h(9, 0)).unwrap();
        let e = s.espacos.iter().find(|e| e.id == pessoal).unwrap();
        assert_eq!((e.ativos, e.arquivados), (0, 2));
        assert_eq!(n.destino_ao_apagar(pessoal).unwrap(), Some(ESPACO_PRIMEIRO));
        n.guardar_dock(|c| c.espaco_aba = Some(pessoal)).unwrap();
        assert_eq!(n.apagar_espaco(pessoal).unwrap(), 2);
        assert_eq!(n.banco.espacos().unwrap().len(), 1);
        assert_eq!(n.banco.espacos_dos_timers().unwrap()[&obra], ESPACO_PRIMEIRO, "o timer do grupo arquivado foi com ele");
        assert_eq!(n.banco.espacos_dos_timers().unwrap()[&leitura], ESPACO_PRIMEIRO);
        assert!(!n.banco.timers().unwrap().iter().any(|t| t.id == leitura), "segue arquivado");
        assert_eq!(n.banco.config_app().unwrap().espaco_aba, None, "a aba dele volta a Todos");
        // o último espaço não arquivado nunca: nem apagar nem arquivar
        assert_eq!(n.apagar_espaco(ESPACO_PRIMEIRO).unwrap_err(), ULTIMO_ESPACO());
        let mut e = n.banco.espacos().unwrap()[0].clone();
        e.arquivado = true;
        assert!(n.editar_espaco(&e, h(9, 0)).is_err());
        // um arquivado pode ser apagado (os itens dele vão para o que sobra)
        let velho = n.criar_espaco("Velho", "#333").unwrap();
        let mut v = n.banco.espacos().unwrap().into_iter().find(|e| e.id == velho).unwrap();
        v.arquivado = true;
        n.editar_espaco(&v, h(9, 0)).unwrap();
        assert_eq!(n.apagar_espaco(velho).unwrap(), 0);
        // nome vazio ou repetido
        assert!(n.criar_espaco("  ", "#111").is_err());
        assert!(n.criar_espaco("geral", "#111").is_err());
        // grupo num espaço arquivado ou que não existe: recusa
        let outro = n.criar_espaco("Outro", "#222").unwrap();
        let mut e = n.banco.espacos().unwrap().into_iter().find(|e| e.id == outro).unwrap();
        e.arquivado = true;
        n.editar_espaco(&e, h(9, 0)).unwrap();
        g.arquivado = false;
        g.espaco_id = outro;
        assert!(n.editar_grupo(&g, h(9, 0)).is_err());
        g.espaco_id = 999;
        assert!(n.editar_grupo(&g, h(9, 0)).is_err());
    }

    /// Revisão da rodada 24: o nome do grupo vai até `GRUPO_NOME_MAX` letras, no grupo novo e no
    /// editado; um nome de antes, mais comprido e sem mudar, segue valendo.
    #[test]
    fn nome_do_grupo_tem_limite() {
        let mut n = nucleo(h(8, 0));
        let cabe = "a".repeat(GRUPO_NOME_MAX);
        let passa = format!("{cabe}b");
        assert!(n.checar_nome_de_grupo(&cabe, None).is_ok());
        assert_eq!(n.checar_nome_de_grupo(&passa, None).unwrap_err(), tfa("erro.nome_max", &["60"]));
        // letras, não bytes
        assert!(n.checar_nome_de_grupo(&"é".repeat(GRUPO_NOME_MAX), None).is_ok());
        // editar: o nome novo comprido não grava
        let id = n.banco.criar_grupo("Casa", "#111").unwrap();
        let mut g = n.banco.grupos().unwrap().into_iter().find(|g| g.id == id).unwrap();
        g.nome = passa.clone();
        assert!(n.editar_grupo(&g, h(9, 0)).is_err());
        assert_eq!(n.banco.grupos().unwrap().iter().find(|g| g.id == id).unwrap().nome, "Casa");
        // um nome de antes (gravado direto no banco), sem mudar: a cor muda
        g.nome = passa.clone();
        n.banco.editar_grupo(&g).unwrap();
        g.cor = "#222".into();
        n.editar_grupo(&g, h(9, 0)).unwrap();
        assert_eq!(n.banco.grupos().unwrap().iter().find(|g| g.id == id).unwrap().cor, "#222");
        // mudar para outro nome comprido, não
        g.nome = format!("{passa}c");
        assert!(n.editar_grupo(&g, h(9, 0)).is_err());
    }

    /// Revisão da rodada 20: o timer rodando num grupo ou espaço arquivado segue à vista (no
    /// "Sem grupo" do espaço dele), no dock e no total do dia.
    #[test]
    fn timer_rodando_em_grupo_ou_espaco_arquivado_segue_a_vista() {
        let mut n = nucleo(h(8, 0));
        let (pessoal, obra, leitura, _) = dois_espacos(&mut n);
        let mut g = n.banco.grupos().unwrap().into_iter().find(|g| g.espaco_id == pessoal).unwrap();
        g.arquivado = true;
        n.editar_grupo(&g, h(8, 0)).unwrap();
        // o Claude abre um trecho na Obra (o grupo dela está arquivado)
        let m = n.motor.play(obra, h(9, 0));
        n.aplicar(&m).unwrap();
        let s = n.snapshot(h(9, 30)).unwrap();
        let sem = s.grupos.iter().find(|x| x.id.is_none() && x.espaco_id == pessoal).unwrap();
        assert_eq!(sem.timers.iter().map(|t| t.id).collect::<Vec<_>>(), [obra, leitura], "no Sem grupo do Pessoal");
        // re-revisão: o timer leva o nome do grupo arquivado (o editar mostra esse grupo)
        assert_eq!(sem.timers[0].grupo_arquivado.as_deref(), Some("Casa"));
        assert_eq!(sem.timers[1].grupo_arquivado, None, "sem grupo");
        // salvar o timer sem mexer no grupo mantém o grupo arquivado
        let t = n.banco.timers().unwrap().into_iter().find(|t| t.id == obra).unwrap();
        n.editar_timer(&t, h(9, 31)).unwrap();
        assert_eq!(n.banco.timers().unwrap().into_iter().find(|t| t.id == obra).unwrap().grupo_id, Some(g.id));
        // pausada, some de novo
        let m = n.motor.pausar(obra, h(9, 30));
        n.aplicar(&m).unwrap();
        let s = n.snapshot(h(9, 40)).unwrap();
        assert!(!s.grupos.iter().flat_map(|g| &g.timers).any(|t| t.id == obra));
        // o espaço arquivado: a Leitura que voltar a rodar aparece no "Sem grupo" dele
        let mut e = n.banco.espacos().unwrap().into_iter().find(|e| e.id == pessoal).unwrap();
        e.arquivado = true;
        n.editar_espaco(&e, h(9, 40)).unwrap();
        let m = n.motor.play(leitura, h(10, 0));
        n.aplicar(&m).unwrap();
        let s = n.snapshot(h(10, 10)).unwrap();
        let sem = s.grupos.iter().find(|x| x.espaco_id == pessoal).unwrap();
        assert_eq!((sem.id, sem.timers.len(), sem.timers[0].rodando), (None, 1, true));
    }

    /// Revisão da rodada 20: com 2 ou mais espaços, o nome repetido leva o espaço no botão.
    #[test]
    fn pergunta_mostra_o_espaco_do_nome_repetido() {
        let mut n = nucleo(h(8, 0));
        let pessoal = n.criar_espaco("Pessoal", "#e86bd0").unwrap();
        let estudos = timer_zerado(&n, "Estudos", h(0, 0));
        n.banco.criar_timer_no(None, pessoal, "Estudos", "#222", h(8, 0), F).unwrap();
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", "G:\\Estudos"));
        n.aplicar(&mud).unwrap();
        let s = n.snapshot(h(9, 0)).unwrap();
        let p = s.perguntas.iter().find(|p| p.tipo == "sessao_nova").unwrap();
        let ligar = p.opcoes.iter().find(|(v, _)| v.starts_with("ligar:")).unwrap();
        assert!(ligar.0.starts_with(&format!("ligar:{estudos}")));
        assert_eq!(ligar.1, "Contar em Estudos · Geral");
    }

    #[test]
    fn atividades_ocultas_nunca_a_padrao() {
        let mut n = nucleo(h(8, 0));
        let t = timer_zerado(&n, "A", h(0, 0));
        let mut x = n.banco.timers().unwrap().into_iter().find(|x| x.id == t).unwrap();
        n.editar_timer_com(&x, h(8, 0), Some(&[ATIVIDADE_REUNIAO, 3])).unwrap();
        assert_eq!(n.banco.atividades_ocultas().unwrap()[&t], [ATIVIDADE_REUNIAO, 3]);
        // recusado antes de gravar qualquer coisa (revisão da rodada 20): o nome não muda
        x.nome = "Outro nome".into();
        assert_eq!(n.editar_timer_com(&x, h(8, 0), Some(&[ATIVIDADE_TRABALHO])).unwrap_err(), PADRAO_NAO_OCULTA());
        assert!(n.editar_timer_com(&x, h(8, 0), Some(&[999])).is_err());
        assert_eq!(n.banco.timers().unwrap()[0].nome, "A");
        assert_eq!(n.banco.atividades_ocultas().unwrap()[&t], [ATIVIDADE_REUNIAO, 3]);
        // sem ocultas (nulo), ficam as de antes; a padrão nova sai delas
        x.nome = "A".into();
        x.atividade_padrao_id = 3;
        n.editar_timer_com(&x, h(9, 0), None).unwrap();
        assert_eq!(n.banco.atividades_ocultas().unwrap()[&t], [ATIVIDADE_REUNIAO]);
        assert_eq!(n.snapshot(h(9, 0)).unwrap().grupos[0].timers[0].atividades_ocultas, [ATIVIDADE_REUNIAO]);
    }

    #[test]
    fn zerar_todos_e_a_virada_limpam_os_zerados_por_atividade() {
        let mut n = nucleo(h(8, 0));
        let t = trabalho_e_reuniao(&mut n);
        n.zerar_atividade(t, ATIVIDADE_REUNIAO, h(10, 30)).unwrap();
        let z = n.zerar_todos(None, h(10, 45)).unwrap();
        assert_eq!(z.limpas, [(t, ATIVIDADE_REUNIAO, h(10, 30))]);
        assert!(n.banco.zerados_atividade().unwrap().is_empty());
        assert_eq!(tr_reu(&n.snapshot(h(10, 50)).unwrap(), t), (0, 5 * MIN));
        n.desfazer_zerar(&z).unwrap();
        assert_eq!(tr_reu(&n.snapshot(h(10, 50)).unwrap(), t), (120 * MIN, 20 * MIN));
        // a virada: o zerado da reunião (antes do corte) sai junto
        let cfg = ConfigApp { zerar_na_virada: true, ..n.banco.config_app().unwrap() };
        let dir = tempfile::tempdir().unwrap();
        n.salvar_config(&cfg, &dir.path().join("hook.json")).unwrap();
        assert!(!n.zerar_na_virada(h(11, 0)).unwrap());
        let d18 = |hh, mm| ms_local(2026, 11, 18, hh, mm, F);
        assert!(n.zerar_na_virada(d18(0, 0) + 1000).unwrap());
        assert!(n.banco.zerados_atividade().unwrap().is_empty());
        assert_eq!(n.banco.zerados().unwrap()[&t], d18(0, 0));
    }

    #[test]
    fn lista_de_ligar_tem_as_disponiveis_primeiro_e_tira_o_chat_sem_nome() {
        use nucleo::motor::{ChatAberto, SinaisDoChat};
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "A", "#a00").unwrap();
        let com = |min: u32, sessao: &str, origem: Option<&str>| EventoHook {
            origem: origem.map(str::to_string),
            ..ev(h(9, min), "UserPromptSubmit", sessao, "G:\\Projeto A")
        };
        for (min, s, o) in [(0, "velha", Some("cli")), (1, "ligada_velha", None), (2, "nova", Some("claude-desktop")), (3, "ligada_nova", Some("claude-vscode"))] {
            let mud = n.motor.evento(&com(min, s, o));
            n.aplicar(&mud).unwrap();
        }
        for s in ["ligada_velha", "ligada_nova"] {
            n.ligar_sessao(s, t, false, false, h(9, 4)).unwrap();
        }
        // uma conversa do Chat com o título conhecido e outra que, reaberto o app, ficou sem nome
        let chat = |id: &str, titulo: &str| SinaisDoChat {
            aba_do_chat: true,
            aberta: Some(ChatAberto { id: id.into(), titulo: titulo.into(), parando: false }),
            rodando: vec![],
            barra: true,
            repetidos: vec![],
        };
        let mud = n.motor.sinais_do_chat(&chat("c1", "Receita"), h(9, 5));
        n.aplicar(&mud).unwrap();
        let mud = n.motor.sinais_do_chat(&chat("c2", "Viagem"), h(9, 6));
        n.aplicar(&mud).unwrap();
        n.motor.sessoes.get_mut("chat:c2").unwrap().nome = None;
        let v = n.sessoes_recentes(h(9, 7)).unwrap();
        let ids: Vec<&str> = v.iter().map(|s| s.id.as_str()).collect();
        assert_eq!(ids, ["chat:c1", "nova", "velha", "ligada_nova", "ligada_velha"]);
        let origens: Vec<Option<&str>> = v.iter().map(|s| s.origem.as_deref()).collect();
        assert_eq!(origens, [Some("chat"), Some("claude-desktop"), Some("cli"), Some("claude-vscode"), None]);
        // ligada, a do Chat sem nome volta à lista (no fim, com as ligadas)
        n.ligar_sessao("chat:c2", t, false, false, h(9, 8)).unwrap();
        let v = n.sessoes_recentes(h(9, 9)).unwrap();
        assert_eq!(v.last().map(|s| s.id.as_str()), Some("ligada_velha"));
        assert!(v.iter().skip(2).any(|s| s.id == "chat:c2"));
    }

    #[test]
    fn sempre_que_for_na_pasta_escolhe_entre_a_de_inicio_e_a_de_agora() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Projetos", "#a00").unwrap();
        for (min, evento, cwd) in [(0, "UserPromptSubmit", "D:\\Projetos"), (1, "PostToolUse", "D:\\Projetos\\cliente\\app")] {
            let mud = n.motor.evento(&ev(h(9, min), evento, "s1", cwd));
            n.aplicar(&mud).unwrap();
        }
        let p = n.snapshot(h(9, 2)).unwrap().perguntas[0].clone();
        assert_eq!(
            p.pastas.iter().map(|x| x.pasta.as_str()).collect::<Vec<_>>(),
            ["D:\\Projetos", "D:\\Projetos\\cliente", "D:\\Projetos\\cliente\\app"]
        );
        // o "+ Novo timer…" com a caixa marcada leva a pasta escolhida (o nome sai dela)
        let pedido = n.pedido_novo_timer("s1", Some("d:/projetos/cliente"), Some(true)).unwrap();
        assert_eq!(
            (pedido.nome.as_str(), pedido.pasta.as_str(), pedido.sempre),
            ("cliente", "D:\\Projetos\\cliente", Some(true))
        );
        assert!(n.pedido_novo_timer("s1", Some("D:\\Outra"), Some(true)).is_err());
        // sem a caixa: a pasta onde a sessão começou, como antes
        let pedido = n.pedido_novo_timer("s1", None, Some(false)).unwrap();
        assert_eq!((pedido.pasta.as_str(), pedido.sempre), ("D:\\Projetos", Some(false)));
        // a do meio
        n.responder(p.id, &format!("ligar:{t}:pasta:D:\\Projetos\\cliente"), h(9, 3)).unwrap();
        assert_eq!(n.banco.regras_pasta().unwrap()[0].pasta, "D:\\Projetos\\cliente");
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(t));
        assert_eq!(n.motor.sessoes["s1"].cwd, "D:\\Projetos", "o cwd gravado segue o de início");
        // o "Ligar" pela janela principal também escolhe a pasta, conferida
        let outro = n.banco.criar_timer(None, "Outro", "#0a0").unwrap();
        assert!(n.ligar_sessao_com("s1", outro, false, false, true, Some("E:\\Fora"), h(9, 4)).is_err());
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(t), "a pasta errada não liga nada");
        n.ligar_sessao_com("s1", outro, false, false, true, Some("D:\\Projetos\\cliente\\app"), h(9, 4)).unwrap();
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(outro));
        assert!(n.banco.regras_pasta().unwrap().iter().any(|r| r.pasta == "D:\\Projetos\\cliente\\app" && r.timer_id == outro));
        // só esta sessão: sem regra
        let mud = n.motor.evento(&ev(h(9, 5), "UserPromptSubmit", "s2", "E:\\Novo"));
        n.aplicar(&mud).unwrap();
        let p = n.snapshot(h(9, 5)).unwrap().perguntas.into_iter().find(|p| p.sessao_id.as_deref() == Some("s2")).unwrap();
        let antes = n.banco.regras_pasta().unwrap().len();
        n.responder(p.id, &format!("ligar:{t}"), h(9, 6)).unwrap();
        assert_eq!(n.banco.regras_pasta().unwrap().len(), antes);
        assert_eq!(n.motor.sessoes["s2"].timer_id, Some(t));
    }

    #[test]
    fn responder_ligar_com_regra_cria_regra_e_liga_desde_o_inicio() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", "G:\\Projeto A"));
        n.aplicar(&mud).unwrap();
        let s = n.snapshot(h(9, 1)).unwrap();
        let p = &s.perguntas[0];
        // o principal liga só esta sessão (rodada 27); a caixa "Sempre que for na pasta" põe a
        // pasta escolhida na resposta
        assert_eq!(p.opcoes[0].0, format!("ligar:{t}"));
        // uma pasta que não é da sessão: erro, e nada muda
        assert!(n.responder(p.id, &format!("ligar:{t}:pasta:G:\\Outra"), h(9, 5)).is_err());
        assert!(n.responder(p.id, &format!("ligar:{t}:pasta:G:\\"), h(9, 5)).is_err());
        assert_eq!((n.motor.perguntas.len(), n.motor.sessoes["s1"].timer_id), (1, None));
        assert!(n.banco.regras_pasta().unwrap().is_empty());
        n.responder(p.id, &format!("ligar:{t}:pasta:g:/projeto a/"), h(9, 5)).unwrap();
        assert!(n.motor.perguntas.is_empty());
        assert_eq!(n.banco.regras_pasta().unwrap()[0].pasta, "G:\\Projeto A");
        assert_eq!(n.motor.regras_pasta.len(), 1);
        let tr = n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert_eq!((tr[0].timer_id, tr[0].inicio), (Some(t), h(9, 0)));
        // sessão nova na mesma pasta já entra no timer e pergunta se está certo (rodada 15)
        let mud = n.motor.evento(&ev(h(9, 10), "UserPromptSubmit", "s2", "G:\\Projeto A\\sub"));
        n.aplicar(&mud).unwrap();
        let s = n.snapshot(h(9, 11)).unwrap();
        assert_eq!(s.rodando, 1);
        assert_eq!(s.perguntas.len(), 1);
        assert_eq!(s.perguntas[0].tipo, "conversa_nova");
        assert_eq!(s.perguntas[0].texto, "Conversa nova: sub · contando em Projeto A pela pasta");
        let valores: Vec<&str> = s.perguntas[0].opcoes.iter().map(|o| o.0.as_str()).collect();
        assert_eq!(valores, ["certo", "outro", "novo"]);
        assert_eq!(s.perguntas[0].sessao_id.as_deref(), Some("s2"));
        n.responder(s.perguntas[0].id, "certo", h(9, 11)).unwrap();
        assert!(n.motor.perguntas.is_empty());
        assert_eq!(n.motor.sessoes["s2"].timer_id, Some(t));
        assert!(n.responder(1, "ligar:999", h(9, 12)).is_err());
    }

    #[test]
    fn conversa_do_chat_pergunta_liga_sem_regra_e_o_titulo_fica_so_em_memoria() {
        use nucleo::motor::{ChatAberto, SinaisDoChat};
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        let sinais = |parando: bool| SinaisDoChat {
            aba_do_chat: true,
            aberta: Some(ChatAberto { id: "0a1b".into(), titulo: "Receita de bolo".into(), parando }),
            rodando: vec![],
            barra: true,
            repetidos: vec![],
        };
        let mud = n.motor.sinais_do_chat(&sinais(true), h(9, 0));
        n.aplicar(&mud).unwrap();
        let s = n.snapshot(h(9, 1)).unwrap();
        assert_eq!(s.perguntas.len(), 1);
        assert_eq!(s.perguntas[0].sessao_id.as_deref(), Some("chat:0a1b"));
        let valores: Vec<&str> = s.perguntas[0].opcoes.iter().map(|o| o.0.as_str()).collect();
        assert_eq!(valores, ["outro", "novo", "nao"], "sem pasta, sem sugestão");
        // a pergunta do Chat não tem pastas: pedir uma é erro
        assert!(s.perguntas[0].pastas.is_empty() && s.perguntas[0].pasta.is_none());
        assert_eq!(s.perguntas[0].origem.as_deref(), Some("chat"));
        assert!(n.responder(s.perguntas[0].id, &format!("ligar:{t}:pasta:G:\\Projeto A"), h(9, 2)).is_err());
        // o "sempre nesta pasta" sem pasta não cria regra: só liga
        n.responder(s.perguntas[0].id, &format!("ligar:{t}:regra"), h(9, 2)).unwrap();
        assert_eq!(n.motor.sessoes["chat:0a1b"].timer_id, Some(t));
        assert!(n.banco.regras_pasta().unwrap().is_empty());
        n.ligar_sessao("chat:0a1b", t, false, true, h(9, 3)).unwrap();
        assert!(n.banco.regras_pasta().unwrap().is_empty());
        // a lista de ligar mostra o título (de memória); o banco não tem
        assert_eq!(n.sessoes_recentes(h(9, 4)).unwrap()[0].nome, "Receita de bolo");
        assert_eq!(n.banco.sessoes_desde(0).unwrap()[0].nome, None);
        // reaberto o app, sem o título até a conversa aparecer de novo
        let Nucleo { banco, .. } = n;
        let mut n = Nucleo::abrir(banco, h(9, 5), F, dia()).unwrap();
        let sem_nome = n.sessoes_recentes(h(9, 5)).unwrap()[0].nome.clone();
        assert!(["Conversa do Chat", "Chat conversation"].contains(&sem_nome.as_str()), "{sem_nome}");
        let mud = n.motor.sinais_do_chat(&sinais(false), h(9, 6));
        n.aplicar(&mud).unwrap();
        assert_eq!(n.sessoes_recentes(h(9, 6)).unwrap()[0].nome, "Receita de bolo");
    }

    #[test]
    fn regras_listam_nome_do_timer_e_remover_tira_do_motor() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        n.criar_regra("G:\\Projeto A", t).unwrap();
        let v = n.regras().unwrap();
        assert_eq!(v, vec![RegraDto { id: v[0].id, pasta: "G:\\Projeto A".into(), timer_id: t, timer_nome: "Projeto A".into() }]);
        n.remover_regra(v[0].id).unwrap();
        assert!(n.regras().unwrap().is_empty());
        assert!(n.motor.regras_pasta.is_empty());
        assert!(n.remover_regra(v[0].id).is_err());
        // sem regra, sessão nova na pasta volta a perguntar
        let mud = n.motor.evento(&ev(h(9, 1), "UserPromptSubmit", "s1", "G:\\Projeto A"));
        n.aplicar(&mud).unwrap();
        assert_eq!(n.motor.perguntas.len(), 1);
    }

    #[test]
    fn ligar_sessao_com_regra_cria_regra() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "X", "#a00").unwrap();
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", "D:\\Novo"));
        n.aplicar(&mud).unwrap();
        n.ligar_sessao("s1", t, false, true, h(9, 30)).unwrap();
        assert_eq!(n.motor.regras_pasta[0].pasta, "D:\\Novo");
        let tr = n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert_eq!(tr.len(), 2);
        assert_eq!((tr[0].timer_id, tr[0].fim, tr[0].motivo_fim), (None, Some(h(9, 30)), Some(MotivoFim::Desligada)));
        assert_eq!((tr[1].timer_id, tr[1].inicio, tr[1].fim), (Some(t), h(9, 30), None));
        assert!(n.ligar_sessao("nada", t, true, false, h(9, 31)).is_err());
    }

    #[test]
    fn novo_timer_pela_pergunta_sugere_o_nome_da_raiz_e_liga_com_regra() {
        let mut n = nucleo(h(9, 0));
        let cwd = r"G:\Empresa\app-web\.claude\worktrees\feat-029-x";
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", cwd));
        n.aplicar(&mud).unwrap();
        let s = n.snapshot(h(9, 1)).unwrap();
        let p = &s.perguntas[0];
        assert!(p.opcoes.iter().any(|(v, r)| v == "novo" && r == "+ Novo timer…"));
        assert!(resposta_de("novo").is_err(), "não é resposta: a página trata");
        let pedido = n.pedido_novo_timer("s1", None, None).unwrap();
        assert_eq!(
            pedido,
            PedidoNovoTimer {
                sessao_id: "s1".into(),
                nome: "app-web".into(),
                pasta: r"G:\Empresa\app-web".into(),
                pasta_mae: false,
                outro_timer_nome: None,
                sempre: None,
                pode_regra: true,
            }
        );
        assert!(n.pedido_novo_timer("nada", None, None).is_err());
        // a página cria o timer e responde a pergunta com ele ("sempre nesta pasta" marcado)
        let t = n.banco.criar_timer(None, &pedido.nome, "#a00").unwrap();
        n.responder(p.id, &format!("ligar:{t}:regra"), h(9, 2)).unwrap();
        assert!(n.motor.perguntas.is_empty(), "a pergunta foi resolvida");
        assert_eq!(n.banco.regras_pasta().unwrap()[0].pasta, pedido.pasta);
        let tr = n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert_eq!((tr[0].timer_id, tr[0].inicio), (Some(t), h(9, 0)), "desde o início");
    }

    #[test]
    fn novo_timer_numa_pasta_que_nao_pode_ter_regra_so_liga_a_sessao() {
        // a sessão começou numa pasta larga (a raiz de um drive; a pasta do usuário, se houver)
        let mut largas = vec!["G:\\".to_string()];
        largas.extend(std::env::var("USERPROFILE").ok().filter(|p| !p.trim().is_empty()));
        for (i, cwd) in largas.iter().enumerate() {
            let mut n = nucleo(h(9, 0));
            let id = format!("s{i}");
            let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", &id, cwd));
            n.aplicar(&mud).unwrap();
            let p = n.snapshot(h(9, 1)).unwrap().perguntas[0].clone();
            let pedido = n.pedido_novo_timer(&id, None, None).unwrap();
            assert!(!pedido.pode_regra, "{cwd}");
            assert_eq!(pedido.sempre, Some(false), "a caixa não vem marcada");
            // a página cria o timer e responde sem a pasta: liga, sem regra e sem erro
            let t = n.banco.criar_timer(None, &format!("Novo {i}"), "#a00").unwrap();
            n.responder(p.id, &format!("ligar:{t}"), h(9, 2)).unwrap();
            assert_eq!(n.motor.sessoes[&id].timer_id, Some(t));
            assert!(n.motor.perguntas.is_empty());
            assert!(n.motor.regras_pasta.is_empty());
            // com a pasta, o ligar recusaria (é o que a caixa escondida evita)
            assert!(n.ligar_sessao_com(&id, t, true, false, true, Some(&pedido.pasta), h(9, 3)).is_err());
        }
        // a pasta de um projeto pode
        let mut n = nucleo(h(9, 0));
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", "G:\\Projeto A"));
        n.aplicar(&mud).unwrap();
        assert!(n.pedido_novo_timer("s1", None, None).unwrap().pode_regra);
    }

    #[test]
    fn ligar_a_conversa_do_chat_com_pasta_e_erro() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        // a conversa do Chat vista (sem responder): a sessão nasce, sem timer
        use nucleo::motor::{ChatAberto, SinaisDoChat};
        let aberta = Some(ChatAberto { id: "0a1b".into(), titulo: "Receita".into(), parando: false });
        let mud = n.motor.sinais_do_chat(&SinaisDoChat { aba_do_chat: true, aberta, barra: true, ..Default::default() }, h(9, 0));
        n.aplicar(&mud).unwrap();
        assert!(n.ligar_sessao_com("chat:0a1b", t, true, false, true, Some("G:\\Projeto A"), h(9, 1)).is_err());
        assert_eq!(n.motor.sessoes["chat:0a1b"].timer_id, None, "não ligou");
        // sem a pasta, liga (sem regra)
        n.ligar_sessao_com("chat:0a1b", t, true, false, true, None, h(9, 1)).unwrap();
        assert_eq!(n.motor.sessoes["chat:0a1b"].timer_id, Some(t));
        assert!(n.motor.regras_pasta.is_empty());
        assert!(!n.pedido_novo_timer("chat:0a1b", None, None).unwrap().pode_regra);
    }

    #[test]
    fn pasta_mae_nao_sugere_a_regra_e_o_pedido_vem_desmarcado() {
        // uma pasta com dois projetos dentro (rodada 15)
        let d = tempfile::tempdir().unwrap();
        let mae = d.path().join("Projetos");
        std::fs::create_dir_all(mae.join("Flow").join(".git")).unwrap();
        std::fs::create_dir_all(mae.join("Cliente A").join(".claude")).unwrap();
        let cwd = mae.to_string_lossy().to_string();
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Projetos", "#a00").unwrap();
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", &cwd));
        n.aplicar(&mud).unwrap();
        let p = n.snapshot(h(9, 1)).unwrap().perguntas[0].clone();
        // o botão liga só esta conversa
        assert_eq!(p.opcoes[0], (format!("ligar:{t}"), "Contar em Projetos".to_string()));
        // conferida quando a pergunta nasceu: o snapshot não lê o disco de novo (revisão)
        assert!(matches!(n.motor.perguntas[0].1, Pergunta::SessaoNova { pasta_mae: true, .. }));
        let pedido = n.pedido_novo_timer("s1", None, None).unwrap();
        assert!(pedido.pasta_mae);
        assert_eq!(pedido.outro_timer_nome, None);
        std::fs::remove_dir_all(mae.join("Cliente A")).unwrap();
        assert_eq!(n.snapshot(h(9, 1)).unwrap().perguntas[0].opcoes[0].0, format!("ligar:{t}"));
        n.responder(p.id, &p.opcoes[0].0, h(9, 2)).unwrap();
        assert!(n.motor.regras_pasta.is_empty(), "nenhuma regra criada");
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(t));
        // pasta-mãe pela regra (a): ancestral da pasta de outra regra
        n.criar_regra("G:\\Empresa\\Projeto\\Flow", t).unwrap();
        let c = n.conferir_regra("G:\\Empresa\\Projeto", Some(t)).unwrap();
        assert!(c.conferencia.pasta_mae);
        // a regra desta pasta já é de outro timer: o nome dele
        let outro = n.banco.criar_timer(None, "Outro", "#0a0").unwrap();
        let c = n.conferir_regra("g:/empresa/projeto/flow", Some(outro)).unwrap();
        assert_eq!((c.conferencia.outro_timer, c.outro_timer_nome.as_deref()), (Some(t), Some("Projetos")));
        assert_eq!(n.conferir_regra("G:\\Empresa\\Projeto\\Flow", Some(t)).unwrap().outro_timer_nome, None);
    }

    fn regras_de(n: &Nucleo) -> Vec<(String, i64)> {
        n.regras().unwrap().into_iter().map(|r| (r.pasta, r.timer_id)).collect()
    }

    #[test]
    fn pastas_deste_timer_acrescentar_trocar_e_tirar() {
        let mut n = nucleo(h(9, 0));
        let be = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        let flow = n.banco.criar_timer(None, "Flow", "#0a0").unwrap();
        // acrescentar: num worktree, a regra fica na raiz
        n.salvar_regra("G:\\Projeto A\\.claude\\worktrees\\x", be, None).unwrap();
        assert_eq!(regras_de(&n), [("G:\\Projeto A".to_string(), be)]);
        assert_eq!(n.motor.regras_pasta.len(), 1, "o motor acompanha");
        // trocar a pasta: a regra antiga sai, a nova entra
        let id = n.regras().unwrap()[0].id;
        n.salvar_regra("G:\\Projeto A2", be, Some(id)).unwrap();
        assert_eq!(regras_de(&n), [("G:\\Projeto A2".to_string(), be)]);
        // trocar uma regra que não é deste timer: recusa
        let id = n.regras().unwrap()[0].id;
        assert!(n.salvar_regra("G:\\Outra", flow, Some(id)).is_err());
        // a pasta de uma regra de outro timer: a regra passa para este (a tela perguntou antes)
        n.salvar_regra("g:/projeto a2/", flow, None).unwrap();
        assert_eq!(regras_de(&n), [("G:\\Projeto A2".to_string(), flow)]);
        // raiz de drive, pasta do usuário e pasta que nunca conta: recusa, sem mexer em nada
        let mut cfg = n.banco.config_app().unwrap();
        cfg.pastas_ignoradas = vec!["G:\\Pessoal".into()];
        let d = tempfile::tempdir().unwrap();
        n.salvar_config(&cfg, &d.path().join("hook.json")).unwrap();
        for p in ["G:\\", "C:\\Users\\alguem", "G:\\Pessoal\\x"] {
            assert!(n.salvar_regra(p, be, None).is_err(), "{p}");
        }
        assert_eq!(regras_de(&n).len(), 1);
        assert!(n.salvar_regra("G:\\X", 999, None).is_err(), "timer que não existe");
        // tirar: as sessões já ligadas continuam (não é retroativo)
        let mud = n.motor.evento(&ev(h(9, 5), "UserPromptSubmit", "s1", "G:\\Projeto A2\\sub"));
        n.aplicar(&mud).unwrap();
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(flow));
        let id = n.regras().unwrap()[0].id;
        n.remover_regra(id).unwrap();
        assert!(n.regras().unwrap().is_empty());
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(flow));
    }

    fn sessao_na_tela(n: &Nucleo, id: &str, agora: Ms) -> SessaoDto {
        n.snapshot(agora).unwrap().grupos.into_iter().flat_map(|g| g.timers).flat_map(|t| t.sessoes).find(|s| s.id == id).unwrap()
    }

    #[test]
    fn cada_dica_uma_vez_so_e_desligadas_nenhuma() {
        let mut n = nucleo(h(9, 0));
        let antes = nucleo::config::ConfigApp::de_json(r#"{"tema":"claro"}"#).unwrap();
        assert!(antes.dicas_ligadas && antes.dicas_vistas.is_empty(), "config de antes: os padrões");
        // pedir e, se der, mostrar até o "Entendi" (a vez é de `nucleo::dicas`; aqui, a config)
        let pedir = |n: &mut Nucleo, id: &str| {
            let mut ok = false;
            n.guardar_dock(|c| {
                ok = c.pode_mostrar_dica(id);
                if ok {
                    c.marcar_dica_vista(id);
                }
            })
            .unwrap();
            ok
        };
        assert!(pedir(&mut n, "zerar"));
        assert!(!pedir(&mut n, "zerar"), "a outra janela não mostra de novo");
        assert!(pedir(&mut n, "pergunta"));
        // o "Salvar" dos Ajustes, com a config velha, não volta as vistas
        let velha = nucleo::config::ConfigApp::default();
        let d = tempfile::tempdir().unwrap();
        n.salvar_config(&velha, &d.path().join("hook.json")).unwrap();
        assert_eq!(n.banco.config_app().unwrap().dicas_vistas, ["zerar", "pergunta"]);
        // desligadas: nenhuma
        n.guardar_dock(|c| c.dicas_ligadas = false).unwrap();
        assert!(!pedir(&mut n, "esperando"));
        // "Mostrar as dicas de novo" e ligadas: todas voltam
        n.guardar_dock(|c| {
            c.dicas_ligadas = true;
            c.dicas_vistas.clear();
        })
        .unwrap();
        assert!(pedir(&mut n, "zerar"));
    }

    #[test]
    fn tour_visto_so_muda_pelo_tour() {
        let mut n = nucleo(h(9, 0));
        // a config de antes (sem o campo) carrega com o tour por ver
        assert!(!nucleo::config::ConfigApp::de_json(r#"{"tema":"claro"}"#).unwrap().tour_visto);
        assert!(!n.banco.config_app().unwrap().tour_visto);
        let velha = n.banco.config_app().unwrap();
        n.guardar_dock(|c| c.tour_visto = true).unwrap();
        // a página manda a config que tinha (velha): o tour visto fica
        let d = tempfile::tempdir().unwrap();
        n.salvar_config(&velha, &d.path().join("hook.json")).unwrap();
        assert!(n.banco.config_app().unwrap().tour_visto);
    }

    #[test]
    fn desfazer_tirar_a_regra_nao_toma_a_pasta_de_outro_timer() {
        let mut n = nucleo(h(9, 0));
        let be = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        let flow = n.banco.criar_timer(None, "Flow", "#0a0").unwrap();
        n.salvar_regra("G:\\Projeto A", be, None).unwrap();
        // tirar e desfazer: volta
        let id = n.regras().unwrap()[0].id;
        n.remover_regra(id).unwrap();
        n.restaurar_regra("G:\\Projeto A", be).unwrap();
        assert_eq!(regras_de(&n), [("G:\\Projeto A".to_string(), be)]);
        // tirar; no meio-tempo a pasta vira regra de Flow; o "Desfazer" não toma
        let id = n.regras().unwrap()[0].id;
        n.remover_regra(id).unwrap();
        n.salvar_regra("g:/projeto a", flow, None).unwrap();
        assert_eq!(n.restaurar_regra("G:\\Projeto A", be).unwrap_err(), "A pasta já é de Flow.");
        assert_eq!(regras_de(&n), [("g:/projeto a".to_string(), flow)], "nada mudou");
    }

    #[test]
    fn lapis_da_sessao_diz_como_entrou_e_qual_regra() {
        let mut n = nucleo(h(9, 0));
        let be = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        let flow = n.banco.criar_timer(None, "Flow", "#0a0").unwrap();
        n.criar_regra("G:\\Projeto", be).unwrap();
        n.criar_regra("G:\\Projeto\\Flow", flow).unwrap();
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", "G:\\Projeto\\Cliente A\\app"));
        n.aplicar(&mud).unwrap();
        let s = sessao_na_tela(&n, "s1", h(9, 1));
        assert_eq!(s.ligacao.as_deref(), Some("regra"));
        assert_eq!((s.regra_id, s.regra_pasta.as_deref()), (Some(n.regras().unwrap()[0].id), Some("G:\\Projeto")));
        // a de prefixo mais longo
        let mud = n.motor.evento(&ev(h(9, 2), "UserPromptSubmit", "s2", "G:\\Projeto\\Flow\\x"));
        n.aplicar(&mud).unwrap();
        assert_eq!(sessao_na_tela(&n, "s2", h(9, 3)).regra_pasta.as_deref(), Some("G:\\Projeto\\Flow"));
        // movida à mão: sem regra
        n.ligar_sessao("s2", be, true, false, h(9, 4)).unwrap();
        let s = sessao_na_tela(&n, "s2", h(9, 5));
        assert_eq!((s.ligacao.as_deref(), s.regra_id), (Some("mao"), None));
        // a lista do "Ligar" (sessoes_recentes) também diz
        let r = n.sessoes_recentes(h(9, 6)).unwrap();
        assert_eq!(r.iter().find(|x| x.id == "s1").unwrap().regra_pasta.as_deref(), Some("G:\\Projeto"));
    }

    #[test]
    fn trocar_a_regra_por_uma_subpasta_da_conversa() {
        let mut n = nucleo(h(9, 0));
        let be = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        n.criar_regra("G:\\Projeto", be).unwrap();
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", "G:\\Projeto\\Cliente A\\app"));
        n.aplicar(&mud).unwrap();
        let id = n.regras().unwrap()[0].id;
        // fora da regra, ou uma subpasta que não tem a conversa: recusa
        for p in ["G:\\", "G:\\Outra", "G:\\Projeto\\Flow"] {
            let e = n.trocar_por_subpasta(id, p, "s1").unwrap_err();
            assert!(e.contains("G:\\Projeto"), "{e}");
        }
        assert_eq!(regras_de(&n), [("G:\\Projeto".to_string(), be)]);
        // a subpasta com a conversa: a regra passa a valer só nela
        n.trocar_por_subpasta(id, "G:\\Projeto\\Cliente A", "s1").unwrap();
        assert_eq!(regras_de(&n), [("G:\\Projeto\\Cliente A".to_string(), be)]);
        // a sessão já ligada fica
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(be));
        // uma conversa nova em G:\Projeto\Flow já não entra
        let mud = n.motor.evento(&ev(h(9, 5), "UserPromptSubmit", "s2", "G:\\Projeto\\Flow"));
        n.aplicar(&mud).unwrap();
        assert_eq!(n.motor.sessoes["s2"].timer_id, None);
        assert!(n.trocar_por_subpasta(999, "G:\\Projeto", "s1").is_err());
    }

    #[test]
    fn regra_criada_num_worktree_fica_na_raiz_do_projeto() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "app-web", "#a00").unwrap();
        let cwd = "G:\\Empresa\\app-web\\.claude\\worktrees\\feat-029-x";
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", cwd));
        n.aplicar(&mud).unwrap();
        let p = n.snapshot(h(9, 1)).unwrap().perguntas[0].clone();
        assert_eq!(p.opcoes[0].0, format!("ligar:{t}"), "sugere o timer pelo nome da raiz");
        // a lista das pastas já traz o projeto, não o worktree
        assert_eq!(p.pastas.iter().map(|x| x.pasta.as_str()).collect::<Vec<_>>(), ["G:\\Empresa\\app-web"]);
        n.responder(p.id, &format!("ligar:{t}:pasta:G:\\Empresa\\app-web"), h(9, 2)).unwrap();
        assert_eq!(n.motor.regras_pasta[0].pasta, "G:\\Empresa\\app-web");
        // sessão nova em outro worktree do projeto já entra no timer (e só pergunta se está certo)
        let mud = n.motor.evento(&ev(h(9, 3), "UserPromptSubmit", "s2", "G:\\Empresa\\app-web\\.claude\\worktrees\\outro"));
        n.aplicar(&mud).unwrap();
        assert!(n.motor.perguntas.iter().all(|(_, p)| matches!(p, Pergunta::ConversaNova { .. })));
        assert_eq!(n.motor.sessoes["s2"].timer_id, Some(t));
    }

    /// Caso do revisor (I1): o app fechou com uma saída da conversa lida esperando a troca suave.
    #[test]
    fn app_fechado_no_meio_da_troca_suave_fecha_a_leitura_na_saida() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Cliente F", "#a00").unwrap();
        let mut n = Nucleo::abrir(b, h(9, 0), F, dia()).unwrap();
        // lendo desde 9:00; às 9:10 você saiu, e o app fechou antes de a saída valer
        n.motor.abertos.push(nucleo::modelo::TrechoAberto { timer_id: Some(t), sessao_id: Some("s1".into()), origem: Origem::Leitura, inicio: h(9, 0), atividade_id: 1 });
        n.banco.aplicar(&[Mudanca::Abriu { timer_id: Some(t), sessao_id: Some("s1".into()), origem: Origem::Leitura, inicio: h(9, 0), atividade_id: 1 }]).unwrap();
        n.motor.troca = Some(nucleo::motor::Troca {
            alvo: None,
            visto: h(9, 10),
            inicio: h(9, 10),
            saiu: Some((h(9, 10), MotivoFim::SaiuDaJanela)),
            cortes: vec![],
            adiados: vec![],
            amostras: 1,
        });
        n.gravar_tique(h(9, 10) + 3_000).unwrap();
        assert_eq!(n.banco.valor(CHAVE_SAIDA_LEITURA).unwrap(), Some(h(9, 10).to_string()));
        let Nucleo { banco, .. } = n;
        let n = Nucleo::abrir(banco, h(11, 0), F, dia()).unwrap();
        let tr = n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert_eq!(tr.len(), 1);
        assert_eq!(tr[0].fim, Some(h(9, 10)), "na saída, não no último tique (9:10:03)");
        assert_eq!(n.banco.valor(CHAVE_SAIDA_LEITURA).unwrap().as_deref(), Some(""));
    }

    #[test]
    fn abrir_fecha_abertos_no_ultimo_tique_e_pergunta() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Cliente F", "#a00").unwrap();
        b.aplicar(&[Mudanca::Abriu { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, inicio: h(8, 0), atividade_id: 1 }])
            .unwrap();
        b.gravar_valor("ultimo_tique", &h(9, 0).to_string()).unwrap();
        b.gravar_valor("fila_offset", "42").unwrap();
        let n = Nucleo::abrir(b, h(9, 40), F, dia()).unwrap();
        assert_eq!(n.fila_offset, 42);
        assert!(n.banco.abertos().unwrap().is_empty());
        assert_eq!(n.banco.valor("ultimo_tique").unwrap(), Some(h(9, 40).to_string()));
        let s = n.snapshot(h(9, 40)).unwrap();
        assert_eq!(s.perguntas[0].texto, format!("O {} ficou fechado 40 min com Cliente F ligado. Retomar?", nucleo::APP_NOME));
    }

    #[test]
    fn encerrar_vale_depois_de_reabrir_no_mesmo_dia_e_cai_no_dia_seguinte() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#a00").unwrap();
        b.criar_regra_pasta("C:\\a", t).unwrap();
        let mut n = Nucleo::abrir(b, h(8, 0), F, dia()).unwrap();
        for (hora, evento) in [(h(8, 0), "UserPromptSubmit"), (h(8, 5), "Stop")] {
            let mud = n.motor.evento(&ev(hora, evento, "s1", "C:\\a"));
            n.aplicar(&mud).unwrap();
        }
        assert_eq!(n.snapshot(h(8, 6)).unwrap().grupos[0].timers[0].situacao.as_deref(), Some("esperando"));
        let mud = n.motor.encerrar(t, h(8, 10));
        n.aplicar(&mud).unwrap();
        // reaberto no mesmo dia: continua encerrado, sem o lembrete
        let n = Nucleo::abrir(n.banco, h(8, 20), F, dia()).unwrap();
        assert!(n.motor.encerrado(Some(t)));
        let timer = n.snapshot(h(8, 21)).unwrap().grupos[0].timers[0].clone();
        assert!(timer.encerrado);
        assert_eq!(timer.situacao, None);
        // no dia seguinte, não
        let amanha = ms_local(2026, 11, 18, 9, 0, F);
        let n = Nucleo::abrir(n.banco, amanha, F, dia().succ_opt().unwrap()).unwrap();
        assert!(!n.motor.encerrado(Some(t)));
    }

    #[test]
    fn abrir_carrega_sessoes_recentes_e_para_as_que_trabalhavam() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#a00").unwrap();
        let mut m = Motor::novo(ConfigMotor::de(&ConfigApp::default(), F), h(8, 0));
        m.regras_pasta = vec![nucleo::modelo::RegraPasta { id: 1, pasta: "C:\\a".into(), timer_id: t }];
        b.aplicar(&m.evento(&ev(h(8, 0), "UserPromptSubmit", "s1", "C:\\a"))).unwrap();
        b.gravar_valor("ultimo_tique", &h(8, 5).to_string()).unwrap();
        let mut n = Nucleo::abrir(b, h(9, 0), F, dia()).unwrap();
        assert_eq!(n.motor.sessoes["s1"].estado, EstadoSessao::Parada);
        assert_eq!(n.motor.sessoes["s1"].timer_id, Some(t));
        // PostToolUse depois de reabrir volta a contar no mesmo timer, sem pergunta
        let mud = n.motor.evento(&ev(h(9, 1), "PostToolUse", "s1", "C:\\a"));
        n.aplicar(&mud).unwrap();
        assert!(n.motor.perguntas.is_empty());
        let s = n.snapshot(h(9, 2)).unwrap();
        assert_eq!(s.grupos[0].timers[0].motivo.as_deref(), Some("claude"));
        assert_eq!(n.sessoes_recentes(h(9, 2)).unwrap()[0].id, "s1");
    }

    #[test]
    fn salvar_config_grava_hook_json_e_atualiza_motor() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("sub").join("hook.json");
        let mut n = nucleo(h(9, 0));
        let cfg = ConfigApp {
            pastas_ignoradas: vec!["G:\\pessoal".into()],
            inatividade_s: 300,
            modo_dev: true,
            ..ConfigApp::default()
        };
        n.salvar_config(&cfg, &p).unwrap();
        let j: serde_json::Value = serde_json::from_slice(&std::fs::read(&p).unwrap()).unwrap();
        assert_eq!(j, serde_json::json!({"pastas_ignoradas": ["G:\\pessoal"]}));
        assert_eq!(n.motor.cfg.inatividade_ms, 300_000);
        assert_eq!(n.motor.cfg.fuso_min, F);
        assert_eq!(n.motor.cfg.pastas_ignoradas, cfg.pastas_ignoradas);
        assert_eq!(n.banco.config_app().unwrap(), cfg);
        // pasta ignorada não vira sessão
        assert!(n.motor.evento(&ev(h(9, 1), "UserPromptSubmit", "s", "g:/pessoal/notas")).is_empty());
    }

    #[test]
    fn salvar_config_recusa_tempo_abaixo_de_1_minuto() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("hook.json");
        let mut n = nucleo(h(9, 0));
        let bons = ConfigApp { uso_s: 60, inatividade_s: 60, sessao_parada_s: 60, modo_dev: true, ..ConfigApp::default() };
        n.salvar_config(&bons, &p).unwrap();
        let ruins = [
            ConfigApp { uso_s: 0, ..bons.clone() },
            ConfigApp { inatividade_s: 59, ..bons.clone() },
            ConfigApp { sessao_parada_s: 1, ..bons.clone() },
            ConfigApp { inatividade_s: 30 * 60 + 1, ..bons.clone() },
            ConfigApp { sessao_parada_s: 24 * 3600 + 1, ..bons.clone() },
        ];
        let erros: Vec<String> = ruins.iter().map(|r| n.salvar_config(r, &p).unwrap_err()).collect();
        assert_eq!(
            erros,
            [
                "Os tempos precisam ser de pelo menos 1 minuto (uso recente)",
                "Os tempos precisam ser de pelo menos 1 minuto (inatividade)",
                "Os tempos precisam ser de pelo menos 1 minuto (sessão parada)",
                "A inatividade vai de 1 a 30 minutos",
                "Uso recente e sessão parada vão até 24 h",
            ]
        );
        let dois = ConfigApp { uso_s: 0, sessao_parada_s: 0, ..bons.clone() };
        assert_eq!(
            n.salvar_config(&dois, &p).unwrap_err(),
            "Os tempos precisam ser de pelo menos 1 minuto (uso recente, sessão parada)"
        );
        // nada mudou: nem banco nem motor
        assert_eq!(n.banco.config_app().unwrap(), bons);
        assert_eq!(n.motor.cfg.uso_ms, 60_000);
        // 30 min ainda vale
        n.salvar_config(&ConfigApp { inatividade_s: 30 * 60, ..bons.clone() }, &p).unwrap();
        assert_eq!(n.motor.cfg.inatividade_ms, 30 * 60_000);
    }

    #[test]
    fn desligar_o_modo_dev_volta_os_tempos_aos_padroes() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("hook.json");
        let mut n = nucleo(h(9, 0));
        let dev = ConfigApp { modo_dev: true, uso_s: 60, inatividade_s: 120, sessao_parada_s: 3600, ..ConfigApp::default() };
        n.salvar_config(&dev, &p).unwrap();
        assert_eq!(n.motor.cfg.inatividade_ms, 120_000);
        n.salvar_config(&ConfigApp { modo_dev: false, ..dev.clone() }, &p).unwrap();
        let c = n.banco.config_app().unwrap();
        assert_eq!((c.modo_dev, c.uso_s, c.inatividade_s, c.sessao_parada_s), (false, 180, 600, 600));
        assert_eq!((n.motor.cfg.uso_ms, n.motor.cfg.inatividade_ms, n.motor.cfg.sessao_parada_ms), (180_000, 600_000, 600_000));
        // sem modo dev, tempos fora dos limites nem dão erro: valem os padrões
        n.salvar_config(&ConfigApp { inatividade_s: 0, uso_s: 99_999, ..ConfigApp::default() }, &p).unwrap();
        assert_eq!(n.banco.config_app().unwrap().inatividade_s, 600);
        // religar não traz os tempos de antes: começa dos padrões
        n.salvar_config(&ConfigApp { modo_dev: true, ..n.banco.config_app().unwrap() }, &p).unwrap();
        assert_eq!(n.banco.config_app().unwrap().inatividade_s, 600);
    }

    #[test]
    fn ajustar_hoje_grava_no_dia_de_trabalho_e_desfaz() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "A", "#a00").unwrap();
        let mud = n.motor.play(t, h(9, 0));
        n.aplicar(&mud).unwrap();
        let id = n.ajustar_hoje(t, None, 15 * MIN, Some("  reunião sem timer "), h(10, 0)).unwrap();
        n.ajustar_hoje(t, None, -5 * MIN, Some("   "), h(10, 1)).unwrap();
        let s = n.snapshot(h(10, 2)).unwrap();
        let timer = &s.grupos[0].timers[0];
        assert_eq!(timer.ajuste_ms, 10 * MIN);
        assert_eq!(timer.ajustes[0].motivo.as_deref(), Some("reunião sem timer"));
        assert_eq!(timer.ajustes[1].motivo, None, "motivo em branco não é guardado");
        assert_eq!(n.banco.ajustes_do_dia("2026-11-17").unwrap().len(), 2);
        n.desfazer_ajuste(id).unwrap();
        assert_eq!(n.snapshot(h(10, 2)).unwrap().grupos[0].timers[0].ajuste_ms, -5 * MIN);
        assert!(n.desfazer_ajuste(id).is_err());
        // recusas
        assert!(n.ajustar_hoje(999, None, MIN, None, h(10, 3)).is_err());
        assert_eq!(n.ajustar_hoje(t, None, 0, None, h(10, 3)).unwrap_err(), "Diga quantos minutos a mais ou a menos");
        assert!(n.ajustar_hoje(t, None, DIA_MS + MIN, None, h(10, 3)).is_err());
        assert!(n.ajustar_hoje(t, None, MIN, Some(&"x".repeat(MOTIVO_MAX + 1)), h(10, 3)).is_err());
        assert!(n.ajustar_hoje(t, None, MIN, Some(&"é".repeat(MOTIVO_MAX)), h(10, 3)).is_ok());
    }

    #[test]
    fn ajuste_na_atividade_escolhida_ou_na_padrao_do_timer() {
        use nucleo::modelo::{ATIVIDADE_ESTUDO, ATIVIDADE_REUNIAO};
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Curso", "#a00").unwrap();
        n.ajustar_hoje(t, Some(ATIVIDADE_REUNIAO), 15 * MIN, None, h(10, 0)).unwrap();
        let mut timer = n.banco.timers().unwrap()[0].clone();
        timer.atividade_padrao_id = ATIVIDADE_ESTUDO;
        n.editar_timer(&timer, h(10, 0)).unwrap();
        n.ajustar_hoje(t, None, 5 * MIN, None, h(10, 1)).unwrap();
        let a: Vec<i64> = n.banco.ajustes_do_dia("2026-11-17").unwrap().iter().map(|a| a.atividade_id).collect();
        assert_eq!(a, [ATIVIDADE_REUNIAO, ATIVIDADE_ESTUDO]);
        // atividade arquivada ou que não existe: recusa
        let mut estudo = n.banco.atividades().unwrap()[2].clone();
        assert!(n.ajustar_hoje(t, Some(99), MIN, None, h(10, 2)).is_err());
        estudo.arquivada = true;
        n.editar_atividade(&estudo, true).unwrap();
        assert!(n.ajustar_hoje(t, Some(ATIVIDADE_ESTUDO), MIN, None, h(10, 2)).is_err());
    }

    #[test]
    fn atividade_padrao_do_timer_vale_no_play_e_ao_reabrir() {
        use nucleo::modelo::ATIVIDADE_ESTUDO;
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Curso", "#a00").unwrap();
        let mut timer = n.banco.timers().unwrap()[0].clone();
        timer.atividade_padrao_id = ATIVIDADE_ESTUDO;
        n.editar_timer(&timer, h(9, 0)).unwrap();
        let mud = n.motor.play(t, h(9, 1));
        n.aplicar(&mud).unwrap();
        assert_eq!(n.banco.abertos().unwrap()[0].atividade_id, ATIVIDADE_ESTUDO);
        // atividade padrão arquivada não entra no timer
        timer.atividade_padrao_id = 99;
        assert!(n.editar_timer(&timer, h(9, 2)).is_err());
        // o mapa vem do banco ao abrir
        let banco = std::mem::replace(&mut n.banco, Banco::abrir_memoria().unwrap());
        let n = Nucleo::abrir(banco, h(9, 1) + 30_000, F, dia()).unwrap();
        assert_eq!(n.motor.atividade_padrao(Some(t)), ATIVIDADE_ESTUDO);
        let s = n.snapshot(h(9, 2)).unwrap();
        assert_eq!(s.grupos[0].timers[0].manual_atividade_id, Some(ATIVIDADE_ESTUDO), "a retomada mantém a atividade");
    }

    #[test]
    fn atividades_renomear_acrescentar_e_arquivar_com_cuidado() {
        use nucleo::modelo::{ATIVIDADE_ESTUDO, ATIVIDADE_REUNIAO, ATIVIDADE_TRABALHO};
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Curso", "#a00").unwrap();
        let arquivado = n.banco.criar_timer(None, "Velho", "#b00").unwrap();
        // renomear e acrescentar, sem nome vazio nem repetido
        let mut reuniao = n.banco.atividades().unwrap()[1].clone();
        reuniao.nome = "  Call ".into();
        n.editar_atividade(&reuniao, false).unwrap();
        assert_eq!(n.banco.atividades().unwrap()[1].nome, "Call");
        assert!(n.criar_atividade(" ", "#000").is_err());
        assert_eq!(n.criar_atividade("call", "#000").unwrap_err(), "Já existe a atividade call");
        assert!(n.editar_atividade(&nucleo::modelo::Atividade { nome: "Estudo".into(), ..reuniao.clone() }, false).is_err());
        let curso = n.criar_atividade("Curso", "#123456").unwrap();
        // a padrão de um timer só arquiva com a confirmação; o timer passa para a primeira que sobra
        let mut tm = n.banco.timers().unwrap()[0].clone();
        tm.atividade_padrao_id = curso;
        n.editar_timer(&tm, h(9, 0)).unwrap();
        let mut c = n.banco.atividades().unwrap().into_iter().find(|a| a.id == curso).unwrap();
        c.arquivada = true;
        assert_eq!(n.editar_atividade(&c, false).unwrap_err(), "Curso é a atividade padrão de Curso");
        assert!(!n.banco.atividades().unwrap().iter().find(|a| a.id == curso).unwrap().arquivada);
        n.editar_atividade(&c, true).unwrap();
        assert_eq!(n.banco.atividades_padrao().unwrap()[&t], ATIVIDADE_TRABALHO);
        assert_eq!(n.motor.atividade_padrao(Some(t)), ATIVIDADE_TRABALHO, "o motor acompanha");
        // um timer arquivado com ela como padrão não pergunta nem muda
        let mut v = n.banco.timers().unwrap()[1].clone();
        assert_eq!(v.id, arquivado);
        v.atividade_padrao_id = ATIVIDADE_ESTUDO;
        v.arquivado = true;
        n.editar_timer(&v, h(9, 0)).unwrap();
        let mut estudo = n.banco.atividades().unwrap()[2].clone();
        estudo.arquivada = true;
        n.editar_atividade(&estudo, false).unwrap();
        // a última que sobra nunca arquiva
        let mut reuniao = n.banco.atividades().unwrap()[1].clone();
        reuniao.arquivada = true;
        n.editar_atividade(&reuniao, false).unwrap();
        let mut trabalho = n.banco.atividades().unwrap()[0].clone();
        trabalho.arquivada = true;
        assert_eq!(n.editar_atividade(&trabalho, true).unwrap_err(), "Precisa sobrar pelo menos uma atividade");
        assert_eq!(n.banco.atividades().unwrap().iter().filter(|a| !a.arquivada).count(), 1);
        // desarquivar volta
        reuniao.arquivada = false;
        n.editar_atividade(&reuniao, false).unwrap();
        assert!(!n.banco.atividades().unwrap().iter().find(|a| a.id == ATIVIDADE_REUNIAO).unwrap().arquivada);
    }

    #[test]
    fn esperas_mudaram_avisa_quando_o_esperando_voce_vence() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "A", "#a00").unwrap();
        n.criar_regra("C:\\a", t).unwrap();
        for e in [ev(h(9, 0), "UserPromptSubmit", "s1", "C:\\a"), ev(h(9, 5), "Stop", "s1", "C:\\a")] {
            let mud = n.motor.evento(&e);
            n.aplicar(&mud).unwrap();
        }
        assert!(n.esperas_mudaram(h(9, 6)), "passou a esperar");
        assert!(!n.esperas_mudaram(h(9, 30)), "nada mudou");
        assert!(n.esperas_mudaram(h(9, 36)), "30 min do dock recolhido: venceu (rodada 13)");
        assert!(!n.esperas_mudaram(h(10, 4)));
        assert!(n.esperas_mudaram(h(10, 5)), "1 h depois de terminar: venceu");
        assert!(!n.esperas_mudaram(h(10, 6)));
    }

    #[test]
    fn ajuste_com_corte_do_dia_vai_para_o_dia_de_trabalho() {
        let d = tempfile::tempdir().unwrap();
        let mut n = nucleo(h(9, 0));
        n.salvar_config(&ConfigApp { corte_min: 180, ..ConfigApp::default() }, &d.path().join("hook.json")).unwrap();
        let t = n.banco.criar_timer(None, "A", "#a00").unwrap();
        // 01:30 do dia 18, com corte às 03:00, ainda é o dia 17
        n.ajustar_hoje(t, None, 30 * MIN, None, ms_local(2026, 11, 18, 1, 30, F)).unwrap();
        assert_eq!(n.banco.ajustes_do_dia("2026-11-17").unwrap().len(), 1);
    }

    #[test]
    fn banco_com_tempo_zero_abre_e_volta_a_salvar() {
        let d = tempfile::tempdir().unwrap();
        let b = Banco::abrir_memoria().unwrap();
        // valor gravado antes da trava (o "0.001" de antes virava 0 s)
        b.gravar_valor("app", r#"{"modo_dev": true, "inatividade_s": 0}"#).unwrap();
        let mut n = Nucleo::abrir(b, h(9, 0), F, dia()).unwrap();
        assert_eq!(n.motor.cfg.inatividade_ms, 60_000);
        let mut c = n.banco.config_app().unwrap();
        assert_eq!(c.inatividade_s, 60);
        c.tema = "claro".into();
        n.salvar_config(&c, &d.path().join("hook.json")).unwrap();
        assert_eq!(n.banco.config_app().unwrap().tema, "claro");
    }

    #[test]
    fn dock_guarda_posicao_e_tamanho_so_do_expandido() {
        let d = tempfile::tempdir().unwrap();
        let n = nucleo(h(9, 0));
        let c = n.guardar_posicao_dock(100, 20, 300, 46).unwrap();
        assert_eq!((c.dock_x, c.dock_y, c.dock_larg, c.dock_alt), (Some(100), Some(20), None, None));
        n.guardar_dock(|c| c.dock_expandido = true).unwrap();
        let c = n.guardar_posicao_dock(90, 20, 330, 450).unwrap();
        assert_eq!((c.dock_x, c.dock_larg, c.dock_alt, c.dock_expandido), (Some(90), Some(330), Some(450), true));
        assert_eq!(n.banco.config_app().unwrap(), c);
        // a config velha da janela principal não desfaz o dock
        let mut n = n;
        let velha = ConfigApp { tema: "claro".into(), ..ConfigApp::default() };
        n.salvar_config(&velha, &d.path().join("hook.json")).unwrap();
        let c = n.banco.config_app().unwrap();
        assert_eq!(c.tema, "claro");
        assert_eq!((c.dock_x, c.dock_larg, c.dock_expandido), (Some(90), Some(330), true));
    }

    #[test]
    fn primeiro_uso_ate_o_primeiro_timer() {
        let n = nucleo(h(9, 0));
        assert!(n.primeiro_uso());
        let g = n.banco.criar_grupo("G", "#a00").unwrap();
        assert!(n.primeiro_uso(), "grupo sem timer ainda é primeiro uso");
        n.banco.criar_timer(Some(g), "X", "#a00").unwrap();
        assert!(!n.primeiro_uso());
    }

    #[test]
    fn ao_fechar_grava_so_bandeja_ou_sair() {
        let n = nucleo(h(9, 0));
        assert_eq!(n.banco.config_app().unwrap().ao_fechar, "perguntar");
        assert_eq!(n.guardar_ao_fechar("bandeja").unwrap().ao_fechar, "bandeja");
        assert_eq!(n.banco.config_app().unwrap().ao_fechar, "bandeja");
        assert!(n.guardar_ao_fechar("perguntar").is_err());
        assert!(n.guardar_ao_fechar("x").is_err());
        assert_eq!(n.guardar_ao_fechar("sair").unwrap().ao_fechar, "sair");
    }

    #[test]
    fn arquivar_timer_rodando_pausa_e_some_do_snapshot() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "A", "#a00").unwrap();
        let mud = n.motor.play(t, h(9, 0));
        n.aplicar(&mud).unwrap();
        let mut timer = n.banco.timers().unwrap()[0].clone();
        timer.arquivado = true;
        n.editar_timer(&timer, h(9, 30)).unwrap();
        assert!(n.motor.abertos.is_empty());
        assert!(n.banco.abertos().unwrap().is_empty());
        let s = n.snapshot(h(9, 31)).unwrap();
        assert!(s.grupos.is_empty());
        assert_eq!(s.rodando, 0);
    }

    // ---- o fim do transcript (rodada 13) ----

    fn ts(em: Ms) -> String {
        chrono::DateTime::from_timestamp_millis(em).unwrap().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
    }
    /// Uma linha de transcript de exemplo: mensagem do usuário com o texto, no instante.
    fn linha_usuario(texto: &str, em: Ms) -> String {
        serde_json::json!({ "type": "user", "message": { "role": "user", "content": [{ "type": "text", "text": texto }] }, "timestamp": ts(em) })
            .to_string()
            + "\n"
    }
    fn linha_resultado(texto: &str, em: Ms) -> String {
        serde_json::json!({ "type": "user", "message": { "role": "user", "content": [{ "type": "tool_result", "tool_use_id": "t1", "content": texto }] }, "timestamp": ts(em) })
            .to_string()
            + "\n"
    }
    fn linha_notificacao(id: &str) -> String {
        serde_json::json!({ "type": "queue-operation", "operation": "enqueue", "content": format!("<task-notification>\n<task-id>{id}</task-id>\n</task-notification>") })
            .to_string()
            + "\n"
    }
    fn acrescentar(p: &Path, texto: &str) {
        let mut f = std::fs::OpenOptions::new().create(true).append(true).open(p).unwrap();
        f.write_all(texto.as_bytes()).unwrap();
    }
    /// Um evento do Claude com o transcript.
    fn ev_tr(t: Ms, evento: &str, transcript: &Path) -> EventoHook {
        EventoHook { transcript: Some(transcript.to_string_lossy().into_owned()), ..ev(t, evento, "s1", "G:\\Projeto A") }
    }
    /// O laço: lê para a frente os transcripts que mudaram e aplica. Devolve quantos leu.
    fn conferir_fins(n: &mut Nucleo, agora: Ms) -> usize {
        let v = n.transcripts_a_conferir(agora);
        let k = v.len();
        let lidos: Vec<_> = v
            .into_iter()
            .map(|(id, c, mut l)| {
                let r = l.ler(&c, agora);
                (id, l, r)
            })
            .collect();
        let mud = n.aplicar_fins(lidos, agora);
        n.aplicar(&mud).unwrap();
        k
    }
    fn sit(n: &Nucleo, agora: Ms) -> Option<String> {
        n.snapshot(agora).unwrap().grupos[0].timers[0].situacao.clone()
    }

    #[test]
    fn stop_na_conversa_fecha_o_trecho_no_instante_da_interrupcao() {
        let d = tempfile::tempdir().unwrap();
        let tr = d.path().join("s1.jsonl");
        acrescentar(&tr, &linha_usuario("conserte o teste", h(8, 53)));
        let mut n = nucleo(h(8, 53));
        let t = n.banco.criar_timer(None, "Flow", "#a00").unwrap();
        n.criar_regra("G:\\Projeto A", t).unwrap();
        let mud = n.motor.evento(&ev_tr(h(8, 53), "UserPromptSubmit", &tr));
        n.aplicar(&mud).unwrap();
        // menos de 10 s sem evento: nem confere
        assert_eq!(conferir_fins(&mut n, h(8, 53) + 9_000), 0);
        // o stop: o Claude Code escreve a interrupção, e nenhum hook vem
        acrescentar(&tr, &linha_usuario("[Request interrupted by user for tool use]", h(8, 54) + 2_000));
        assert_eq!(conferir_fins(&mut n, h(8, 55)), 1);
        let trechos = n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        // no banco vai como "terminou" (uma versão anterior do app lê; revisão da rodada 13)
        assert_eq!((trechos[0].fim, trechos[0].motivo_fim), (Some(h(8, 54) + 2_000), Some(MotivoFim::Terminou)));
        assert_eq!(sit(&n, h(8, 56)).as_deref(), Some("esperando"));
        assert!(!n.snapshot(h(8, 56)).unwrap().grupos[0].timers[0].rodando);
        // a mensagem seguinte reabre normalmente
        acrescentar(&tr, &linha_usuario("de novo", h(8, 58)));
        let mud = n.motor.evento(&ev_tr(h(8, 58), "UserPromptSubmit", &tr));
        n.aplicar(&mud).unwrap();
        assert!(n.snapshot(h(8, 59)).unwrap().grupos[0].timers[0].rodando);
        // só as linhas novas são lidas: a interrupção de antes não fecha de novo
        acrescentar(&tr, &linha_resultado("ok", h(8, 59)));
        assert_eq!(conferir_fins(&mut n, h(9, 0)), 1);
        assert_eq!(n.banco.abertos().unwrap().len(), 1);
    }

    #[test]
    fn transcript_que_nao_muda_nao_e_relido() {
        let d = tempfile::tempdir().unwrap();
        let tr = d.path().join("s1.jsonl");
        acrescentar(&tr, &linha_usuario("oi", h(9, 0)));
        let mut n = nucleo(h(9, 0));
        let mud = n.motor.evento(&ev_tr(h(9, 0), "UserPromptSubmit", &tr));
        n.aplicar(&mud).unwrap();
        assert_eq!(conferir_fins(&mut n, h(9, 1)), 1);
        assert_eq!(conferir_fins(&mut n, h(9, 1) + 5_000), 0, "o mesmo arquivo");
        assert_eq!(conferir_fins(&mut n, h(9, 2)), 0);
        acrescentar(&tr, &linha_resultado("mais", h(9, 2)));
        assert_eq!(conferir_fins(&mut n, h(9, 3)), 1, "cresceu");
        assert_eq!(n.leitores["s1"].offset, Some(std::fs::metadata(&tr).unwrap().len()));
        // parada: não confere
        n.motor.sessoes.get_mut("s1").unwrap().estado = EstadoSessao::Parada;
        acrescentar(&tr, &linha_resultado("mais", h(9, 4)));
        assert_eq!(conferir_fins(&mut n, h(9, 4)), 0);
    }

    #[test]
    fn segundo_plano_ate_a_tarefa_terminar_ou_vencer() {
        let d = tempfile::tempdir().unwrap();
        let tr = d.path().join("s1.jsonl");
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Cliente C", "#a00").unwrap();
        n.criar_regra("G:\\Projeto A", t).unwrap();
        let mud = n.motor.evento(&ev_tr(h(9, 0), "UserPromptSubmit", &tr));
        n.aplicar(&mud).unwrap();
        acrescentar(&tr, &linha_resultado("Command running in background with ID: bw54hpb9u. Output is being written to: x", h(9, 1)));
        acrescentar(&tr, &linha_resultado("Async agent launched successfully.\nagentId: a1b2c3 (internal ID)", h(9, 2)));
        // mais de 64 KB depois dos lançamentos (a cauda fixa da primeira versão os perdia)
        for _ in 0..80 {
            acrescentar(&tr, &linha_resultado(&"x".repeat(1000), h(9, 3)));
        }
        let mud = n.motor.evento(&ev_tr(h(9, 5), "Stop", &tr));
        n.aplicar(&mud).unwrap();
        conferir_fins(&mut n, h(9, 5) + 1_000);
        assert_eq!(sit(&n, h(9, 6)).as_deref(), Some("segundo_plano"));
        assert!(!n.snapshot(h(9, 6)).unwrap().grupos[0].timers[0].rodando, "o segundo plano não conta");
        // o comando terminou; o subagente segue
        acrescentar(&tr, &linha_notificacao("bw54hpb9u"));
        conferir_fins(&mut n, h(9, 21));
        assert_eq!(sit(&n, h(9, 21)).as_deref(), Some("segundo_plano"));
        // o subagente terminou: "esperando você"
        acrescentar(&tr, &linha_notificacao("a1b2c3"));
        conferir_fins(&mut n, h(9, 41));
        assert_eq!(sit(&n, h(9, 41)).as_deref(), Some("esperando"));
        assert!(n.motor.segundo_plano.is_empty());
    }

    #[test]
    fn servidor_esquecido_vence_2_h_depois_do_lancamento_e_nao_volta() {
        let d = tempfile::tempdir().unwrap();
        let tr = d.path().join("s1.jsonl");
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Site", "#a00").unwrap();
        n.criar_regra("G:\\Projeto A", t).unwrap();
        let mud = n.motor.evento(&ev_tr(h(9, 0), "UserPromptSubmit", &tr));
        n.aplicar(&mud).unwrap();
        acrescentar(&tr, &linha_resultado("Command running in background with ID: dev", h(9, 1)));
        // a linha do lançamento é lida ainda com a sessão trabalhando (10 s sem evento) ...
        assert_eq!(conferir_fins(&mut n, h(9, 2)), 1);
        // ... e o `Stop` chega sem o arquivo mudar: o "segundo plano" vale mesmo assim
        let mud = n.motor.evento(&ev_tr(h(9, 5), "Stop", &tr));
        n.aplicar(&mud).unwrap();
        assert_eq!(conferir_fins(&mut n, h(9, 5) + 1_000), 0);
        assert_eq!(sit(&n, h(9, 6)).as_deref(), Some("segundo_plano"));
        // um turno novo às 10:00 também acaba em "segundo plano" (o servidor ainda vale)
        let mud = n.motor.evento(&ev_tr(h(10, 0), "UserPromptSubmit", &tr));
        n.aplicar(&mud).unwrap();
        let mud = n.motor.evento(&ev_tr(h(10, 5), "Stop", &tr));
        n.aplicar(&mud).unwrap();
        conferir_fins(&mut n, h(10, 6));
        assert_eq!(sit(&n, h(10, 6)).as_deref(), Some("segundo_plano"));
        // às 11:01 (2 h do lançamento) a tarefa é esquecida de vez
        conferir_fins(&mut n, h(11, 1));
        assert_eq!(sit(&n, h(11, 1)).as_deref(), Some("esperando"));
        assert!(n.leitores["s1"].pendentes.is_empty());
        // o turno seguinte termina em "esperando você", não em "segundo plano"
        let mud = n.motor.evento(&ev_tr(h(11, 30), "UserPromptSubmit", &tr));
        n.aplicar(&mud).unwrap();
        acrescentar(&tr, &linha_resultado("ok", h(11, 31)));
        let mud = n.motor.evento(&ev_tr(h(11, 35), "Stop", &tr));
        n.aplicar(&mud).unwrap();
        conferir_fins(&mut n, h(11, 36));
        assert_eq!(sit(&n, h(11, 36)).as_deref(), Some("esperando"));
    }

    #[test]
    fn pausado_vence_no_dock_e_avisa_a_tela_sem_evento() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "Flow", "#a00").unwrap();
        let mud = n.motor.play(t, h(9, 0));
        n.aplicar(&mud).unwrap();
        let mud = n.motor.pausar(t, h(9, 10));
        n.aplicar(&mud).unwrap();
        n.esperas_mudaram(h(9, 11));
        assert!(!n.esperas_mudaram(h(9, 15)));
        // 10 min depois do ⏸ (o padrão): o pausado sai do dock recolhido, e a tela é avisada
        assert!(n.esperas_mudaram(h(9, 20) + 1));
        assert!(!n.esperas_mudaram(h(9, 25)));
    }

    #[test]
    fn lembrete_vencido_ou_virada_avisa_a_tela_sem_evento() {
        let mut n = nucleo(h(9, 0));
        let t = n.banco.criar_timer(None, "A", "#a00").unwrap();
        n.criar_regra("G:\\Projeto A", t).unwrap();
        let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", "G:\\Projeto A"));
        n.aplicar(&mud).unwrap();
        let mud = n.motor.evento(&ev(h(9, 8), "Stop", "s1", "G:\\Projeto A"));
        n.aplicar(&mud).unwrap();
        assert!(n.esperas_mudaram(h(9, 9)), "a sessão passou a esperar");
        assert!(!n.esperas_mudaram(h(9, 20)));
        // 30 min do dock recolhido: avisa
        assert!(n.esperas_mudaram(h(9, 38) + 1));
        assert!(!n.esperas_mudaram(h(9, 40)));
        // a virada do dia avisa (o "até a virada" do app)
        assert!(n.esperas_mudaram(ms_local(2026, 11, 18, 0, 0, F) + 1));
    }

    /// Medida da conferência do fim do transcript: a primeira leitura de um transcript de 5 MB
    /// (os últimos 2 MB), a leitura do que foi acrescentado e a conferência de 20 sessões sem
    /// mudança (só o `metadata`).
    /// `cargo test --release -p work-timer medir_conferencia -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn medir_conferencia_do_fim_do_transcript() {
        use nucleo::fim_do_transcript::LeitorDoTranscript;
        let d = tempfile::tempdir().unwrap();
        let tr = d.path().join("grande.jsonl");
        let linha = linha_resultado(&"x".repeat(2000), h(9, 0));
        let mut s = String::new();
        while s.len() < 5 * 1024 * 1024 {
            s.push_str(&linha);
        }
        s.push_str(&linha_usuario("[Request interrupted by user]", h(9, 0)));
        std::fs::write(&tr, &s).unwrap();
        let vezes = 50;
        let t0 = std::time::Instant::now();
        for _ in 0..vezes {
            let mut l = LeitorDoTranscript::default();
            assert_eq!(l.ler(&tr, h(9, 1)), Some(Some(h(9, 0))));
        }
        println!("primeira leitura (os últimos 2 MB de 5 MB): {:.3} ms", t0.elapsed().as_secs_f64() * 1000.0 / vezes as f64);
        let mut l = LeitorDoTranscript::default();
        l.ler(&tr, h(9, 1));
        let mut total = std::time::Duration::ZERO;
        for _ in 0..vezes {
            acrescentar(&tr, &linha);
            let t0 = std::time::Instant::now();
            assert_eq!(l.ler(&tr, h(9, 1)), Some(None));
            total += t0.elapsed();
        }
        println!("ler o que foi acrescentado (uma linha de 2 KB): {:.3} ms", total.as_secs_f64() * 1000.0 / vezes as f64);
        let mut n = nucleo(h(9, 0));
        for i in 0..20 {
            let mud = n.motor.evento(&EventoHook { transcript: Some(tr.to_string_lossy().into_owned()), ..ev(h(9, 0), "UserPromptSubmit", &format!("s{i}"), "G:\\Projeto A") });
            n.aplicar(&mud).unwrap();
        }
        conferir_fins(&mut n, h(9, 1));
        let t0 = std::time::Instant::now();
        for _ in 0..200 {
            assert_eq!(conferir_fins(&mut n, h(9, 1)), 0);
        }
        println!("conferir 20 sessões sem mudança: {:.3} ms", t0.elapsed().as_secs_f64() * 1000.0 / 200.0);
    }

    #[test]
    fn titulo_so_e_relido_se_o_transcript_mudou_e_a_sessao_esta_ativa() {
        let d = tempfile::tempdir().unwrap();
        let tr = d.path().join("s.jsonl");
        std::fs::write(&tr, "{}\n").unwrap();
        let mut n = nucleo(h(9, 0));
        let mut e = ev(h(9, 0), "UserPromptSubmit", "s1", "C:\\p");
        e.transcript = Some(tr.to_string_lossy().into_owned());
        n.motor.evento(&e);
        assert_eq!(n.titulos_a_ler(h(9, 0)).len(), 1);
        assert!(n.titulos_a_ler(h(9, 0) + 30_000).is_empty(), "menos de 1 min");
        assert!(n.titulos_a_ler(h(9, 2)).is_empty(), "arquivo igual não é relido");
        std::fs::write(&tr, "{}\n{\"mais\":1}\n").unwrap();
        assert_eq!(n.titulos_a_ler(h(9, 4)).len(), 1, "mudou: relê");
        // terminou: segue conferindo por 5 min depois do último evento, depois para
        let mut e = ev(h(9, 5), "Stop", "s1", "C:\\p");
        e.transcript = Some(tr.to_string_lossy().into_owned());
        n.motor.evento(&e);
        std::fs::write(&tr, "{}\n{\"mais\":1}\n{\"e mais\":2}\n").unwrap();
        assert_eq!(n.titulos_a_ler(h(9, 9)).len(), 1);
        std::fs::write(&tr, "{}\n").unwrap();
        assert!(n.titulos_a_ler(h(9, 15)).is_empty(), "esperando há mais de 5 min: 10 min de folga");
        assert_eq!(n.titulos_a_ler(h(9, 20)).len(), 1, "parada e renomeada: relê com folga");
        assert!(n.titulos_a_ler(h(9, 31)).is_empty(), "parada e igual: não relê");
    }

    /// O caso do Cliente B (rodada 7): a 5ee991 foi ligada sozinha a Flow pela regra e movida à mão para
    /// Cliente B; a conversa continua na 536b82, com o mesmo nome e a mesma pasta.
    const FLOW: &str = "G:\\Empresa\\app-web";
    const NOME_DA_CONVERSA: &str = "Projeto: avaliação da demo do Cliente B";
    fn caso_do_cliente_b(n: &mut Nucleo) -> (i64, i64) {
        let flow = n.banco.criar_timer(None, "Flow", "#111").unwrap();
        let cliente_b = n.banco.criar_timer(None, "Cliente B", "#222").unwrap();
        n.criar_regra(FLOW, flow).unwrap();
        let mud = n.motor.evento(&ev(h(10, 0), "UserPromptSubmit", "5ee991", FLOW));
        n.aplicar(&mud).unwrap();
        let mud = n.aplicar_titulos(vec![("5ee991".into(), Some(NOME_DA_CONVERSA.into()))], h(10, 1));
        n.aplicar(&mud).unwrap();
        n.ligar_sessao("5ee991", cliente_b, false, false, h(10, 48)).unwrap();
        let mud = n.motor.evento(&ev(h(11, 0), "Stop", "5ee991", FLOW));
        n.aplicar(&mud).unwrap();
        (flow, cliente_b)
    }

    #[test]
    fn conversa_continuada_herda_o_timer_quando_o_titulo_chega() {
        let mut n = nucleo(h(9, 0));
        let (flow, cliente_b) = caso_do_cliente_b(&mut n);
        let mud = n.motor.evento(&ev(h(11, 35), "UserPromptSubmit", "536b82", FLOW));
        n.aplicar(&mud).unwrap();
        // até o título chegar, a regra de pasta vale (e o dock pergunta se está certo)
        let s = n.snapshot(h(11, 35) + 2_000).unwrap();
        assert!(s.avisos.is_empty());
        assert_eq!(s.perguntas.len(), 1);
        assert_eq!((s.perguntas[0].tipo.as_str(), s.perguntas[0].sessao_id.as_deref()), ("conversa_nova", Some("536b82")));
        assert!(s.perguntas[0].texto.ends_with("contando em Flow pela pasta"), "{}", s.perguntas[0].texto);
        assert_eq!(n.motor.sessoes["536b82"].timer_id, Some(flow));
        let mud = n.aplicar_titulos(vec![("536b82".into(), Some(NOME_DA_CONVERSA.into()))], h(11, 36));
        n.aplicar(&mud).unwrap();
        // herdada: a pergunta sai (a herdada não pergunta)
        assert!(n.motor.perguntas.is_empty());
        // desde o início no Cliente B, no banco também, e o aviso diz a origem
        let trechos = n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert!(trechos.iter().filter(|t| t.sessao_id.as_deref() == Some("536b82")).all(|t| t.timer_id == Some(cliente_b)));
        let s = n.snapshot(h(11, 36) + 1_000).unwrap();
        assert_eq!(s.avisos.len(), 1);
        assert_eq!(s.avisos[0].texto, "Ligada a Cliente B pela conversa anterior");
        assert!(n.snapshot(h(11, 36) + 15_000).unwrap().avisos.is_empty(), "some em 15 s");
        // gravado: reaberto, segue herdada
        let gravada = n.banco.sessoes_desde(0).unwrap().into_iter().find(|s| s.id == "536b82").unwrap();
        assert_eq!((gravada.timer_id, gravada.ligacao), (Some(cliente_b), Some(Ligacao::Herdada)));
    }

    #[test]
    fn ao_abrir_a_continuada_ligada_pela_regra_herda_sem_aviso() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("w.db");
        let cliente_b = {
            let mut n = Nucleo::abrir(Banco::abrir(&p).unwrap(), h(9, 0), F, dia()).unwrap();
            let (_, cliente_b) = caso_do_cliente_b(&mut n);
            // a versão de antes: a 536b82 ficou em Flow, com o nome, sem a herança
            let mud = n.motor.evento(&ev(h(11, 35), "UserPromptSubmit", "536b82", FLOW));
            n.aplicar(&mud).unwrap();
            n.motor.sessoes.get_mut("536b82").unwrap().nome = Some(NOME_DA_CONVERSA.into());
            let s = n.motor.sessoes["536b82"].clone();
            n.aplicar(&[Mudanca::Sessao(s)]).unwrap();
            cliente_b
        };
        let n = Nucleo::abrir(Banco::abrir(&p).unwrap(), h(12, 0), F, dia()).unwrap();
        assert_eq!(n.motor.sessoes["536b82"].timer_id, Some(cliente_b));
        let trechos = n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert!(trechos.iter().filter(|t| t.sessao_id.as_deref() == Some("536b82")).all(|t| t.timer_id == Some(cliente_b) && t.fim.is_some()));
        assert!(n.snapshot(h(12, 0)).unwrap().avisos.is_empty());
    }

    fn h18(hh: u32, mm: u32) -> Ms {
        ms_local(2026, 11, 18, hh, mm, F)
    }
    /// Timer de cada trecho da sessão, separado em antes e depois de `corte`.
    fn timers_da_sessao(n: &Nucleo, sessao: &str, corte: Ms) -> (Vec<Option<i64>>, Vec<Option<i64>>) {
        let v = n.banco.trechos_entre(h(0, 0), h18(23, 0)).unwrap();
        let da = |antes: bool| {
            v.iter()
                .filter(|t| t.sessao_id.as_deref() == Some(sessao) && (t.inicio < corte) == antes)
                .map(|t| t.timer_id)
                .collect::<Vec<_>>()
        };
        (da(true), da(false))
    }

    #[test]
    fn conversa_nova_respondida_no_dia_seguinte_deixa_ontem_onde_estava() {
        let mut n = nucleo(h(9, 0));
        let be = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        let flow = n.banco.criar_timer(None, "Flow", "#0a0").unwrap();
        n.criar_regra("G:\\Empresa\\Projeto", be).unwrap();
        // às 23:40 a conversa nova conta em Projeto A pela regra e segue depois da meia-noite
        for (t, e) in [(h(23, 40), "UserPromptSubmit"), (h(23, 55), "PostToolUse"), (h18(0, 5), "PostToolUse")] {
            let mud = n.motor.evento(&ev(t, e, "c1", "G:\\Empresa\\Projeto"));
            n.aplicar(&mud).unwrap();
        }
        // às 9:00 do dia seguinte: "Outro timer… → Flow", desde o início
        let p = n.snapshot(h18(9, 0)).unwrap().perguntas.into_iter().find(|p| p.tipo == "conversa_nova").unwrap();
        n.responder(p.id, &format!("ligar:{flow}"), h18(9, 0)).unwrap();
        let (ontem, hoje) = timers_da_sessao(&n, "c1", h18(0, 0));
        assert!(!ontem.is_empty() && ontem.iter().all(|t| *t == Some(be)), "ontem fica em Projeto A: {ontem:?}");
        assert!(!hoje.is_empty() && hoje.iter().all(|t| *t == Some(flow)), "hoje: {hoje:?}");
    }

    #[test]
    fn mover_do_lapis_desde_o_inicio_leva_so_hoje() {
        let mut n = nucleo(h(9, 0));
        let be = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
        let flow = n.banco.criar_timer(None, "Flow", "#0a0").unwrap();
        n.criar_regra("G:\\Projeto A", be).unwrap();
        for (t, e) in [(h(23, 40), "UserPromptSubmit"), (h(23, 55), "PostToolUse"), (h18(0, 5), "PostToolUse")] {
            let mud = n.motor.evento(&ev(t, e, "s1", "G:\\Projeto A"));
            n.aplicar(&mud).unwrap();
        }
        n.ligar_sessao_com("s1", flow, true, true, false, None, h18(9, 0)).unwrap();
        let (ontem, hoje) = timers_da_sessao(&n, "s1", h18(0, 0));
        assert!(!ontem.is_empty() && ontem.iter().all(|t| *t == Some(be)), "{ontem:?}");
        assert!(!hoje.is_empty() && hoje.iter().all(|t| *t == Some(flow)), "{hoje:?}");
    }

    #[test]
    fn desligada_e_religada_deixa_no_timer_antigo_o_que_veio_antes() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("w.db");
        let (be, flow) = {
            let mut n = Nucleo::abrir(Banco::abrir(&p).unwrap(), h(9, 0), F, dia()).unwrap();
            let be = n.banco.criar_timer(None, "Projeto A", "#a00").unwrap();
            let flow = n.banco.criar_timer(None, "Flow", "#0a0").unwrap();
            n.criar_regra("G:\\Projeto A", be).unwrap();
            for (t, e) in [(h(9, 0), "UserPromptSubmit"), (h(9, 50), "Stop")] {
                let mud = n.motor.evento(&ev(t, e, "s1", "G:\\Projeto A"));
                n.aplicar(&mud).unwrap();
            }
            // o "Desligar deste timer" do ✎: o tempo de antes fica em Projeto A
            let mud = n.motor.desligar_sessao("s1", h(10, 0));
            n.aplicar(&mud).unwrap();
            (be, flow)
        };
        // o instante do desligar fica gravado: vale também depois de reabrir
        let mut n = Nucleo::abrir(Banco::abrir(&p).unwrap(), h(10, 30), F, dia()).unwrap();
        assert_eq!(n.motor.sessoes["s1"].desligada_em, Some(h(10, 0)));
        // a conversa segue sem timer e é ligada de novo a Flow, desde o início
        for (t, e) in [(h(10, 30), "UserPromptSubmit"), (h(10, 45), "Stop")] {
            let mud = n.motor.evento(&ev(t, e, "s1", "G:\\Projeto A"));
            n.aplicar(&mud).unwrap();
        }
        n.ligar_sessao("s1", flow, true, false, h(11, 0)).unwrap();
        let v = n.banco.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        let da = |antes: bool| -> Vec<Option<i64>> {
            v.iter().filter(|t| t.sessao_id.as_deref() == Some("s1") && (t.inicio < h(10, 0)) == antes).map(|t| t.timer_id).collect()
        };
        assert_eq!(da(true), [Some(be)], "antes do desligar: fica em Projeto A");
        assert!(!da(false).is_empty() && da(false).iter().all(|t| *t == Some(flow)), "depois: {:?}", da(false));
    }

    #[test]
    fn heranca_depois_da_virada_so_move_os_trechos_de_hoje() {
        let mut n = nucleo(h(9, 0));
        let (flow, cliente_b) = caso_do_cliente_b(&mut n);
        // a continuação conta em Flow pela regra desde ontem à noite e segue depois da meia-noite
        for (t, e) in [(h(23, 50), "UserPromptSubmit"), (h(23, 58), "PostToolUse"), (h18(0, 5), "PostToolUse")] {
            let mud = n.motor.evento(&ev(t, e, "536b82", FLOW));
            n.aplicar(&mud).unwrap();
        }
        // o nome chega (ou muda) hoje: só os trechos de hoje vão para o Cliente B
        let mud = n.aplicar_titulos(vec![("536b82".into(), Some(NOME_DA_CONVERSA.into()))], h18(0, 10));
        n.aplicar(&mud).unwrap();
        let (ontem, hoje) = timers_da_sessao(&n, "536b82", h18(0, 0));
        assert_eq!(ontem, [Some(flow)], "ontem fica como estava (pode já ter sido lançado)");
        assert!(!hoje.is_empty() && hoje.iter().all(|t| *t == Some(cliente_b)), "hoje: {hoje:?}");
        assert_eq!(n.motor.abertos.iter().find(|t| t.sessao_id.as_deref() == Some("536b82")).unwrap().timer_id, Some(cliente_b));
    }

    #[test]
    fn sessao_ligada_pela_regra_antes_de_abrir_nao_avisa_de_novo() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("w.db");
        {
            let mut n = Nucleo::abrir(Banco::abrir(&p).unwrap(), h(9, 0), F, dia()).unwrap();
            let flow = n.banco.criar_timer(None, "Flow", "#111").unwrap();
            n.criar_regra(FLOW, flow).unwrap();
            let mud = n.motor.evento(&ev(h(9, 0), "UserPromptSubmit", "s1", FLOW));
            n.aplicar(&mud).unwrap();
            assert_eq!(n.snapshot(h(9, 0)).unwrap().perguntas.len(), 1);
            // "Usar a regra direto": o aviso
            let mut cfg = n.banco.config_app().unwrap();
            cfg.conversa_nova = nucleo::config::CONVERSA_NOVA_REGRA.into();
            n.salvar_config(&cfg, &d.path().join("hook.json")).unwrap();
            let mud = n.motor.evento(&ev(h(9, 1), "UserPromptSubmit", "s2", FLOW));
            n.aplicar(&mud).unwrap();
            assert_eq!(n.snapshot(h(9, 1)).unwrap().avisos.len(), 1);
            assert_eq!(n.snapshot(h(9, 1)).unwrap().perguntas.len(), 1, "só a da s1");
        }
        let mut n = Nucleo::abrir(Banco::abrir(&p).unwrap(), h(10, 0), F, dia()).unwrap();
        assert!(!n.motor.cfg.confirmar_conversa_nova, "a opção ficou gravada");
        for s in ["s1", "s2"] {
            let mud = n.motor.evento(&ev(h(10, 0), "UserPromptSubmit", s, FLOW));
            n.aplicar(&mud).unwrap();
        }
        let s = n.snapshot(h(10, 0)).unwrap();
        assert!(s.avisos.is_empty() && s.perguntas.is_empty(), "não é sessão nova");
    }

    #[test]
    fn ao_abrir_no_dia_seguinte_nenhum_trecho_de_ontem_muda() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("w.db");
        let (flow, cliente_b) = {
            let mut n = Nucleo::abrir(Banco::abrir(&p).unwrap(), h(9, 0), F, dia()).unwrap();
            let ids = caso_do_cliente_b(&mut n);
            for (t, e) in [(h(11, 35), "UserPromptSubmit"), (h(11, 50), "Stop")] {
                let mud = n.motor.evento(&ev(t, e, "536b82", FLOW));
                n.aplicar(&mud).unwrap();
            }
            n.motor.sessoes.get_mut("536b82").unwrap().nome = Some(NOME_DA_CONVERSA.into());
            let s = n.motor.sessoes["536b82"].clone();
            n.aplicar(&[Mudanca::Sessao(s)]).unwrap();
            ids
        };
        let amanha = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        let n = Nucleo::abrir(Banco::abrir(&p).unwrap(), h18(9, 0), F, amanha).unwrap();
        // a sessão passa a contar no Cliente B daqui em diante, mas as horas de ontem ficam em Flow
        assert_eq!(n.motor.sessoes["536b82"].timer_id, Some(cliente_b));
        let (ontem, hoje) = timers_da_sessao(&n, "536b82", h18(0, 0));
        assert_eq!(ontem, [Some(flow)]);
        assert!(hoje.is_empty());
    }

    #[test]
    fn sessao_parada_com_nome_nao_e_lida_ao_abrir_mas_a_renomeada_e() {
        let d = tempfile::tempdir().unwrap();
        let tr = d.path().join("s.jsonl");
        std::fs::write(&tr, "{}\n").unwrap();
        let mut n = nucleo(h(9, 0));
        for (min, evento) in [(0, "UserPromptSubmit"), (1, "Stop")] {
            let mut e = ev(h(8, min), evento, "s1", "C:\\p");
            e.transcript = Some(tr.to_string_lossy().into_owned());
            n.motor.evento(&e);
        }
        n.motor.sessoes.get_mut("s1").unwrap().nome = Some("Nome antigo".into());
        assert!(n.titulos_a_ler(h(9, 0)).is_empty(), "ao abrir, só guarda como está o transcript");
        assert!(n.titulos_a_ler(h(9, 15)).is_empty(), "igual: não lê");
        std::fs::write(&tr, "{}\n{\"renomeou\":1}\n").unwrap();
        assert!(n.titulos_a_ler(h(9, 20)).is_empty(), "menos de 10 min desde a última conferência");
        assert_eq!(n.titulos_a_ler(h(9, 25)).len(), 1);
    }

    #[test]
    fn tique_gravado_a_cada_ciclo_so_com_algo_rodando() {
        let mut n = nucleo(h(9, 0));
        let lido = |n: &Nucleo| n.banco.valor("ultimo_tique").unwrap().unwrap().parse::<Ms>().unwrap();
        n.gravar_tique(h(9, 0) + 5_000).unwrap();
        assert_eq!(lido(&n), h(9, 0), "parado: espera 1 min");
        n.gravar_tique(h(9, 1)).unwrap();
        assert_eq!(lido(&n), h(9, 1));
        let t = n.banco.criar_timer(None, "A", "#a00").unwrap();
        let mud = n.motor.play(t, h(9, 1));
        n.aplicar(&mud).unwrap();
        n.gravar_tique(h(9, 1) + 5_000).unwrap();
        assert_eq!(lido(&n), h(9, 1) + 5_000, "rodando: todo ciclo");
    }

    #[test]
    fn nome_identificador_e_executaveis_do_work_timer() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        // o productName é o nome interno (pasta de instalação, registro, autostart); o de
        // exibição vem de `APP_NOME` (títulos postos pelo Rust, instalador pelo build.rs)
        assert_eq!(conf["productName"], nucleo::NOME_INTERNO);
        assert_eq!(conf["productName"], "Work Timer", "troca só no lançamento");
        assert_eq!(conf["identifier"], "io.github.eaeniko.worktimer");
        assert_eq!(conf["mainBinaryName"], "work-timer");
        assert_eq!(conf["bundle"]["externalBin"][0], format!("binaries/{}", fila::caminhos::HOOK_NOME));
        assert_eq!(conf["bundle"]["windows"]["nsis"]["installerIcon"], "icons/icon.ico");
        // rodada 18: a autoria nas propriedades do exe (CompanyName e LegalCopyright, pelo
        // tauri-build) e no "Aplicativos instalados" (o Publisher do instalador)
        assert_eq!(conf["bundle"]["publisher"], "eaeniko");
        assert_eq!(conf["bundle"]["copyright"], "© 2026 Nikolai Cinotti (@eaeniko)");
        let janelas = conf["app"]["windows"].as_array().unwrap();
        assert_eq!(janelas[0]["title"], "", "posto pelo Rust ao criar a janela");
        assert_eq!(janelas[1]["title"], "", "posto pelo Rust no setup");
        assert_eq!(conf["bundle"]["windows"]["nsis"]["template"], "gen/nsis/instalador.nsi");
        // a pasta de dados não depende do identificador (fica fora do AppData)
        assert_eq!(fila::caminhos::PASTA_DADOS, ".worktimer");
    }

    /// O modelo do instalador gerado pelo build.rs: o nome de exibição nos textos e atalhos; o
    /// interno (o productName de antes) na pasta, no registro e no autostart, para atualizar a
    /// instalação existente no mesmo lugar.
    #[test]
    fn instalador_com_o_nome_de_exibicao_e_a_instalacao_no_mesmo_lugar() {
        let nsi = include_str!("../gen/nsis/instalador.nsi").replace("\r\n", "\n");
        assert!(!nsi.contains("__NOME_APP__"));
        assert!(nsi.contains(&format!("!define PRODUCTNAME \"{}\"", nucleo::APP_NOME)));
        assert!(nsi.contains("!define NOMEINTERNO \"{{product_name}}\""));
        for interno in [
            r#"!define UNINSTKEY "Software\Microsoft\Windows\CurrentVersion\Uninstall\${NOMEINTERNO}""#,
            r#"!define MANUPRODUCTKEY "${MANUKEY}\${NOMEINTERNO}""#,
            r#"StrCpy $INSTDIR "$LOCALAPPDATA\${NOMEINTERNO}""#,
            r#"DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${NOMEINTERNO}""#,
        ] {
            assert!(nsi.contains(interno), "{interno}");
        }
        for exibicao in [r#"Name "${PRODUCTNAME}""#, r#"WriteRegStr SHCTX "${UNINSTKEY}" "DisplayName" "${PRODUCTNAME}""#] {
            assert!(nsi.contains(exibicao), "{exibicao}");
        }
        // nenhuma pasta nem chave com o nome de exibição
        assert!(!nsi.contains(r#"$LOCALAPPDATA\${PRODUCTNAME}"#));
        assert!(!nsi.contains(r#"Uninstall\${PRODUCTNAME}"#));
        // os atalhos de antes viram atalhos com o nome novo; o da área de trabalho também na seção
        // de instalação (sem depender da caixa da última página)
        let area = r#"!insertmacro TrocarAtalhoAntigo "$DESKTOP\${NOMEINTERNO}.lnk" "$DESKTOP\${PRODUCTNAME}.lnk""#;
        assert_eq!(nsi.matches(area).count(), 2);
        let secao = &nsi[nsi.find("Section Install").unwrap()..];
        assert!(secao[..secao.find("SectionEnd").unwrap()].contains(area));
        // desinstalar antes de outra versão é atualização: o autostart e os atalhos ficam
        assert!(nsi.contains("${AndIf} $R0 <> 0\n        StrCpy $R1 \"$R1 /UPDATE\""));
    }

    // ---- backups ----

    fn local(d: u32, hh: u32, mm: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap().and_hms_opt(hh, mm, 0).unwrap()
    }

    /// Pasta de dados nova (`.worktimer`) com o banco aberto num `Nucleo`.
    fn nucleo_em(base: &Path, agora: Ms) -> Nucleo {
        std::fs::create_dir_all(base).unwrap();
        Nucleo::abrir(Banco::abrir(&fila::caminhos::banco_em(base)).unwrap(), agora, F, dia()).unwrap()
    }

    #[test]
    fn backup_automatico_ao_abrir_e_a_cada_6_h_so_com_timers() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("backups");
        let mut n = nucleo(h(9, 0));
        assert_eq!(n.backup_se_devido(&dir, local(17, 9, 0), true).unwrap(), None, "primeiro uso: nada a guardar");
        n.banco.criar_timer(None, "A", "#a00").unwrap();
        let p = n.backup_se_devido(&dir, local(17, 9, 0), true).unwrap().expect("primeira abertura do dia");
        assert_eq!(p.file_name().unwrap(), "2026-11-17_0900.db");
        assert_eq!(n.backup_se_devido(&dir, local(17, 14, 59), false).unwrap(), None);
        assert!(n.backup_se_devido(&dir, local(17, 15, 0), false).unwrap().is_some(), "6 h depois");
        // outro núcleo (o app reaberto) lê a pasta: já tem de hoje
        let mut outro = nucleo(h(9, 0));
        outro.banco.criar_timer(None, "A", "#a00").unwrap();
        assert_eq!(outro.backup_se_devido(&dir, local(17, 16, 0), true).unwrap(), None);
        assert!(outro.backup_se_devido(&dir, local(18, 8, 0), true).unwrap().is_some(), "abertura de outro dia");
        // a rotação deixa os 14 mais recentes e o último de cada dia anterior (17 e 18)
        for i in 0..20 {
            n.backup_agora(&dir, local(19, 0, i)).unwrap();
        }
        let l = dados::listar(&dir);
        assert_eq!(l.len(), dados::MANTER_BACKUPS + 2);
        assert_eq!(l[0].nome, "2026-11-19_0019.db");
        assert_eq!(l[14].nome, "2026-11-18_0800.db");
        assert_eq!(l[15].nome, "2026-11-17_1500.db");
    }

    #[test]
    fn restaurar_faz_backup_antes_e_reabre_sem_reiniciar() {
        let d = tempfile::tempdir().unwrap();
        let base = d.path().join(".worktimer");
        let banco = fila::caminhos::banco_em(&base);
        let dir = base.join("backups");
        let hook_json = base.join("hook.json");
        let mut n = nucleo_em(&base, h(9, 0));
        n.banco.criar_timer(None, "Antes", "#a00").unwrap();
        n.salvar_config(&ConfigApp { pastas_ignoradas: vec!["C:\\velha".into()], ..ConfigApp::default() }, &hook_json).unwrap();
        let bk = n.backup_agora(&dir, local(17, 9, 0)).unwrap();
        // depois do backup: outro timer, outra pasta ignorada, o início com o Windows ligado e a
        // fila lida até 120
        n.banco.criar_timer(None, "Depois", "#a00").unwrap();
        let cfg = ConfigApp { pastas_ignoradas: vec!["C:\\nova".into()], iniciar_com_windows: true, ..ConfigApp::default() };
        n.salvar_config(&cfg, &hook_json).unwrap();
        n.fila_offset = 120;
        let nome = bk.file_name().unwrap().to_string_lossy().into_owned();

        assert!(n.restaurar_backup(&dir, "../worktimer.db", &banco, local(17, 10, 0), h(10, 0), F, dia()).is_err());
        assert!(n.restaurar_backup(&dir, "2026-01-01_0000.db", &banco, local(17, 10, 0), h(10, 0), F, dia()).is_err());
        n.restaurar_backup(&dir, &nome, &banco, local(17, 10, 0), h(10, 0), F, dia()).unwrap();
        n.gravar_hook_json(&hook_json).unwrap();

        let nomes: Vec<String> = n.banco.timers().unwrap().into_iter().map(|t| t.nome).collect();
        assert_eq!(nomes, ["Antes"]);
        assert_eq!(n.motor.cfg.pastas_ignoradas, ["C:\\velha"], "o motor reabriu com a config do backup");
        assert!(n.banco.config_app().unwrap().iniciar_com_windows, "o início com o Windows fica como estava (o registro manda)");
        let j: serde_json::Value = serde_json::from_slice(&std::fs::read(&hook_json).unwrap()).unwrap();
        assert_eq!(j["pastas_ignoradas"][0], "C:\\velha");
        assert_eq!(n.fila_offset, 120, "a fila não é relida");
        // o estado de antes de restaurar virou backup, com os dois timers
        let l = dados::listar(&dir);
        assert_eq!(l.iter().map(|b| b.nome.as_str()).collect::<Vec<_>>(), ["2026-11-17_1000.db", "2026-11-17_0900.db"]);
        let antes = Banco::abrir(&dir.join("2026-11-17_1000.db")).unwrap();
        assert_eq!(antes.timers().unwrap().len(), 2);
        assert!(!base.join("restaurando.db").exists());
        // o banco reaberto é o arquivo de verdade: outro núcleo vê o restaurado
        drop(n);
        let n = nucleo_em(&base, h(10, 1));
        assert_eq!(n.banco.timers().unwrap().len(), 1);
    }

    /// Com a pasta cheia (14), restaurar o mais antigo: o backup de antes de restaurar não pode
    /// empurrá-lo para fora.
    #[test]
    fn restaurar_o_mais_antigo_de_14_nao_o_apaga() {
        let d = tempfile::tempdir().unwrap();
        let base = d.path().join(".worktimer");
        let banco = fila::caminhos::banco_em(&base);
        let dir = base.join("backups");
        let mut n = nucleo_em(&base, h(9, 0));
        n.banco.criar_timer(None, "Primeiro", "#a00").unwrap();
        // 14 backups no mesmo dia (a regra do "um por dia" não salva nenhum)
        for i in 0..14 {
            if i == 1 {
                n.banco.criar_timer(None, "Segundo", "#a00").unwrap();
            }
            n.backup_agora(&dir, local(17, 8, i)).unwrap();
        }
        let l = dados::listar(&dir);
        assert_eq!(l.len(), 14);
        let mais_antigo = l.last().unwrap().nome.clone();
        assert_eq!(mais_antigo, "2026-11-17_0800.db");
        n.restaurar_backup(&dir, &mais_antigo, &banco, local(17, 9, 0), h(9, 30), F, dia()).unwrap();
        assert_eq!(n.banco.timers().unwrap().len(), 1, "voltou ao mais antigo");
        let nomes: Vec<String> = dados::listar(&dir).into_iter().map(|b| b.nome).collect();
        assert!(nomes.contains(&mais_antigo), "o escolhido continua na lista");
        assert!(nomes.contains(&"2026-11-17_0900.db".to_string()), "o de antes de restaurar também");
        // dá para voltar: restaurar o de antes devolve os dois timers
        n.restaurar_backup(&dir, "2026-11-17_0900.db", &banco, local(17, 9, 5), h(9, 35), F, dia()).unwrap();
        assert_eq!(n.banco.timers().unwrap().len(), 2);
    }

    /// Um banco válido com um timer, para as trocas.
    fn banco_com(caminho: &Path, timer: &str) {
        Banco::abrir(caminho).unwrap().criar_timer(None, timer, "#a00").unwrap();
    }

    fn abrir_de_verdade(p: &Path) -> Result<Nucleo, String> {
        Banco::abrir(p).map_err(texto).and_then(|b| Nucleo::abrir(b, h(9, 0), F, dia()))
    }

    fn timers_de(n: &Nucleo) -> Vec<String> {
        n.banco.timers().unwrap().into_iter().map(|t| t.nome).collect()
    }

    #[test]
    fn troca_do_banco_que_falha_volta_ao_de_antes() {
        let d = tempfile::tempdir().unwrap();
        let banco = d.path().join("worktimer.db");
        banco_com(&banco, "Atual");
        let mut n = abrir_de_verdade(&banco).unwrap();

        // 1. o novo não confere (não é um banco): nada muda, nem fecha o atual
        let lixo = d.path().join("lixo.db");
        std::fs::write(&lixo, vec![7u8; 4096]).unwrap();
        assert!(n.trocar_banco(&lixo, &banco, h(9, 0), F, dia()).is_err());
        assert_eq!(timers_de(&n), ["Atual"]);

        // 2. o novo está preso por outro programa (sem compartilhar a exclusão): o rename para o
        // lugar falha depois de o atual ter virado ".anterior"; tudo volta
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            let novo = d.path().join("novo2.db");
            banco_com(&novo, "Novo2");
            let preso = std::fs::OpenOptions::new().read(true).share_mode(1).open(&novo).unwrap();
            let e = n.trocar_banco(&novo, &banco, h(9, 0), F, dia()).unwrap_err();
            drop(preso);
            assert!(e.contains("não consegui pôr o banco novo no lugar") && e.contains("Nada mudou"), "{e}");
            assert_eq!(timers_de(&n), ["Atual"]);
            assert!(n.quebrado.is_none());
            assert!(!d.path().join("worktimer.db.anterior").exists(), "o de antes voltou para o lugar");
        }

        // 3. o novo foi para o lugar, mas não abre: volta o de antes, e o novo fica de lado
        let novo = d.path().join("novo3.db");
        banco_com(&novo, "Novo3");
        let mut primeira = true;
        let r = n.trocar_banco_com(&novo, &banco, &mut |p| {
            if std::mem::take(&mut primeira) {
                return Err("migração quebrou".into());
            }
            abrir_de_verdade(p)
        });
        let e = r.unwrap_err();
        assert!(e.contains("o banco novo não abriu: migração quebrou") && e.contains("Nada mudou"), "{e}");
        assert_eq!(timers_de(&n), ["Atual"], "reabriu o de antes");
        assert!(n.quebrado.is_none());
        assert!(novo.exists(), "o novo que não abriu volta para o lugar dele");
        assert_eq!(timers_de(&abrir_de_verdade(&banco).unwrap()), ["Atual"]);

        // 4. nada abre (nem o de antes): quebrado, e não grava nem mostra estado velho
        let novo = d.path().join("novo4.db");
        banco_com(&novo, "Novo4");
        let e = n.trocar_banco_com(&novo, &banco, &mut |_| Err("disco sumiu".into())).unwrap_err();
        assert!(e.contains("o de antes também não abriu"), "{e}");
        assert!(n.quebrado.is_some());
        assert!(n.snapshot(h(9, 1)).unwrap_err().contains("O banco não abriu"));
        let mud = n.motor.play(1, h(9, 1));
        assert!(n.aplicar(&mud).is_err(), "nada vai para o banco em memória");
        assert!(n.backup_agora(&d.path().join("bk"), local(17, 9, 1)).is_err(), "nem backup do banco em memória");
        assert!(n.guardar_dock(|c| c.dock_x = Some(1)).is_err());
        assert!(n.salvar_config(&ConfigApp::default(), &d.path().join("hook.json")).is_err());
        assert!(n.ajustar_hoje(1, None, 60_000, None, h(9, 1)).is_err());
        // o arquivo de antes continua no disco, intacto
        assert_eq!(timers_de(&abrir_de_verdade(&banco).unwrap()), ["Atual"]);
    }

    /// O novo foi para o lugar, não abriu e nem consegue voltar para o caminho dele: vai para
    /// ".falhou", e o de antes volta mesmo assim.
    #[test]
    fn banco_novo_que_nao_abre_nem_sai_vai_para_falhou() {
        let d = tempfile::tempdir().unwrap();
        let banco = d.path().join("worktimer.db");
        banco_com(&banco, "Atual");
        let mut n = abrir_de_verdade(&banco).unwrap();
        let novo = d.path().join("novo.db");
        banco_com(&novo, "Novo");
        let mut primeira = true;
        let e = n
            .trocar_banco_com(&novo, &banco, &mut |p| {
                if std::mem::take(&mut primeira) {
                    // o caminho do novo ficou ocupado (uma pasta): ele não consegue voltar
                    std::fs::create_dir(&novo).unwrap();
                    return Err("não abre".into());
                }
                abrir_de_verdade(p)
            })
            .unwrap_err();
        assert!(e.contains("Nada mudou"), "{e}");
        assert_eq!(timers_de(&n), ["Atual"], "o de antes voltou");
        assert!(n.quebrado.is_none());
        assert_eq!(timers_de(&abrir_de_verdade(&d.path().join("worktimer.db.falhou")).unwrap()), ["Novo"]);
    }

    #[test]
    fn troca_que_da_certo_guarda_o_de_antes_com_o_wal() {
        let d = tempfile::tempdir().unwrap();
        let banco = d.path().join("worktimer.db");
        banco_com(&banco, "Atual");
        let mut n = abrir_de_verdade(&banco).unwrap();
        let novo = d.path().join("novo.db");
        banco_com(&novo, "Novo");
        n.trocar_banco(&novo, &banco, h(9, 0), F, dia()).unwrap();
        assert_eq!(timers_de(&n), ["Novo"]);
        assert!(!novo.exists());
        let anterior = d.path().join("worktimer.db.anterior");
        assert_eq!(timers_de(&abrir_de_verdade(&anterior).unwrap()), ["Atual"]);
    }

    #[test]
    fn ao_minimizar_grava_so_as_escolhas_conhecidas() {
        let n = nucleo(h(9, 0));
        assert_eq!(n.banco.config_app().unwrap().ao_minimizar, "perguntar");
        assert_eq!(n.guardar_ao_minimizar("bandeja").unwrap().ao_minimizar, "bandeja");
        assert_eq!(n.guardar_ao_minimizar("barra").unwrap().ao_minimizar, "barra");
        assert!(n.guardar_ao_minimizar("perguntar").is_err());
        assert!(n.guardar_ao_minimizar("x").is_err());
        assert_eq!(n.banco.config_app().unwrap().ao_minimizar, "barra");
    }

    #[test]
    fn reabrir_em_menos_de_2_min_retoma_o_manual_sem_pergunta() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Cliente F", "#a00").unwrap();
        b.aplicar(&[Mudanca::Abriu { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, inicio: h(8, 0), atividade_id: 1 }])
            .unwrap();
        b.gravar_valor("ultimo_tique", &h(9, 0).to_string()).unwrap();
        let n = Nucleo::abrir(b, h(9, 1), F, dia()).unwrap();
        assert_eq!(n.banco.abertos().unwrap().len(), 1);
        let s = n.snapshot(h(9, 1)).unwrap();
        assert!(s.perguntas.is_empty());
        assert_eq!(s.rodando, 1);
        assert_eq!(s.grupos[0].timers[0].trechos[0].inicio, h(8, 0));
    }

    #[test]
    fn atualizar_fuso_so_quando_o_dia_muda() {
        let mut n = nucleo(h(9, 0));
        n.atualizar_fuso(-120, dia());
        assert_eq!(n.motor.cfg.fuso_min, F);
        n.atualizar_fuso(-120, dia().succ_opt().unwrap());
        assert_eq!((n.fuso_min, n.motor.cfg.fuso_min), (-120, -120));
    }
}
