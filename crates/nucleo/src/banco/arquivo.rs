// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Spec 004: os timers escondidos, o tempo guardado de cada timer, o destino das conversas e das
//! regras ao arquivar, e o apagar de verdade (com as linhas guardadas para o "Desfazer").
use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use rusqlite::types::ValueRef;
use rusqlite::{params, params_from_iter, Result};
use serde::{Deserialize, Serialize};

use super::Banco;
use crate::modelo::{Ajuste, Ligacao, Trecho};
use crate::relatorio::{com_ajustes, totais};
use crate::tempo::{dia_de, inicio_do_dia, proximo_corte, Ms};

/// As tabelas que o apagar guarda e o "Desfazer" põe de volta, na ordem de pôr de volta (o grupo
/// e o timer antes do que aponta para eles), com a coluna que liga a linha ao timer (ou ao grupo).
const TABELAS_DO_TIMER: [(&str, &str); 6] = [
    ("timers", "id"),
    ("trechos", "timer_id"),
    ("ajustes", "timer_id"),
    ("regras_pasta", "timer_id"),
    ("timer_atividades_ocultas", "timer_id"),
    ("zerados_atividade", "timer_id"),
];

/// O tempo guardado de um timer (spec 004): o total de sempre (cada dia como o dia mostra: a
/// soma das atividades, com os ajustes) e os dias de trabalho com tempo.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct TempoGuardado {
    pub total_ms: Ms,
    pub dias: BTreeSet<NaiveDate>,
}

/// As linhas de uma tabela, como estavam (para o "Desfazer").
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Linhas {
    pub tabela: String,
    pub colunas: Vec<String>,
    pub valores: Vec<Vec<serde_json::Value>>,
}

/// O que um apagar sem tempo tirou do banco (o "Desfazer" põe tudo de volta, com os mesmos ids).
#[derive(Clone, Debug, PartialEq, Default, Serialize, Deserialize)]
pub struct Apagado {
    /// As linhas apagadas, tabela por tabela, na ordem de pôr de volta.
    pub linhas: Vec<Linhas>,
    /// Os timers que só mudaram de lugar ("Só o grupo"): o timer, o grupo, o espaço e o
    /// arquivado de antes.
    pub lugares: Vec<(i64, Option<i64>, i64, bool)>,
    /// As sessões que ficaram sem timer: a sessão, o timer e a ligação de antes.
    pub sessoes: Vec<(String, i64, Option<String>)>,
}

/// `?1, ?2, …` para `n` valores.
fn marcas(n: usize) -> String {
    (1..=n).map(|i| format!("?{i}")).collect::<Vec<_>>().join(", ")
}

fn para_json(v: ValueRef<'_>) -> Result<serde_json::Value> {
    Ok(match v {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(i) => i.into(),
        ValueRef::Real(f) => serde_json::Number::from_f64(f).map_or(serde_json::Value::Null, serde_json::Value::Number),
        ValueRef::Text(t) => String::from_utf8_lossy(t).into_owned().into(),
        ValueRef::Blob(_) => return Err(rusqlite::Error::InvalidQuery),
    })
}

fn do_json(v: &serde_json::Value) -> rusqlite::types::Value {
    use rusqlite::types::Value;
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Integer(i64::from(*b)),
        serde_json::Value::Number(n) => n.as_i64().map_or_else(|| Value::Real(n.as_f64().unwrap_or(0.0)), Value::Integer),
        serde_json::Value::String(s) => Value::Text(s.clone()),
        outro => Value::Text(outro.to_string()),
    }
}

/// O erro do `por_de_volta` é de um id (ou de uma pasta de regra) que já foi usado de novo?
pub fn lugar_ocupado(e: &rusqlite::Error) -> bool {
    e.sqlite_error_code() == Some(rusqlite::ErrorCode::ConstraintViolation)
}

impl Banco {
    /// Os timers escondidos (spec 004): arquivados, ou num grupo arquivado, ou num espaço
    /// arquivado (o do grupo, para o timer com grupo).
    pub fn timers_escondidos(&self) -> Result<BTreeSet<i64>> {
        let mut st = self.conn.prepare(
            "SELECT t.id FROM timers t
             LEFT JOIN grupos g ON g.id = t.grupo_id
             LEFT JOIN espacos e ON e.id = COALESCE(g.espaco_id, t.espaco_id)
             WHERE t.arquivado = 1 OR COALESCE(g.arquivado, 0) = 1 OR COALESCE(e.arquivado, 0) = 1",
        )?;
        let v = st.query_map([], |r| r.get(0))?;
        v.collect()
    }

    /// O tempo guardado de cada timer com tempo (os arquivados também), pelo corte e o fuso de
    /// agora. Trecho aberto conta até `agora`. Varre o histórico inteiro: só para o que o usuário
    /// abre (os Arquivados, o diálogo de apagar), nunca a cada ciclo.
    pub fn tempo_guardado(&self, agora: Ms, corte_min: u32, fuso_min: i32) -> Result<BTreeMap<i64, TempoGuardado>> {
        let trechos = self.trechos_entre(Ms::MIN, Ms::MAX)?;
        let ajustes = self.ajustes_criados_desde(Ms::MIN)?;
        let mut por_dia: BTreeMap<NaiveDate, (Vec<Trecho>, Vec<Ajuste>)> = BTreeMap::new();
        // o trecho entra em cada dia que ele toca (o corte pode ter mudado depois de gravado; revisão M7)
        for t in trechos.into_iter().filter(|t| !t.descartado && t.timer_id.is_some()) {
            let fim = t.fim.unwrap_or(agora).max(t.inicio);
            let mut dia = dia_de(t.inicio, corte_min, fuso_min);
            let ultimo = dia_de((fim - 1).max(t.inicio), corte_min, fuso_min);
            while dia < ultimo {
                por_dia.entry(dia).or_default().0.push(t.clone());
                dia = dia.succ_opt().unwrap_or(ultimo);
            }
            por_dia.entry(ultimo).or_default().0.push(t);
        }
        for a in ajustes {
            if let Ok(d) = NaiveDate::parse_from_str(&a.dia, "%Y-%m-%d") {
                por_dia.entry(d).or_default().1.push(a);
            }
        }
        let mut out: BTreeMap<i64, TempoGuardado> = BTreeMap::new();
        for (dia, (trechos, ajustes)) in por_dia {
            let de = inicio_do_dia(dia, corte_min, fuso_min);
            let ate = proximo_corte(de, corte_min, fuso_min);
            let t = com_ajustes(totais(&trechos, de, ate, agora), &ajustes);
            for (timer, ms) in t.por_timer.into_iter().filter(|(_, ms)| *ms > 0) {
                let g = out.entry(timer).or_default();
                g.total_ms += ms;
                g.dias.insert(dia);
            }
        }
        Ok(out)
    }

    /// As sessões (de sempre) ligadas a um destes timers, com a ligação.
    pub fn sessoes_dos_timers(&self, ids: &[i64]) -> Result<Vec<(String, i64, Option<String>)>> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let mut st = self
            .conn
            .prepare(&format!("SELECT id, timer_id, ligacao FROM sessoes WHERE timer_id IN ({}) ORDER BY id", marcas(ids.len())))?;
        let v = st.query_map(params_from_iter(ids), |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?;
        v.collect()
    }

    /// O destino das conversas e das regras de pasta destes timers (spec 004, ao arquivar ou
    /// apagar), numa transação: com `para`, as sessões passam a ele, ligadas à mão, e as regras
    /// também; sem, as sessões ficam sem timer (nem ligação), desligadas em `agora` (ligá-las de
    /// novo "desde o início" não leva o histórico delas; revisão C1), e as regras saem.
    pub fn mover_ligacoes(&self, de: &[i64], para: Option<i64>, agora: Ms) -> Result<()> {
        if de.is_empty() {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        let lista = marcas(de.len());
        let n = de.len() + 1;
        let mut valores: Vec<rusqlite::types::Value> = de.iter().map(|i| (*i).into()).collect();
        match para {
            Some(destino) => {
                valores.push(destino.into());
                tx.execute(
                    &format!("UPDATE sessoes SET timer_id = ?{n}, ligacao = '{}' WHERE timer_id IN ({lista})", Ligacao::Mao.as_str()),
                    params_from_iter(&valores),
                )?;
                // a mesma pasta não tem duas regras (UNIQUE): a do destino, se já havia, fica
                tx.execute(&format!("UPDATE regras_pasta SET timer_id = ?{n} WHERE timer_id IN ({lista})"), params_from_iter(&valores))?;
            }
            None => {
                valores.push(agora.into());
                tx.execute(
                    &format!("UPDATE sessoes SET timer_id = NULL, ligacao = NULL, desligada_em = ?{n} WHERE timer_id IN ({lista})"),
                    params_from_iter(&valores),
                )?;
                valores.pop();
                tx.execute(&format!("DELETE FROM regras_pasta WHERE timer_id IN ({lista})"), params_from_iter(&valores))?;
            }
        }
        tx.commit()
    }

    /// As linhas de `tabela` com `coluna` num de `ids`.
    fn linhas_de(&self, tabela: &str, coluna: &str, ids: &[i64]) -> Result<Linhas> {
        let mut st =
            self.conn.prepare(&format!("SELECT * FROM {tabela} WHERE {coluna} IN ({}) ORDER BY rowid", marcas(ids.len())))?;
        let colunas: Vec<String> = st.column_names().into_iter().map(str::to_string).collect();
        let n = colunas.len();
        let mut valores = vec![];
        let mut rows = st.query(params_from_iter(ids))?;
        while let Some(r) = rows.next()? {
            valores.push((0..n).map(|i| para_json(r.get_ref(i)?)).collect::<Result<Vec<_>>>()?);
        }
        Ok(Linhas { tabela: tabela.to_string(), colunas, valores })
    }

    /// Apaga os timers `ids` e tudo o que é deles (trechos e ajustes, descartados ou não, regras,
    /// atividades ocultas, zerados), numa transação; as sessões ligadas a eles ficam sem timer.
    /// Devolve as linhas (`Apagado`), para o "Desfazer". O app fecha antes o que está aberto
    /// neles (`Motor::esconder`).
    pub fn apagar_timers(&self, ids: &[i64]) -> Result<Apagado> {
        let mut a = Apagado::default();
        if ids.is_empty() {
            return Ok(a);
        }
        a.sessoes = self.sessoes_dos_timers(ids)?;
        for (tabela, coluna) in TABELAS_DO_TIMER {
            let l = self.linhas_de(tabela, coluna, ids)?;
            if !l.valores.is_empty() {
                a.linhas.push(l);
            }
        }
        let tx = self.conn.unchecked_transaction()?;
        let lista = marcas(ids.len());
        tx.execute(&format!("UPDATE sessoes SET timer_id = NULL, ligacao = NULL WHERE timer_id IN ({lista})"), params_from_iter(ids))?;
        // o que aponta para o timer antes dele
        for (tabela, coluna) in TABELAS_DO_TIMER.iter().rev() {
            tx.execute(&format!("DELETE FROM {tabela} WHERE {coluna} IN ({lista})"), params_from_iter(ids))?;
        }
        tx.commit()?;
        self.mexeu_no_passado(None);
        Ok(a)
    }

    /// "Só o grupo" (spec 004): os timers dele (os arquivados também) vão para o "Sem grupo" do
    /// espaço do grupo, com as horas, e o grupo sai. Os de um grupo arquivado ficam arquivados (não
    /// voltam à tela sozinhos). Devolve o grupo e os lugares de antes, para o "Desfazer".
    pub fn apagar_so_o_grupo(&self, id: i64) -> Result<Apagado> {
        let mut a = Apagado { linhas: vec![self.linhas_de("grupos", "id", &[id])?], ..Default::default() };
        if a.linhas[0].valores.is_empty() {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        let (espaco, arquivado): (i64, bool) =
            self.conn.query_row("SELECT espaco_id, arquivado FROM grupos WHERE id = ?1", params![id], |r| Ok((r.get(0)?, r.get(1)?)))?;
        let mut st = self.conn.prepare("SELECT id, grupo_id, espaco_id, arquivado FROM timers WHERE grupo_id = ?1 ORDER BY id")?;
        a.lugares =
            st.query_map(params![id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?.collect::<Result<_>>()?;
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE timers SET grupo_id = NULL, espaco_id = ?2, arquivado = (arquivado OR ?3) WHERE grupo_id = ?1",
            params![id, espaco, arquivado],
        )?;
        tx.execute("DELETE FROM grupos WHERE id = ?1", params![id])?;
        tx.commit()?;
        self.mexeu_no_passado(None);
        Ok(a)
    }

    /// Apaga a linha do grupo (sem timers: o app apaga antes os timers dele). Devolve a linha.
    pub fn apagar_grupo_vazio(&self, id: i64) -> Result<Linhas> {
        let l = self.linhas_de("grupos", "id", &[id])?;
        let n: i64 = self.conn.query_row("SELECT count(*) FROM timers WHERE grupo_id = ?1", params![id], |r| r.get(0))?;
        if l.valores.is_empty() || n > 0 {
            return Err(rusqlite::Error::QueryReturnedNoRows);
        }
        self.conn.execute("DELETE FROM grupos WHERE id = ?1", params![id])?;
        Ok(l)
    }

    /// O "Desfazer" do apagar: põe de volta as linhas (com os mesmos ids), os lugares e as
    /// ligações das sessões (só das que seguem sem timer), numa transação. Só aceita as tabelas e
    /// as colunas que o apagar guarda; um id que já foi usado de novo recusa tudo.
    pub fn por_de_volta(&self, a: &Apagado) -> Result<()> {
        let tx = self.conn.unchecked_transaction()?;
        for l in &a.linhas {
            if l.tabela != "grupos" && !TABELAS_DO_TIMER.iter().any(|(t, _)| *t == l.tabela) {
                return Err(rusqlite::Error::InvalidQuery);
            }
            let existem: Vec<String> = {
                let mut st = tx.prepare(&format!("PRAGMA table_info({})", l.tabela))?;
                st.query_map([], |r| r.get::<_, String>(1))?.collect::<Result<_>>()?
            };
            if l.colunas.is_empty() || l.colunas.iter().any(|c| !existem.contains(c)) {
                return Err(rusqlite::Error::InvalidQuery);
            }
            let sql = format!("INSERT INTO {} ({}) VALUES ({})", l.tabela, l.colunas.join(", "), marcas(l.colunas.len()));
            for linha in &l.valores {
                if linha.len() != l.colunas.len() {
                    return Err(rusqlite::Error::InvalidQuery);
                }
                tx.execute(&sql, params_from_iter(linha.iter().map(do_json)))?;
            }
        }
        for (timer, grupo, espaco, arquivado) in &a.lugares {
            tx.execute(
                "UPDATE timers SET grupo_id = ?2, espaco_id = ?3, arquivado = ?4 WHERE id = ?1",
                params![timer, grupo, espaco, arquivado],
            )?;
        }
        for (sessao, timer, ligacao) in &a.sessoes {
            tx.execute(
                "UPDATE sessoes SET timer_id = ?2, ligacao = ?3 WHERE id = ?1 AND timer_id IS NULL",
                params![sessao, timer, ligacao],
            )?;
        }
        tx.commit()?;
        self.mexeu_no_passado(None);
        Ok(())
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::{EstadoSessao, MotivoFim, Mudanca, Origem, Sessao};
    use crate::tempo::ms_local;

    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms {
        ms_local(2026, 11, 20, hh, mm, F)
    }
    fn banco() -> Banco {
        Banco::abrir_memoria().unwrap()
    }
    fn sessao(id: &str, timer: i64) -> Sessao {
        Sessao {
            id: id.into(),
            cwd: r"D:\proj".into(),
            nome: None,
            transcript: None,
            timer_id: Some(timer),
            estado: EstadoSessao::Parada,
            ultimo_evento: h(9, 0),
            terminou_em: None,
            criada_em: h(9, 0),
            ligacao: Some(Ligacao::Regra),
            desligada_em: None,
            origem: None,
            conta: None,
        }
    }
    fn trecho(b: &Banco, timer: i64, de: Ms, ate: Ms) {
        b.aplicar(&[
            Mudanca::Abriu { timer_id: Some(timer), sessao_id: None, origem: Origem::Manual, inicio: de, atividade_id: 1 },
            Mudanca::Fechou { timer_id: Some(timer), sessao_id: None, origem: Origem::Manual, fim: ate, motivo: MotivoFim::Pausa },
        ])
        .unwrap();
    }

    #[test]
    fn escondidos_sao_os_arquivados_e_os_de_grupo_ou_espaco_arquivado() {
        let b = banco();
        let g = b.criar_grupo("Casa", "#fff").unwrap();
        let solto = b.criar_timer(None, "Solto", "#fff").unwrap();
        let arq = b.criar_timer(None, "Velho", "#fff").unwrap();
        let do_grupo = b.criar_timer(Some(g), "Obra", "#fff").unwrap();
        assert!(b.timers_escondidos().unwrap().is_empty());
        let mut t = b.timers().unwrap().into_iter().find(|t| t.id == arq).unwrap();
        t.arquivado = true;
        b.editar_timer(&t).unwrap();
        let mut gr = b.grupos().unwrap().into_iter().find(|x| x.id == g).unwrap();
        gr.arquivado = true;
        b.editar_grupo(&gr).unwrap();
        assert_eq!(b.timers_escondidos().unwrap(), [arq, do_grupo].into());
        // o espaço arquivado esconde o timer sem grupo dele
        let mut e = b.espacos().unwrap().remove(0);
        e.arquivado = true;
        b.editar_espaco(&e).unwrap();
        assert!(b.timers_escondidos().unwrap().contains(&solto));
        assert_eq!(b.timers_com(true).unwrap().len(), 3);
        assert_eq!(b.grupos_com(true).unwrap().len(), 1);
    }

    #[test]
    fn tempo_guardado_soma_os_dias_com_os_ajustes() {
        let b = banco();
        let a = b.criar_timer(None, "A", "#fff").unwrap();
        trecho(&b, a, h(9, 0), h(10, 0));
        trecho(&b, a, h(9, 30), h(10, 30)); // sobreposto: conta uma vez
        trecho(&b, a, ms_local(2026, 11, 21, 9, 0, F), ms_local(2026, 11, 21, 9, 20, F));
        b.criar_ajuste(a, 1, "2026-11-21", 10 * 60_000, None, h(11, 0)).unwrap();
        let g = &b.tempo_guardado(h(23, 0), 0, F).unwrap()[&a];
        assert_eq!(g.total_ms, 90 * 60_000 + 30 * 60_000);
        assert_eq!(g.dias.len(), 2);
        // um trecho que atravessa a meia-noite conta nos dois dias, inteiro (revisão M7)
        let c = b.criar_timer(None, "C", "#fff").unwrap();
        trecho(&b, c, h(23, 0), ms_local(2026, 11, 21, 1, 0, F));
        let g = &b.tempo_guardado(h(23, 0) + 5 * 3_600_000, 0, F).unwrap()[&c];
        assert_eq!((g.total_ms, g.dias.len()), (2 * 3_600_000, 2));
    }

    #[test]
    fn mover_e_soltar_as_ligacoes() {
        let b = banco();
        let a = b.criar_timer(None, "A", "#fff").unwrap();
        let c = b.criar_timer(None, "C", "#fff").unwrap();
        let s = sessao("s1", a);
        b.salvar_sessao(&s).unwrap();
        b.criar_regra_pasta("D:\\proj", a).unwrap();
        b.mover_ligacoes(&[a], Some(c), h(10, 0)).unwrap();
        assert_eq!(b.sessoes_dos_timers(&[c]).unwrap(), vec![("s1".into(), c, Some("mao".into()))]);
        assert_eq!(b.regras_pasta().unwrap()[0].timer_id, c);
        b.mover_ligacoes(&[c], None, h(10, 0)).unwrap();
        assert!(b.sessoes_dos_timers(&[c]).unwrap().is_empty());
        assert_eq!(b.sessoes_desde(0).unwrap()[0].desligada_em, Some(h(10, 0)), "solta como o Desligar");
        assert!(b.regras_pasta().unwrap().is_empty());
    }

    #[test]
    fn apagar_e_por_de_volta_com_os_mesmos_ids() {
        let b = banco();
        let a = b.criar_timer(None, "A", "#fff").unwrap();
        let fica = b.criar_timer(None, "Fica", "#fff").unwrap();
        trecho(&b, a, h(9, 0), h(10, 0));
        trecho(&b, fica, h(9, 0), h(10, 0));
        b.criar_ajuste(a, 1, "2026-11-20", 60_000, Some("x"), h(11, 0)).unwrap();
        b.criar_regra_pasta("D:\\proj", a).unwrap();
        b.guardar_atividades_ocultas(a, &[2]).unwrap();
        let s = sessao("s1", a);
        b.salvar_sessao(&s).unwrap();
        let antes = b.tempo_guardado(h(23, 0), 0, F).unwrap();
        let ap = b.apagar_timers(&[a]).unwrap();
        assert!(!b.timers().unwrap().iter().any(|t| t.id == a));
        assert!(b.regras_pasta().unwrap().is_empty());
        assert!(b.sessoes_dos_timers(&[a]).unwrap().is_empty());
        assert!(!b.tempo_guardado(h(23, 0), 0, F).unwrap().contains_key(&a));
        assert_eq!(b.tempo_guardado(h(23, 0), 0, F).unwrap()[&fica], antes[&fica], "o outro não muda");
        // o Desfazer, passando pelo JSON como a tela faz
        let ap: Apagado = serde_json::from_str(&serde_json::to_string(&ap).unwrap()).unwrap();
        b.por_de_volta(&ap).unwrap();
        assert_eq!(b.tempo_guardado(h(23, 0), 0, F).unwrap(), antes);
        assert_eq!(b.regras_pasta().unwrap()[0].timer_id, a);
        assert_eq!(b.atividades_ocultas().unwrap()[&a], vec![2]);
        assert_eq!(b.sessoes_dos_timers(&[a]).unwrap(), vec![("s1".into(), a, Some("regra".into()))]);
        // tabela fora da lista: recusa
        let ruim = Apagado { linhas: vec![Linhas { tabela: "config".into(), colunas: vec!["chave".into()], valores: vec![] }], ..Default::default() };
        assert!(b.por_de_volta(&ruim).is_err());
    }

    #[test]
    fn so_o_grupo_leva_os_timers_ao_sem_grupo_e_volta() {
        let b = banco();
        let g = b.criar_grupo("Casa", "#fff").unwrap();
        let t = b.criar_timer(Some(g), "Obra", "#fff").unwrap();
        let ap = b.apagar_so_o_grupo(g).unwrap();
        assert!(b.grupos_com(true).unwrap().is_empty());
        assert_eq!(b.timers().unwrap().iter().find(|x| x.id == t).unwrap().grupo_id, None);
        b.por_de_volta(&ap).unwrap();
        assert_eq!(b.timers().unwrap().iter().find(|x| x.id == t).unwrap().grupo_id, Some(g));
        assert!(b.apagar_grupo_vazio(g).is_err(), "com timer, não");
        // o grupo arquivado: os timers ficam arquivados no "Sem grupo", e o Desfazer devolve como era
        let mut gr = b.grupos().unwrap().remove(0);
        gr.arquivado = true;
        b.editar_grupo(&gr).unwrap();
        let ap = b.apagar_so_o_grupo(g).unwrap();
        assert!(b.timers().unwrap().is_empty(), "segue fora da tela");
        assert!(b.timers_escondidos().unwrap().contains(&t));
        b.por_de_volta(&ap).unwrap();
        let x = b.timers_com(true).unwrap().into_iter().find(|x| x.id == t).unwrap();
        assert_eq!((x.grupo_id, x.arquivado), (Some(g), false));
    }
}
