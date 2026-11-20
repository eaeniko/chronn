// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
use chrono::{DateTime, Days, NaiveDate};

pub type Ms = i64;
pub const MIN: Ms = 60_000;

/// Instante (ms UTC) de uma data/hora local com o fuso dado em minutos (-180 = UTC-3).
pub fn ms_local(ano: i32, mes: u32, dia: u32, h: u32, m: u32, fuso_min: i32) -> Ms {
    let naive = NaiveDate::from_ymd_opt(ano, mes, dia)
        .and_then(|d| d.and_hms_opt(h, m, 0))
        .expect("data/hora local invalida");
    naive.and_utc().timestamp_millis() - fuso_min as i64 * MIN
}

/// Dia (de trabalho) a que o instante pertence, considerando fuso e corte em minutos apos a meia-noite local.
pub fn dia_de(t: Ms, corte_min: u32, fuso_min: i32) -> NaiveDate {
    let deslocado = t + fuso_min as i64 * MIN - corte_min as i64 * MIN;
    DateTime::from_timestamp_millis(deslocado)
        .expect("instante fora do intervalo")
        .date_naive()
}

/// Instante em que o dia de trabalho comeca (meia-noite local + corte).
pub fn inicio_do_dia(dia: NaiveDate, corte_min: u32, fuso_min: i32) -> Ms {
    dia.and_hms_opt(0, 0, 0).unwrap().and_utc().timestamp_millis()
        + corte_min as i64 * MIN
        - fuso_min as i64 * MIN
}

/// Primeiro instante do dia de trabalho seguinte ao de `t`.
pub fn proximo_corte(t: Ms, corte_min: u32, fuso_min: i32) -> Ms {
    let prox = dia_de(t, corte_min, fuso_min)
        .checked_add_days(Days::new(1))
        .expect("data fora do intervalo");
    inicio_do_dia(prox, corte_min, fuso_min)
}


#[cfg(test)]
mod testes {
    use super::*;
    const F: i32 = -180;
    fn h(hh: u32, mm: u32) -> Ms { ms_local(2026, 11, 17, hh, mm, F) }

    #[test] fn dia_de_respeita_fuso_e_corte() {
        let d17 = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        let d18 = NaiveDate::from_ymd_opt(2026, 11, 18).unwrap();
        assert_eq!(dia_de(h(23, 30), 0, F), d17);
        assert_eq!(dia_de(ms_local(2026, 11, 18, 1, 30, F), 0, F), d18);
        assert_eq!(dia_de(ms_local(2026, 11, 18, 1, 30, F), 180, F), d17); // corte às 03:00
    }
    #[test] fn proximo_corte_e_meia_noite_local() {
        assert_eq!(proximo_corte(h(23, 30), 0, F), ms_local(2026, 11, 18, 0, 0, F));
        assert_eq!(inicio_do_dia(NaiveDate::from_ymd_opt(2026, 11, 17).unwrap(), 0, F), h(0, 0));
    }
}
