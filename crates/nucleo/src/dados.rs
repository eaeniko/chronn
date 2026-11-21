// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Cópias do banco: os backups (`backups\AAAA-MM-DD_HHMM.db`), a cópia de antes de migrar o
//! esquema e a cópia do hook. Tudo recebe as pastas explícitas; nada aqui conhece o perfil real.
//!
//! O banco é sempre copiado pelo SQLite (`VACUUM INTO`), que leva junto o que ainda está no WAL.

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use chrono::{Duration, NaiveDate, NaiveDateTime};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::banco::Banco;

/// Quantos backups ficam na pasta: os 14 mais recentes e mais um por dia dos últimos 14 dias
/// com backup (com o app aberto são 4 por dia; só os recentes cobririam 3 dias e meio).
pub const MANTER_BACKUPS: usize = 14;
/// Com o app aberto, um backup a cada 6 h.
pub const INTERVALO_BACKUP_H: i64 = 6;

fn texto<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

/// `PRAGMA quick_check` num banco (aberto só para leitura): recusa a cópia inconsistente antes
/// de ela virar o banco do app.
pub fn conferir_banco(caminho: &Path) -> Result<(), String> {
    let conn = Connection::open_with_flags(caminho, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| crate::idioma::tfa("erro.copia_abrir", &[&e.to_string()]))?;
    let r: String = conn
        .query_row("PRAGMA quick_check", [], |r| r.get(0))
        .map_err(|e| crate::idioma::tfa("erro.copia_nao_confere", &[&e.to_string()]))?;
    if r != "ok" {
        return Err(crate::idioma::tfa("erro.copia_nao_confere", &[&r]));
    }
    Ok(())
}

// ---- backups ----

/// Um backup da pasta.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Backup {
    /// Nome do arquivo (`2026-11-18_0930.db`; no mesmo minuto, `2026-11-18_0930-2.db`).
    pub nome: String,
    /// Data e hora locais, ISO ("2026-11-18T09:30"): a tela formata pelo idioma (rodada 23,
    /// revisão I1).
    pub quando: String,
    pub bytes: u64,
    #[serde(skip)]
    pub momento: NaiveDateTime,
    #[serde(skip)]
    pub n: u32,
}

/// Nome do backup feito em `quando` (hora local); `n` > 1 desempata o mesmo minuto.
pub fn nome_do_backup(quando: NaiveDateTime, n: u32) -> String {
    let base = quando.format("%Y-%m-%d_%H%M");
    if n <= 1 { format!("{base}.db") } else { format!("{base}-{n}.db") }
}

/// O contrário de `nome_do_backup`; `None` para qualquer outro arquivo.
pub fn ler_nome(nome: &str) -> Option<(NaiveDateTime, u32)> {
    let (data, resto) = nome.strip_suffix(".db")?.split_once('_')?;
    let (hhmm, n) = match resto.split_once('-') {
        Some((h, n)) => (h, n.parse::<u32>().ok().filter(|n| *n >= 2)?),
        None => (resto, 1),
    };
    if data.len() != 10 || hhmm.len() != 4 || !hhmm.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let quando = NaiveDateTime::parse_from_str(&format!("{data} {hhmm}"), "%Y-%m-%d %H%M").ok()?;
    Some((quando, n))
}

/// Backups da pasta, o mais recente primeiro.
pub fn listar(dir: &Path) -> Vec<Backup> {
    let Ok(rd) = std::fs::read_dir(dir) else { return vec![] };
    let mut v: Vec<Backup> = rd
        .filter_map(|e| e.ok())
        .filter_map(|e| {
            let nome = e.file_name().to_string_lossy().into_owned();
            let (momento, n) = ler_nome(&nome)?;
            let m = e.metadata().ok().filter(|m| m.is_file())?;
            Some(Backup { quando: momento.format("%Y-%m-%dT%H:%M").to_string(), nome, bytes: m.len(), momento, n })
        })
        .collect();
    v.sort_by_key(|b| std::cmp::Reverse((b.momento, b.n)));
    v
}

/// Hora de fazer backup? Ao abrir o app: se ainda não há backup de hoje. Sempre: se o último
/// tem 6 h ou mais (ou não há nenhum).
pub fn backup_devido(ultimo: Option<NaiveDateTime>, agora: NaiveDateTime, abertura: bool) -> bool {
    match ultimo {
        None => true,
        Some(u) => (abertura && u.date() != agora.date()) || agora - u >= Duration::hours(INTERVALO_BACKUP_H),
    }
}

/// Faz um backup do banco em `dir` com o nome da hora `agora` (local). Grava num temporário e
/// renomeia: um backup pela metade nunca aparece na lista. Não roda a rotação.
pub fn fazer_backup(banco: &Banco, dir: &Path, agora: NaiveDateTime) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| crate::idioma::tfa("erro.backup_pasta", &[&e.to_string()]))?;
    let mut n = 1;
    let (destino, tmp) = loop {
        let nome = nome_do_backup(agora, n);
        let destino = dir.join(&nome);
        let tmp = dir.join(format!("{nome}.tmp"));
        if !destino.exists() {
            break (destino, tmp);
        }
        n += 1;
    };
    let _ = std::fs::remove_file(&tmp);
    banco.copiar_para(&tmp).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        crate::idioma::tfa("erro.backup_fazer", &[&e.to_string()])
    })?;
    std::fs::rename(&tmp, &destino).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        crate::idioma::tfa("erro.backup_fazer", &[&e.to_string()])
    })?;
    Ok(destino)
}

/// Os backups que a rotação guarda (nomes): os `manter` mais recentes, o mais recente de cada um
/// dos últimos `manter` dias com backup, e os `protegidos`.
pub fn guardados(lista: &[Backup], manter: usize, protegidos: &[&str]) -> BTreeSet<String> {
    let mut fica: BTreeSet<String> = lista.iter().take(manter).map(|b| b.nome.clone()).collect();
    let mut dias: BTreeSet<NaiveDate> = BTreeSet::new();
    // a lista vem do mais recente para o mais antigo: o primeiro de cada dia é o mais recente dele
    for b in lista {
        if dias.len() == manter && !dias.contains(&b.momento.date()) {
            break;
        }
        if dias.insert(b.momento.date()) {
            fica.insert(b.nome.clone());
        }
    }
    fica.extend(protegidos.iter().map(|p| p.to_string()));
    fica
}

/// Apaga os backups que a rotação não guarda (`guardados`); devolve os nomes apagados. Os
/// `protegidos` (o backup que está sendo restaurado, o de antes de restaurar) nunca saem.
pub fn rotacionar(dir: &Path, manter: usize, protegidos: &[&str]) -> Vec<String> {
    let lista = listar(dir);
    let fica = guardados(&lista, manter, protegidos);
    lista
        .into_iter()
        .filter(|b| !fica.contains(&b.nome))
        .filter(|b| std::fs::remove_file(dir.join(&b.nome)).is_ok())
        .map(|b| b.nome)
        .collect()
}

fn com_sufixo(p: &Path, sufixo: &str) -> PathBuf {
    let mut s = p.as_os_str().to_owned();
    s.push(sufixo);
    PathBuf::from(s)
}

// ---- antes de migrar o esquema ----

/// Nome da cópia de antes de migrar um banco da versão `versao` do esquema:
/// `antes-migracao-v4-20261118-0930.db` (`n` > 1 desempata o mesmo minuto). Não tem o formato de
/// `nome_do_backup`, então a rotação nunca a apaga (e ela não aparece na lista dos Ajustes: serve
/// para voltar a uma versão anterior do app, à mão).
pub fn nome_antes_de_migrar(versao: i64, quando: NaiveDateTime, n: u32) -> String {
    let base = format!("antes-migracao-v{versao}-{}", quando.format("%Y%m%d-%H%M"));
    if n <= 1 { format!("{base}.db") } else { format!("{base}-{n}.db") }
}

/// Antes de o app abrir o banco (`Banco::abrir`, que migra): se o esquema dele é mais antigo que
/// o deste app, copia o banco inteiro, com o WAL, para `dir` (`VACUUM INTO` num temporário,
/// renomeado depois). Banco que não existe, novo (versão 0) ou já na versão atual: nada. Devolve
/// a cópia feita. Não altera o banco.
pub fn backup_antes_de_migrar(banco: &Path, dir: &Path, agora: NaiveDateTime) -> Result<Option<PathBuf>, String> {
    if !banco.is_file() {
        return Ok(None);
    }
    let conn = Connection::open_with_flags(banco, OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| format!("não consegui abrir o banco para a cópia de antes da migração: {e}"))?;
    let versao: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0)).map_err(texto)?;
    if versao == 0 || versao >= crate::banco::VERSAO_ESQUEMA {
        return Ok(None);
    }
    std::fs::create_dir_all(dir).map_err(|e| crate::idioma::tfa("erro.backup_pasta", &[&e.to_string()]))?;
    let mut n = 1;
    let destino = loop {
        let d = dir.join(nome_antes_de_migrar(versao, agora, n));
        if !d.exists() {
            break d;
        }
        n += 1;
    };
    let tmp = com_sufixo(&destino, ".tmp");
    let _ = std::fs::remove_file(&tmp);
    conn.execute("VACUUM INTO ?1", [tmp.to_string_lossy()]).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("não consegui copiar o banco antes da migração: {e}")
    })?;
    drop(conn);
    std::fs::rename(&tmp, &destino).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("não consegui copiar o banco antes da migração: {e}")
    })?;
    Ok(Some(destino))
}

// ---- backup → banco ----

/// Copia um backup para `destino` (que não pode existir) pelo SQLite, sem alterar o backup, e
/// confere a cópia. Recusa o arquivo que não é um banco do app.
pub fn copiar_backup(backup: &Path, destino: &Path) -> Result<(), String> {
    let conn = Connection::open_with_flags(backup, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)
        .map_err(|e| crate::idioma::tfa("erro.backup_abrir", &[&e.to_string()]))?;
    let ok: bool = conn
        .query_row("SELECT count(*) = 2 FROM sqlite_master WHERE type = 'table' AND name IN ('timers', 'trechos')", [], |r| r.get(0))
        .map_err(|e| format!("{}: {e}", crate::idioma::tfa("erro.backup_nao_e_banco", &[])))?;
    if !ok {
        return Err(crate::idioma::tfa("erro.backup_nao_e_banco", &[]));
    }
    conn.execute("VACUUM INTO ?1", [destino.to_string_lossy()]).map_err(|e| crate::idioma::tfa("erro.backup_copiar", &[&e.to_string()]))?;
    drop(conn);
    if let Err(e) = conferir_banco(destino) {
        let _ = std::fs::remove_file(destino);
        return Err(e);
    }
    Ok(())
}

// ---- cópia do hook ----

/// Põe em `destino` (a pasta de dados) a cópia do hook que está em `origem` (a pasta de
/// instalação), se ela não existe ou é diferente. Grava ao lado (até o disco) e troca de uma vez;
/// com o hook em uso, a troca falha e o laço tenta de novo mais tarde. Devolve se copiou.
pub fn atualizar_hook(origem: &Path, destino: &Path) -> Result<bool, String> {
    let novo = std::fs::read(origem).map_err(|e| format!("não achei o hook em {}: {e}", origem.display()))?;
    if std::fs::read(destino).is_ok_and(|atual| atual == novo) {
        return Ok(false);
    }
    if let Some(pai) = destino.parent() {
        std::fs::create_dir_all(pai).map_err(|e| format!("não consegui criar {}: {e}", pai.display()))?;
    }
    let tmp = com_sufixo(destino, ".novo");
    let trocar = || -> std::io::Result<()> {
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&novo)?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, destino)
    };
    trocar().map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("não consegui atualizar o hook (em uso?), tento de novo mais tarde: {e}")
    })?;
    Ok(true)
}

#[cfg(test)]
mod testes {
    use super::*;
    use chrono::NaiveDate;

    fn dt(d: u32, h: u32, m: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, d).unwrap().and_hms_opt(h, m, 0).unwrap()
    }

    // ---- backups ----

    #[test]
    fn copia_de_antes_da_migracao_so_com_esquema_antigo_e_fora_da_rotacao() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("backups");
        let p = d.path().join("worktimer.db");
        // não existe: nada
        assert_eq!(backup_antes_de_migrar(&p, &dir, dt(18, 9, 30)).unwrap(), None);
        {
            // banco da versão 1 (o formato mais antigo), com um timer e um trecho
            let c = Connection::open(&p).unwrap();
            c.execute_batch(&format!("BEGIN; {} COMMIT;", crate::banco::ESQUEMA_V1)).unwrap();
            c.execute_batch("INSERT INTO timers (nome, cor) VALUES ('A', '#111'); INSERT INTO trechos (timer_id, origem, inicio, fim) VALUES (1, 'manual', 1, 2);").unwrap();
        }
        let copia = backup_antes_de_migrar(&p, &dir, dt(18, 9, 30)).unwrap().unwrap();
        assert_eq!(copia, dir.join("antes-migracao-v1-20261118-0930.db"));
        // a cópia é o banco de antes, inteiro, na versão dele
        let c = Connection::open(&copia).unwrap();
        let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        let trechos: i64 = c.query_row("SELECT count(*) FROM trechos", [], |r| r.get(0)).unwrap();
        assert_eq!((v, trechos), (1, 1));
        drop(c);
        // o mesmo minuto não sobrescreve
        assert_eq!(backup_antes_de_migrar(&p, &dir, dt(18, 9, 30)).unwrap().unwrap(), dir.join("antes-migracao-v1-20261118-0930-2.db"));
        // migrado, não copia de novo
        drop(Banco::abrir(&p).unwrap());
        assert_eq!(backup_antes_de_migrar(&p, &dir, dt(18, 9, 31)).unwrap(), None);
        // a rotação (com mais de 14 backups) não apaga as cópias de antes da migração
        for i in 0..20 {
            std::fs::write(dir.join(nome_do_backup(dt(1 + i, 10, 0), 1)), b"x").unwrap();
        }
        rotacionar(&dir, MANTER_BACKUPS, &[]);
        assert!(copia.exists());
        assert!(dir.join("antes-migracao-v1-20261118-0930-2.db").exists());
        assert_eq!(listar(&dir).len(), MANTER_BACKUPS);
        // banco novo (versão 0): nada
        let novo = d.path().join("novo.db");
        drop(Connection::open(&novo).unwrap());
        assert_eq!(backup_antes_de_migrar(&novo, &dir, dt(18, 9, 32)).unwrap(), None);
    }

    #[test]
    fn nome_do_backup_vai_e_volta() {
        assert_eq!(nome_do_backup(dt(18, 9, 5), 1), "2026-11-18_0905.db");
        assert_eq!(nome_do_backup(dt(18, 9, 5), 2), "2026-11-18_0905-2.db");
        assert_eq!(ler_nome("2026-11-18_0905.db"), Some((dt(18, 9, 5), 1)));
        assert_eq!(ler_nome("2026-11-18_0905-3.db"), Some((dt(18, 9, 5), 3)));
        for ruim in ["2026-11-18_0905.db.tmp", "x.db", "2026-11-18_905.db", "2026-11-18_0905-1.db", "2026-13-18_0905.db", "worktimer.db"] {
            assert_eq!(ler_nome(ruim), None, "{ruim}");
        }
    }

    #[test]
    fn backup_ao_abrir_uma_vez_por_dia_e_a_cada_6_h() {
        assert!(backup_devido(None, dt(18, 9, 0), false));
        assert!(backup_devido(Some(dt(17, 23, 0)), dt(18, 1, 0), true), "primeira abertura do dia");
        assert!(!backup_devido(Some(dt(17, 23, 0)), dt(18, 1, 0), false), "aberto: só depois de 6 h");
        assert!(!backup_devido(Some(dt(18, 8, 0)), dt(18, 9, 0), true), "já tem de hoje");
        assert!(!backup_devido(Some(dt(18, 8, 0)), dt(18, 13, 59), false));
        assert!(backup_devido(Some(dt(18, 8, 0)), dt(18, 14, 0), false));
    }

    fn banco_com_timer(nome: &str) -> Banco {
        let b = Banco::abrir_memoria().unwrap();
        b.criar_timer(None, nome, "#a00").unwrap();
        b
    }

    #[test]
    fn backup_e_um_banco_inteiro_e_nao_sobrescreve_no_mesmo_minuto() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path().join("backups");
        let b = banco_com_timer("A");
        let p1 = fazer_backup(&b, &dir, dt(18, 9, 0)).unwrap();
        let p2 = fazer_backup(&b, &dir, dt(18, 9, 0)).unwrap();
        assert_eq!(p1.file_name().unwrap(), "2026-11-18_0900.db");
        assert_eq!(p2.file_name().unwrap(), "2026-11-18_0900-2.db");
        let lido = Banco::abrir(&p1).unwrap();
        assert_eq!(lido.timers().unwrap()[0].nome, "A");
        let l = listar(&dir);
        assert_eq!(l.iter().map(|b| b.nome.as_str()).collect::<Vec<_>>(), ["2026-11-18_0900-2.db", "2026-11-18_0900.db"]);
        assert_eq!(l[1].quando, "2026-11-18T09:00");
        assert!(l[1].bytes > 0);
        assert!(!dir.join("2026-11-18_0900.db.tmp").exists());
    }

    #[test]
    fn rotacao_mantem_os_14_mais_recentes() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path();
        let b = banco_com_timer("A");
        for i in 0..20u32 {
            fazer_backup(&b, dir, dt(1 + i, 10, 0)).unwrap();
        }
        // arquivos que não são backup ficam
        std::fs::write(dir.join("leia-me.txt"), "x").unwrap();
        let apagados = rotacionar(dir, MANTER_BACKUPS, &[]);
        assert_eq!(apagados.len(), 6);
        assert!(apagados.contains(&"2026-11-01_1000.db".to_string()));
        let l = listar(dir);
        assert_eq!(l.len(), 14);
        assert_eq!(l[0].nome, "2026-11-20_1000.db");
        assert_eq!(l[13].nome, "2026-11-07_1000.db");
        assert!(dir.join("leia-me.txt").exists());
        assert!(rotacionar(dir, MANTER_BACKUPS, &[]).is_empty());
    }

    #[test]
    fn rotacao_guarda_tambem_um_por_dia_e_os_protegidos() {
        let d = tempfile::tempdir().unwrap();
        let dir = d.path();
        let b = banco_com_timer("A");
        // 4 por dia (app aberto) durante 6 dias: os 14 mais recentes cobririam só 3 dias e meio
        for dia in 1..=6u32 {
            for h in [0, 6, 12, 18] {
                fazer_backup(&b, dir, dt(dia, h, 0)).unwrap();
            }
        }
        rotacionar(dir, MANTER_BACKUPS, &["2026-11-01_0000.db"]);
        let nomes: Vec<String> = listar(dir).into_iter().map(|b| b.nome).collect();
        assert_eq!(nomes.len(), 14 + 2 + 1, "14 recentes, o último dos dias 1 e 2, e o protegido");
        assert_eq!(&nomes[..14], &listar(dir).iter().take(14).map(|b| b.nome.clone()).collect::<Vec<_>>()[..]);
        for fica in ["2026-11-02_1800.db", "2026-11-01_1800.db", "2026-11-01_0000.db"] {
            assert!(nomes.contains(&fica.to_string()), "{fica}");
        }
        assert!(!nomes.contains(&"2026-11-01_0600.db".to_string()));
        // mais de 14 dias: o 15º dia mais antigo sai
        let d2 = tempfile::tempdir().unwrap();
        for dia in 1..=16u32 {
            fazer_backup(&b, d2.path(), dt(dia, 9, 0)).unwrap();
        }
        assert_eq!(rotacionar(d2.path(), MANTER_BACKUPS, &[]), ["2026-11-02_0900.db", "2026-11-01_0900.db"]);
    }

    #[test]
    fn copiar_backup_recusa_o_que_nao_e_banco_do_app() {
        let d = tempfile::tempdir().unwrap();
        let b = banco_com_timer("A");
        let bk = fazer_backup(&b, d.path(), dt(18, 9, 0)).unwrap();
        let antes = std::fs::read(&bk).unwrap();
        let destino = d.path().join("restaurado.db");
        copiar_backup(&bk, &destino).unwrap();
        assert_eq!(Banco::abrir(&destino).unwrap().timers().unwrap()[0].nome, "A");
        assert_eq!(std::fs::read(&bk).unwrap(), antes, "o backup não muda");
        let outro = d.path().join("outro.db");
        Connection::open(&outro).unwrap().execute("CREATE TABLE x (a)", []).unwrap();
        assert!(copiar_backup(&outro, &d.path().join("y.db")).unwrap_err().contains("não é um banco do"));
        std::fs::write(d.path().join("texto.db"), "não sou banco").unwrap();
        assert!(copiar_backup(&d.path().join("texto.db"), &d.path().join("z.db")).is_err());
    }

    // ---- cópia do hook ----

    #[test]
    fn hook_copiado_quando_falta_ou_muda() {
        let d = tempfile::tempdir().unwrap();
        let origem = d.path().join("instalacao").join("work-timer-hook.exe");
        std::fs::create_dir_all(origem.parent().unwrap()).unwrap();
        std::fs::write(&origem, b"v1").unwrap();
        let destino = d.path().join(".worktimer").join("bin").join("work-timer-hook.exe");
        assert!(atualizar_hook(&origem, &destino).unwrap(), "não existia");
        assert_eq!(std::fs::read(&destino).unwrap(), b"v1");
        assert!(!atualizar_hook(&origem, &destino).unwrap(), "igual: não copia");
        std::fs::write(&origem, b"v2 maior").unwrap();
        assert!(atualizar_hook(&origem, &destino).unwrap(), "mudou: copia");
        assert_eq!(std::fs::read(&destino).unwrap(), b"v2 maior");
        assert!(!com_sufixo(&destino, ".novo").exists());
        assert!(atualizar_hook(&d.path().join("nada.exe"), &destino).is_err());
    }

    /// O hook rodando (arquivo aberto sem compartilhar): a troca falha, a cópia velha fica e a
    /// próxima tentativa (o laço) consegue.
    #[cfg(windows)]
    #[test]
    fn hook_em_uso_fica_para_a_proxima_abertura() {
        use std::os::windows::fs::OpenOptionsExt;
        let d = tempfile::tempdir().unwrap();
        let origem = d.path().join("work-timer-hook.exe");
        let destino = d.path().join("bin").join("work-timer-hook.exe");
        std::fs::write(&origem, b"v1").unwrap();
        atualizar_hook(&origem, &destino).unwrap();
        std::fs::write(&origem, b"v2").unwrap();
        let em_uso = std::fs::OpenOptions::new().read(true).share_mode(0).open(&destino).unwrap();
        assert!(atualizar_hook(&origem, &destino).unwrap_err().contains("tento de novo mais tarde"));
        drop(em_uso);
        assert_eq!(std::fs::read(&destino).unwrap(), b"v1");
        assert!(!com_sufixo(&destino, ".novo").exists());
        assert!(atualizar_hook(&origem, &destino).unwrap());
        assert_eq!(std::fs::read(&destino).unwrap(), b"v2");
    }
}
