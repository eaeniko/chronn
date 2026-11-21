// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Result};

use crate::config::ConfigApp;
use crate::modelo::*;
use crate::motor::{chave_de_pasta, raiz_do_projeto, regra_para};
use crate::relatorio::{acumulado, Acumulado, Zerados};
use crate::tempo::{dia_de, inicio_do_dia, Ms};

/// Esquema da versão 1 (o banco novo passa por ele e depois pelas migrações).
pub(crate) const ESQUEMA_V1: &str = "
CREATE TABLE grupos (id INTEGER PRIMARY KEY, nome TEXT NOT NULL, cor TEXT NOT NULL, link_projeto TEXT, card_padrao TEXT, ordem INTEGER NOT NULL DEFAULT 0, arquivado INTEGER NOT NULL DEFAULT 0);
CREATE TABLE timers (id INTEGER PRIMARY KEY, grupo_id INTEGER REFERENCES grupos(id), nome TEXT NOT NULL, cor TEXT NOT NULL, link_card TEXT, ordem INTEGER NOT NULL DEFAULT 0, arquivado INTEGER NOT NULL DEFAULT 0);
CREATE TABLE sessoes (id TEXT PRIMARY KEY, cwd TEXT NOT NULL, nome TEXT, transcript TEXT, timer_id INTEGER REFERENCES timers(id), estado TEXT NOT NULL, ultimo_evento INTEGER NOT NULL, terminou_em INTEGER, criada_em INTEGER NOT NULL);
CREATE TABLE trechos (id INTEGER PRIMARY KEY, timer_id INTEGER REFERENCES timers(id), sessao_id TEXT, origem TEXT NOT NULL, inicio INTEGER NOT NULL, fim INTEGER, motivo_fim TEXT, descartado INTEGER NOT NULL DEFAULT 0);
CREATE INDEX trechos_inicio ON trechos(inicio);
CREATE TABLE regras_pasta (id INTEGER PRIMARY KEY, pasta TEXT NOT NULL UNIQUE, timer_id INTEGER NOT NULL REFERENCES timers(id));
CREATE TABLE config (chave TEXT PRIMARY KEY, valor TEXT NOT NULL);
PRAGMA user_version = 1;
";

/// Versão 2: nomes dados aos links do Zoho (o link só traz códigos) e ajustes à mão do tempo.
const MIGRACAO_V2: &str = "
ALTER TABLE timers ADD COLUMN link_card_nome TEXT;
ALTER TABLE grupos ADD COLUMN link_projeto_nome TEXT;
ALTER TABLE grupos ADD COLUMN card_padrao_nome TEXT;
CREATE TABLE ajustes (id INTEGER PRIMARY KEY, timer_id INTEGER NOT NULL REFERENCES timers(id), dia TEXT NOT NULL, delta_ms INTEGER NOT NULL, motivo TEXT, criado_em INTEGER NOT NULL);
CREATE INDEX ajustes_dia ON ajustes(dia);
PRAGMA user_version = 2;
";

/// Versão 3: o "Zerar hoje" descarta também os ajustes à mão (e o "Desfazer" os devolve).
const MIGRACAO_V3: &str = "
ALTER TABLE ajustes ADD COLUMN descartado INTEGER NOT NULL DEFAULT 0;
PRAGMA user_version = 3;
";

/// Versão 4: como a sessão foi ligada ao timer ("regra", "mao" ou "herdada"): a ligação à mão
/// vence a regra de pasta e passa para a conversa continuada (`Motor::herdar`).
const MIGRACAO_V4: &str = "
ALTER TABLE sessoes ADD COLUMN ligacao TEXT;
PRAGMA user_version = 4;
";

/// Versão 5 (`atividades_sql`): as atividades (Trabalho, Reunião, Estudo...). Cada trecho tem uma (os de antes
/// ficam com Trabalho), cada timer tem a padrão (Trabalho) e cada ajuste à mão soma numa
/// (Trabalho). Feita em Rust (`migrar_v5`): cada passo confere se já está feito, então rodar de
/// novo não muda nada. As colunas novas têm o padrão Trabalho, e por isso um app da versão 4 abre
/// um banco da 5 (o que ele grava cai em Trabalho).
/// Os nomes nascem no idioma do Windows (rodada 23; o banco novo ainda não tem config) e depois
/// são dados da pessoa.
fn atividades_sql() -> String {
    let i = crate::idioma::da_escolha(crate::idioma::IDIOMA_SISTEMA);
    let n = |k| crate::idioma::nome_padrao(i, k).replace('\'', "''");
    format!(
        "CREATE TABLE IF NOT EXISTS atividades (id INTEGER PRIMARY KEY, nome TEXT NOT NULL, cor TEXT NOT NULL, ordem INTEGER NOT NULL DEFAULT 0, arquivada INTEGER NOT NULL DEFAULT 0);
INSERT OR IGNORE INTO atividades (id, nome, cor, ordem) VALUES (1, '{}', '#5b8def', 0), (2, '{}', '#e8a23a', 1), (3, '{}', '#3fb68b', 2);",
        n("padrao.trabalho"),
        n("padrao.reuniao"),
        n("padrao.estudo")
    )
}

/// Colunas da versão 5: (tabela, coluna). Todas `INTEGER NOT NULL DEFAULT 1` (Trabalho). Sem
/// `REFERENCES`: com as chaves estrangeiras ligadas, o SQLite não acrescenta coluna com chave
/// estrangeira e padrão que não é nulo.
const COLUNAS_V5: [(&str, &str); 3] =
    [("trechos", "atividade_id"), ("timers", "atividade_padrao_id"), ("ajustes", "atividade_id")];

/// Chave da config com os timers encerrados (■) e o dia deles (`Mudanca::Encerrados`).
pub const CHAVE_ENCERRADOS: &str = "encerrados";

/// Versão do esquema que este código conhece.
pub const VERSAO_ESQUEMA: i64 = 9;

/// A marca do app no banco (rodada 18): o `PRAGMA application_id` com "CHRN", gravado na
/// criação e na abertura de um banco sem marca (0). Um banco com outra marca fica como está. Não
/// muda o esquema nem o `user_version`.
fn marcar_application_id(conn: &Connection) -> Result<()> {
    let id: i32 = conn.query_row("PRAGMA application_id", [], |r| r.get(0))?;
    if id == 0 {
        conn.pragma_update(None, "application_id", crate::APPLICATION_ID)?;
    }
    Ok(())
}

/// Cor do primeiro espaço (o "Trabalho" da migração, o "Geral" do banco novo).
pub const COR_ESPACO_PRIMEIRO: &str = "#5b8def";

/// A versão 9 (rodada 20): os espaços, um nível acima dos grupos.
/// - a tabela `espacos` (id, nome, cor, ordem, arquivado);
/// - `grupos.espaco_id` e `timers.espaco_id` (`INTEGER NOT NULL DEFAULT 1`; o do timer vale só
///   sem grupo: o "Sem grupo" de cada espaço);
/// - a tabela `timer_atividades_ocultas` (timer, atividade): as atividades que o painel do timer
///   não mostra.
///
/// O primeiro espaço (id 1) é "Trabalho" num banco que já tem grupos ou timers (todos vão para
/// ele pelo padrão das colunas) e "Geral" num banco novo. Cada passo confere se já está feito, então
/// rodar de novo não muda nada; um app da versão 8 abre o banco (as colunas e as tabelas ficam de
/// fora do que ele lê, e o que ele grava cai no espaço 1).
fn migrar_v9(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS espacos (id INTEGER PRIMARY KEY, nome TEXT NOT NULL, cor TEXT NOT NULL, ordem INTEGER NOT NULL DEFAULT 0, arquivado INTEGER NOT NULL DEFAULT 0);
         CREATE TABLE IF NOT EXISTS timer_atividades_ocultas (timer_id INTEGER NOT NULL REFERENCES timers(id), atividade_id INTEGER NOT NULL, PRIMARY KEY (timer_id, atividade_id));",
    )?;
    let tem_espaco: bool = conn.query_row("SELECT EXISTS (SELECT 1 FROM espacos)", [], |r| r.get(0))?;
    if !tem_espaco {
        let com_dados: bool = conn.query_row(
            "SELECT EXISTS (SELECT 1 FROM grupos) OR EXISTS (SELECT 1 FROM timers)",
            [],
            |r| r.get(0),
        )?;
        // no idioma do Windows (rodada 23)
        let i = crate::idioma::da_escolha(crate::idioma::IDIOMA_SISTEMA);
        let nome = crate::idioma::nome_padrao(i, if com_dados { "padrao.trabalho" } else { "padrao.geral" });
        conn.execute(
            "INSERT INTO espacos (id, nome, cor, ordem) VALUES (?1, ?2, ?3, 0)",
            params![ESPACO_PRIMEIRO, nome, COR_ESPACO_PRIMEIRO],
        )?;
    }
    for tabela in ["grupos", "timers"] {
        if !tem_coluna(conn, tabela, "espaco_id")? {
            conn.execute_batch(&format!("ALTER TABLE {tabela} ADD COLUMN espaco_id INTEGER NOT NULL DEFAULT {ESPACO_PRIMEIRO};"))?;
        }
    }
    conn.execute_batch("PRAGMA user_version = 9;")
}

/// A versão 8 (rodada 19): `zerados_atividade`, o último "Zerar" só de uma atividade de um timer
/// (o ↺ "Só Reunião"). Uma atividade conta desde o mais recente entre o zerar do timer e o dela.
/// Os bancos de antes ficam sem nenhum: os números são os mesmos. Rodar de novo não muda nada; um
/// app da versão 7 abre o banco (a tabela fica de fora do que ele lê e grava).
fn migrar_v8(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS zerados_atividade (timer_id INTEGER NOT NULL REFERENCES timers(id), atividade_id INTEGER NOT NULL, zerado_em INTEGER NOT NULL, PRIMARY KEY (timer_id, atividade_id));
         PRAGMA user_version = 8;",
    )
}

/// A versão 7 (rodada 15): `sessoes.desligada_em`, o instante do último "Desligar" da sessão.
/// Nula nas sessões de antes. Rodar de novo não muda nada; um app da versão 6 abre o banco (a
/// coluna fica de fora do que ele grava).
fn migrar_v7(conn: &Connection) -> Result<()> {
    if !tem_coluna(conn, "sessoes", "desligada_em")? {
        conn.execute_batch("ALTER TABLE sessoes ADD COLUMN desligada_em INTEGER;")?;
    }
    conn.execute_batch("PRAGMA user_version = 7;")
}

/// A versão 6: `timers.zerado_em`, o instante do último "Zerar" de cada timer (o número do timer
/// é o acumulado desde ele). Na migração, cada timer recebe o início do dia de trabalho atual
/// (`inicio_hoje`, pelo corte configurado): ao atualizar, os números são os mesmos de hoje. O
/// `ALTER TABLE` não aceita um instante como padrão: o padrão é 0 ("desde sempre"; para um timer
/// sem tempo nenhum, o mesmo número), que só um app da versão 5 grava; este grava o início do dia
/// ao criar (`criar_timer_em`). Rodar de novo não muda nada. Os ajustes já guardam o instante de criação
/// (`criado_em`) desde a versão 2.
fn migrar_v6(conn: &Connection, inicio_hoje: Ms) -> Result<()> {
    if !tem_coluna(conn, "timers", "zerado_em")? {
        conn.execute_batch("ALTER TABLE timers ADD COLUMN zerado_em INTEGER NOT NULL DEFAULT 0;")?;
        conn.execute("UPDATE timers SET zerado_em = ?1", params![inicio_hoje])?;
    }
    conn.execute_batch("PRAGMA user_version = 6;")
}

/// Início do dia de trabalho de `agora` pelo corte gravado na config do banco (sem config, ou com
/// ela estragada, a meia-noite).
fn inicio_de_hoje(conn: &Connection, agora: Ms, fuso_min: i32) -> Result<Ms> {
    let j: Option<String> =
        conn.query_row("SELECT valor FROM config WHERE chave = 'app'", [], |r| r.get(0)).optional()?;
    let corte = j.and_then(|j| ConfigApp::de_json(&j).ok()).map(|c| c.corte_min).unwrap_or(0);
    Ok(inicio_do_dia(dia_de(agora, corte, fuso_min), corte, fuso_min))
}

/// O relógio do sistema: agora (ms) e o fuso local em minutos.
fn relogio_do_sistema() -> (Ms, i32) {
    let l = chrono::Local::now();
    (l.timestamp_millis(), l.offset().local_minus_utc() / 60)
}

/// A coluna existe na tabela?
fn tem_coluna(conn: &Connection, tabela: &str, coluna: &str) -> Result<bool> {
    let mut st = conn.prepare(&format!("PRAGMA table_info({tabela})"))?;
    let nomes = st.query_map([], |r| r.get::<_, String>(1))?;
    for n in nomes {
        if n? == coluna {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A versão 5 (`atividades_sql`, `COLUNAS_V5`), sem repetir o que já está feito.
fn migrar_v5(conn: &Connection) -> Result<()> {
    conn.execute_batch(&atividades_sql())?;
    for (tabela, coluna) in COLUNAS_V5 {
        if !tem_coluna(conn, tabela, coluna)? {
            conn.execute_batch(&format!("ALTER TABLE {tabela} ADD COLUMN {coluna} INTEGER NOT NULL DEFAULT 1;"))?;
        }
    }
    conn.execute_batch("PRAGMA user_version = 5;")
}

fn texto_invalido(col: usize, s: String) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        col,
        rusqlite::types::Type::Text,
        format!("valor desconhecido: {s}").into(),
    )
}

/// O que um espaço tem (`Banco::conteudo_do_espaco`): grupos e timers sem grupo, não arquivados e
/// arquivados.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ConteudoDoEspaco {
    pub ativos: i64,
    pub arquivados: i64,
}

pub struct Banco {
    conn: Connection,
    /// O acumulado dos dias fechados (`acumulado_antes`), guardado: o snapshot roda a cada ciclo
    /// e não varre o histórico de novo enquanto nada do passado muda (`mexeu_no_passado`).
    cache: RefCell<Option<CacheAcumulado>>,
}

/// O que identifica um acumulado guardado: o início de hoje, o corte, o fuso e, de cada timer, o
/// grupo e o zerar (o do timer e o de cada atividade).
#[derive(Clone, Debug, PartialEq)]
struct ChaveAcumulado {
    inicio_hoje: Ms,
    corte_min: u32,
    fuso_min: i32,
    timers: Vec<(i64, Option<i64>, Ms)>,
    /// O zerar de cada atividade desses timers (rodada 19).
    atividades: Vec<((i64, i64), Ms)>,
    /// O espaço de cada timer (rodada 20).
    espacos: Vec<(i64, i64)>,
}

struct CacheAcumulado {
    chave: ChaveAcumulado,
    valor: Acumulado,
}

impl Banco {
    /// Abre (e migra) o banco. A migração para a versão 6 usa o relógio do sistema.
    pub fn abrir(caminho: &Path) -> Result<Banco> {
        let (agora, fuso) = relogio_do_sistema();
        Self::abrir_em(caminho, agora, fuso)
    }

    /// `abrir` com o relógio de fora (`agora` e o fuso): o início do dia que a migração para a
    /// versão 6 grava em cada timer.
    pub fn abrir_em(caminho: &Path, agora: Ms, fuso_min: i32) -> Result<Banco> {
        Self::iniciar(Connection::open(caminho)?, agora, fuso_min)
    }

    /// Cópia consistente do banco inteiro (com o que ainda está no WAL) num arquivo novo:
    /// `VACUUM INTO`. O destino não pode existir.
    pub fn copiar_para(&self, destino: &Path) -> Result<()> {
        self.conn.execute("VACUUM INTO ?1", params![destino.to_string_lossy()])?;
        Ok(())
    }

    /// Algum timer (arquivado ou não)?
    pub fn tem_timers(&self) -> Result<bool> {
        self.conn.query_row("SELECT EXISTS (SELECT 1 FROM timers)", [], |r| r.get(0))
    }

    pub fn abrir_memoria() -> Result<Banco> {
        let (agora, fuso) = relogio_do_sistema();
        Self::iniciar(Connection::open_in_memory()?, agora, fuso)
    }

    fn iniciar(conn: Connection, agora: Ms, fuso_min: i32) -> Result<Banco> {
        conn.pragma_update(None, "foreign_keys", true)?;
        // WAL + NORMAL: a gravação de cada ciclo não força o disco toda vez (em memória, o SQLite ignora).
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let versao: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
        // todos os passos numa transação só: um banco antigo abre e migra; se algo falhar, fica
        // como estava
        let passos = match versao {
            0 => format!("{ESQUEMA_V1} {MIGRACAO_V2} {MIGRACAO_V3} {MIGRACAO_V4}"),
            1 => format!("{MIGRACAO_V2} {MIGRACAO_V3} {MIGRACAO_V4}"),
            2 => format!("{MIGRACAO_V3} {MIGRACAO_V4}"),
            3 => MIGRACAO_V4.to_string(),
            _ => String::new(),
        };
        if versao < VERSAO_ESQUEMA {
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(&passos)?;
            migrar_v5(&tx)?;
            let inicio_hoje = inicio_de_hoje(&tx, agora, fuso_min)?;
            migrar_v6(&tx, inicio_hoje)?;
            migrar_v7(&tx)?;
            migrar_v8(&tx)?;
            migrar_v9(&tx)?;
            tx.commit()?;
        }
        marcar_application_id(&conn)?;
        let b = Banco { conn, cache: RefCell::new(None) };
        b.normalizar_regras_pasta()?;
        // sempre (barato): numa versão 4 só sobra sessão com timer e sem origem se o app caiu entre
        // a migração e a marcação, ou se um app da versão 3 gravou sessões neste banco
        b.marcar_ligacoes_antigas()?;
        Ok(b)
    }

    /// Sessões com timer e sem origem (as de antes da versão 4): a origem é posta sem adivinhar
    /// à mão. O timer que a regra de pasta de agora daria é "regra". Outro timer só é "mao" com
    /// prova no banco de que a pessoa a moveu (`movida_a_mao`). Sem prova, é "deduzida" (pode ter sido uma regra trocada depois), que não passa o timer
    /// adiante. Sem timer, fica sem.
    fn marcar_ligacoes_antigas(&self) -> Result<()> {
        let sessoes: Vec<(String, String, i64)> = {
            let mut st = self.conn.prepare(
                "SELECT id, cwd, timer_id FROM sessoes WHERE timer_id IS NOT NULL AND ligacao IS NULL",
            )?;
            let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            v.collect::<Result<_>>()?
        };
        if sessoes.is_empty() {
            return Ok(());
        }
        let regras = self.regras_pasta()?;
        let tx = self.conn.unchecked_transaction()?;
        for (id, cwd, timer) in sessoes {
            let pela_regra = regra_para(&regras, &cwd);
            let l = if pela_regra == Some(timer) {
                Ligacao::Regra
            } else if movida_a_mao(&tx, &id, pela_regra)? {
                Ligacao::Mao
            } else {
                Ligacao::Deduzida
            };
            tx.execute("UPDATE sessoes SET ligacao = ?1 WHERE id = ?2", params![l.as_str(), id])?;
        }
        tx.commit()
    }

    /// Regras de pasta de worktree (`<projeto>\.claude\worktrees\<nome>`) passam para a raiz do
    /// projeto. Se a raiz ja tem regra (ou duas regras viram a mesma pasta, sem diferenciar
    /// maiusculas nem barras), fica a mais antiga. Grupos sem regra de worktree nao sao mexidos.
    fn normalizar_regras_pasta(&self) -> Result<()> {
        // por id: a primeira de cada grupo e a mais antiga
        let mut grupos: std::collections::BTreeMap<String, Vec<RegraPasta>> = Default::default();
        for r in self.regras_pasta()? {
            grupos.entry(chave_de_pasta(raiz_do_projeto(&r.pasta))).or_default().push(r);
        }
        let mut mudar: Vec<(i64, String)> = vec![];
        let mut apagar: Vec<i64> = vec![];
        for g in grupos.values() {
            if g.iter().all(|r| raiz_do_projeto(&r.pasta) == r.pasta) {
                continue;
            }
            let fica = &g[0];
            apagar.extend(g[1..].iter().map(|r| r.id));
            let raiz = raiz_do_projeto(&fica.pasta);
            if raiz != fica.pasta {
                mudar.push((fica.id, raiz.to_string()));
            }
        }
        if mudar.is_empty() && apagar.is_empty() {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        for id in apagar {
            tx.execute("DELETE FROM regras_pasta WHERE id = ?1", params![id])?;
        }
        for (id, pasta) in mudar {
            tx.execute("UPDATE regras_pasta SET pasta = ?1 WHERE id = ?2", params![pasta, id])?;
        }
        tx.commit()
    }

    // ---- grupos ----

    /// Cria o grupo no espaço padrão (`espaco_padrao`; `criar_grupo_em` escolhe o espaço).
    pub fn criar_grupo(&self, nome: &str, cor: &str) -> Result<i64> {
        self.criar_grupo_em(nome, cor, self.espaco_padrao()?)
    }

    /// O espaço padrão (revisão da rodada 20): o primeiro não arquivado, na ordem (sem nenhum, o
    /// primeiro que houver; sem espaço nenhum, o 1 das colunas). Vale para o que é criado sem
    /// dizer o espaço: o primeiro uso, os comandos sem `espaco_id`.
    pub fn espaco_padrao(&self) -> Result<i64> {
        Ok(self
            .conn
            .query_row("SELECT id FROM espacos ORDER BY arquivado, ordem, id LIMIT 1", [], |r| r.get(0))
            .optional()?
            .unwrap_or(ESPACO_PRIMEIRO))
    }

    pub fn criar_grupo_em(&self, nome: &str, cor: &str, espaco_id: i64) -> Result<i64> {
        self.conn.execute("INSERT INTO grupos (nome, cor, espaco_id) VALUES (?1, ?2, ?3)", params![nome, cor, espaco_id])?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn editar_grupo(&self, g: &Grupo) -> Result<()> {
        self.conn.execute(
            "UPDATE grupos SET nome=?1, cor=?2, link_projeto=?3, card_padrao=?4, ordem=?5, arquivado=?6,
             link_projeto_nome=?7, card_padrao_nome=?8, espaco_id=?9 WHERE id=?10",
            params![
                g.nome, g.cor, g.link_projeto, g.card_padrao, g.ordem, g.arquivado,
                g.link_projeto_nome, g.card_padrao_nome, g.espaco_id, g.id
            ],
        )?;
        Ok(())
    }

    /// O espaço de cada grupo (os arquivados também).
    pub fn espacos_dos_grupos(&self) -> Result<BTreeMap<i64, i64>> {
        let mut st = self.conn.prepare("SELECT id, espaco_id FROM grupos")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        v.collect()
    }

    /// "Mover para outro espaço…" do grupo (rodada 21): só muda o `grupos.espaco_id`. Os timers
    /// do grupo vão junto (contam no espaço do grupo), com todo o histórico; nenhum trecho muda.
    pub fn mover_grupo(&self, id: i64, espaco_id: i64) -> Result<bool> {
        Ok(self.conn.execute("UPDATE grupos SET espaco_id = ?1 WHERE id = ?2", params![espaco_id, id])? > 0)
    }

    /// "Mover para…" do timer (rodada 21): só muda o `grupo_id` e o `espaco_id` do timer (com
    /// grupo, o espaço do grupo). Nenhum trecho muda: o histórico vai junto.
    pub fn mover_timer(&self, id: i64, grupo_id: Option<i64>, espaco_id: i64) -> Result<bool> {
        Ok(self.conn.execute(
            "UPDATE timers SET grupo_id = ?1, espaco_id = ?2 WHERE id = ?3",
            params![grupo_id, espaco_id, id],
        )? > 0)
    }

    /// O nome de cada grupo (os arquivados também).
    pub fn nomes_dos_grupos(&self) -> Result<BTreeMap<i64, String>> {
        let mut st = self.conn.prepare("SELECT id, nome FROM grupos")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        v.collect()
    }

    /// O espaço de cada timer (os arquivados também): o do grupo, ou o do timer sem grupo.
    pub fn espacos_dos_timers(&self) -> Result<BTreeMap<i64, i64>> {
        let mut st = self.conn.prepare(
            "SELECT t.id, coalesce(g.espaco_id, t.espaco_id) FROM timers t LEFT JOIN grupos g ON g.id = t.grupo_id",
        )?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        v.collect()
    }

    // ---- espaços (rodada 20) ----

    /// Todos os espaços (os arquivados também), por `ordem`, `id`.
    pub fn espacos(&self) -> Result<Vec<Espaco>> {
        let mut st = self.conn.prepare("SELECT id, nome, cor, ordem, arquivado FROM espacos ORDER BY ordem, id")?;
        let v = st.query_map([], |r| {
            Ok(Espaco { id: r.get(0)?, nome: r.get(1)?, cor: r.get(2)?, ordem: r.get(3)?, arquivado: r.get(4)? })
        })?;
        v.collect()
    }

    /// Acrescenta um espaço no fim da lista; devolve o id.
    pub fn criar_espaco(&self, nome: &str, cor: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO espacos (nome, cor, ordem) VALUES (?1, ?2, (SELECT coalesce(max(ordem), -1) + 1 FROM espacos))",
            params![nome, cor],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn editar_espaco(&self, e: &Espaco) -> Result<()> {
        self.conn.execute(
            "UPDATE espacos SET nome=?1, cor=?2, ordem=?3, arquivado=?4 WHERE id=?5",
            params![e.nome, e.cor, e.ordem, e.arquivado, e.id],
        )?;
        Ok(())
    }

    /// O que o espaço tem: os grupos e os timers sem grupo, os não arquivados (`ativos`) e os
    /// arquivados (`arquivados`). Um timer num grupo vai com o grupo.
    pub fn conteudo_do_espaco(&self, id: i64) -> Result<ConteudoDoEspaco> {
        self.conn.query_row(
            "SELECT (SELECT count(*) FROM grupos WHERE espaco_id = ?1 AND arquivado = 0)
                  + (SELECT count(*) FROM timers WHERE grupo_id IS NULL AND espaco_id = ?1 AND arquivado = 0),
                    (SELECT count(*) FROM grupos WHERE espaco_id = ?1 AND arquivado = 1)
                  + (SELECT count(*) FROM timers WHERE grupo_id IS NULL AND espaco_id = ?1 AND arquivado = 1)",
            params![id],
            |r| Ok(ConteudoDoEspaco { ativos: r.get(0)?, arquivados: r.get(1)? }),
        )
    }

    /// Apaga o espaço (revisão da rodada 20), só sem grupo nem timer sem grupo não arquivado. O
    /// que é arquivado nele (grupos, timers sem grupo) vai, ainda arquivado, para o espaço
    /// `destino`; tudo numa transação só. Devolve quantos itens arquivados foram, ou nulo se o
    /// espaço tem algo à vista (nada muda).
    pub fn apagar_espaco(&self, id: i64, destino: i64) -> Result<Option<i64>> {
        let c = self.conteudo_do_espaco(id)?;
        if c.ativos > 0 {
            return Ok(None);
        }
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("UPDATE grupos SET espaco_id = ?1 WHERE espaco_id = ?2", params![destino, id])?;
        tx.execute("UPDATE timers SET espaco_id = ?1 WHERE espaco_id = ?2", params![destino, id])?;
        tx.execute("DELETE FROM espacos WHERE id = ?1", params![id])?;
        tx.commit()?;
        Ok(Some(c.arquivados))
    }

    // ---- atividades ocultas no painel do timer (rodada 20) ----

    /// Timer → as atividades que o painel dele não mostra.
    pub fn atividades_ocultas(&self) -> Result<BTreeMap<i64, Vec<i64>>> {
        let mut st = self.conn.prepare("SELECT timer_id, atividade_id FROM timer_atividades_ocultas ORDER BY timer_id, atividade_id")?;
        let v = st.query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)))?;
        let mut m: BTreeMap<i64, Vec<i64>> = BTreeMap::new();
        for x in v {
            let (t, a) = x?;
            m.entry(t).or_default().push(a);
        }
        Ok(m)
    }

    /// Troca as atividades ocultas do timer pelas dadas.
    pub fn guardar_atividades_ocultas(&self, timer_id: i64, ocultas: &[i64]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute("DELETE FROM timer_atividades_ocultas WHERE timer_id = ?1", params![timer_id])?;
        for a in ocultas {
            tx.execute(
                "INSERT OR IGNORE INTO timer_atividades_ocultas (timer_id, atividade_id) VALUES (?1, ?2)",
                params![timer_id, a],
            )?;
        }
        tx.commit()
    }

    /// Grupos nao arquivados, por `ordem`, `id`.
    pub fn grupos(&self) -> Result<Vec<Grupo>> {
        let mut st = self.conn.prepare(
            "SELECT id, nome, cor, link_projeto, card_padrao, ordem, arquivado, link_projeto_nome, card_padrao_nome, espaco_id
             FROM grupos WHERE arquivado=0 ORDER BY ordem, id",
        )?;
        let v = st.query_map([], |r| {
            Ok(Grupo {
                id: r.get(0)?,
                nome: r.get(1)?,
                cor: r.get(2)?,
                link_projeto: r.get(3)?,
                card_padrao: r.get(4)?,
                ordem: r.get(5)?,
                arquivado: r.get(6)?,
                link_projeto_nome: r.get(7)?,
                card_padrao_nome: r.get(8)?,
                espaco_id: r.get(9)?,
            })
        })?;
        v.collect()
    }

    // ---- timers ----

    /// `criar_timer_em` com o relógio do sistema.
    pub fn criar_timer(&self, grupo_id: Option<i64>, nome: &str, cor: &str) -> Result<i64> {
        let (agora, fuso) = relogio_do_sistema();
        self.criar_timer_em(grupo_id, nome, cor, agora, fuso)
    }

    /// Cria o timer zerado no início do dia de trabalho de `agora` (pelo corte da config), como a
    /// migração faz (revisão da rodada 11, decisão do controlador). Um timer novo não tem trecho
    /// próprio, então o número dele é o mesmo; e a sessão ligada a ele "desde o início" (o "+ Novo
    /// timer…" da pergunta, "Ligar uma sessão", a herança) conta no número também o que veio antes
    /// da criação.
    pub fn criar_timer_em(&self, grupo_id: Option<i64>, nome: &str, cor: &str, agora: Ms, fuso_min: i32) -> Result<i64> {
        self.criar_timer_no(grupo_id, self.espaco_padrao()?, nome, cor, agora, fuso_min)
    }

    /// `criar_timer_em` com o espaço do timer sem grupo (rodada 20). Com grupo, o timer guarda o
    /// espaço do grupo (o que vale é sempre o do grupo).
    pub fn criar_timer_no(&self, grupo_id: Option<i64>, espaco_id: i64, nome: &str, cor: &str, agora: Ms, fuso_min: i32) -> Result<i64> {
        let zerado = inicio_de_hoje(&self.conn, agora, fuso_min)?;
        let espaco = match grupo_id {
            Some(g) => self
                .conn
                .query_row("SELECT espaco_id FROM grupos WHERE id = ?1", params![g], |r| r.get(0))
                .optional()?
                .unwrap_or(espaco_id),
            None => espaco_id,
        };
        self.conn.execute(
            "INSERT INTO timers (grupo_id, nome, cor, zerado_em, espaco_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![grupo_id, nome, cor, zerado, espaco],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// O instante do último "Zerar" de cada timer (os arquivados também).
    pub fn zerados(&self) -> Result<BTreeMap<i64, Ms>> {
        let mut st = self.conn.prepare("SELECT id, zerado_em FROM timers")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        v.collect()
    }

    /// O último "Zerar" de cada atividade de cada timer (rodada 19), os que existem.
    pub fn zerados_atividade(&self) -> Result<BTreeMap<(i64, i64), Ms>> {
        let mut st = self.conn.prepare("SELECT timer_id, atividade_id, zerado_em FROM zerados_atividade")?;
        let v = st.query_map([], |r| Ok(((r.get(0)?, r.get(1)?), r.get(2)?)))?;
        v.collect()
    }

    /// Os dois zerados juntos (`relatorio::Zerados`): o de cada timer e o de cada atividade.
    pub fn todos_os_zerados(&self) -> Result<Zerados> {
        Ok(Zerados { timers: self.zerados()?, atividades: self.zerados_atividade()? })
    }

    /// "Zerar" dos timers `ids` (com `None`, todos, os arquivados também) no instante `em`: só
    /// grava o `zerado_em`, nada é apagado. Devolve o valor de antes de cada um que existe (para o
    /// "Desfazer"). O zerar de cada atividade desses timers sai (`zerar_com_atividades`).
    pub fn zerar(&self, ids: Option<&[i64]>, em: Ms) -> Result<Vec<(i64, Ms)>> {
        Ok(self.zerar_com_atividades(ids, em)?.0)
    }

    /// O `zerar` que também devolve os zerados por atividade que saíram (timer, atividade, valor),
    /// para o "Desfazer" pôr de volta (rodada 19). O timer todo zerado agora deixa cada atividade
    /// dele contando daqui: o zerar de cada uma não vale mais nada e sai.
    #[allow(clippy::type_complexity)]
    pub fn zerar_com_atividades(&self, ids: Option<&[i64]>, em: Ms) -> Result<(Vec<(i64, Ms)>, Vec<(i64, i64, Ms)>)> {
        let tx = self.conn.unchecked_transaction()?;
        let antes: Vec<(i64, Ms)> = {
            let mut st = tx.prepare("SELECT id, zerado_em FROM timers ORDER BY id")?;
            let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
            v.collect::<Result<Vec<_>>>()?
                .into_iter()
                .filter(|(id, _)| ids.is_none_or(|ids| ids.contains(id)))
                .collect()
        };
        let limpas: Vec<(i64, i64, Ms)> = {
            let mut st = tx.prepare(
                "SELECT timer_id, atividade_id, zerado_em FROM zerados_atividade WHERE zerado_em <= ?1 ORDER BY timer_id, atividade_id",
            )?;
            let v = st.query_map(params![em], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
            v.collect::<Result<Vec<_>>>()?.into_iter().filter(|(t, _, _)| antes.iter().any(|(id, _)| id == t)).collect()
        };
        for (id, _) in &antes {
            tx.execute("UPDATE timers SET zerado_em = ?1 WHERE id = ?2", params![em, id])?;
        }
        for (t, a, _) in &limpas {
            tx.execute("DELETE FROM zerados_atividade WHERE timer_id = ?1 AND atividade_id = ?2", params![t, a])?;
        }
        tx.commit()?;
        Ok((antes, limpas))
    }

    /// "Zerar" só a `atividade` do `timer` no instante `em` (rodada 19): as outras atividades do
    /// timer seguem contando. Devolve o zerar de antes dela (`None`: não tinha), para o
    /// "Desfazer". Nada é apagado.
    pub fn zerar_atividade(&self, timer: i64, atividade: i64, em: Ms) -> Result<Option<Ms>> {
        let tx = self.conn.unchecked_transaction()?;
        let antes: Option<Ms> = tx
            .query_row(
                "SELECT zerado_em FROM zerados_atividade WHERE timer_id = ?1 AND atividade_id = ?2",
                params![timer, atividade],
                |r| r.get(0),
            )
            .optional()?;
        tx.execute(
            "INSERT INTO zerados_atividade (timer_id, atividade_id, zerado_em) VALUES (?1, ?2, ?3)
             ON CONFLICT (timer_id, atividade_id) DO UPDATE SET zerado_em = excluded.zerado_em",
            params![timer, atividade, em],
        )?;
        tx.commit()?;
        Ok(antes)
    }

    /// "Desfazer" do zerar: cada timer volta ao `zerado_em` de antes, se ainda estiver no `em` que
    /// o zerar gravou (um zerar mais novo não é desfeito pelo "Desfazer" de um antigo), e os
    /// zerados por atividade que o zerar tirou (`limpas`) voltam para os timers que voltaram (se
    /// a atividade não foi zerada de novo depois). Devolve quantos timers voltaram.
    pub fn desfazer_zerar(&self, antes: &[(i64, Ms)], limpas: &[(i64, i64, Ms)], em: Ms) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let mut n = 0;
        for (id, valor) in antes {
            let voltou = tx.execute(
                "UPDATE timers SET zerado_em = ?1 WHERE id = ?2 AND zerado_em = ?3",
                params![valor, id, em],
            )?;
            n += voltou;
            if voltou == 0 {
                continue;
            }
            for (_, a, z) in limpas.iter().filter(|(t, _, _)| t == id) {
                tx.execute(
                    "INSERT OR IGNORE INTO zerados_atividade (timer_id, atividade_id, zerado_em) VALUES (?1, ?2, ?3)",
                    params![id, a, z],
                )?;
            }
        }
        tx.commit()?;
        Ok(n)
    }

    /// "Desfazer" do zerar de uma atividade: ela volta ao zerar de antes (`None`: sem zerar
    /// próprio), se ainda estiver no `em` que o zerar gravou. Devolve se voltou.
    pub fn desfazer_zerar_atividade(&self, timer: i64, atividade: i64, antes: Option<Ms>, em: Ms) -> Result<bool> {
        let n = match antes {
            Some(v) => self.conn.execute(
                "UPDATE zerados_atividade SET zerado_em = ?1 WHERE timer_id = ?2 AND atividade_id = ?3 AND zerado_em = ?4",
                params![v, timer, atividade, em],
            )?,
            None => self.conn.execute(
                "DELETE FROM zerados_atividade WHERE timer_id = ?1 AND atividade_id = ?2 AND zerado_em = ?3",
                params![timer, atividade, em],
            )?,
        };
        Ok(n > 0)
    }

    /// "Zerar sozinho na virada": os timers zerados antes de `corte` passam a ele (todos, os
    /// arquivados também); um zerar à mão depois do corte fica. Os zerados por atividade até o
    /// corte saem (o timer todo conta desde ele); os depois do corte ficam. Devolve quantos timers
    /// mudaram.
    pub fn zerar_no_corte(&self, corte: Ms) -> Result<usize> {
        let tx = self.conn.unchecked_transaction()?;
        let n = tx.execute("UPDATE timers SET zerado_em = ?1 WHERE zerado_em < ?1", params![corte])?;
        tx.execute("DELETE FROM zerados_atividade WHERE zerado_em <= ?1", params![corte])?;
        tx.commit()?;
        Ok(n)
    }

    // ---- acumulado desde o zerar ----

    /// O acumulado dos dias fechados (antes de `inicio_hoje`) dos `timers` dados (timer → grupo),
    /// cada um desde o seu zerar (`relatorio::acumulado`). A tela soma a parte de hoje, que anda a
    /// cada segundo. Fica guardado até mudar o dia, o corte, o fuso, um zerar, um grupo ou a lista
    /// de timers, ou até algo do passado mudar (`mexeu_no_passado`). Timer zerado hoje não lê nada
    /// do banco.
    pub fn acumulado_antes(
        &self,
        timers: &BTreeMap<i64, Option<i64>>,
        espacos: &BTreeMap<i64, i64>,
        inicio_hoje: Ms,
        corte_min: u32,
        fuso_min: i32,
    ) -> Result<Acumulado> {
        let zerados = self.todos_os_zerados()?;
        let chave = ChaveAcumulado {
            inicio_hoje,
            corte_min,
            fuso_min,
            timers: timers.iter().map(|(id, g)| (*id, *g, zerados.do_timer(*id))).collect(),
            atividades: zerados.atividades.iter().filter(|((t, _), _)| timers.contains_key(t)).map(|(k, v)| (*k, *v)).collect(),
            espacos: espacos.iter().map(|(t, e)| (*t, *e)).collect(),
        };
        if let Some(c) = self.cache.borrow().as_ref().filter(|c| c.chave == chave) {
            return Ok(c.valor.clone());
        }
        let desde = chave.timers.iter().map(|(_, _, z)| *z).min().unwrap_or(inicio_hoje);
        let valor = if desde >= inicio_hoje {
            Acumulado::default()
        } else {
            let trechos = self.trechos_entre(desde, inicio_hoje)?;
            let ajustes = self.ajustes_criados_desde(desde)?;
            acumulado(&trechos, &ajustes, &zerados, timers, espacos, inicio_hoje, inicio_hoje, corte_min, fuso_min)
        };
        *self.cache.borrow_mut() = Some(CacheAcumulado { chave, valor: valor.clone() });
        Ok(valor)
    }

    /// Algo mudou nos trechos ou nos ajustes a partir de `instante` (`None`: não se sabe quando):
    /// antes do início de hoje do acumulado guardado, ele é jogado fora.
    fn mexeu_no_passado(&self, instante: Option<Ms>) {
        let mut c = self.cache.borrow_mut();
        if c.as_ref().is_some_and(|c| instante.is_none_or(|i| i < c.chave.inicio_hoje)) {
            *c = None;
        }
    }

    /// Há um acumulado guardado? (testes)
    #[cfg(test)]
    pub(crate) fn tem_acumulado_guardado(&self) -> bool {
        self.cache.borrow().is_some()
    }

    /// `editar_timer` e as atividades ocultas do timer (`ocultas`) numa transação só (revisão da
    /// rodada 20).
    pub fn editar_timer_com_ocultas(&self, t: &Timer, ocultas: &[i64]) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE timers SET grupo_id=?1, nome=?2, cor=?3, link_card=?4, ordem=?5, arquivado=?6, link_card_nome=?7,
             atividade_padrao_id=?8, espaco_id=?9 WHERE id=?10",
            params![
                t.grupo_id, t.nome, t.cor, t.link_card, t.ordem, t.arquivado, t.link_card_nome, t.atividade_padrao_id,
                t.espaco_id, t.id
            ],
        )?;
        tx.execute("DELETE FROM timer_atividades_ocultas WHERE timer_id = ?1", params![t.id])?;
        for a in ocultas {
            tx.execute(
                "INSERT OR IGNORE INTO timer_atividades_ocultas (timer_id, atividade_id) VALUES (?1, ?2)",
                params![t.id, a],
            )?;
        }
        tx.commit()
    }

    pub fn editar_timer(&self, t: &Timer) -> Result<()> {
        self.conn.execute(
            "UPDATE timers SET grupo_id=?1, nome=?2, cor=?3, link_card=?4, ordem=?5, arquivado=?6, link_card_nome=?7,
             atividade_padrao_id=?8, espaco_id=?9 WHERE id=?10",
            params![
                t.grupo_id, t.nome, t.cor, t.link_card, t.ordem, t.arquivado, t.link_card_nome, t.atividade_padrao_id,
                t.espaco_id, t.id
            ],
        )?;
        Ok(())
    }

    /// Timers nao arquivados, por `ordem`, `id`.
    pub fn timers(&self) -> Result<Vec<Timer>> {
        let mut st = self.conn.prepare(
            "SELECT id, grupo_id, nome, cor, link_card, ordem, arquivado, link_card_nome, atividade_padrao_id, espaco_id
             FROM timers WHERE arquivado=0 ORDER BY ordem, id",
        )?;
        let v = st.query_map([], |r| {
            Ok(Timer {
                id: r.get(0)?,
                grupo_id: r.get(1)?,
                nome: r.get(2)?,
                cor: r.get(3)?,
                link_card: r.get(4)?,
                ordem: r.get(5)?,
                arquivado: r.get(6)?,
                link_card_nome: r.get(7)?,
                atividade_padrao_id: r.get(8)?,
                espaco_id: r.get(9)?,
            })
        })?;
        v.collect()
    }

    /// A atividade padrão de cada timer (arquivados também: uma sessão pode seguir ligada a um).
    pub fn atividades_padrao(&self) -> Result<std::collections::BTreeMap<i64, i64>> {
        let mut st = self.conn.prepare("SELECT id, atividade_padrao_id FROM timers")?;
        let v = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        v.collect()
    }

    // ---- atividades ----

    /// Todas as atividades (as arquivadas também: trechos antigos apontam para elas), por
    /// `ordem`, `id`.
    pub fn atividades(&self) -> Result<Vec<Atividade>> {
        let mut st = self.conn.prepare("SELECT id, nome, cor, ordem, arquivada FROM atividades ORDER BY ordem, id")?;
        let v = st.query_map([], |r| {
            Ok(Atividade { id: r.get(0)?, nome: r.get(1)?, cor: r.get(2)?, ordem: r.get(3)?, arquivada: r.get(4)? })
        })?;
        v.collect()
    }

    /// Acrescenta uma atividade no fim da lista; devolve o id.
    pub fn criar_atividade(&self, nome: &str, cor: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO atividades (nome, cor, ordem) VALUES (?1, ?2, (SELECT COALESCE(MAX(ordem), -1) + 1 FROM atividades))",
            params![nome, cor],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn editar_atividade(&self, a: &Atividade) -> Result<()> {
        self.conn.execute(
            "UPDATE atividades SET nome=?1, cor=?2, ordem=?3, arquivada=?4 WHERE id=?5",
            params![a.nome, a.cor, a.ordem, a.arquivada, a.id],
        )?;
        Ok(())
    }

    /// Os timers (não arquivados) com `de` como padrão passam a usar `para`; devolve quantos.
    pub fn trocar_atividade_padrao(&self, de: i64, para: i64) -> Result<usize> {
        self.conn.execute(
            "UPDATE timers SET atividade_padrao_id = ?2 WHERE atividade_padrao_id = ?1 AND arquivado = 0",
            params![de, para],
        )
    }

    // ---- trechos ----

    pub fn aplicar(&self, m: &[Mudanca]) -> Result<()> {
        for x in m {
            match x {
                Mudanca::Abriu { inicio, .. } => self.mexeu_no_passado(Some(*inicio)),
                Mudanca::Fechou { fim, .. } => self.mexeu_no_passado(Some(*fim)),
                Mudanca::Reatribuir { .. } => self.mexeu_no_passado(None),
                Mudanca::ReatribuirDesde { desde, .. } => self.mexeu_no_passado(Some(*desde)),
                Mudanca::Sessao(_) | Mudanca::Encerrados { .. } | Mudanca::Pergunta(..) | Mudanca::PerguntaResolvida(_) => {}
            }
        }
        let tx = self.conn.unchecked_transaction()?;
        for x in m {
            match x {
                Mudanca::Abriu { timer_id, sessao_id, origem, inicio, atividade_id } => {
                    tx.execute(
                        "INSERT INTO trechos (timer_id, sessao_id, origem, inicio, atividade_id) VALUES (?1, ?2, ?3, ?4, ?5)",
                        params![timer_id, sessao_id, origem.as_str(), inicio, atividade_id],
                    )?;
                }
                Mudanca::Fechou { timer_id, sessao_id, origem, fim, motivo } => {
                    tx.execute(
                        "UPDATE trechos SET fim = MAX(?1, inicio), motivo_fim = ?2
                         WHERE fim IS NULL AND timer_id IS ?3 AND sessao_id IS ?4 AND origem = ?5",
                        params![fim, motivo.no_banco(), timer_id, sessao_id, origem.as_str()],
                    )?;
                }
                // os trechos de uma sessão (Claude, leitura) ficam com a atividade padrão do timer
                // novo; sem timer, com a que tinham
                Mudanca::Reatribuir { sessao_id, timer_id } => {
                    tx.execute(
                        "UPDATE trechos SET timer_id = ?1,
                         atividade_id = COALESCE((SELECT atividade_padrao_id FROM timers WHERE id = ?1), atividade_id)
                         WHERE sessao_id = ?2",
                        params![timer_id, sessao_id],
                    )?;
                    tx.execute("UPDATE sessoes SET timer_id = ?1 WHERE id = ?2", params![timer_id, sessao_id])?;
                }
                Mudanca::ReatribuirDesde { sessao_id, timer_id, desde } => {
                    tx.execute(
                        "UPDATE trechos SET timer_id = ?1,
                         atividade_id = COALESCE((SELECT atividade_padrao_id FROM timers WHERE id = ?1), atividade_id)
                         WHERE sessao_id = ?2 AND inicio >= ?3",
                        params![timer_id, sessao_id, desde],
                    )?;
                    tx.execute("UPDATE sessoes SET timer_id = ?1 WHERE id = ?2", params![timer_id, sessao_id])?;
                }
                Mudanca::Sessao(s) => salvar_sessao_em(&tx, s)?,
                Mudanca::Encerrados { dia, timers } => {
                    let j = serde_json::json!({ "dia": dia, "timers": timers }).to_string();
                    tx.execute(
                        "INSERT INTO config (chave, valor) VALUES (?1, ?2)
                         ON CONFLICT(chave) DO UPDATE SET valor = excluded.valor",
                        params![CHAVE_ENCERRADOS, j],
                    )?;
                }
                Mudanca::Pergunta(..) | Mudanca::PerguntaResolvida(_) => {}
            }
        }
        tx.commit()
    }

    /// Trechos que tocam [de, ate), abertos incluidos.
    pub fn trechos_entre(&self, de: Ms, ate: Ms) -> Result<Vec<Trecho>> {
        let mut st = self.conn.prepare(
            "SELECT id, timer_id, sessao_id, origem, inicio, fim, motivo_fim, descartado, atividade_id FROM trechos
             WHERE inicio < ?2 AND (fim IS NULL OR fim > ?1) ORDER BY inicio, id",
        )?;
        let v = st.query_map(params![de, ate], |r| {
            let origem: String = r.get(3)?;
            let motivo: Option<String> = r.get(6)?;
            Ok(Trecho {
                id: r.get(0)?,
                timer_id: r.get(1)?,
                sessao_id: r.get(2)?,
                origem: Origem::de_str(&origem).ok_or_else(|| texto_invalido(3, origem))?,
                inicio: r.get(4)?,
                fim: r.get(5)?,
                // desconhecido (de uma versão mais nova): o motivo genérico, sem erro
                motivo_fim: motivo.as_deref().map(MotivoFim::do_banco),
                descartado: r.get(7)?,
                atividade_id: r.get(8)?,
            })
        })?;
        v.collect()
    }

    pub fn abertos(&self) -> Result<Vec<TrechoAberto>> {
        let mut st = self.conn.prepare(
            "SELECT timer_id, sessao_id, origem, inicio, atividade_id FROM trechos WHERE fim IS NULL AND descartado = 0
             ORDER BY inicio, id",
        )?;
        let v = st.query_map([], |r| {
            let origem: String = r.get(2)?;
            Ok(TrechoAberto {
                timer_id: r.get(0)?,
                sessao_id: r.get(1)?,
                origem: Origem::de_str(&origem).ok_or_else(|| texto_invalido(2, origem))?,
                inicio: r.get(3)?,
                atividade_id: r.get(4)?,
            })
        })?;
        v.collect()
    }

    /// Descarta os trechos do timer que tocam [de, ate) e ainda nao estavam descartados; devolve os ids.
    pub fn descartar(&self, timer_id: i64, de: Ms, ate: Ms) -> Result<Vec<i64>> {
        self.mexeu_no_passado(Some(de));
        let tx = self.conn.unchecked_transaction()?;
        let ids: Vec<i64> = {
            let mut st = tx.prepare(
                "SELECT id FROM trechos WHERE timer_id = ?1 AND descartado = 0
                 AND inicio < ?3 AND (fim IS NULL OR fim > ?2) ORDER BY id",
            )?;
            let v = st.query_map(params![timer_id, de, ate], |r| r.get(0))?;
            v.collect::<Result<_>>()?
        };
        for id in &ids {
            tx.execute("UPDATE trechos SET descartado = 1 WHERE id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(ids)
    }

    pub fn restaurar(&self, ids: &[i64]) -> Result<()> {
        self.mexeu_no_passado(None);
        let tx = self.conn.unchecked_transaction()?;
        for id in ids {
            tx.execute("UPDATE trechos SET descartado = 0 WHERE id = ?1", params![id])?;
        }
        tx.commit()
    }

    // ---- ajustes a mao ----

    /// Grava um ajuste do timer no dia de trabalho `dia` (AAAA-MM-DD), na atividade; devolve o id.
    pub fn criar_ajuste(
        &self,
        timer_id: i64,
        atividade_id: i64,
        dia: &str,
        delta_ms: Ms,
        motivo: Option<&str>,
        criado_em: Ms,
    ) -> Result<i64> {
        // os ajustes são raros (um clique): qualquer um joga fora o acumulado guardado
        self.mexeu_no_passado(None);
        self.conn.execute(
            "INSERT INTO ajustes (timer_id, dia, delta_ms, motivo, criado_em, atividade_id) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![timer_id, dia, delta_ms, motivo, criado_em, atividade_id],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    /// Ajustes do dia de trabalho que valem (sem os descartados pelo "Zerar hoje"), na ordem em
    /// que foram feitos.
    pub fn ajustes_do_dia(&self, dia: &str) -> Result<Vec<Ajuste>> {
        let mut st = self.conn.prepare(
            "SELECT id, timer_id, dia, delta_ms, motivo, criado_em, atividade_id FROM ajustes
             WHERE dia = ?1 AND descartado = 0 ORDER BY criado_em, id",
        )?;
        let v = st.query_map(params![dia], |r| {
            Ok(Ajuste {
                id: r.get(0)?,
                timer_id: r.get(1)?,
                dia: r.get(2)?,
                delta_ms: r.get(3)?,
                motivo: r.get(4)?,
                criado_em: r.get(5)?,
                atividade_id: r.get(6)?,
            })
        })?;
        v.collect()
    }

    /// Ajustes que valem criados a partir de `t`, de qualquer dia (o acumulado desde o zerar).
    pub fn ajustes_criados_desde(&self, t: Ms) -> Result<Vec<Ajuste>> {
        let mut st = self.conn.prepare(
            "SELECT id, timer_id, dia, delta_ms, motivo, criado_em, atividade_id FROM ajustes
             WHERE criado_em >= ?1 AND descartado = 0 ORDER BY criado_em, id",
        )?;
        let v = st.query_map(params![t], |r| {
            Ok(Ajuste {
                id: r.get(0)?,
                timer_id: r.get(1)?,
                dia: r.get(2)?,
                delta_ms: r.get(3)?,
                motivo: r.get(4)?,
                criado_em: r.get(5)?,
                atividade_id: r.get(6)?,
            })
        })?;
        v.collect()
    }

    /// Desfaz um ajuste; `false` se ele nao existia.
    pub fn remover_ajuste(&self, id: i64) -> Result<bool> {
        self.mexeu_no_passado(None);
        Ok(self.conn.execute("DELETE FROM ajustes WHERE id = ?1", params![id])? > 0)
    }

    /// "Zerar hoje": descarta os ajustes do timer no dia que ainda valiam; devolve os ids.
    pub fn descartar_ajustes(&self, timer_id: i64, dia: &str) -> Result<Vec<i64>> {
        self.mexeu_no_passado(None);
        let tx = self.conn.unchecked_transaction()?;
        let ids: Vec<i64> = {
            let mut st =
                tx.prepare("SELECT id FROM ajustes WHERE timer_id = ?1 AND dia = ?2 AND descartado = 0 ORDER BY id")?;
            let v = st.query_map(params![timer_id, dia], |r| r.get(0))?;
            v.collect::<Result<_>>()?
        };
        for id in &ids {
            tx.execute("UPDATE ajustes SET descartado = 1 WHERE id = ?1", params![id])?;
        }
        tx.commit()?;
        Ok(ids)
    }

    /// "Desfazer" do zerar: os ajustes voltam a valer.
    pub fn restaurar_ajustes(&self, ids: &[i64]) -> Result<()> {
        self.mexeu_no_passado(None);
        let tx = self.conn.unchecked_transaction()?;
        for id in ids {
            tx.execute("UPDATE ajustes SET descartado = 0 WHERE id = ?1", params![id])?;
        }
        tx.commit()
    }

    // ---- sessoes ----

    pub fn salvar_sessao(&self, s: &Sessao) -> Result<()> {
        salvar_sessao_em(&self.conn, s)
    }

    pub fn sessoes_desde(&self, t: Ms) -> Result<Vec<Sessao>> {
        let mut st = self.conn.prepare(
            "SELECT id, cwd, nome, transcript, timer_id, estado, ultimo_evento, terminou_em, criada_em, ligacao, desligada_em
             FROM sessoes WHERE ultimo_evento >= ?1 ORDER BY ultimo_evento, id",
        )?;
        let v = st.query_map(params![t], |r| {
            let estado: String = r.get(5)?;
            // valor desconhecido (de uma versão mais nova): sem origem
            let ligacao: Option<String> = r.get(9)?;
            Ok(Sessao {
                id: r.get(0)?,
                cwd: r.get(1)?,
                nome: r.get(2)?,
                transcript: r.get(3)?,
                timer_id: r.get(4)?,
                estado: EstadoSessao::de_str(&estado).ok_or_else(|| texto_invalido(5, estado))?,
                ultimo_evento: r.get(6)?,
                terminou_em: r.get(7)?,
                criada_em: r.get(8)?,
                ligacao: ligacao.as_deref().and_then(Ligacao::de_str),
                desligada_em: r.get(10)?,
            })
        })?;
        v.collect()
    }

    // ---- regras de pasta ----

    pub fn regras_pasta(&self) -> Result<Vec<RegraPasta>> {
        let mut st = self.conn.prepare("SELECT id, pasta, timer_id FROM regras_pasta ORDER BY id")?;
        let v = st.query_map([], |r| Ok(RegraPasta { id: r.get(0)?, pasta: r.get(1)?, timer_id: r.get(2)? }))?;
        v.collect()
    }

    /// Cria a regra; se a pasta ja tem regra, troca o timer. Pasta de worktree vira a raiz do projeto.
    pub fn criar_regra_pasta(&self, pasta: &str, timer_id: i64) -> Result<i64> {
        let pasta = raiz_do_projeto(pasta);
        // a mesma pasta escrita de outro jeito (maiusculas, barras): troca o timer dela
        let chave = chave_de_pasta(pasta);
        if let Some(r) = self.regras_pasta()?.into_iter().find(|r| chave_de_pasta(&r.pasta) == chave) {
            self.conn.execute("UPDATE regras_pasta SET timer_id = ?1 WHERE id = ?2", params![timer_id, r.id])?;
            return Ok(r.id);
        }
        self.conn.execute(
            "INSERT INTO regras_pasta (pasta, timer_id) VALUES (?1, ?2)
             ON CONFLICT(pasta) DO UPDATE SET timer_id = excluded.timer_id",
            params![pasta, timer_id],
        )?;
        self.conn.query_row("SELECT id FROM regras_pasta WHERE pasta = ?1", params![pasta], |r| r.get(0))
    }

    /// Regras com o nome do timer (inclusive de timer arquivado), por `id`.
    pub fn regras_pasta_com_timer(&self) -> Result<Vec<(RegraPasta, String)>> {
        let mut st = self.conn.prepare(
            "SELECT r.id, r.pasta, r.timer_id, COALESCE(t.nome, '') FROM regras_pasta r
             LEFT JOIN timers t ON t.id = r.timer_id ORDER BY r.id",
        )?;
        let v = st.query_map([], |r| {
            Ok((RegraPasta { id: r.get(0)?, pasta: r.get(1)?, timer_id: r.get(2)? }, r.get(3)?))
        })?;
        v.collect()
    }

    /// Remove a regra; `false` se ela nao existia.
    pub fn remover_regra_pasta(&self, id: i64) -> Result<bool> {
        Ok(self.conn.execute("DELETE FROM regras_pasta WHERE id = ?1", params![id])? > 0)
    }

    // ---- configuracao ----

    /// Config gravada, com os tempos que valem (`ConfigApp::de_json`): sem o modo desenvolvedor,
    /// os padrões; com ele, de 1 minuto ao máximo de cada um. Config de antes do modo, com tempos
    /// personalizados, liga o modo.
    pub fn config_app(&self) -> Result<ConfigApp> {
        match self.valor("app")? {
            None => Ok(ConfigApp::default()),
            Some(j) => ConfigApp::de_json(&j).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
            }),
        }
    }

    pub fn salvar_config_app(&self, c: &ConfigApp) -> Result<()> {
        let j = serde_json::to_string(c).map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        self.gravar_valor("app", &j)
    }

    /// Timers encerrados (■) gravados e o dia de trabalho deles (AAAA-MM-DD). Valor estragado
    /// conta como nenhum: o pior caso e o lembrete voltar.
    pub fn encerrados(&self) -> Result<Option<(String, Vec<i64>)>> {
        #[derive(serde::Deserialize)]
        struct Gravado {
            dia: String,
            timers: Vec<i64>,
        }
        Ok(self
            .valor(CHAVE_ENCERRADOS)?
            .and_then(|j| serde_json::from_str::<Gravado>(&j).ok())
            .map(|g| (g.dia, g.timers)))
    }

    pub fn valor(&self, chave: &str) -> Result<Option<String>> {
        self.conn
            .query_row("SELECT valor FROM config WHERE chave = ?1", params![chave], |r| r.get(0))
            .optional()
    }

    pub fn gravar_valor(&self, chave: &str, v: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO config (chave, valor) VALUES (?1, ?2)
             ON CONFLICT(chave) DO UPDATE SET valor = excluded.valor",
            params![chave, v],
        )?;
        Ok(())
    }
}

/// Prova de que a sessão foi movida à mão de um timer para outro (uma das duas):
/// - um trecho dela fechado ao desligar (`desligada`, o "daqui para frente" com algo aberto) e o
///   trecho seguinte dela (por início) noutro timer;
/// - trechos dela em dois ou mais timers, com o trecho mais recente num timer diferente do que a
///   regra de pasta de agora daria (`pela_regra`). Uma regra trocada depois não gera isso: a regra
///   só liga sessão nova, então a sessão fica inteira no timer da regra da época. É o caso da
///   mudança sem nada aberto (nenhum trecho fecha ao mover).
fn movida_a_mao(conn: &Connection, sessao_id: &str, pela_regra: Option<i64>) -> Result<bool> {
    let mut st = conn.prepare(
        "SELECT timer_id, motivo_fim FROM trechos WHERE sessao_id = ?1 ORDER BY inicio, id",
    )?;
    let v: Vec<(Option<i64>, Option<String>)> =
        st.query_map(params![sessao_id], |r| Ok((r.get(0)?, r.get(1)?)))?.collect::<Result<_>>()?;
    let ao_desligar =
        v.windows(2).any(|w| w[0].1.as_deref() == Some(MotivoFim::Desligada.as_str()) && w[0].0 != w[1].0);
    let timers: std::collections::BTreeSet<i64> = v.iter().filter_map(|(t, _)| *t).collect();
    let ultimo = v.last().and_then(|(t, _)| *t);
    let em_dois = timers.len() >= 2 && ultimo.is_some() && ultimo != pela_regra;
    Ok(ao_desligar || em_dois)
}

fn salvar_sessao_em(conn: &Connection, s: &Sessao) -> Result<()> {
    conn.execute(
        "INSERT INTO sessoes (id, cwd, nome, transcript, timer_id, estado, ultimo_evento, terminou_em, criada_em, ligacao, desligada_em)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
         ON CONFLICT(id) DO UPDATE SET cwd=excluded.cwd, nome=excluded.nome, transcript=excluded.transcript,
            timer_id=excluded.timer_id, estado=excluded.estado, ultimo_evento=excluded.ultimo_evento,
            terminou_em=excluded.terminou_em, criada_em=excluded.criada_em, ligacao=excluded.ligacao,
            desligada_em=excluded.desligada_em",
        params![
            s.id, s.cwd, s.nome, s.transcript, s.timer_id, s.estado.as_str(),
            s.ultimo_evento, s.terminou_em, s.criada_em, s.ligacao.map(|l| l.as_str()), s.desligada_em
        ],
    )?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::config::ConfigApp;
    use crate::relatorio::totais;
    use crate::tempo::*;
    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms { ms_local(2026, 11, 17, hh, mm, F) }

    fn sessao(id: &str, timer: Option<i64>) -> Sessao {
        Sessao { id: id.into(), cwd: "C:\\x".into(), nome: None, transcript: None, timer_id: timer,
            estado: EstadoSessao::Trabalhando, ultimo_evento: h(9, 0), terminou_em: None, criada_em: h(9, 0), ligacao: None, desligada_em: None }
    }

    fn application_id(conn: &Connection) -> i32 {
        conn.query_row("PRAGMA application_id", [], |r| r.get(0)).unwrap()
    }

    #[test] fn o_banco_leva_a_marca_chrn_na_criacao_e_na_abertura() {
        assert_eq!(crate::APPLICATION_ID, i32::from_be_bytes(*b"CHRN"));
        // criado agora
        assert_eq!(application_id(&Banco::abrir_memoria().unwrap().conn), 0x4348_524E);
        let d = tempfile::tempdir().unwrap();
        // um banco de antes da marca (0): a abertura marca, sem mexer no esquema
        let antigo = d.path().join("antigo.db");
        Banco::abrir_em(&antigo, h(9, 0), F).unwrap();
        {
            let c = Connection::open(&antigo).unwrap();
            c.pragma_update(None, "application_id", 0).unwrap();
        }
        let b = Banco::abrir_em(&antigo, h(9, 0), F).unwrap();
        assert_eq!(application_id(&b.conn), 0x4348_524E);
        let versao: i64 = b.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(versao, VERSAO_ESQUEMA);
        drop(b);
        // um banco com outra marca fica com a dele
        let outro = d.path().join("outro.db");
        Banco::abrir_em(&outro, h(9, 0), F).unwrap();
        {
            let c = Connection::open(&outro).unwrap();
            c.pragma_update(None, "application_id", 7).unwrap();
        }
        assert_eq!(application_id(&Banco::abrir_em(&outro, h(9, 0), F).unwrap().conn), 7);
    }

    #[test] fn cria_grupo_e_timers_e_lista_em_ordem() {
        let b = Banco::abrir_memoria().unwrap();
        let g = b.criar_grupo("Empresa", "#fff").unwrap();
        let t1 = b.criar_timer(Some(g), "A", "#111").unwrap();
        let t2 = b.criar_timer(None, "B", "#222").unwrap();
        assert!(t2 > t1);
        let gs = b.grupos().unwrap();
        assert_eq!(gs.len(), 1);
        assert_eq!(gs[0].nome, "Empresa");
        let ts = b.timers().unwrap();
        assert_eq!(ts.iter().map(|t| t.nome.as_str()).collect::<Vec<_>>(), ["A", "B"]);
        assert_eq!(ts[0].grupo_id, Some(g));
        assert_eq!(ts[1].grupo_id, None);
        let mut g2 = gs[0].clone();
        g2.arquivado = true;
        b.editar_grupo(&g2).unwrap();
        assert!(b.grupos().unwrap().is_empty());
        let mut t = ts[0].clone();
        t.nome = "A2".into();
        b.editar_timer(&t).unwrap();
        assert_eq!(b.timers().unwrap()[0].nome, "A2");
    }

    #[test] fn aplicar_abre_e_fecha_trecho() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#111").unwrap();
        b.aplicar(&[Mudanca::Abriu { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, inicio: h(9, 0), atividade_id: 1 }]).unwrap();
        assert_eq!(b.abertos().unwrap().len(), 1);
        b.aplicar(&[Mudanca::Fechou { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, fim: h(9, 30), motivo: MotivoFim::Pausa }]).unwrap();
        assert!(b.abertos().unwrap().is_empty());
        let v = b.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].fim, Some(h(9, 30)));
        assert_eq!(v[0].motivo_fim, Some(MotivoFim::Pausa));
    }

    #[test] fn fechar_trecho_sem_timer_casa_com_null() {
        let b = Banco::abrir_memoria().unwrap();
        b.aplicar(&[Mudanca::Abriu { timer_id: None, sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(9, 0), atividade_id: 1 }]).unwrap();
        b.aplicar(&[Mudanca::Fechou { timer_id: None, sessao_id: Some("s1".into()), origem: Origem::Claude, fim: h(9, 10), motivo: MotivoFim::Terminou }]).unwrap();
        assert!(b.abertos().unwrap().is_empty());
        assert_eq!(b.trechos_entre(h(0, 0), h(23, 0)).unwrap()[0].fim, Some(h(9, 10)));
    }

    #[test] fn fechar_com_fim_antes_do_inicio_usa_o_inicio() {
        let b = Banco::abrir_memoria().unwrap();
        b.aplicar(&[Mudanca::Abriu { timer_id: None, sessao_id: None, origem: Origem::Manual, inicio: h(9, 0), atividade_id: 1 }]).unwrap();
        b.aplicar(&[Mudanca::Fechou { timer_id: None, sessao_id: None, origem: Origem::Manual, fim: h(8, 0), motivo: MotivoFim::Pausa }]).unwrap();
        assert_eq!(b.trechos_entre(h(0, 0), h(23, 0)).unwrap()[0].fim, Some(h(9, 0)));
    }

    #[test] fn reatribuir_move_trechos_e_sessao() {
        let b = Banco::abrir_memoria().unwrap();
        for i in 1..=3 { b.criar_timer(None, &format!("T{i}"), "#000").unwrap(); }
        b.aplicar(&[
            Mudanca::Sessao(sessao("s1", None)),
            Mudanca::Abriu { timer_id: None, sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(9, 0), atividade_id: 1 },
            Mudanca::Fechou { timer_id: None, sessao_id: Some("s1".into()), origem: Origem::Claude, fim: h(9, 10), motivo: MotivoFim::Terminou },
            Mudanca::Abriu { timer_id: None, sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(10, 0), atividade_id: 1 },
        ]).unwrap();
        b.aplicar(&[Mudanca::Reatribuir { sessao_id: "s1".into(), timer_id: Some(3) }]).unwrap();
        let v = b.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert_eq!(v.len(), 2);
        assert!(v.iter().all(|t| t.timer_id == Some(3)));
        let s = b.sessoes_desde(0).unwrap();
        assert_eq!(s[0].timer_id, Some(3));
    }

    #[test] fn descartar_e_restaurar() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#111").unwrap();
        for (i, f) in [(h(9, 0), h(10, 0)), (h(11, 0), h(12, 0))] {
            b.aplicar(&[
                Mudanca::Abriu { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, inicio: i, atividade_id: 1 },
                Mudanca::Fechou { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, fim: f, motivo: MotivoFim::Pausa },
            ]).unwrap();
        }
        let tot = |b: &Banco| totais(&b.trechos_entre(h(0, 0), h(23, 0)).unwrap(), h(0, 0), h(23, 0), h(23, 0)).dia_ms;
        assert_eq!(tot(&b), 120 * MIN);
        let ids = b.descartar(t, h(0, 0), h(23, 0)).unwrap();
        assert_eq!(ids.len(), 2);
        assert_eq!(tot(&b), 0);
        b.restaurar(&ids).unwrap();
        assert_eq!(tot(&b), 120 * MIN);
    }

    #[test] fn sessoes_e_regras_de_pasta() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#111").unwrap();
        let mut s = sessao("s1", Some(t));
        b.salvar_sessao(&s).unwrap();
        s.estado = EstadoSessao::Parada;
        s.nome = Some("n".into());
        b.salvar_sessao(&s).unwrap();
        let v = b.sessoes_desde(h(8, 0)).unwrap();
        assert_eq!(v, vec![s]);
        assert!(b.sessoes_desde(h(12, 0)).unwrap().is_empty());
        b.criar_regra_pasta("C:\\p", t).unwrap();
        let t2 = b.criar_timer(None, "B", "#222").unwrap();
        b.criar_regra_pasta("C:\\p", t2).unwrap();
        let r = b.regras_pasta().unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].timer_id, t2);
    }

    #[test] fn regra_de_worktree_fica_na_raiz_do_projeto() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Flow", "#111").unwrap();
        let id = b.criar_regra_pasta("G:\\Empresa\\app-web\\.claude\\worktrees\\feat-029-x", t).unwrap();
        let r = b.regras_pasta().unwrap();
        assert_eq!((r[0].id, r[0].pasta.as_str()), (id, "G:\\Empresa\\app-web"));
        // outro worktree do mesmo projeto: a mesma regra
        assert_eq!(b.criar_regra_pasta("G:\\Empresa\\app-web\\.claude\\worktrees\\outro", t).unwrap(), id);
        assert_eq!(b.regras_pasta().unwrap().len(), 1);
    }

    #[test] fn abrir_o_banco_migra_regras_de_worktree() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("p.db");
        let (flow, outro, velho) = {
            let b = Banco::abrir(&p).unwrap();
            let flow = b.criar_timer(None, "Flow", "#111").unwrap();
            let outro = b.criar_timer(None, "Outro", "#222").unwrap();
            let velho = b.criar_timer(None, "Velho", "#333").unwrap();
            // gravadas como as versoes antigas gravavam (sem normalizar)
            for (pasta, timer) in [
                ("G:\\Empresa\\app-web\\.claude\\worktrees\\feat-029-pagina-do-projeto-57b711", flow),
                ("G:\\Empresa\\app-web\\.claude\\worktrees\\feat-030", outro),
                ("D:\\Velho", velho),
                ("D:\\Velho\\.claude\\worktrees\\a", outro),
                ("C:\\Sem\\worktree", outro),
            ] {
                b.conn.execute("INSERT INTO regras_pasta (pasta, timer_id) VALUES (?1, ?2)", params![pasta, timer]).unwrap();
            }
            (flow, outro, velho)
        };
        let b = Banco::abrir(&p).unwrap();
        let v: Vec<(String, i64)> = b.regras_pasta().unwrap().into_iter().map(|r| (r.pasta, r.timer_id)).collect();
        assert_eq!(
            v,
            [
                // as duas viraram a mesma pasta: fica a mais antiga
                ("G:\\Empresa\\app-web".to_string(), flow),
                // a raiz ja tinha regra, mais antiga: a do worktree sai
                ("D:\\Velho".to_string(), velho),
                ("C:\\Sem\\worktree".to_string(), outro),
            ]
        );
        // reabrir de novo nao muda nada
        drop(b);
        let b = Banco::abrir(&p).unwrap();
        assert_eq!(b.regras_pasta().unwrap().len(), 3);
    }

    #[test] fn migrar_compara_sem_maiusculas_nem_barras() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("p.db");
        let (a, b_, c) = {
            let b = Banco::abrir(&p).unwrap();
            let a = b.criar_timer(None, "A", "#111").unwrap();
            let b_ = b.criar_timer(None, "B", "#222").unwrap();
            let c = b.criar_timer(None, "C", "#333").unwrap();
            for (pasta, timer) in [
                ("G:\\P\\.claude\\worktrees\\x", a),
                ("g:/p/", b_),
                ("h:\\q\\", c),
                ("H:\\Q\\.claude\\worktrees\\y", a),
                // nenhum worktree no grupo: duplicadas antigas ficam como estao
                ("D:\\R", a),
                ("d:/r", b_),
            ] {
                b.conn.execute("INSERT INTO regras_pasta (pasta, timer_id) VALUES (?1, ?2)", params![pasta, timer]).unwrap();
            }
            (a, b_, c)
        };
        let b = Banco::abrir(&p).unwrap();
        let v: Vec<(String, i64)> = b.regras_pasta().unwrap().into_iter().map(|r| (r.pasta, r.timer_id)).collect();
        assert_eq!(
            v,
            [
                ("G:\\P".to_string(), a),
                ("h:\\q\\".to_string(), c),
                ("D:\\R".to_string(), a),
                ("d:/r".to_string(), b_),
            ]
        );
    }

    #[test] fn criar_regra_com_a_pasta_escrita_de_outro_jeito_troca_o_timer() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#111").unwrap();
        let t2 = b.criar_timer(None, "B", "#222").unwrap();
        let id = b.criar_regra_pasta("G:\\Proj", t).unwrap();
        assert_eq!(b.criar_regra_pasta("g:/proj/.claude/worktrees/x", t2).unwrap(), id);
        let r = b.regras_pasta().unwrap();
        assert_eq!((r.len(), r[0].pasta.as_str(), r[0].timer_id), (1, "G:\\Proj", t2));
    }

    #[test] fn migrar_worktree_mais_antigo_que_a_regra_da_raiz_fica_com_o_mais_antigo() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("p.db");
        let (a, b_) = {
            let b = Banco::abrir(&p).unwrap();
            let a = b.criar_timer(None, "A", "#111").unwrap();
            let b_ = b.criar_timer(None, "B", "#222").unwrap();
            b.conn.execute("INSERT INTO regras_pasta (pasta, timer_id) VALUES ('G:\\P\\.claude\\worktrees\\x', ?1)", params![a]).unwrap();
            b.conn.execute("INSERT INTO regras_pasta (pasta, timer_id) VALUES ('G:\\P', ?1)", params![b_]).unwrap();
            (a, b_)
        };
        let b = Banco::abrir(&p).unwrap();
        let v: Vec<(String, i64)> = b.regras_pasta().unwrap().into_iter().map(|r| (r.pasta, r.timer_id)).collect();
        assert_eq!(v, [("G:\\P".to_string(), a)]);
        assert_ne!(a, b_);
    }

    #[test] fn regras_com_nome_do_timer_e_remover() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Projeto A", "#111").unwrap();
        let t2 = b.criar_timer(None, "Velho", "#222").unwrap();
        let r1 = b.criar_regra_pasta("G:\\Projeto A", t).unwrap();
        b.criar_regra_pasta("D:\\Velho", t2).unwrap();
        // timer arquivado continua com nome na lista
        b.editar_timer(&Timer { id: t2, grupo_id: None, nome: "Velho".into(), cor: "#222".into(), link_card: None, ordem: 0, arquivado: true, link_card_nome: None, atividade_padrao_id: 1, espaco_id: 1 }).unwrap();
        let v = b.regras_pasta_com_timer().unwrap();
        assert_eq!(v.len(), 2);
        assert_eq!((v[0].0.pasta.as_str(), v[0].1.as_str()), ("G:\\Projeto A", "Projeto A"));
        assert_eq!(v[1].1, "Velho");
        assert!(b.remover_regra_pasta(r1).unwrap());
        assert!(!b.remover_regra_pasta(r1).unwrap(), "já removida");
        let r = b.regras_pasta().unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0].pasta, "D:\\Velho");
    }

    #[test] fn config_com_tempo_abaixo_de_1_minuto_carrega_com_60() {
        let b = Banco::abrir_memoria().unwrap();
        b.gravar_valor("app", r#"{"modo_dev": true, "inatividade_s": 0, "uso_s": 59, "teto_leitura_s": 1200, "sessao_parada_s": 0, "tema": "claro"}"#).unwrap();
        let c = b.config_app().unwrap();
        assert_eq!((c.uso_s, c.inatividade_s, c.sessao_parada_s), (60, 60, 60));
        assert_eq!(c.tema, "claro");
        assert_eq!(c.ao_fechar, "perguntar", "config antiga, sem o campo");
    }

    #[test] fn config_antiga_com_tempos_personalizados_abre_no_modo_dev() {
        let b = Banco::abrir_memoria().unwrap();
        // tempos mudados antes do modo dev existir: o modo liga e os valores ficam (a inatividade
        // de 24 h desce para o máximo, 30 min)
        b.gravar_valor("app", r#"{"inatividade_s": 86400, "uso_s": 60, "sessao_parada_s": 120, "tema": "claro"}"#).unwrap();
        let c = b.config_app().unwrap();
        assert_eq!((c.uso_s, c.inatividade_s, c.sessao_parada_s, c.modo_dev), (60, 1800, 120, true));
        assert_eq!(c.tema, "claro");
        // gravada de novo, já com o campo: segue no modo dev
        b.salvar_config_app(&c).unwrap();
        assert_eq!(b.config_app().unwrap(), c);
    }

    #[test] fn config_padrao_quando_vazio() {
        let b = Banco::abrir_memoria().unwrap();
        let c = b.config_app().unwrap();
        assert_eq!(c, ConfigApp::default());
        assert_eq!(c.tema, "sistema", "rodada 21: o padrão segue o sistema");
        assert!(c.painel_linha, "a linha do tempo no Painel, ligada");
        assert_eq!(c.uso_s, 180);
        assert_eq!(c.inatividade_s, 600);
        assert_eq!(c.sessao_parada_s, 600);
        assert_eq!(c.arredondamento_min, 5);
        assert_eq!(c.programas_claude, ["claude", "code", "windowsterminal", "powershell", "pwsh", "cmd", "antigravity"]);
        let mut c2 = c.clone();
        c2.tema = "claro".into();
        c2.dock_x = Some(10);
        b.salvar_config_app(&c2).unwrap();
        assert_eq!(b.config_app().unwrap(), c2);
        assert_eq!(b.valor("x").unwrap(), None);
        b.gravar_valor("x", "1").unwrap();
        b.gravar_valor("x", "2").unwrap();
        assert_eq!(b.valor("x").unwrap(), Some("2".into()));
    }

    #[test] fn banco_em_arquivo_usa_wal() {
        let d = tempfile::tempdir().unwrap();
        let b = Banco::abrir(&d.path().join("p.db")).unwrap();
        let modo: String = b.conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).unwrap();
        assert_eq!(modo, "wal");
        let sinc: i64 = b.conn.query_row("PRAGMA synchronous", [], |r| r.get(0)).unwrap();
        assert_eq!(sinc, 1);
    }
    #[test] fn versao_7_grava_o_desligar_da_sessao() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("w.db");
        {
            // um banco da versão 6, sem a coluna
            let b = Banco::abrir_em(&p, h(9, 0), F).unwrap();
            b.conn.execute_batch("ALTER TABLE sessoes DROP COLUMN desligada_em; PRAGMA user_version = 6;").unwrap();
            b.conn.execute_batch("INSERT INTO sessoes (id, cwd, estado, ultimo_evento, criada_em) VALUES ('velha', 'C:\\x', 'parada', 100, 100);").unwrap();
        }
        let b = Banco::abrir_em(&p, h(9, 0), F).unwrap();
        let v: i64 = b.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, VERSAO_ESQUEMA);
        assert_eq!(b.sessoes_desde(0).unwrap()[0].desligada_em, None, "a de antes: sem desligar");
        let mut s = sessao("s1", None);
        s.desligada_em = Some(h(9, 30));
        b.salvar_sessao(&s).unwrap();
        assert_eq!(b.sessoes_desde(0).unwrap().iter().find(|x| x.id == "s1").unwrap().desligada_em, Some(h(9, 30)));
        s.desligada_em = Some(h(10, 0));
        b.salvar_sessao(&s).unwrap();
        assert_eq!(b.sessoes_desde(0).unwrap().iter().find(|x| x.id == "s1").unwrap().desligada_em, Some(h(10, 0)));
    }
    #[test] fn banco_novo_tem_versao_9_com_as_tres_atividades_e_o_espaco_geral() {
        let b = Banco::abrir_memoria().unwrap();
        let v: i64 = b.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, VERSAO_ESQUEMA);
        assert_eq!(v, 9);
        let a = b.atividades().unwrap();
        assert_eq!(
            a.iter().map(|x| (x.id, x.nome.as_str(), x.arquivada)).collect::<Vec<_>>(),
            [(ATIVIDADE_TRABALHO, "Trabalho", false), (ATIVIDADE_REUNIAO, "Reunião", false), (ATIVIDADE_ESTUDO, "Estudo", false)]
        );
        let t = b.criar_timer(None, "A", "#111").unwrap();
        assert_eq!(b.timers().unwrap()[0].atividade_padrao_id, ATIVIDADE_TRABALHO);
        assert_eq!(b.atividades_padrao().unwrap()[&t], ATIVIDADE_TRABALHO);
        // rodada 20: o banco novo nasce com o espaço "Geral"
        assert_eq!(
            b.espacos().unwrap(),
            [Espaco { id: ESPACO_PRIMEIRO, nome: "Geral".into(), cor: COR_ESPACO_PRIMEIRO.into(), ordem: 0, arquivado: false }]
        );
        assert_eq!(b.espacos_dos_timers().unwrap()[&t], ESPACO_PRIMEIRO);
    }

    /// Linhas das tabelas de antes da versão 5, só com as colunas de antes (para comparar antes e
    /// depois da migração).
    fn linhas_antigas(c: &Connection) -> Vec<String> {
        let mut v = vec![];
        for (tabela, colunas) in [
            ("grupos", "id, nome, cor, link_projeto, card_padrao, ordem, arquivado, link_projeto_nome, card_padrao_nome"),
            ("timers", "id, grupo_id, nome, cor, link_card, ordem, arquivado, link_card_nome"),
            ("sessoes", "id, cwd, nome, transcript, timer_id, estado, ultimo_evento, terminou_em, criada_em, ligacao"),
            ("trechos", "id, timer_id, sessao_id, origem, inicio, fim, motivo_fim, descartado"),
            ("regras_pasta", "id, pasta, timer_id"),
            ("config", "chave, valor"),
            ("ajustes", "id, timer_id, dia, delta_ms, motivo, criado_em, descartado"),
        ] {
            let mut st = c.prepare(&format!("SELECT {colunas} FROM {tabela} ORDER BY 1")).unwrap();
            let n = st.column_count();
            let rows = st
                .query_map([], |r| {
                    let campos: Vec<String> =
                        (0..n).map(|i| format!("{:?}", r.get::<_, rusqlite::types::Value>(i).unwrap())).collect();
                    Ok(format!("{tabela}: {}", campos.join(" | ")))
                })
                .unwrap();
            v.extend(rows.map(|r| r.unwrap()));
        }
        v
    }

    /// O banco real do usuário: veio da versão anterior do app (versão 1, com o que ela gravava), passou
    /// pelas versões 2, 3 e 4 no Work Timer e tem de tudo: grupos com links e nomes, timer
    /// arquivado, sessões com as quatro origens, trechos de todas as origens (um aberto, um
    /// descartado, cortes do dia, app fechado), ajustes (um descartado pelo "Zerar hoje"), regras
    /// de pasta e as chaves de config (app, encerrados, fila, tique, saída da leitura, filas antigas).
    fn banco_real_v4(p: &Path) {
        let c = Connection::open(p).unwrap();
        c.execute_batch(&format!("BEGIN; {ESQUEMA_V1} COMMIT;")).unwrap();
        c.execute_batch(
            "INSERT INTO grupos (id, nome, cor, link_projeto, card_padrao, ordem) VALUES
               (1, 'Cliente B', '#4f8cff', 'https://sprints.zoho.com/workspace/exemplo#P146', 'https://sprints.zoho.com/workspace/exemplo#P146/itemdetails/I7', 0),
               (2, 'Agendas', '#ff6b4a', NULL, NULL, 1);
             INSERT INTO timers (id, grupo_id, nome, cor, link_card, ordem, arquivado) VALUES
               (1, 1, 'Flow', '#4f8cff', NULL, 0, 0), (2, 1, 'Cliente B', '#a06bff', 'https://sprints.zoho.com/workspace/exemplo#P146/itemdetails/I9', 1, 0),
               (3, 2, 'Cliente A', '#ffb020', NULL, 0, 0), (4, NULL, 'Velho', '#888888', NULL, 9, 1);
             INSERT INTO regras_pasta (pasta, timer_id) VALUES ('G:\\Empresa\\app-web', 1);
             INSERT INTO sessoes (id, cwd, nome, transcript, timer_id, estado, ultimo_evento, terminou_em, criada_em) VALUES
               ('5ee991', 'G:\\Empresa\\app-web', 'Flow: spec 018', 'C:\\x.jsonl', 2, 'esperando_voce', 2000, 1900, 100),
               ('a1', 'G:\\Empresa\\app-web', NULL, NULL, 1, 'parada', 1500, NULL, 900),
               ('b2', 'D:\\Pessoal', NULL, NULL, NULL, 'parada', 1200, NULL, 1100);
             INSERT INTO trechos (timer_id, sessao_id, origem, inicio, fim, motivo_fim, descartado) VALUES
               (1, '5ee991', 'claude', 100, 400, 'desligada', 0),
               (2, '5ee991', 'claude', 400, 900, 'terminou', 0),
               (2, '5ee991', 'leitura', 900, 1000, 'saiu_da_janela', 0),
               (1, 'a1', 'claude', 950, 1200, 'corte_do_dia', 0),
               (1, 'a1', 'claude', 1200, 1300, 'app_fechado', 0),
               (3, NULL, 'manual', 500, 800, 'inatividade', 0),
               (3, NULL, 'manual', 1000, 1100, 'pausa', 1),
               (NULL, 'b2', 'claude', 1100, 1200, 'sessao_parou', 0),
               (3, NULL, 'manual', 1900, NULL, NULL, 0);
             INSERT INTO config (chave, valor) VALUES ('fila_offset', '48211'), ('ultimo_tique', '2000');",
        ).unwrap();
        c.execute_batch(&format!("BEGIN; {MIGRACAO_V2} {MIGRACAO_V3} {MIGRACAO_V4} COMMIT;")).unwrap();
        c.execute_batch(
            "UPDATE grupos SET link_projeto_nome = 'Cliente B', card_padrao_nome = 'Backlog' WHERE id = 1;
             UPDATE timers SET link_card_nome = 'Painel' WHERE id = 2;
             UPDATE sessoes SET ligacao = 'mao' WHERE id = '5ee991';
             UPDATE sessoes SET ligacao = 'regra' WHERE id = 'a1';
             INSERT INTO sessoes (id, cwd, nome, timer_id, estado, ultimo_evento, criada_em, ligacao) VALUES
               ('c3', 'G:\\Empresa\\app-web', 'Flow: spec 018', 2, 'trabalhando', 2100, 2050, 'herdada'),
               ('d4', 'G:\\Outro', NULL, 3, 'parada', 2100, 2060, 'deduzida');
             INSERT INTO ajustes (timer_id, dia, delta_ms, motivo, criado_em, descartado) VALUES
               (3, '2026-11-18', 900000, 'reunião longe do PC', 1500, 0), (2, '2026-11-18', -300000, NULL, 1600, 1);
             INSERT INTO config (chave, valor) VALUES
               ('app', '{\"tema\":\"claro\",\"corte_min\":0}'),
               ('encerrados', '{\"dia\":\"2026-11-18\",\"timers\":[1]}'),
               ('saida_leitura', ''),
               ('filas_antigas', '[[\"C:\\\\Users\\\\x\\\\AppData\\\\Local\\\\app-anterior\\\\fila\\\\eventos.jsonl\",1234]]');",
        ).unwrap();
        let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 4);
    }

    #[test] fn banco_real_da_versao_4_vira_9_sem_perder_nada_e_tudo_em_trabalho() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("worktimer.db");
        banco_real_v4(&p);
        let antes = linhas_antigas(&Connection::open(&p).unwrap());
        assert!(antes.len() > 25);
        let b = Banco::abrir_em(&p, ms_local(2026, 11, 18, 14, 0, F), F).unwrap();
        let v: i64 = b.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 9);
        // todos os timers (o arquivado também) zerados no início de hoje: os números são os de hoje
        assert_eq!(b.zerados().unwrap().into_iter().collect::<Vec<_>>(), (1..=4).map(|t| (t, ms_local(2026, 11, 18, 0, 0, F))).collect::<Vec<_>>());
        assert_eq!(linhas_antigas(&b.conn), antes, "nenhuma linha antiga muda");
        assert_eq!(b.atividades().unwrap().len(), 3);
        let n = |sql: &str| b.conn.query_row(sql, [], |r| r.get::<_, i64>(0)).unwrap();
        assert_eq!(n("SELECT count(*) FROM trechos WHERE atividade_id <> 1"), 0, "os trechos antigos viram Trabalho");
        assert_eq!(n("SELECT count(*) FROM timers WHERE atividade_padrao_id <> 1"), 0, "todo timer começa em Trabalho");
        assert_eq!(n("SELECT count(*) FROM ajustes WHERE atividade_id <> 1"), 0, "os ajustes antigos somam em Trabalho");
        // o que o app lê depois de migrar
        assert_eq!(b.abertos().unwrap(), vec![TrechoAberto { timer_id: Some(3), sessao_id: None, origem: Origem::Manual, inicio: 1900, atividade_id: 1 }]);
        assert!(b.trechos_entre(0, 3000).unwrap().iter().all(|t| t.atividade_id == ATIVIDADE_TRABALHO));
        assert_eq!(b.ajustes_do_dia("2026-11-18").unwrap().iter().map(|a| (a.timer_id, a.atividade_id)).collect::<Vec<_>>(), [(3, 1)]);
        assert_eq!(b.encerrados().unwrap(), Some(("2026-11-18".into(), vec![1])));
        assert_eq!(b.config_app().unwrap().tema, "claro");
        assert_eq!(b.sessoes_desde(0).unwrap().iter().filter_map(|s| s.ligacao).count(), 4);
        // reabrir (outro dia) não migra de novo nem duplica as atividades, nem muda o zerar
        drop(b);
        let b = Banco::abrir_em(&p, ms_local(2026, 11, 19, 9, 0, F), F).unwrap();
        assert_eq!(linhas_antigas(&b.conn), antes);
        assert_eq!(b.atividades().unwrap().len(), 3);
        assert!(b.zerados().unwrap().values().all(|z| *z == ms_local(2026, 11, 18, 0, 0, F)));
    }

    #[test] fn bancos_das_versoes_1_a_4_chegam_na_9_com_trabalho_e_zerados_hoje() {
        let d = tempfile::tempdir().unwrap();
        for (versao, passos) in [
            (1, ESQUEMA_V1.to_string()),
            (2, format!("{ESQUEMA_V1} {MIGRACAO_V2}")),
            (3, format!("{ESQUEMA_V1} {MIGRACAO_V2} {MIGRACAO_V3}")),
            (4, format!("{ESQUEMA_V1} {MIGRACAO_V2} {MIGRACAO_V3} {MIGRACAO_V4}")),
        ] {
            let p = d.path().join(format!("v{versao}.db"));
            {
                let c = Connection::open(&p).unwrap();
                c.execute_batch(&format!("BEGIN; {passos} COMMIT;")).unwrap();
                c.execute_batch(
                    "INSERT INTO timers (nome, cor) VALUES ('A', '#111');
                     INSERT INTO trechos (timer_id, origem, inicio, fim) VALUES (1, 'manual', 1, 2), (1, 'claude', 3, NULL);",
                ).unwrap();
                if versao >= 2 {
                    c.execute("INSERT INTO ajustes (timer_id, dia, delta_ms, criado_em) VALUES (1, '2026-11-17', 60000, 1)", []).unwrap();
                }
                let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
                assert_eq!(v, versao);
            }
            let b = Banco::abrir_em(&p, h(15, 30), F).unwrap();
            let v: i64 = b.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
            assert_eq!(v, 9, "da versão {versao}");
            assert_eq!(b.zerados().unwrap()[&1], h(0, 0), "da versão {versao}: zerado no início de hoje");
            assert_eq!(b.atividades().unwrap().len(), 3);
            assert_eq!(b.trechos_entre(0, 10).unwrap().iter().map(|t| t.atividade_id).collect::<Vec<_>>(), [1, 1]);
            assert_eq!(b.abertos().unwrap()[0].atividade_id, 1);
            assert_eq!(b.timers().unwrap()[0].atividade_padrao_id, 1);
            if versao >= 2 {
                assert_eq!(b.ajustes_do_dia("2026-11-17").unwrap()[0].atividade_id, 1);
            }
        }
    }

    /// Um banco da versão 5 com o corte do dia às 03:00, dois timers (um arquivado) e um trecho.
    fn banco_v5(p: &Path) {
        let c = Connection::open(p).unwrap();
        c.execute_batch(&format!("BEGIN; {ESQUEMA_V1} {MIGRACAO_V2} {MIGRACAO_V3} {MIGRACAO_V4} COMMIT;")).unwrap();
        migrar_v5(&c).unwrap();
        c.execute_batch(
            "INSERT INTO timers (nome, cor) VALUES ('A', '#111');
             INSERT INTO timers (nome, cor, arquivado) VALUES ('Velho', '#222', 1);
             INSERT INTO trechos (timer_id, origem, inicio, fim) VALUES (1, 'manual', 1, 2);
             INSERT INTO config (chave, valor) VALUES ('app', '{\"corte_min\":180}');",
        )
        .unwrap();
        let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 5);
    }

    #[test] fn migracao_da_5_para_a_6_zera_cada_timer_no_inicio_do_dia_pelo_corte() {
        let d = tempfile::tempdir().unwrap();
        // 01:30 do dia 18 com o corte às 03:00: o dia de trabalho ainda é o 17, que começou às 03:00
        let p = d.path().join("madrugada.db");
        banco_v5(&p);
        let b = Banco::abrir_em(&p, ms_local(2026, 11, 18, 1, 30, F), F).unwrap();
        assert_eq!(b.zerados().unwrap().into_iter().collect::<Vec<_>>(), [(1, h(3, 0)), (2, h(3, 0))]);
        // às 10:00 do dia 18: o dia começou às 03:00 do 18
        let p = d.path().join("manha.db");
        banco_v5(&p);
        let b = Banco::abrir_em(&p, ms_local(2026, 11, 18, 10, 0, F), F).unwrap();
        assert!(b.zerados().unwrap().values().all(|z| *z == ms_local(2026, 11, 18, 3, 0, F)));
        assert_eq!(b.trechos_entre(0, 10).unwrap().len(), 1, "nada se perde");
        // rodar a migração de novo (ou reabrir outro dia) não muda nada
        migrar_v6(&b.conn, 0).unwrap();
        drop(b);
        let b = Banco::abrir_em(&p, ms_local(2026, 11, 25, 10, 0, F), F).unwrap();
        assert!(b.zerados().unwrap().values().all(|z| *z == ms_local(2026, 11, 18, 3, 0, F)));
    }

    /// Um timer com o zerar dado (os testes do acumulado).
    fn timer_zerado(b: &Banco, nome: &str, zerado: Ms) -> i64 {
        let t = b.criar_timer(None, nome, "#111").unwrap();
        b.zerar(Some(&[t]), zerado).unwrap();
        t
    }

    #[test] fn timer_novo_comeca_zerado_no_inicio_do_dia_de_trabalho() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer_em(None, "Novo", "#111", h(14, 20), F).unwrap();
        assert_eq!(b.zerados().unwrap()[&t], h(0, 0));
        // com o corte às 03:00, à 01:30 do dia 18 o dia de trabalho ainda é o 17
        b.salvar_config_app(&ConfigApp { corte_min: 180, ..ConfigApp::default() }).unwrap();
        let t = b.criar_timer_em(None, "Madrugada", "#111", ms_local(2026, 11, 18, 1, 30, F), F).unwrap();
        assert_eq!(b.zerados().unwrap()[&t], h(3, 0));
        // sem o instante: o início do dia de hoje pelo relógio do sistema, nunca "desde sempre"
        let (agora, fuso) = relogio_do_sistema();
        let sem = b.criar_timer(None, "Sem instante", "#111").unwrap();
        let z = b.zerados().unwrap()[&sem];
        assert!(z > 0 && z <= agora && z > agora - 25 * 60 * MIN, "{z} {agora} {fuso}");
    }

    #[test] fn zerar_um_todos_e_desfazer() {
        let b = Banco::abrir_memoria().unwrap();
        let a = timer_zerado(&b, "A", h(8, 0));
        let c = timer_zerado(&b, "B", h(8, 0));
        let arq = timer_zerado(&b, "Arquivado", h(7, 0));
        let mut t = b.timers().unwrap().into_iter().find(|t| t.id == arq).unwrap();
        t.arquivado = true;
        b.editar_timer(&t).unwrap();
        assert_eq!(b.zerar(Some(&[a]), h(10, 0)).unwrap(), [(a, h(8, 0))]);
        let z = b.zerar(None, h(11, 0)).unwrap();
        assert_eq!(z, [(a, h(10, 0)), (c, h(8, 0)), (arq, h(7, 0))], "todos, o arquivado também");
        assert!(b.zerados().unwrap().values().all(|v| *v == h(11, 0)));
        // A foi zerado de novo depois: o "Desfazer" do todos não mexe nele
        b.zerar(Some(&[a]), h(11, 30)).unwrap();
        assert_eq!(b.desfazer_zerar(&z, &[], h(11, 0)).unwrap(), 2);
        assert_eq!(b.zerados().unwrap().into_iter().collect::<Vec<_>>(), [(a, h(11, 30)), (c, h(8, 0)), (arq, h(7, 0))]);
        // timer que não existe: nada
        assert!(b.zerar(Some(&[999]), h(12, 0)).unwrap().is_empty());
        // o corte da virada só avança quem foi zerado antes dele
        assert_eq!(b.zerar_no_corte(h(11, 15)).unwrap(), 2);
        assert_eq!(b.zerados().unwrap().into_iter().collect::<Vec<_>>(), [(a, h(11, 30)), (c, h(11, 15)), (arq, h(11, 15))]);
    }

    #[test] fn acumulado_guardado_cai_ao_mover_uma_sessao_de_ontem() {
        let b = Banco::abrir_memoria().unwrap();
        let a = timer_zerado(&b, "A", 0);
        let outro = timer_zerado(&b, "B", 0);
        let ontem = |hh, mm| ms_local(2026, 11, 16, hh, mm, F);
        let abriu = |t: i64, i: Ms| Mudanca::Abriu { timer_id: Some(t), sessao_id: Some("s".into()), origem: Origem::Claude, inicio: i, atividade_id: 1 };
        let fechou = |t: i64, f: Ms| Mudanca::Fechou { timer_id: Some(t), sessao_id: Some("s".into()), origem: Origem::Claude, fim: f, motivo: MotivoFim::Terminou };
        b.aplicar(&[abriu(a, ontem(9, 0)), fechou(a, ontem(11, 0))]).unwrap();
        let timers: BTreeMap<i64, Option<i64>> = [(a, None), (outro, None)].into();
        let antes = |b: &Banco| b.acumulado_antes(&timers, &BTreeMap::new(), h(0, 0), 0, F).unwrap().por_timer();
        assert_eq!(antes(&b), [(a, 120 * MIN)].into());
        assert!(b.tem_acumulado_guardado());
        // o que é só de hoje não joga fora o guardado
        b.aplicar(&[abriu(outro, h(9, 0))]).unwrap();
        b.aplicar(&[fechou(outro, h(9, 30))]).unwrap();
        assert!(b.tem_acumulado_guardado());
        // mover a sessão de ontem para o outro timer
        b.aplicar(&[Mudanca::Reatribuir { sessao_id: "s".into(), timer_id: Some(outro) }]).unwrap();
        assert!(!b.tem_acumulado_guardado());
        assert_eq!(antes(&b), [(outro, 120 * MIN)].into());
        // um trecho que fecha no passado (o app ficou fechado na virada) também
        assert!(b.tem_acumulado_guardado());
        b.aplicar(&[abriu(a, ontem(23, 0))]).unwrap();
        assert!(!b.tem_acumulado_guardado(), "abriu ontem");
        antes(&b);
        b.aplicar(&[fechou(a, ontem(23, 30))]).unwrap();
        assert!(!b.tem_acumulado_guardado(), "fechou ontem");
        assert_eq!(antes(&b), [(a, 30 * MIN), (outro, 120 * MIN)].into());
        // descartar, restaurar e os ajustes também
        type Mexida = Box<dyn Fn(&Banco)>;
        let mexidas: Vec<Mexida> = vec![
            Box::new(|b| b.restaurar(&[]).unwrap()),
            Box::new(|b| {
                b.criar_ajuste(1, 1, "2026-11-17", MIN, None, h(9, 0)).unwrap();
            }),
            Box::new(|b| b.restaurar_ajustes(&[]).unwrap()),
            Box::new(|b| {
                b.descartar(1, h(0, 0) - 1, h(23, 0)).unwrap();
            }),
        ];
        for f in mexidas {
            antes(&b);
            f(&b);
            assert!(!b.tem_acumulado_guardado());
        }
        // um zerar muda a chave: o guardado não vale para ele
        antes(&b);
        b.zerar(Some(&[outro]), ontem(10, 0)).unwrap();
        assert_eq!(antes(&b)[&outro], 60 * MIN, "só das 10:00 às 11:00 de ontem");
    }

    /// Um banco da versão 8 montado de verdade (rodada 20): o banco real da versão 4
    /// (`banco_real_v4`) passa pelas migrações 5 a 8 deste app, com uso da versão 8 por cima (um
    /// timer e um grupo novos, um zerar só da reunião).
    fn banco_real_v8(p: &Path) {
        banco_real_v4(p);
        let c = Connection::open(p).unwrap();
        let tx = c.unchecked_transaction().unwrap();
        migrar_v5(&tx).unwrap();
        migrar_v6(&tx, ms_local(2026, 11, 18, 0, 0, F)).unwrap();
        migrar_v7(&tx).unwrap();
        migrar_v8(&tx).unwrap();
        tx.commit().unwrap();
        c.execute_batch(
            "INSERT INTO grupos (id, nome, cor, ordem) VALUES (3, 'Estudos', '#3fb68b', 2);
             INSERT INTO timers (id, grupo_id, nome, cor, zerado_em, atividade_padrao_id) VALUES (5, 3, 'Curso', '#3fb68b', 0, 3);
             INSERT INTO trechos (timer_id, origem, inicio, fim, atividade_id) VALUES (5, 'manual', 2200, 2300, 3), (2, 'manual', 2300, 2400, 2);
             INSERT INTO zerados_atividade (timer_id, atividade_id, zerado_em) VALUES (2, 2, 2350);",
        )
        .unwrap();
        let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 8);
        assert!(!tem_coluna(&c, "grupos", "espaco_id").unwrap());
    }

    #[test] fn versao_9_espacos_migra_um_banco_v8_de_verdade_tudo_em_trabalho() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("worktimer.db");
        banco_real_v8(&p);
        let (antes, trechos_antes) = {
            let c = Connection::open(&p).unwrap();
            let n: i64 = c.query_row("SELECT count(*) FROM trechos", [], |r| r.get(0)).unwrap();
            (linhas_antigas(&c), n)
        };
        // a cópia de antes da migração leva a versão 8 no nome
        let dir = d.path().join("backups");
        let quando = chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap().and_hms_opt(9, 30, 0).unwrap();
        let copia = crate::dados::backup_antes_de_migrar(&p, &dir, quando).unwrap().unwrap();
        assert_eq!(copia, dir.join("antes-migracao-v8-20261120-0930.db"));
        let b = Banco::abrir_em(&p, ms_local(2026, 11, 20, 9, 30, F), F).unwrap();
        let v: i64 = b.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 9);
        // um espaço, "Trabalho", com todos os grupos e timers (os arquivados também)
        assert_eq!(b.espacos().unwrap().iter().map(|e| (e.id, e.nome.as_str(), e.arquivado)).collect::<Vec<_>>(), [(1, "Trabalho", false)]);
        assert!(b.espacos_dos_grupos().unwrap().values().all(|e| *e == 1));
        let timers = b.espacos_dos_timers().unwrap();
        assert_eq!(timers.len(), 5, "o arquivado e o sem grupo também");
        assert!(timers.values().all(|e| *e == 1));
        let sem_grupo: i64 = b.conn.query_row("SELECT espaco_id FROM timers WHERE grupo_id IS NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(sem_grupo, 1, "o sem grupo (o Velho, arquivado) vai para Trabalho");
        // nada se perde: as linhas de antes, os trechos, o zerar da reunião
        let n: i64 = b.conn.query_row("SELECT count(*) FROM trechos", [], |r| r.get(0)).unwrap();
        assert_eq!(n, trechos_antes);
        assert_eq!(linhas_antigas(&b.conn), antes, "nenhuma linha antiga muda");
        assert_eq!(b.zerados_atividade().unwrap()[&(2, 2)], 2350);
        assert!(b.atividades_ocultas().unwrap().is_empty());
        // rodar de novo não muda nada (nem cria outro espaço)
        migrar_v9(&b.conn).unwrap();
        assert_eq!(b.espacos().unwrap().len(), 1);
        drop(b);
        // um banco da versão 8 sem dados ganha "Geral"
        let vazio = d.path().join("vazio.db");
        {
            let c = Connection::open(&vazio).unwrap();
            c.execute_batch(&format!("BEGIN; {ESQUEMA_V1} {MIGRACAO_V2} {MIGRACAO_V3} {MIGRACAO_V4} COMMIT;")).unwrap();
            let tx = c.unchecked_transaction().unwrap();
            migrar_v5(&tx).unwrap();
            migrar_v6(&tx, 0).unwrap();
            migrar_v7(&tx).unwrap();
            migrar_v8(&tx).unwrap();
            tx.commit().unwrap();
        }
        assert_eq!(Banco::abrir(&vazio).unwrap().espacos().unwrap()[0].nome, "Geral");
    }

    #[test] fn espacos_criar_editar_e_apagar_so_vazio() {
        let b = Banco::abrir_memoria().unwrap();
        let pessoal = b.criar_espaco("Pessoal", "#e86bd0").unwrap();
        assert_eq!(b.espacos().unwrap().iter().map(|e| (e.nome.as_str(), e.ordem)).collect::<Vec<_>>(), [("Geral", 0), ("Pessoal", 1)]);
        let g = b.criar_grupo_em("Casa", "#111", pessoal).unwrap();
        let solto = b.criar_timer_no(None, pessoal, "Leitura", "#222", h(9, 0), F).unwrap();
        let no_grupo = b.criar_timer_no(Some(g), ESPACO_PRIMEIRO, "Obra", "#333", h(9, 0), F).unwrap();
        let m = b.espacos_dos_timers().unwrap();
        assert_eq!((m[&solto], m[&no_grupo]), (pessoal, pessoal), "o do grupo vale");
        let conteudo = |b: &Banco| b.conteudo_do_espaco(pessoal).unwrap();
        assert_eq!(conteudo(&b), ConteudoDoEspaco { ativos: 2, arquivados: 0 }, "o grupo e o timer sem grupo");
        // com algo à vista, não apaga
        assert_eq!(b.apagar_espaco(pessoal, ESPACO_PRIMEIRO).unwrap(), None);
        // revisão da rodada 20: só com arquivados, apaga e leva os arquivados (ainda arquivados)
        // para o destino
        b.conn.execute("UPDATE grupos SET arquivado = 1", []).unwrap();
        assert_eq!(conteudo(&b), ConteudoDoEspaco { ativos: 1, arquivados: 1 }, "o timer do grupo vai com o grupo");
        assert_eq!(b.apagar_espaco(pessoal, ESPACO_PRIMEIRO).unwrap(), None, "ainda o timer sem grupo");
        b.conn.execute("UPDATE timers SET arquivado = 1 WHERE id = ?1", params![solto]).unwrap();
        assert_eq!(b.apagar_espaco(pessoal, ESPACO_PRIMEIRO).unwrap(), Some(2));
        assert_eq!(b.espacos().unwrap().len(), 1);
        assert_eq!(b.espacos_dos_grupos().unwrap()[&g], ESPACO_PRIMEIRO);
        let m = b.espacos_dos_timers().unwrap();
        assert_eq!((m[&solto], m[&no_grupo]), (ESPACO_PRIMEIRO, ESPACO_PRIMEIRO));
        let arq: Vec<bool> = {
            let mut st = b.conn.prepare("SELECT arquivado FROM grupos ORDER BY id").unwrap();
            st.query_map([], |r| r.get(0)).unwrap().map(|x| x.unwrap()).collect()
        };
        assert_eq!(arq, [true], "segue arquivado");
        // renomear, cor, ordem, arquivar
        let mut e = b.espacos().unwrap()[0].clone();
        e.nome = "Trabalho".into();
        e.arquivado = true;
        b.editar_espaco(&e).unwrap();
        assert_eq!(b.espacos().unwrap()[0], e);
    }

    /// Revisão da rodada 20: o espaço padrão é o primeiro não arquivado, na ordem (não o 1 fixo).
    #[test] fn espaco_padrao_e_o_primeiro_nao_arquivado() {
        let b = Banco::abrir_memoria().unwrap();
        assert_eq!(b.espaco_padrao().unwrap(), ESPACO_PRIMEIRO);
        let pessoal = b.criar_espaco("Pessoal", "#e86bd0").unwrap();
        let mut geral = b.espacos().unwrap()[0].clone();
        geral.arquivado = true;
        b.editar_espaco(&geral).unwrap();
        assert_eq!(b.espaco_padrao().unwrap(), pessoal);
        // o que é criado sem dizer o espaço vai para ele
        let g = b.criar_grupo("Casa", "#111").unwrap();
        let t = b.criar_timer(None, "Leitura", "#222").unwrap();
        assert_eq!(b.espacos_dos_grupos().unwrap()[&g], pessoal);
        assert_eq!(b.espacos_dos_timers().unwrap()[&t], pessoal);
        // a ordem manda
        let outro = b.criar_espaco("Outro", "#333").unwrap();
        let mut e = b.espacos().unwrap().into_iter().find(|e| e.id == outro).unwrap();
        e.ordem = -1;
        b.editar_espaco(&e).unwrap();
        assert_eq!(b.espaco_padrao().unwrap(), outro);
    }

    #[test] fn editar_timer_com_ocultas_numa_transacao_so() {
        let b = Banco::abrir_memoria().unwrap();
        let id = b.criar_timer(None, "A", "#111").unwrap();
        let mut t = b.timers().unwrap()[0].clone();
        t.nome = "B".into();
        b.editar_timer_com_ocultas(&t, &[ATIVIDADE_ESTUDO]).unwrap();
        assert_eq!((b.timers().unwrap()[0].nome.as_str(), &b.atividades_ocultas().unwrap()[&id]), ("B", &vec![ATIVIDADE_ESTUDO]));
        // um timer que não existe: nada é gravado (nem as ocultas)
        let mut x = t.clone();
        x.id = 999;
        assert!(b.editar_timer_com_ocultas(&x, &[ATIVIDADE_REUNIAO]).is_err(), "a chave estrangeira recusa");
        assert!(!b.atividades_ocultas().unwrap().contains_key(&999));
    }

    #[test] fn atividades_ocultas_guardar_e_ler() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#111").unwrap();
        let u = b.criar_timer(None, "B", "#222").unwrap();
        assert!(b.atividades_ocultas().unwrap().is_empty());
        b.guardar_atividades_ocultas(t, &[ATIVIDADE_ESTUDO, ATIVIDADE_REUNIAO]).unwrap();
        b.guardar_atividades_ocultas(u, &[ATIVIDADE_ESTUDO]).unwrap();
        assert_eq!(b.atividades_ocultas().unwrap(), [(t, vec![ATIVIDADE_REUNIAO, ATIVIDADE_ESTUDO]), (u, vec![ATIVIDADE_ESTUDO])].into());
        // trocar substitui; vazio tira todas
        b.guardar_atividades_ocultas(t, &[ATIVIDADE_ESTUDO]).unwrap();
        b.guardar_atividades_ocultas(u, &[]).unwrap();
        assert_eq!(b.atividades_ocultas().unwrap(), [(t, vec![ATIVIDADE_ESTUDO])].into());
    }

    #[test] fn versao_8_zerados_por_atividade_migra_copia_e_e_idempotente() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("v7.db");
        let t;
        {
            // um banco da versão 7, sem a tabela, com um timer zerado às 8:00
            let b = Banco::abrir_em(&p, h(9, 0), F).unwrap();
            t = timer_zerado(&b, "A", h(8, 0));
            b.conn.execute_batch("DROP TABLE zerados_atividade; PRAGMA user_version = 7;").unwrap();
        }
        // a cópia de antes da migração leva a versão de antes no nome
        let dir = d.path().join("backups");
        let quando = chrono::NaiveDate::from_ymd_opt(2026, 11, 20).unwrap().and_hms_opt(9, 30, 0).unwrap();
        let copia = crate::dados::backup_antes_de_migrar(&p, &dir, quando).unwrap().unwrap();
        assert_eq!(copia, dir.join("antes-migracao-v7-20261120-0930.db"));
        let b = Banco::abrir_em(&p, h(9, 0), F).unwrap();
        let v: i64 = b.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 9);
        assert!(b.zerados_atividade().unwrap().is_empty(), "os números ficam os mesmos");
        assert_eq!(b.zerados().unwrap()[&t], h(8, 0));
        // rodar de novo não muda nada
        b.zerar_atividade(t, 2, h(10, 0)).unwrap();
        migrar_v8(&b.conn).unwrap();
        assert_eq!(b.zerados_atividade().unwrap(), [((t, 2), h(10, 0))].into());
        drop(b);
        // de uma versão bem antiga (1) até a 8, numa transação só
        let p1 = d.path().join("v1.db");
        {
            let c = Connection::open(&p1).unwrap();
            c.execute_batch(&format!("BEGIN; {ESQUEMA_V1} COMMIT;")).unwrap();
        }
        let b = Banco::abrir_em(&p1, h(9, 0), F).unwrap();
        let v: i64 = b.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 9);
        assert!(b.zerados_atividade().unwrap().is_empty());
    }

    #[test] fn zerar_uma_atividade_o_timer_todo_desfazer_e_a_virada() {
        let b = Banco::abrir_memoria().unwrap();
        let a = timer_zerado(&b, "A", h(8, 0));
        let c = timer_zerado(&b, "B", h(8, 0));
        // só a reunião do A
        assert_eq!(b.zerar_atividade(a, 2, h(9, 0)).unwrap(), None);
        assert_eq!(b.zerar_atividade(a, 2, h(9, 30)).unwrap(), Some(h(9, 0)));
        b.zerar_atividade(c, 1, h(9, 40)).unwrap();
        let z = b.todos_os_zerados().unwrap();
        assert_eq!((z.da_atividade(a, 2), z.da_atividade(a, 1), z.do_timer(a)), (h(9, 30), h(8, 0), h(8, 0)));
        // desfazer o segundo: volta ao primeiro; um "Desfazer" velho (outro instante) não mexe
        assert!(!b.desfazer_zerar_atividade(a, 2, None, h(9, 0)).unwrap());
        assert!(b.desfazer_zerar_atividade(a, 2, Some(h(9, 0)), h(9, 30)).unwrap());
        assert_eq!(b.zerados_atividade().unwrap()[&(a, 2)], h(9, 0));
        // o timer todo: o zerar das atividades dele sai (o do B fica)
        let (antes, limpas) = b.zerar_com_atividades(Some(&[a]), h(10, 0)).unwrap();
        assert_eq!((antes, limpas.clone()), (vec![(a, h(8, 0))], vec![(a, 2, h(9, 0))]));
        assert_eq!(b.zerados_atividade().unwrap(), [((c, 1), h(9, 40))].into());
        // desfazer o timer todo devolve o zerar da reunião
        assert_eq!(b.desfazer_zerar(&[(a, h(8, 0))], &limpas, h(10, 0)).unwrap(), 1);
        assert_eq!(b.zerados_atividade().unwrap()[&(a, 2)], h(9, 0));
        // a virada: os zerados até o corte saem; um depois do corte fica
        b.zerar_atividade(c, 2, h(11, 0)).unwrap();
        b.zerar_no_corte(h(10, 30)).unwrap();
        assert_eq!(b.zerados_atividade().unwrap(), [((c, 2), h(11, 0))].into());
    }

    #[test] fn acumulado_guardado_conta_cada_atividade_desde_o_seu_zerar() {
        let b = Banco::abrir_memoria().unwrap();
        let a = timer_zerado(&b, "A", 0);
        let ontem = |hh, mm| ms_local(2026, 11, 16, hh, mm, F);
        let abriu = |at: i64, i: Ms| Mudanca::Abriu { timer_id: Some(a), sessao_id: None, origem: Origem::Manual, inicio: i, atividade_id: at };
        let fechou = |f: Ms| Mudanca::Fechou { timer_id: Some(a), sessao_id: None, origem: Origem::Manual, fim: f, motivo: MotivoFim::Pausa };
        // ontem: trabalho das 9:00 às 11:00 e reunião das 11:00 às 12:00
        b.aplicar(&[abriu(1, ontem(9, 0))]).unwrap();
        b.aplicar(&[fechou(ontem(11, 0))]).unwrap();
        b.aplicar(&[abriu(2, ontem(11, 0))]).unwrap();
        b.aplicar(&[fechou(ontem(12, 0))]).unwrap();
        let timers: BTreeMap<i64, Option<i64>> = [(a, None)].into();
        let antes = |b: &Banco| b.acumulado_antes(&timers, &BTreeMap::new(), h(0, 0), 0, F).unwrap().por_atividade;
        assert_eq!(antes(&b), [((a, 1), 120 * MIN), ((a, 2), 60 * MIN)].into());
        // zerar só a reunião às 11:30 de ontem: o guardado não vale mais, e o trabalho fica
        b.zerar_atividade(a, 2, ontem(11, 30)).unwrap();
        assert_eq!(antes(&b), [((a, 1), 120 * MIN), ((a, 2), 30 * MIN)].into());
    }

    #[test] fn migracao_da_5_e_idempotente_e_pula_o_que_ja_existe() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("meio.db");
        {
            // versão 4 com parte da 5 já feita (a tabela e uma coluna): a migração completa o resto
            let c = Connection::open(&p).unwrap();
            c.execute_batch(&format!("BEGIN; {ESQUEMA_V1} {MIGRACAO_V2} {MIGRACAO_V3} {MIGRACAO_V4} COMMIT;")).unwrap();
            c.execute_batch(
                "CREATE TABLE atividades (id INTEGER PRIMARY KEY, nome TEXT NOT NULL, cor TEXT NOT NULL, ordem INTEGER NOT NULL DEFAULT 0, arquivada INTEGER NOT NULL DEFAULT 0);
                 INSERT INTO atividades (id, nome, cor, ordem) VALUES (1, 'Dev', '#000000', 0);
                 ALTER TABLE trechos ADD COLUMN atividade_id INTEGER NOT NULL DEFAULT 1;",
            ).unwrap();
        }
        let b = Banco::abrir(&p).unwrap();
        let a = b.atividades().unwrap();
        assert_eq!(a.iter().map(|x| x.nome.as_str()).collect::<Vec<_>>(), ["Dev", "Reunião", "Estudo"], "a que já existia fica");
        assert!(tem_coluna(&b.conn, "timers", "atividade_padrao_id").unwrap());
        assert!(tem_coluna(&b.conn, "ajustes", "atividade_id").unwrap());
        // rodar de novo não muda nada
        migrar_v5(&b.conn).unwrap();
        assert_eq!(b.atividades().unwrap(), a);
    }

    #[test] fn migracao_que_falha_no_meio_deixa_o_banco_como_estava() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("v3.db");
        {
            // uma tabela "atividades" estranha (sem a cor): a inserção das três falha
            let c = Connection::open(&p).unwrap();
            c.execute_batch(&format!("BEGIN; {ESQUEMA_V1} {MIGRACAO_V2} {MIGRACAO_V3} COMMIT;")).unwrap();
            c.execute_batch("CREATE TABLE atividades (id INTEGER PRIMARY KEY, nome TEXT NOT NULL);").unwrap();
        }
        assert!(Banco::abrir(&p).is_err());
        let c = Connection::open(&p).unwrap();
        let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, 3, "nem a versão 4 ficou: a transação é uma só");
        assert!(!tem_coluna(&c, "sessoes", "ligacao").unwrap());
        assert!(!tem_coluna(&c, "trechos", "atividade_id").unwrap());
    }

    #[test] fn motivo_de_uma_versao_mais_nova_nao_impede_a_leitura() {
        let b = Banco::abrir_memoria().unwrap();
        b.criar_timer(None, "A", "#111").unwrap();
        b.conn.execute(
            "INSERT INTO trechos (timer_id, origem, inicio, fim, motivo_fim) VALUES (1, 'manual', 1, 5, 'motivo_do_futuro'), (1, 'manual', 6, 9, 'trocou_atividade')",
            [],
        ).unwrap();
        let v = b.trechos_entre(0, 10).unwrap();
        assert_eq!(v.iter().map(|t| t.motivo_fim).collect::<Vec<_>>(), [Some(MotivoFim::Outro), Some(MotivoFim::TrocouAtividade)]);
    }

    #[test] fn app_da_versao_4_grava_num_banco_da_5_em_trabalho() {
        let b = Banco::abrir_memoria().unwrap();
        b.criar_timer(None, "A", "#111").unwrap();
        // o INSERT de uma versão 4 (sem as colunas novas)
        b.conn.execute("INSERT INTO trechos (timer_id, origem, inicio) VALUES (1, 'manual', 5)", []).unwrap();
        b.conn.execute("INSERT INTO ajustes (timer_id, dia, delta_ms, criado_em) VALUES (1, 'd', 1, 1)", []).unwrap();
        assert_eq!(b.abertos().unwrap()[0].atividade_id, ATIVIDADE_TRABALHO);
        assert_eq!(b.ajustes_do_dia("d").unwrap()[0].atividade_id, ATIVIDADE_TRABALHO);
    }

    #[test] fn atividades_criar_editar_arquivar_e_trocar_a_padrao() {
        let b = Banco::abrir_memoria().unwrap();
        let curso = b.criar_atividade("Curso", "#123456").unwrap();
        assert_eq!(b.atividades().unwrap().last().map(|a| (a.id, a.ordem)), Some((curso, 3)), "vai para o fim");
        let mut a = b.atividades().unwrap()[1].clone();
        a.nome = "Call".into();
        a.cor = "#abcdef".into();
        b.editar_atividade(&a).unwrap();
        assert_eq!(b.atividades().unwrap()[1], a);
        let t1 = b.criar_timer(None, "Curso.dev", "#111").unwrap();
        let t2 = b.criar_timer(None, "Projeto A", "#222").unwrap();
        let mut t = b.timers().unwrap()[0].clone();
        t.atividade_padrao_id = curso;
        b.editar_timer(&t).unwrap();
        assert_eq!(b.atividades_padrao().unwrap()[&t1], curso);
        assert_eq!(b.trocar_atividade_padrao(curso, ATIVIDADE_ESTUDO).unwrap(), 1);
        assert_eq!(b.atividades_padrao().unwrap()[&t1], ATIVIDADE_ESTUDO);
        assert_eq!(b.atividades_padrao().unwrap()[&t2], ATIVIDADE_TRABALHO);
    }

    #[test] fn trechos_e_ajustes_guardam_a_atividade() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "Cliente A", "#111").unwrap();
        b.aplicar(&[
            Mudanca::Abriu { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, inicio: h(9, 0), atividade_id: ATIVIDADE_REUNIAO },
        ]).unwrap();
        assert_eq!(b.abertos().unwrap()[0].atividade_id, ATIVIDADE_REUNIAO);
        b.aplicar(&[
            Mudanca::Fechou { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, fim: h(10, 0), motivo: MotivoFim::TrocouAtividade },
            Mudanca::Abriu { timer_id: Some(t), sessao_id: None, origem: Origem::Manual, inicio: h(10, 0), atividade_id: ATIVIDADE_TRABALHO },
        ]).unwrap();
        let v = b.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        // a troca vai como "pausa", que uma versão anterior do app sabe ler
        assert_eq!(
            v.iter().map(|x| (x.atividade_id, x.fim, x.motivo_fim)).collect::<Vec<_>>(),
            [(ATIVIDADE_REUNIAO, Some(h(10, 0)), Some(MotivoFim::Pausa)), (ATIVIDADE_TRABALHO, None, None)]
        );
        let gravado: String = b.conn.query_row("SELECT motivo_fim FROM trechos WHERE fim IS NOT NULL", [], |r| r.get(0)).unwrap();
        assert_eq!(gravado, "pausa");
        b.criar_ajuste(t, ATIVIDADE_REUNIAO, "2026-11-17", 30 * MIN, None, 1).unwrap();
        assert_eq!(b.ajustes_do_dia("2026-11-17").unwrap()[0].atividade_id, ATIVIDADE_REUNIAO);
    }

    #[test] fn reatribuir_leva_a_atividade_padrao_do_timer_novo() {
        let b = Banco::abrir_memoria().unwrap();
        let t1 = b.criar_timer(None, "Projeto A", "#111").unwrap();
        let t2 = b.criar_timer(None, "Curso", "#222").unwrap();
        let mut curso = b.timers().unwrap()[1].clone();
        curso.atividade_padrao_id = ATIVIDADE_ESTUDO;
        b.editar_timer(&curso).unwrap();
        b.aplicar(&[
            Mudanca::Sessao(sessao("s1", Some(t1))),
            Mudanca::Abriu { timer_id: Some(t1), sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(8, 0), atividade_id: 1 },
            Mudanca::Fechou { timer_id: Some(t1), sessao_id: Some("s1".into()), origem: Origem::Claude, fim: h(8, 30), motivo: MotivoFim::Terminou },
            Mudanca::Abriu { timer_id: Some(t1), sessao_id: Some("s1".into()), origem: Origem::Leitura, inicio: h(10, 0), atividade_id: 1 },
        ]).unwrap();
        b.aplicar(&[Mudanca::ReatribuirDesde { sessao_id: "s1".into(), timer_id: t2, desde: h(9, 0) }]).unwrap();
        let at = |b: &Banco| b.trechos_entre(h(0, 0), h(23, 0)).unwrap().iter().map(|t| (t.timer_id, t.atividade_id)).collect::<Vec<_>>();
        assert_eq!(at(&b), [(Some(t1), 1), (Some(t2), ATIVIDADE_ESTUDO)]);
        b.aplicar(&[Mudanca::Reatribuir { sessao_id: "s1".into(), timer_id: Some(t1) }]).unwrap();
        assert_eq!(at(&b), [(Some(t1), 1), (Some(t1), 1)]);
        // desligada (sem timer): a atividade fica
        b.aplicar(&[Mudanca::Reatribuir { sessao_id: "s1".into(), timer_id: Some(t2) }]).unwrap();
        b.aplicar(&[Mudanca::Reatribuir { sessao_id: "s1".into(), timer_id: None }]).unwrap();
        assert_eq!(at(&b), [(None, ATIVIDADE_ESTUDO), (None, ATIVIDADE_ESTUDO)]);
    }

    #[test] fn banco_da_versao_3_marca_a_mao_so_com_prova_de_movimento() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("v3.db");
        {
            let c = Connection::open(&p).unwrap();
            c.execute_batch(&format!("BEGIN; {ESQUEMA_V1} {MIGRACAO_V2} {MIGRACAO_V3} COMMIT;")).unwrap();
            c.execute("INSERT INTO timers (nome, cor) VALUES ('Flow', '#111'), ('Cliente B', '#222')", []).unwrap();
            c.execute("INSERT INTO regras_pasta (pasta, timer_id) VALUES ('D:/Proj/Flow', 1)", []).unwrap();
            for (id, timer) in [("pela-regra", "1"), ("movida", "2"), ("regra-trocada", "2"), ("movida-sem-nada-aberto", "2"), ("sem", "NULL")] {
                c.execute(
                    &format!(
                        "INSERT INTO sessoes (id, cwd, timer_id, estado, ultimo_evento, criada_em)
                         VALUES ('{id}', 'D:/Proj/Flow', {timer}, 'parada', 10, 1)"
                    ),
                    [],
                ).unwrap();
            }
            // "movida": contou em Flow pela regra e foi movida "daqui para frente" para Cliente B (o caso da 5ee991)
            c.execute_batch(
                "INSERT INTO trechos (timer_id, sessao_id, origem, inicio, fim, motivo_fim) VALUES
                   (1, 'movida', 'claude', 1, 5, 'desligada'), (2, 'movida', 'claude', 5, 9, 'terminou');
                 -- \"regra-trocada\": contou sempre em Cliente B (a regra apontava para Cliente B na época)
                 INSERT INTO trechos (timer_id, sessao_id, origem, inicio, fim, motivo_fim) VALUES
                   (2, 'regra-trocada', 'claude', 1, 5, 'terminou'), (2, 'regra-trocada', 'claude', 6, 9, 'terminou');
                 -- o formato da 5ee991 no banco real: movida sem nada aberto (nenhum trecho
                 -- 'desligada'), trechos em Flow (a regra de agora) e depois em Cliente B
                 INSERT INTO trechos (timer_id, sessao_id, origem, inicio, fim, motivo_fim) VALUES
                   (1, 'movida-sem-nada-aberto', 'leitura', 10, 11, 'saiu_da_janela'),
                   (1, 'movida-sem-nada-aberto', 'leitura', 12, 13, 'nova_mensagem'),
                   (1, 'movida-sem-nada-aberto', 'claude', 13, 14, 'terminou'),
                   (1, 'movida-sem-nada-aberto', 'leitura', 15, 16, 'outra_sessao'),
                   (2, 'movida-sem-nada-aberto', 'claude', 20, 21, 'terminou'),
                   (2, 'movida-sem-nada-aberto', 'leitura', 22, 23, 'saiu_da_janela');",
            ).unwrap();
        }
        let b = Banco::abrir(&p).unwrap();
        let l = |id: &str| b.sessoes_desde(0).unwrap().into_iter().find(|s| s.id == id).unwrap().ligacao;
        assert_eq!(l("pela-regra"), Some(Ligacao::Regra));
        assert_eq!(l("movida"), Some(Ligacao::Mao), "trecho fechado ao desligar e o seguinte noutro timer");
        assert_eq!(l("regra-trocada"), Some(Ligacao::Deduzida), "regra trocada depois não vira à mão");
        assert_eq!(l("movida-sem-nada-aberto"), Some(Ligacao::Mao), "o caso real da 5ee991");
        assert_eq!(l("sem"), None);
        // a ligação grava e volta
        let mut s = b.sessoes_desde(0).unwrap().into_iter().find(|s| s.id == "sem").unwrap();
        s.ligacao = Some(Ligacao::Herdada);
        b.salvar_sessao(&s).unwrap();
        assert_eq!(l("sem"), Some(Ligacao::Herdada));
        // já na versão 4: uma sessão com timer e sem origem (o app caiu antes da marcação, ou um
        // app da versão 3 a gravou) é marcada ao abrir de novo
        b.conn.execute("UPDATE sessoes SET ligacao = NULL WHERE id = 'pela-regra'", []).unwrap();
        drop(b);
        let b = Banco::abrir(&p).unwrap();
        assert_eq!(b.sessoes_desde(0).unwrap().into_iter().find(|s| s.id == "pela-regra").unwrap().ligacao, Some(Ligacao::Regra));
    }

    #[test] fn reatribuir_desde_so_move_os_trechos_a_partir_do_instante() {
        let b = Banco::abrir_memoria().unwrap();
        for i in 1..=2 { b.criar_timer(None, &format!("T{i}"), "#000").unwrap(); }
        b.aplicar(&[
            Mudanca::Sessao(sessao("s1", Some(1))),
            Mudanca::Abriu { timer_id: Some(1), sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(8, 0), atividade_id: 1 },
            Mudanca::Fechou { timer_id: Some(1), sessao_id: Some("s1".into()), origem: Origem::Claude, fim: h(8, 30), motivo: MotivoFim::Terminou },
            Mudanca::Abriu { timer_id: Some(1), sessao_id: Some("s1".into()), origem: Origem::Claude, inicio: h(10, 0), atividade_id: 1 },
        ]).unwrap();
        b.aplicar(&[Mudanca::ReatribuirDesde { sessao_id: "s1".into(), timer_id: 2, desde: h(9, 0) }]).unwrap();
        let v = b.trechos_entre(h(0, 0), h(23, 0)).unwrap();
        assert_eq!(v.iter().map(|t| t.timer_id).collect::<Vec<_>>(), [Some(1), Some(2)]);
        assert_eq!(b.sessoes_desde(0).unwrap()[0].timer_id, Some(2));
    }

    #[test] fn banco_da_versao_2_migra_para_a_3_e_os_ajustes_seguem_valendo() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("v2.db");
        {
            // banco como a versao 2 criava, com um ajuste
            let c = Connection::open(&p).unwrap();
            c.execute_batch(&format!("BEGIN; {ESQUEMA_V1} {MIGRACAO_V2} COMMIT;")).unwrap();
            c.execute("INSERT INTO timers (nome, cor) VALUES ('A', '#111')", []).unwrap();
            c.execute("INSERT INTO ajustes (timer_id, dia, delta_ms, criado_em) VALUES (1, '2026-11-17', 900000, 1)", []).unwrap();
            let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
            assert_eq!(v, 2);
        }
        let b = Banco::abrir(&p).unwrap();
        let v: i64 = b.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, VERSAO_ESQUEMA);
        assert_eq!(b.ajustes_do_dia("2026-11-17").unwrap()[0].delta_ms, 15 * MIN, "o ajuste antigo vale");
        assert_eq!(b.descartar_ajustes(1, "2026-11-17").unwrap().len(), 1);
        drop(b);
        // reabrir nao migra de novo e o descarte fica
        let b = Banco::abrir(&p).unwrap();
        assert!(b.ajustes_do_dia("2026-11-17").unwrap().is_empty());
    }

    #[test] fn descartar_e_restaurar_ajustes_do_timer_no_dia() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#111").unwrap();
        let outro = b.criar_timer(None, "B", "#222").unwrap();
        b.criar_ajuste(t, 1, "2026-11-17", 30 * MIN, None, 1).unwrap();
        b.criar_ajuste(t, 1, "2026-11-17", -20 * MIN, None, 2).unwrap();
        b.criar_ajuste(t, 1, "2026-11-16", 60 * MIN, None, 3).unwrap();
        b.criar_ajuste(outro, 1, "2026-11-17", 5 * MIN, None, 4).unwrap();
        let ids = b.descartar_ajustes(t, "2026-11-17").unwrap();
        assert_eq!(ids.len(), 2);
        let v = b.ajustes_do_dia("2026-11-17").unwrap();
        assert_eq!(v.iter().map(|a| a.timer_id).collect::<Vec<_>>(), [outro], "só o do outro timer fica");
        assert_eq!(b.ajustes_do_dia("2026-11-16").unwrap().len(), 1, "outro dia não muda");
        assert!(b.descartar_ajustes(t, "2026-11-17").unwrap().is_empty(), "já descartados");
        b.restaurar_ajustes(&ids).unwrap();
        assert_eq!(b.ajustes_do_dia("2026-11-17").unwrap().len(), 3);
    }

    #[test] fn banco_da_versao_1_abre_e_migra_sem_perder_nada() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("v1.db");
        {
            // banco como a versao 1 criava, com dados
            let c = Connection::open(&p).unwrap();
            c.execute_batch(&format!("BEGIN; {ESQUEMA_V1} COMMIT;")).unwrap();
            c.execute(
                "INSERT INTO grupos (nome, cor, link_projeto, card_padrao) VALUES ('INV', '#111',
                 'https://sprints.zoho.com/workspace/exemplo#P146', 'https://sprints.zoho.com/workspace/exemplo#P146/itemdetails/I7')",
                [],
            ).unwrap();
            c.execute("INSERT INTO timers (grupo_id, nome, cor, link_card) VALUES (1, 'Projeto A', '#222', 'x')", []).unwrap();
            c.execute("INSERT INTO trechos (timer_id, origem, inicio, fim) VALUES (1, 'manual', 1, 2)", []).unwrap();
            let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
            assert_eq!(v, 1);
        }
        let b = Banco::abrir(&p).unwrap();
        let v: i64 = b.conn.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, VERSAO_ESQUEMA);
        let g = b.grupos().unwrap()[0].clone();
        assert_eq!((g.nome.as_str(), g.link_projeto_nome.as_deref(), g.card_padrao_nome.as_deref()), ("INV", None, None));
        assert!(g.card_padrao.as_deref().unwrap().ends_with("I7"));
        let t = b.timers().unwrap()[0].clone();
        assert_eq!((t.nome.as_str(), t.link_card.as_deref(), t.link_card_nome.as_deref()), ("Projeto A", Some("x"), None));
        assert_eq!(b.trechos_entre(0, 10).unwrap().len(), 1);
        // as partes novas funcionam
        b.editar_grupo(&Grupo { link_projeto_nome: Some("INV - Inovação".into()), ..g }).unwrap();
        assert_eq!(b.grupos().unwrap()[0].link_projeto_nome.as_deref(), Some("INV - Inovação"));
        b.criar_ajuste(t.id, 1, "2026-11-17", 15 * MIN, Some("reunião"), 5).unwrap();
        assert_eq!(b.ajustes_do_dia("2026-11-17").unwrap().len(), 1);
        // reabrir nao migra de novo
        drop(b);
        let b = Banco::abrir(&p).unwrap();
        assert_eq!(b.ajustes_do_dia("2026-11-17").unwrap().len(), 1);
        assert_eq!(b.grupos().unwrap()[0].link_projeto_nome.as_deref(), Some("INV - Inovação"));
    }

    #[test] fn ajustes_por_dia_e_desfazer() {
        let b = Banco::abrir_memoria().unwrap();
        let t = b.criar_timer(None, "A", "#111").unwrap();
        let a1 = b.criar_ajuste(t, 1, "2026-11-17", 15 * MIN, Some("reunião sem timer"), h(10, 0)).unwrap();
        b.criar_ajuste(t, 1, "2026-11-17", -5 * MIN, None, h(11, 0)).unwrap();
        b.criar_ajuste(t, 1, "2026-11-18", 60 * MIN, None, h(12, 0)).unwrap();
        let v = b.ajustes_do_dia("2026-11-17").unwrap();
        assert_eq!(v.iter().map(|a| a.delta_ms).collect::<Vec<_>>(), [15 * MIN, -5 * MIN]);
        assert_eq!((v[0].timer_id, v[0].motivo.as_deref(), v[0].criado_em), (t, Some("reunião sem timer"), h(10, 0)));
        assert!(b.remover_ajuste(a1).unwrap());
        assert!(!b.remover_ajuste(a1).unwrap(), "já removido");
        assert_eq!(b.ajustes_do_dia("2026-11-17").unwrap().len(), 1);
        // timer inexistente nao entra (chave estrangeira)
        assert!(b.criar_ajuste(999, 1, "2026-11-17", MIN, None, 0).is_err());
    }

    #[test] fn nomes_dos_links_gravam_e_voltam() {
        let b = Banco::abrir_memoria().unwrap();
        let g = b.criar_grupo("INV", "#111").unwrap();
        let t = b.criar_timer(Some(g), "Painel", "#222").unwrap();
        let mut gr = b.grupos().unwrap()[0].clone();
        gr.card_padrao = Some("https://sprints.zoho.com/workspace/exemplo#P146/itemdetails/I7".into());
        gr.card_padrao_nome = Some("Backlog geral".into());
        b.editar_grupo(&gr).unwrap();
        assert_eq!(b.grupos().unwrap()[0], gr);
        let mut tm = b.timers().unwrap()[0].clone();
        assert_eq!(tm.id, t);
        tm.link_card_nome = Some("Painel novo".into());
        b.editar_timer(&tm).unwrap();
        assert_eq!(b.timers().unwrap()[0], tm);
    }

    #[test] fn banco_em_arquivo_persiste_e_reabre() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("p.db");
        {
            let b = Banco::abrir(&p).unwrap();
            b.criar_grupo("G", "#000").unwrap();
        }
        let b = Banco::abrir(&p).unwrap();
        assert_eq!(b.grupos().unwrap().len(), 1);
    }

    #[test] fn encerrados_gravam_e_voltam() {
        let b = Banco::abrir_memoria().unwrap();
        assert_eq!(b.encerrados().unwrap(), None);
        b.aplicar(&[Mudanca::Encerrados { dia: "2026-11-17".into(), timers: vec![2, 5] }]).unwrap();
        assert_eq!(b.encerrados().unwrap(), Some(("2026-11-17".into(), vec![2, 5])));
        b.aplicar(&[Mudanca::Encerrados { dia: "2026-11-17".into(), timers: vec![] }]).unwrap();
        assert_eq!(b.encerrados().unwrap(), Some(("2026-11-17".into(), vec![])));
        // valor estragado conta como nenhum
        b.gravar_valor(CHAVE_ENCERRADOS, "lixo").unwrap();
        assert_eq!(b.encerrados().unwrap(), None);
    }
}
