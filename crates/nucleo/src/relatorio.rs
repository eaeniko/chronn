// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use std::collections::BTreeMap;

use chrono::NaiveDate;

use crate::modelo::{Ajuste, Trecho};
use crate::tempo::{dia_de, inicio_do_dia, proximo_corte, Ms};

/// Duracao da uniao de intervalos `[ini, fim)` (sobreposicoes contam uma vez).
pub fn uniao_ms(intervalos: &[(Ms, Ms)]) -> Ms {
    let mut v: Vec<(Ms, Ms)> = intervalos.iter().copied().filter(|(i, f)| f > i).collect();
    v.sort();
    let mut total = 0;
    let mut atual: Option<(Ms, Ms)> = None;
    for (i, f) in v {
        match atual {
            Some((ai, af)) if i <= af => atual = Some((ai, af.max(f))),
            Some((ai, af)) => {
                total += af - ai;
                atual = Some((i, f));
            }
            None => atual = Some((i, f)),
        }
    }
    if let Some((ai, af)) = atual {
        total += af - ai;
    }
    total
}

/// Os totais do dia. Por timer e atividade, a união dos trechos de cada atividade: dentro de uma
/// atividade, o Claude, a leitura e o ▶ sobrepostos contam uma vez só. Por timer, a **soma** das
/// atividades dele: cada atividade é um lançamento à parte, então o Claude trabalhando das 9:00 às
/// 11:00 durante uma reunião das 10:00 às 11:10 dá 3h 10min no timer (2h + 1h 10min). O dia, a
/// união de tudo (o tempo real): ele pode ficar menor que a soma dos timers.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Totais {
    pub dia_ms: Ms,
    /// Timer → a soma das atividades dele.
    pub por_timer: BTreeMap<i64, Ms>,
    /// (timer, atividade) → tempo.
    pub por_atividade: BTreeMap<(i64, i64), Ms>,
    /// Timer → o tempo real dele (a união dos trechos, sem dobrar entre atividades; com os
    /// ajustes, `real_ajustado`). É o que o timer põe no dia.
    pub real_por_timer: BTreeMap<i64, Ms>,
}

/// Totais em `[de, ate)`. Trecho aberto conta ate `agora`; ignora descartados e sem timer.
pub fn totais(trechos: &[Trecho], de: Ms, ate: Ms, agora: Ms) -> Totais {
    let mut todos: Vec<(Ms, Ms)> = Vec::new();
    let mut por: BTreeMap<i64, Vec<(Ms, Ms)>> = BTreeMap::new();
    let mut por_atividade: BTreeMap<(i64, i64), Vec<(Ms, Ms)>> = BTreeMap::new();
    for t in trechos {
        let Some(timer) = t.timer_id else { continue };
        if t.descartado {
            continue;
        }
        let i = t.inicio.max(de);
        let f = t.fim.unwrap_or(agora).min(ate);
        if f <= i {
            continue;
        }
        todos.push((i, f));
        por.entry(timer).or_default().push((i, f));
        por_atividade.entry((timer, t.atividade_id)).or_default().push((i, f));
    }
    let por_atividade: BTreeMap<(i64, i64), Ms> = por_atividade.into_iter().map(|(k, v)| (k, uniao_ms(&v))).collect();
    Totais {
        dia_ms: uniao_ms(&todos),
        por_timer: somar_por_timer(&por_atividade),
        por_atividade,
        real_por_timer: por.into_iter().map(|(k, v)| (k, uniao_ms(&v))).collect(),
    }
}

/// Timer → a soma das atividades dele.
fn somar_por_timer(por_atividade: &BTreeMap<(i64, i64), Ms>) -> BTreeMap<i64, Ms> {
    let mut v: BTreeMap<i64, Ms> = BTreeMap::new();
    for ((timer, _), ms) in por_atividade {
        *v.entry(*timer).or_default() += ms;
    }
    v
}

/// O que o ajuste à mão `soma` muda numa atividade com `uniao` de tempo contado: nunca tira mais
/// do que ela tem (o total dela fica em zero, não abaixo).
pub fn ajuste_efetivo(uniao: Ms, soma: Ms) -> Ms {
    soma.max(-uniao)
}

/// O tempo real do timer com os ajustes (o que ele põe no dia): a união dos trechos mais o que os
/// ajustes mudaram nas atividades dele (`mudou`), nunca abaixo da maior atividade dele já ajustada
/// nem de zero. Com atividades sobrepostas (o Claude durante a reunião), tirar o tempo de uma não
/// tira do dia a parte que a outra continua cobrindo: −1h 10min na reunião do Cliente A deixa no dia as
/// 2h do Claude.
pub fn real_ajustado(uniao: Ms, mudou: Ms, maior_atividade: Ms) -> Ms {
    (uniao + mudou).max(maior_atividade).max(0)
}

/// Totais com os ajustes à mão do dia. Cada ajuste soma na atividade dele: por timer e atividade,
/// a união dos trechos daquela atividade mais a soma dos ajustes nela, nunca abaixo de zero. O
/// timer, a soma das atividades já ajustadas. O dia, a união de todos mais o que os ajustes mudaram
/// no tempo real de cada timer (`real_ajustado` menos a união dele), nunca abaixo do maior tempo
/// real de timer (com timers sobrepostos, um ajuste negativo poderia deixar o dia menor que o
/// tempo real de um deles) nem de zero. Timer sem trecho e com ajuste positivo entra com o ajuste.
pub fn com_ajustes(mut t: Totais, ajustes: &[Ajuste]) -> Totais {
    let mut soma: BTreeMap<(i64, i64), Ms> = BTreeMap::new();
    for a in ajustes {
        *soma.entry((a.timer_id, a.atividade_id)).or_default() += a.delta_ms;
    }
    let mut no_timer: BTreeMap<i64, Ms> = BTreeMap::new();
    for (chave, s) in soma {
        let base = t.por_atividade.get(&chave).copied().unwrap_or(0);
        let efetivo = ajuste_efetivo(base, s);
        t.por_atividade.insert(chave, base + efetivo);
        *no_timer.entry(chave.0).or_default() += efetivo;
    }
    for (timer, s) in no_timer {
        let base = t.real_por_timer.get(&timer).copied().unwrap_or(0);
        let maior = t.por_atividade.range((timer, i64::MIN)..=(timer, i64::MAX)).map(|(_, v)| *v).max().unwrap_or(0);
        let real = real_ajustado(base, s, maior);
        t.real_por_timer.insert(timer, real);
        t.dia_ms += real - base;
    }
    t.por_timer = somar_por_timer(&t.por_atividade);
    let maior = t.real_por_timer.values().copied().max().unwrap_or(0);
    t.dia_ms = t.dia_ms.max(maior).max(0);
    t
}

/// O acumulado desde o último "Zerar" de cada timer (rodada 11): o número do timer não volta a
/// zero na virada do dia. As contas são as do dia, feitas dia a dia e somadas: dentro de uma
/// atividade, a união; cada atividade com os seus ajustes do dia, nunca negativa; o timer, a soma
/// das atividades; o grupo, a soma dos timers, e o tempo real do grupo, a união dos timers dele
/// (cada dia como o total do dia, `com_ajustes`).
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Acumulado {
    /// (timer, atividade) → tempo.
    pub por_atividade: BTreeMap<(i64, i64), Ms>,
    /// Grupo (`None` = sem grupo) → o tempo real dos timers dele no período de cada um.
    pub real_por_grupo: BTreeMap<Option<i64>, Ms>,
    /// Espaço → o tempo real dos timers dele no período de cada um (rodada 20): a união de cada
    /// dia, como a do grupo, com os timers do espaço.
    pub real_por_espaco: BTreeMap<i64, Ms>,
}

impl Acumulado {
    /// Timer → a soma das atividades dele.
    pub fn por_timer(&self) -> BTreeMap<i64, Ms> {
        somar_por_timer(&self.por_atividade)
    }
}

/// Os "Zerar" (rodada 19): o de cada timer (o ↺ do timer todo, o "Zerar todos" e a virada) e o
/// de cada atividade de um timer (o ↺ só de uma atividade). Uma atividade conta desde o mais
/// recente dos dois; o timer é a soma das atividades, cada uma desde o seu zerar.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Zerados {
    /// Timer → o último zerar do timer todo (sem: 0, "desde sempre").
    pub timers: BTreeMap<i64, Ms>,
    /// (timer, atividade) → o último zerar só daquela atividade.
    pub atividades: BTreeMap<(i64, i64), Ms>,
}

impl Zerados {
    /// O último zerar do timer todo.
    pub fn do_timer(&self, timer: i64) -> Ms {
        self.timers.get(&timer).copied().unwrap_or(0)
    }

    /// Desde quando a atividade do timer conta: o mais recente entre o zerar do timer e o dela.
    pub fn da_atividade(&self, timer: i64, atividade: i64) -> Ms {
        self.do_timer(timer).max(self.atividades.get(&(timer, atividade)).copied().unwrap_or(0))
    }
}

impl From<BTreeMap<i64, Ms>> for Zerados {
    fn from(timers: BTreeMap<i64, Ms>) -> Zerados {
        Zerados { timers, atividades: BTreeMap::new() }
    }
}

/// O acumulado (`Acumulado`) dos `timers` dados (timer → grupo), cada atividade desde o seu zerar
/// (`Zerados::da_atividade`),
/// até `ate`; trecho aberto conta até `agora`. Dos trechos, só a parte depois do zerar (o trecho
/// que atravessa o instante conta a partir dele); dos ajustes, os criados depois do zerar da
/// atividade deles, cada um
/// no dia dele, e só os dos dias que começam antes de `ate`. Ignora os descartados, os sem timer e
/// os de timer fora de `timers`. Com `ate` no início de hoje, é o que os dias fechados somam.
/// `timers` leva a chave do grupo de cada timer; `espacos`, o espaço de cada um (um timer fora
/// dele não entra no `real_por_espaco`).
#[allow(clippy::too_many_arguments)]
pub fn acumulado(
    trechos: &[Trecho],
    ajustes: &[Ajuste],
    zerados: &Zerados,
    timers: &BTreeMap<i64, Option<i64>>,
    espacos: &BTreeMap<i64, i64>,
    ate: Ms,
    agora: Ms,
    corte_min: u32,
    fuso_min: i32,
) -> Acumulado {
    // dia → (pedaços de trecho no dia, ajustes do dia)
    let mut por_dia: BTreeMap<NaiveDate, (Vec<Trecho>, Vec<Ajuste>)> = BTreeMap::new();
    for t in trechos {
        let Some(timer) = t.timer_id.filter(|id| timers.contains_key(id)) else { continue };
        if t.descartado {
            continue;
        }
        let mut i = t.inicio.max(zerados.da_atividade(timer, t.atividade_id));
        let f = t.fim.unwrap_or(agora).min(ate);
        // o trecho que atravessa um corte do dia (não deveria) é dividido nele
        while i < f {
            let fim_do_dia = proximo_corte(i, corte_min, fuso_min).min(f);
            let pedaco = Trecho { inicio: i, fim: Some(fim_do_dia), ..t.clone() };
            por_dia.entry(dia_de(i, corte_min, fuso_min)).or_default().0.push(pedaco);
            i = fim_do_dia;
        }
    }
    for a in ajustes {
        if !timers.contains_key(&a.timer_id) || a.criado_em < zerados.da_atividade(a.timer_id, a.atividade_id) {
            continue;
        }
        let Ok(dia) = NaiveDate::parse_from_str(&a.dia, "%Y-%m-%d") else { continue };
        if inicio_do_dia(dia, corte_min, fuso_min) < ate {
            por_dia.entry(dia).or_default().1.push(a.clone());
        }
    }
    let mut r = Acumulado::default();
    let grupo = |timer: i64| timers.get(&timer).copied().flatten();
    for (dia, (pedacos, ajs)) in por_dia {
        let de = inicio_do_dia(dia, corte_min, fuso_min);
        let fim = proximo_corte(de, corte_min, fuso_min);
        let t = com_ajustes(totais(&pedacos, de, fim, agora), &ajs);
        for (k, ms) in t.por_atividade {
            *r.por_atividade.entry(k).or_default() += ms;
        }
        // o tempo real de cada grupo no dia: a conta do total do dia, só com os timers dele
        let mut grupos: BTreeMap<Option<i64>, (Vec<Trecho>, Vec<Ajuste>)> = BTreeMap::new();
        for p in pedacos {
            grupos.entry(grupo(p.timer_id.unwrap_or_default())).or_default().0.push(p);
        }
        for a in ajs {
            grupos.entry(grupo(a.timer_id)).or_default().1.push(a);
        }
        // o do espaço, do mesmo jeito, com os timers dele
        let mut por_espaco: BTreeMap<i64, (Vec<Trecho>, Vec<Ajuste>)> = BTreeMap::new();
        for (ps, aj) in grupos.values() {
            for p in ps {
                if let Some(e) = p.timer_id.and_then(|t| espacos.get(&t)) {
                    por_espaco.entry(*e).or_default().0.push(p.clone());
                }
            }
            for a in aj {
                if let Some(e) = espacos.get(&a.timer_id) {
                    por_espaco.entry(*e).or_default().1.push(a.clone());
                }
            }
        }
        for (g, (ps, aj)) in grupos {
            *r.real_por_grupo.entry(g).or_default() += com_ajustes(totais(&ps, de, fim, agora), &aj).dia_ms;
        }
        for (e, (ps, aj)) in por_espaco {
            *r.real_por_espaco.entry(e).or_default() += com_ajustes(totais(&ps, de, fim, agora), &aj).dia_ms;
        }
    }
    r
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::modelo::*;
    use crate::tempo::*;
    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms { ms_local(2026, 11, 17, hh, mm, F) }

    #[test] fn uniao_nao_conta_em_dobro() {
        assert_eq!(uniao_ms(&[(h(9, 0), h(10, 0)), (h(9, 30), h(10, 30))]), 90 * MIN);
        assert_eq!(uniao_ms(&[(h(9, 0), h(9, 10)), (h(11, 0), h(11, 5))]), 15 * MIN);
    }
    #[test] fn totais_ignoram_descartados_e_sem_timer_e_recortam_o_dia() {
        let tr = |id, timer: Option<i64>, i, f: Option<Ms>, desc| Trecho { id, timer_id: timer, sessao_id: None, origem: Origem::Manual, inicio: i, fim: f, motivo_fim: None, descartado: desc, atividade_id: 1 };
        let v = vec![
            tr(1, Some(1), h(9, 0), Some(h(10, 0)), false),
            tr(2, Some(2), h(9, 30), Some(h(10, 30)), false),
            tr(3, Some(1), h(11, 0), Some(h(12, 0)), true),
            tr(4, None, h(13, 0), Some(h(14, 0)), false),
            tr(5, Some(2), h(23, 0), None, false),
        ];
        let t = totais(&v, h(0, 0), ms_local(2026, 11, 18, 0, 0, F), h(23, 20));
        assert_eq!(t.por_timer[&1], 60 * MIN);
        assert_eq!(t.por_timer[&2], 60 * MIN + 20 * MIN);
        assert_eq!(t.dia_ms, 90 * MIN + 20 * MIN);
    }

    fn aj(timer_id: i64, delta_ms: Ms) -> Ajuste {
        Ajuste { id: 0, timer_id, dia: "2026-11-17".into(), delta_ms, motivo: None, criado_em: 0, atividade_id: 1 }
    }
    fn manual(id: i64, timer: i64, i: Ms, f: Ms) -> Trecho {
        Trecho { id, timer_id: Some(timer), sessao_id: None, origem: Origem::Manual, inicio: i, fim: Some(f), motivo_fim: None, descartado: false, atividade_id: 1 }
    }

    #[test] fn ajustes_somam_no_timer_e_no_dia() {
        // A 9:00–10:00, B 9:30–10:30 (dia: 1h30 de uniao)
        let v = vec![manual(1, 1, h(9, 0), h(10, 0)), manual(2, 2, h(9, 30), h(10, 30))];
        let base = totais(&v, h(0, 0), h(23, 0), h(23, 0));
        let t = com_ajustes(base.clone(), &[aj(1, 15 * MIN), aj(1, -5 * MIN), aj(2, -20 * MIN)]);
        assert_eq!(t.por_timer[&1], 70 * MIN);
        assert_eq!(t.por_timer[&2], 40 * MIN);
        assert_eq!(t.dia_ms, 90 * MIN + 10 * MIN - 20 * MIN);
        // sem ajustes, nada muda
        assert_eq!(com_ajustes(base, &[]), totais(&v, h(0, 0), h(23, 0), h(23, 0)));
    }

    #[test] fn ajuste_nunca_deixa_o_timer_nem_o_dia_abaixo_de_zero() {
        let v = vec![manual(1, 1, h(9, 0), h(9, 10)), manual(2, 2, h(11, 0), h(12, 0))];
        let t = com_ajustes(totais(&v, h(0, 0), h(23, 0), h(23, 0)), &[aj(1, -30 * MIN)]);
        // A tinha 10 min: fica em zero, e o dia perde so esses 10 min
        assert_eq!(t.por_timer[&1], 0);
        assert_eq!(t.por_timer[&2], 60 * MIN);
        assert_eq!(t.dia_ms, 60 * MIN);
        // timers que se sobrepoem, os dois tirados: o dia para em zero
        let v = vec![manual(1, 1, h(9, 0), h(10, 0)), manual(2, 2, h(9, 0), h(10, 0))];
        let t = com_ajustes(totais(&v, h(0, 0), h(23, 0), h(23, 0)), &[aj(1, -60 * MIN), aj(2, -60 * MIN)]);
        assert_eq!((t.por_timer[&1], t.por_timer[&2], t.dia_ms), (0, 0, 0));
        assert_eq!(ajuste_efetivo(10 * MIN, -30 * MIN), -10 * MIN);
        assert_eq!(ajuste_efetivo(10 * MIN, 30 * MIN), 30 * MIN);
    }

    #[test] fn dia_nunca_fica_abaixo_do_maior_timer() {
        // A e B das 9:00 as 10:00 (uniao 60 min); A perde 60 min: A 0, B 60, o dia fica em 60, nao 0
        let v = vec![manual(1, 1, h(9, 0), h(10, 0)), manual(2, 2, h(9, 0), h(10, 0))];
        let t = com_ajustes(totais(&v, h(0, 0), h(23, 0), h(23, 0)), &[aj(1, -60 * MIN)]);
        assert_eq!((t.por_timer[&1], t.por_timer[&2], t.dia_ms), (0, 60 * MIN, 60 * MIN));
        // B ainda ganha 15: o dia acompanha o maior timer (75)
        let t = com_ajustes(totais(&v, h(0, 0), h(23, 0), h(23, 0)), &[aj(1, -60 * MIN), aj(2, 15 * MIN)]);
        assert_eq!((t.por_timer[&2], t.dia_ms), (75 * MIN, 75 * MIN));
    }

    // ---- atividades (rodada 9) ----

    const W: i64 = ATIVIDADE_TRABALHO;
    const R: i64 = ATIVIDADE_REUNIAO;
    fn tr(id: i64, timer: i64, origem: Origem, atividade_id: i64, i: Ms, f: Option<Ms>) -> Trecho {
        Trecho { id, timer_id: Some(timer), sessao_id: None, origem, inicio: i, fim: f, motivo_fim: None, descartado: false, atividade_id }
    }
    fn aj_em(timer_id: i64, atividade_id: i64, delta_ms: Ms) -> Ajuste {
        Ajuste { atividade_id, ..aj(timer_id, delta_ms) }
    }

    /// Cliente A: o Claude trabalhando das 9:00 às 11:00 e uma reunião manual das 10:00 às 11:10. O
    /// trabalho 2h, a reunião 1h 10min, e o timer a soma das duas (3h 10min: dois lançamentos);
    /// o dia é a união de tudo.
    #[test] fn claude_e_reuniao_sobrepostos_no_mesmo_timer() {
        let v = vec![
            tr(1, 1, Origem::Claude, W, h(9, 0), Some(h(11, 0))),
            tr(2, 1, Origem::Manual, R, h(10, 0), Some(h(11, 10))),
            // outro timer, das 10:30 às 12:00
            tr(3, 2, Origem::Manual, W, h(10, 30), Some(h(12, 0))),
        ];
        let t = totais(&v, h(0, 0), h(23, 0), h(23, 0));
        assert_eq!(t.por_timer[&1], 190 * MIN, "2h + 1h 10min");
        assert_eq!(t.real_por_timer[&1], 130 * MIN, "a união, sem dobrar");
        assert_eq!(t.por_atividade[&(1, W)], 120 * MIN);
        assert_eq!(t.por_atividade[&(1, R)], 70 * MIN);
        assert_eq!(t.por_atividade[&(2, W)], 90 * MIN);
        assert_eq!(t.dia_ms, 180 * MIN, "9:00 às 12:00");
        // trecho aberto conta até agora, na atividade dele
        let v = vec![tr(1, 1, Origem::Claude, W, h(9, 0), Some(h(10, 0))), tr(2, 1, Origem::Manual, R, h(9, 30), None)];
        let t = totais(&v, h(0, 0), h(23, 0), h(10, 30));
        assert_eq!((t.por_timer[&1], t.por_atividade[&(1, W)], t.por_atividade[&(1, R)]), (120 * MIN, 60 * MIN, 60 * MIN));
        assert_eq!((t.real_por_timer[&1], t.dia_ms), (90 * MIN, 90 * MIN));
    }

    #[test] fn ajuste_soma_na_atividade_e_no_timer_nunca_negativo() {
        let v = vec![tr(1, 1, Origem::Claude, W, h(9, 0), Some(h(11, 0))), tr(2, 1, Origem::Manual, R, h(10, 0), Some(h(11, 10)))];
        let base = totais(&v, h(0, 0), h(23, 0), h(23, 0));
        // +20 de reunião (longe do PC): reunião 1h 30, timer 3h 30 (2h + 1h 30), dia 2h 30
        let t = com_ajustes(base.clone(), &[aj_em(1, R, 20 * MIN)]);
        assert_eq!((t.por_atividade[&(1, R)], t.por_atividade[&(1, W)], t.por_timer[&1], t.dia_ms), (90 * MIN, 120 * MIN, 210 * MIN, 150 * MIN));
        // tirar 2h da reunião: ela para em zero (só tinha 1h 10), e o timer e o dia ficam com as
        // 2h do Claude, que existiram (a reunião só cobria sozinha os 10 min depois das 11:00)
        let t = com_ajustes(base.clone(), &[aj_em(1, R, -120 * MIN)]);
        assert_eq!((t.por_atividade[&(1, R)], t.por_timer[&1], t.dia_ms), (0, 120 * MIN, 120 * MIN));
        // tirar tudo das duas (sobrepostas): cada uma para em zero e o timer também, não abaixo
        let t = com_ajustes(base.clone(), &[aj_em(1, R, -70 * MIN), aj_em(1, W, -120 * MIN)]);
        assert_eq!((t.por_atividade[&(1, R)], t.por_atividade[&(1, W)], t.por_timer[&1], t.dia_ms), (0, 0, 0, 0));
        // ajuste numa atividade sem tempo: positivo entra, negativo não tira nada
        let t = com_ajustes(base, &[aj_em(1, ATIVIDADE_ESTUDO, 15 * MIN), aj_em(1, ATIVIDADE_ESTUDO, -60 * MIN)]);
        assert_eq!((t.por_atividade[&(1, ATIVIDADE_ESTUDO)], t.por_timer[&1], t.dia_ms), (0, 190 * MIN, 130 * MIN));
        let t = com_ajustes(Totais::default(), &[aj_em(1, ATIVIDADE_ESTUDO, 15 * MIN)]);
        assert_eq!((t.por_atividade[&(1, ATIVIDADE_ESTUDO)], t.por_timer[&1], t.dia_ms), (15 * MIN, 15 * MIN, 15 * MIN));
    }

    /// A decisão da rodada 10: o Claude das 9:00 às 11:00 e a reunião das 10:00 às 11:10. O timer
    /// é a soma dos dois lançamentos (3h 10min), o dia a união (2h 10min). Com −1h 10min em
    /// Reunião: Reunião 0, timer 2h, dia 2h.
    #[test] fn timer_soma_as_atividades_e_o_dia_e_a_uniao() {
        let v = vec![tr(1, 1, Origem::Claude, W, h(9, 0), Some(h(11, 0))), tr(2, 1, Origem::Manual, R, h(10, 0), Some(h(11, 10)))];
        let base = totais(&v, h(0, 0), h(23, 0), h(23, 0));
        assert_eq!((base.por_timer[&1], base.dia_ms), (190 * MIN, 130 * MIN));
        // sem ajustes, nada muda
        assert_eq!(com_ajustes(base.clone(), &[]), base);
        let t = com_ajustes(base, &[aj_em(1, R, -70 * MIN)]);
        assert_eq!((t.por_atividade[&(1, R)], t.por_timer[&1], t.dia_ms), (0, 120 * MIN, 120 * MIN));
        // o timer pode passar do dia (sem a regra antiga da "maior atividade")
        assert!(com_ajustes(totais(&v, h(0, 0), h(23, 0), h(23, 0)), &[aj_em(1, W, 30 * MIN)]).por_timer[&1] > 190 * MIN);
    }

    /// O caso da revisão: Claude 9:00–11:00, reunião 10:00–11:10 (que não aconteceu) e −1h 10min
    /// em Reunião. Reunião 0, Trabalho 2h, timer 2h; o dia (com outro timer das 12:00 às 13:00)
    /// perde só os 10 min que eram só da reunião.
    #[test] fn ajuste_negativo_na_reuniao_nao_tira_as_horas_do_claude() {
        let v = vec![
            tr(1, 1, Origem::Claude, W, h(9, 0), Some(h(11, 0))),
            tr(2, 1, Origem::Manual, R, h(10, 0), Some(h(11, 10))),
            tr(3, 2, Origem::Manual, W, h(12, 0), Some(h(13, 0))),
        ];
        let base = totais(&v, h(0, 0), h(23, 0), h(23, 0));
        assert_eq!(base.dia_ms, 190 * MIN);
        let t = com_ajustes(base, &[aj_em(1, R, -70 * MIN)]);
        assert_eq!(
            (t.por_atividade[&(1, R)], t.por_atividade[&(1, W)], t.por_timer[&1], t.por_timer[&2], t.dia_ms),
            (0, 120 * MIN, 120 * MIN, 60 * MIN, 180 * MIN)
        );
        assert_eq!(real_ajustado(130 * MIN, -70 * MIN, 120 * MIN), 120 * MIN);
        assert_eq!(real_ajustado(130 * MIN, 20 * MIN, 120 * MIN), 150 * MIN);
        assert_eq!(real_ajustado(10 * MIN, -60 * MIN, 0), 0);
    }

    #[test] fn timer_sem_trecho_com_ajuste_entra_no_total() {
        let t = com_ajustes(Totais::default(), &[aj(7, 45 * MIN), aj(8, -10 * MIN)]);
        assert_eq!((t.por_timer[&7], t.por_timer[&8], t.dia_ms), (45 * MIN, 0, 45 * MIN));
    }

    // ---- acumulado desde o zerar (rodada 11) ----

    fn d(dia: u32, hh: u32, mm: u32) -> Ms { ms_local(2026, 11, dia, hh, mm, F) }
    fn aj_dia(timer_id: i64, atividade_id: i64, delta_ms: Ms, dia: &str, criado_em: Ms) -> Ajuste {
        Ajuste { id: 0, timer_id, dia: dia.into(), delta_ms, motivo: None, criado_em, atividade_id }
    }
    fn so(timers: &[(i64, Option<i64>)]) -> BTreeMap<i64, Option<i64>> { timers.iter().copied().collect() }
    fn zer(v: &[(i64, Ms)]) -> Zerados { Zerados::from(v.iter().copied().collect::<BTreeMap<_, _>>()) }

    #[test] fn acumulado_soma_os_dias_com_as_contas_de_cada_dia() {
        // dia 16: Claude 9:00–11:00 e reunião 10:00–11:10; dia 17: ▶ 9:00–10:00 e o Claude desde 14:00
        let v = vec![
            tr(1, 1, Origem::Claude, W, d(16, 9, 0), Some(d(16, 11, 0))),
            tr(2, 1, Origem::Manual, R, d(16, 10, 0), Some(d(16, 11, 10))),
            tr(3, 1, Origem::Manual, W, d(17, 9, 0), Some(d(17, 10, 0))),
            tr(4, 1, Origem::Claude, W, d(17, 14, 0), None),
        ];
        let z = zer(&[(1, d(16, 0, 0))]);
        let agora = d(17, 15, 0);
        let tudo = acumulado(&v, &[], &z, &so(&[(1, None)]), &BTreeMap::new(), d(18, 0, 0), agora, 0, F);
        assert_eq!((tudo.por_atividade[&(1, W)], tudo.por_atividade[&(1, R)]), (4 * 60 * MIN, 70 * MIN));
        assert_eq!(tudo.por_timer()[&1], 5 * 60 * MIN + 10 * MIN, "a soma das atividades");
        assert_eq!(tudo.real_por_grupo[&None], 130 * MIN + 2 * 60 * MIN, "o tempo real: a união de cada dia");
        // só os dias fechados (o que a tela recebe; hoje ela soma)
        let antes = acumulado(&v, &[], &z, &so(&[(1, None)]), &BTreeMap::new(), d(17, 0, 0), agora, 0, F);
        assert_eq!(antes.por_timer()[&1], 190 * MIN);
        // zerado hoje: os dias fechados não somam nada
        let hoje = acumulado(&v, &[], &zer(&[(1, d(17, 0, 0))]), &so(&[(1, None)]), &BTreeMap::new(), d(17, 0, 0), agora, 0, F);
        assert_eq!(hoje, Acumulado::default());
    }

    #[test] fn trecho_que_atravessa_o_zerar_conta_so_depois_dele() {
        // zerado às 10:00 com o Claude desde 9:00 (fechou às 11:00) e o ▶ aberto desde 9:30
        let v = vec![
            tr(1, 1, Origem::Claude, W, d(17, 9, 0), Some(d(17, 11, 0))),
            tr(2, 1, Origem::Manual, W, d(17, 9, 30), None),
        ];
        let a = acumulado(&v, &[], &zer(&[(1, d(17, 10, 0))]), &so(&[(1, None)]), &BTreeMap::new(), d(18, 0, 0), d(17, 12, 0), 0, F);
        assert_eq!(a.por_timer()[&1], 2 * 60 * MIN, "10:00 às 12:00, e não 3h");
        // o trecho que passa da meia-noite (não deveria) fica em dois dias
        let v = vec![tr(1, 1, Origem::Manual, W, d(16, 23, 0), Some(d(17, 1, 0)))];
        let a = acumulado(&v, &[aj_dia(1, W, -90 * MIN, "2026-11-16", d(16, 23, 30))], &zer(&[(1, 0)]), &so(&[(1, None)]), &BTreeMap::new(), d(18, 0, 0), d(17, 12, 0), 0, F);
        assert_eq!(a.por_timer()[&1], 60 * MIN, "o −1h 30min do dia 16 tira só a hora daquele dia");
    }

    #[test] fn ajuste_conta_so_se_feito_depois_do_zerar_e_nunca_deixa_um_dia_negativo() {
        let v = vec![
            tr(1, 1, Origem::Manual, W, d(16, 9, 0), Some(d(16, 10, 0))),
            tr(2, 1, Origem::Manual, W, d(17, 10, 0), Some(d(17, 11, 0))),
        ];
        let ajs = [
            aj_dia(1, W, -2 * 60 * MIN, "2026-11-16", d(16, 18, 0)),
            aj_dia(1, W, 30 * MIN, "2026-11-17", d(17, 9, 0)),
            aj_dia(1, W, 15 * MIN, "2026-11-17", d(17, 10, 30)),
            aj_dia(1, R, 20 * MIN, "2026-11-17", d(17, 10, 40)),
        ];
        let timers = so(&[(1, None)]);
        // zerado às 10:00 do 17: fica o dia 17 depois do zerar, com os dois ajustes feitos depois
        let a = acumulado(&v, &ajs, &zer(&[(1, d(17, 10, 0))]), &timers, &BTreeMap::new(), d(18, 0, 0), d(17, 12, 0), 0, F);
        assert_eq!((a.por_atividade[&(1, W)], a.por_atividade[&(1, R)]), (75 * MIN, 20 * MIN));
        // zerado no dia 16: o −2h do dia 16 para em zero naquele dia, e o 17 conta inteiro
        let a = acumulado(&v, &ajs, &zer(&[(1, d(16, 0, 0))]), &timers, &BTreeMap::new(), d(18, 0, 0), d(17, 12, 0), 0, F);
        assert_eq!(a.por_atividade[&(1, W)], 60 * MIN + 45 * MIN);
        // só os dias fechados: os ajustes de hoje ficam para a tela
        let a = acumulado(&v, &ajs, &zer(&[(1, d(16, 0, 0))]), &timers, &BTreeMap::new(), d(17, 0, 0), d(17, 12, 0), 0, F);
        assert_eq!(a.por_timer()[&1], 0);
    }

    #[test] fn zerar_so_uma_atividade_nos_dias_fechados_mantem_as_outras() {
        // dia 16: trabalho das 9:00 às 11:00 e reunião das 10:00 às 11:00 (sobrepostas)
        let v = vec![
            tr(1, 1, Origem::Claude, W, d(16, 9, 0), Some(d(16, 11, 0))),
            tr(2, 1, Origem::Manual, R, d(16, 10, 0), Some(d(16, 11, 0))),
        ];
        let ajs = [aj_dia(1, R, 10 * MIN, "2026-11-16", d(16, 10, 15)), aj_dia(1, R, 5 * MIN, "2026-11-16", d(16, 10, 45))];
        // a reunião zerada às 10:30; o timer, desde sempre
        let mut z = zer(&[(1, 0)]);
        z.atividades.insert((1, R), d(16, 10, 30));
        assert_eq!((z.da_atividade(1, R), z.da_atividade(1, W)), (d(16, 10, 30), 0));
        let a = acumulado(&v, &ajs, &z, &so(&[(1, Some(10))]), &BTreeMap::new(), d(17, 0, 0), d(17, 9, 0), 0, F);
        assert_eq!(a.por_atividade[&(1, W)], 2 * 60 * MIN, "o trabalho fica inteiro");
        assert_eq!(a.por_atividade[&(1, R)], 30 * MIN + 5 * MIN, "a reunião desde as 10:30, só com o ajuste feito depois");
        assert_eq!(a.por_timer()[&1], 155 * MIN, "o timer, a soma das atividades");
        assert_eq!(a.real_por_grupo[&Some(10)], 2 * 60 * MIN + 5 * MIN, "o tempo real do grupo: a união do que conta");
        // o zerar do timer mais novo que o da atividade vence
        z.timers.insert(1, d(16, 10, 40));
        let a = acumulado(&v, &ajs, &z, &so(&[(1, None)]), &BTreeMap::new(), d(17, 0, 0), d(17, 9, 0), 0, F);
        assert_eq!((a.por_atividade[&(1, W)], a.por_atividade[&(1, R)]), (20 * MIN, 20 * MIN + 5 * MIN));
    }

    #[test] fn tempo_real_do_espaco_e_a_uniao_dos_timers_dele() {
        // dia 16: A (espaço 1) das 9:00 às 10:00, B (espaço 1, sem grupo) das 9:30 às 10:30 e C
        // (espaço 2) das 9:00 às 9:20; um ajuste de +10 min no C
        let v = vec![
            tr(1, 1, Origem::Manual, W, d(16, 9, 0), Some(d(16, 10, 0))),
            tr(2, 2, Origem::Manual, W, d(16, 9, 30), Some(d(16, 10, 30))),
            tr(3, 3, Origem::Manual, W, d(16, 9, 0), Some(d(16, 9, 20))),
        ];
        let z = zer(&[(1, 0), (2, 0), (3, 0)]);
        let grupos = so(&[(1, Some(10)), (2, Some(-1)), (3, Some(-2))]);
        let espacos: BTreeMap<i64, i64> = [(1, 1), (2, 1), (3, 2)].into();
        let ajs = [aj_dia(3, W, 10 * MIN, "2026-11-16", d(16, 12, 0))];
        let a = acumulado(&v, &ajs, &z, &grupos, &espacos, d(17, 0, 0), d(17, 9, 0), 0, F);
        assert_eq!(a.real_por_espaco[&1], 90 * MIN, "a união de A e B, e não 2h");
        assert_eq!(a.real_por_espaco[&2], 30 * MIN, "com o ajuste");
        // o espaço é a soma dos timers dele no número; o tempo real, a união
        assert_eq!(a.por_timer()[&1] + a.por_timer()[&2], 2 * 60 * MIN);
        // o "Sem grupo" de cada espaço tem a sua chave
        assert_eq!((a.real_por_grupo[&Some(-1)], a.real_por_grupo[&Some(-2)]), (60 * MIN, 30 * MIN));
    }

    #[test] fn tempo_real_do_grupo_e_a_uniao_dos_timers_cada_um_desde_o_seu_zerar() {
        // A e B das 9:00 às 10:00 do dia 16, no mesmo grupo; B zerado às 9:30; C noutro grupo
        let v = vec![
            tr(1, 1, Origem::Manual, W, d(16, 9, 0), Some(d(16, 10, 0))),
            tr(2, 2, Origem::Manual, W, d(16, 9, 0), Some(d(16, 10, 0))),
            tr(3, 3, Origem::Manual, W, d(16, 9, 0), Some(d(16, 9, 20))),
            tr(4, 4, Origem::Manual, W, d(16, 9, 0), Some(d(16, 9, 20))),
        ];
        let z = zer(&[(1, 0), (2, d(16, 9, 30)), (3, 0), (4, 0)]);
        // o 4 não está na tela (arquivado): fica fora
        let a = acumulado(&v, &[], &z, &so(&[(1, Some(10)), (2, Some(10)), (3, None)]), &BTreeMap::new(), d(17, 0, 0), d(17, 9, 0), 0, F);
        assert_eq!((a.por_timer()[&1], a.por_timer()[&2]), (60 * MIN, 30 * MIN));
        assert_eq!(a.real_por_grupo[&Some(10)], 60 * MIN, "a soma é 1h 30min; o tempo real, 1h");
        assert_eq!(a.real_por_grupo[&None], 20 * MIN);
        assert!(!a.por_timer().contains_key(&4));
    }
}
