// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Diagnóstico da leitura no app do Claude (spec 001, US 001.5). Em 23 e 24/11 a leitura do app
//! parou de contar por dois dias sem ninguém saber: a consulta lenta e a thread presa só iam para
//! o `log_debug!`, que some no release. Aqui ficam, em memória e num arquivo pequeno, contadores
//! por hora das consultas da conversa aberta, e o estado "ok" ou "falhando desde".
//!
//! Privacidade: só contagens e instantes. Nenhum título, id de sessão ou nome de janela.
use std::collections::VecDeque;

use serde::{Deserialize, Serialize};

use crate::tempo::{Ms, MIN};

/// Uso da janela do app do Claude sem reconhecer nenhuma conversa, com o Claude trabalhando em
/// sessões do app nesse tempo, para a leitura ser dada como falhando.
pub const FALHA_APOS_MS: Ms = 30 * MIN;
/// As horas guardadas (as últimas dois dias).
pub const HORAS_GUARDADAS: usize = 48;
/// Entre duas amostras em uso, no máximo isto conta como uso: o laço é de 5 s, e uma pausa maior
/// (o PC dormindo, ou outra janela no meio: revisão M5) não soma.
pub const PASSO_MAX_MS: Ms = 6_000;
/// Amostras reconhecidas seguidas para a falha voltar a "ok" (revisão M4: um acerto solto, com a
/// consulta instável, não esconde a falha; a leitura também só abre com duas seguidas).
pub const RECONHECIDAS_PARA_OK: u32 = 2;
/// O arquivo é gravado no máximo uma vez a cada isto.
pub const GRAVAR_A_CADA_MS: Ms = MIN;
const HORA_MS: Ms = 60 * MIN;

/// O que a consulta da conversa disse numa amostra da janela do app do Claude.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Resultado {
    /// A conversa de uma sessão conhecida (ou de várias com o mesmo nome).
    Sessao,
    /// Uma conversa cujo título não é de nenhuma sessão conhecida.
    SemSessao,
    /// Nenhuma conversa: a consulta falhou, passou do limite ou a página não tinha uma.
    SemConversa,
}

/// As contagens de uma hora. `inicio` é o começo da hora (UTC).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hora {
    pub inicio: Ms,
    /// Amostras com a janela do app do Claude em uso em que a conversa foi consultada.
    pub consultas: u32,
    pub sessao: u32,
    pub sem_sessao: u32,
    pub sem_conversa: u32,
    /// Consultas que passaram do limite de 300 ms.
    pub atrasadas: u32,
    /// Amostras que usaram a última conversa no lugar de uma consulta lenta.
    pub seguradas: u32,
    /// Trocas da thread da consulta (travou ou caiu).
    pub recomecos: u32,
}

/// O estado da leitura no app, para a tela.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeituraApp {
    /// "ok", "falhando" ou "sem_uso" (o app do Claude ainda não foi usado com o Chronn aberto).
    pub estado: String,
    /// Desde quando falha (só com "falhando").
    pub desde: Option<Ms>,
}

/// As consultas lentas de uma amostra (as diferenças dos contadores do leitor).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Lentas {
    pub atrasadas: u32,
    pub seguradas: u32,
    pub recomecos: u32,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct DiagnosticoLeitura {
    pub horas: VecDeque<Hora>,
    /// A última amostra em uso (para somar o uso).
    ultima_em_uso: Option<Ms>,
    /// A primeira amostra em uso sem reconhecer conversa, depois da última reconhecida.
    sem_reconhecer_desde: Option<Ms>,
    /// O uso somado desde `sem_reconhecer_desde`.
    uso_sem_reconhecer_ms: Ms,
    /// Amostras reconhecidas seguidas (para voltar a "ok").
    reconhecidas: u32,
    pub falhando_desde: Option<Ms>,
    /// Algo mudou desde a última gravação do arquivo.
    sujo: bool,
    gravado_em: Option<Ms>,
}

/// O arquivo do diagnóstico.
#[derive(Serialize)]
struct Arquivo<'a> {
    estado: &'a LeituraApp,
    horas: &'a VecDeque<Hora>,
}

/// O arquivo lido na abertura: só as horas (o estado recomeça).
#[derive(Deserialize)]
struct ArquivoLido {
    #[serde(default)]
    horas: Vec<Hora>,
}

impl DiagnosticoLeitura {
    /// Tira as horas de mais de `HORAS_GUARDADAS` horas de relógio antes de `t` (revisão M2).
    fn podar(&mut self, t: Ms) {
        let limite = t - t.rem_euclid(HORA_MS) - (HORAS_GUARDADAS as Ms - 1) * HORA_MS;
        while self.horas.front().is_some_and(|h| h.inicio < limite) {
            self.horas.pop_front();
        }
    }

    fn hora(&mut self, t: Ms) -> &mut Hora {
        let inicio = t - t.rem_euclid(HORA_MS);
        if self.horas.back().is_none_or(|h| h.inicio != inicio) {
            self.horas.push_back(Hora { inicio, ..Default::default() });
        }
        self.podar(t);
        self.horas.back_mut().expect("acabou de entrar")
    }

    /// As horas do arquivo gravado antes (revisão I3: o PC é religado quase todo dia, e o arquivo
    /// precisa guardar as falhas de ontem). Só as das últimas `HORAS_GUARDADAS` horas antes de
    /// `agora`, em ordem e sem repetir; um arquivo estragado não carrega nada.
    pub fn carregar(&mut self, texto: &str, agora: Ms) {
        let Ok(lido) = serde_json::from_str::<ArquivoLido>(texto) else { return };
        let mut horas: Vec<Hora> = lido.horas.into_iter().filter(|h| h.inicio <= agora && h.inicio.rem_euclid(HORA_MS) == 0).collect();
        horas.sort_by_key(|h| h.inicio);
        horas.dedup_by_key(|h| h.inicio);
        self.horas = horas.into();
        self.podar(agora);
    }

    /// Uma amostra da janela do app do Claude em que a conversa foi consultada. `em_uso`: a
    /// janela em foco e você usando o PC (só essas contam para a falha). `neutra`: a aba do Chat à
    /// vista (lá a conversa do Code não aparece, e não conta nem para um lado nem para o outro).
    /// `ultimo_evento_do_app`: o evento mais recente de uma sessão do app (`claude-desktop`).
    /// Devolve se o estado mudou (ok ↔ falhando).
    pub fn registrar(&mut self, t: Ms, r: Resultado, lentas: Lentas, em_uso: bool, neutra: bool, ultimo_evento_do_app: Option<Ms>) -> bool {
        // só as amostras em uso contam (revisão M1)
        if !em_uso {
            return false;
        }
        let h = self.hora(t);
        h.consultas += 1;
        match r {
            Resultado::Sessao => h.sessao += 1,
            Resultado::SemSessao => h.sem_sessao += 1,
            Resultado::SemConversa => h.sem_conversa += 1,
        }
        h.atrasadas += lentas.atrasadas;
        h.seguradas += lentas.seguradas;
        h.recomecos += lentas.recomecos;
        self.sujo = true;
        let antes = self.falhando_desde;
        let passo = self.ultima_em_uso.map_or(0, |u| (t - u).clamp(0, PASSO_MAX_MS));
        self.ultima_em_uso = Some(t);
        match (r, neutra) {
            (_, true) => {}
            (Resultado::Sessao, false) => {
                self.reconhecidas += 1;
                // a primeira reconhecida já zera a conta; com a falha dada, voltar a "ok" pede
                // duas seguidas
                if self.falhando_desde.is_none() || self.reconhecidas >= RECONHECIDAS_PARA_OK {
                    self.sem_reconhecer_desde = None;
                    self.uso_sem_reconhecer_ms = 0;
                    self.falhando_desde = None;
                }
            }
            (_, false) => {
                self.reconhecidas = 0;
                if self.sem_reconhecer_desde.is_none() {
                    self.sem_reconhecer_desde = Some(t);
                    self.uso_sem_reconhecer_ms = 0;
                } else {
                    self.uso_sem_reconhecer_ms += passo;
                }
            }
        }
        if self.falhando_desde.is_none() {
            if let Some(desde) = self.sem_reconhecer_desde {
                let claude_no_app = ultimo_evento_do_app.is_some_and(|e| e >= desde);
                if self.uso_sem_reconhecer_ms >= FALHA_APOS_MS && claude_no_app {
                    self.falhando_desde = Some(desde);
                }
            }
        }
        self.falhando_desde != antes
    }

    /// O estado para a tela.
    pub fn estado(&self) -> LeituraApp {
        match (self.falhando_desde, self.ultima_em_uso) {
            (Some(desde), _) => LeituraApp { estado: "falhando".into(), desde: Some(desde) },
            (None, Some(_)) => LeituraApp { estado: "ok".into(), desde: None },
            (None, None) => LeituraApp { estado: "sem_uso".into(), desde: None },
        }
    }

    /// O texto do arquivo, se mudou algo e já passou `GRAVAR_A_CADA_MS` da última gravação (ou
    /// se o estado acabou de mudar: `ja`). Marca como gravado.
    pub fn para_gravar(&mut self, agora: Ms, ja: bool) -> Option<String> {
        if !self.sujo || (!ja && self.gravado_em.is_some_and(|g| agora - g < GRAVAR_A_CADA_MS)) {
            return None;
        }
        self.sujo = false;
        self.gravado_em = Some(agora);
        serde_json::to_string_pretty(&Arquivo { estado: &self.estado(), horas: &self.horas }).ok()
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const S: Ms = 1_000;
    const T0: Ms = 1_795_700_000_000;
    const NADA: Lentas = Lentas { atrasadas: 0, seguradas: 0, recomecos: 0 };

    /// Amostras a cada 5 s, de `de` por `ms`, todas com o mesmo resultado.
    fn uso(d: &mut DiagnosticoLeitura, de: Ms, ms: Ms, r: Resultado, evento: Option<Ms>) -> bool {
        let mut mudou = false;
        let mut t = de;
        while t <= de + ms {
            mudou |= d.registrar(t, r, NADA, true, false, evento);
            t += 5 * S;
        }
        mudou
    }

    #[test]
    fn sem_uso_ate_a_primeira_amostra_e_ok_depois() {
        let mut d = DiagnosticoLeitura::default();
        assert_eq!(d.estado().estado, "sem_uso");
        d.registrar(T0, Resultado::Sessao, NADA, true, false, None);
        assert_eq!(d.estado(), LeituraApp { estado: "ok".into(), desde: None });
    }

    #[test]
    fn falha_com_30_min_de_uso_sem_conversa_e_o_claude_trabalhando_no_app() {
        let mut d = DiagnosticoLeitura::default();
        uso(&mut d, T0, 60 * S, Resultado::Sessao, None);
        let desde = T0 + 65 * S;
        assert!(!uso(&mut d, desde, 29 * MIN, Resultado::SemConversa, Some(desde + MIN)));
        assert_eq!(d.estado().estado, "ok", "29 min ainda não");
        assert!(uso(&mut d, desde + 29 * MIN + 5 * S, 2 * MIN, Resultado::SemConversa, Some(desde + MIN)));
        assert_eq!(d.estado(), LeituraApp { estado: "falhando".into(), desde: Some(desde) });
        // um acerto solto não esconde a falha; dois seguidos voltam a "ok"
        assert!(!d.registrar(desde + 40 * MIN, Resultado::Sessao, NADA, true, false, None));
        assert!(!d.registrar(desde + 40 * MIN + 5 * S, Resultado::SemConversa, NADA, true, false, None));
        assert!(!d.registrar(desde + 40 * MIN + 10 * S, Resultado::Sessao, NADA, true, false, None));
        assert_eq!(d.estado().estado, "falhando");
        assert!(d.registrar(desde + 40 * MIN + 15 * S, Resultado::Sessao, NADA, true, false, None));
        assert_eq!(d.estado().estado, "ok");
    }

    #[test]
    fn sem_o_claude_trabalhando_no_app_nao_falha() {
        // por exemplo, só a tela inicial do Code aberta: nenhum evento de sessão do app nesse tempo
        let mut d = DiagnosticoLeitura::default();
        let evento_antigo = Some(T0 - 5 * MIN);
        uso(&mut d, T0, 45 * MIN, Resultado::SemConversa, evento_antigo);
        assert_eq!(d.estado().estado, "ok");
        // o título sem sessão conta como não reconhecido
        let mut d = DiagnosticoLeitura::default();
        uso(&mut d, T0, 31 * MIN, Resultado::SemSessao, Some(T0 + 10 * MIN));
        assert_eq!(d.estado().estado, "falhando");
    }

    #[test]
    fn so_o_uso_soma_e_o_pc_dormindo_nao() {
        let mut d = DiagnosticoLeitura::default();
        let ev = Some(T0 + 5 * S);
        // 20 min de uso, o PC dorme 2 h, mais 5 min: 25 min, ainda não
        uso(&mut d, T0, 20 * MIN, Resultado::SemConversa, ev);
        uso(&mut d, T0 + 140 * MIN, 5 * MIN, Resultado::SemConversa, ev);
        assert_eq!(d.estado().estado, "ok");
        // amostras fora de uso (a janela debaixo do mouse) não somam nem entram nas contagens
        let consultas = d.horas.iter().map(|h| h.consultas).sum::<u32>();
        let mut t = T0 + 150 * MIN;
        while t < T0 + 200 * MIN {
            d.registrar(t, Resultado::SemConversa, NADA, false, false, ev);
            t += 5 * S;
        }
        assert_eq!(d.estado().estado, "ok");
        assert_eq!(d.horas.iter().map(|h| h.consultas).sum::<u32>(), consultas);
        // a aba do Chat à vista também não
        let mut t = T0 + 200 * MIN;
        while t < T0 + 240 * MIN {
            d.registrar(t, Resultado::SemConversa, NADA, true, true, ev);
            t += 5 * S;
        }
        assert_eq!(d.estado().estado, "ok");
        uso(&mut d, T0 + 240 * MIN + 5 * S, 6 * MIN, Resultado::SemConversa, ev);
        assert_eq!(d.estado().estado, "falhando");
    }

    #[test]
    fn contadores_por_hora_e_so_as_ultimas_48() {
        let mut d = DiagnosticoLeitura::default();
        let lenta = Lentas { atrasadas: 1, seguradas: 1, recomecos: 0 };
        d.registrar(T0, Resultado::Sessao, NADA, true, false, None);
        d.registrar(T0 + S, Resultado::SemConversa, lenta, true, false, None);
        d.registrar(T0 + 2 * S, Resultado::SemSessao, Lentas { recomecos: 1, ..NADA }, true, false, None);
        let h = d.horas.back().unwrap();
        assert_eq!((h.consultas, h.sessao, h.sem_conversa, h.sem_sessao, h.atrasadas, h.seguradas, h.recomecos), (3, 1, 1, 1, 1, 1, 1));
        assert_eq!(h.inicio % HORA_MS, 0);
        for i in 0..60 {
            d.registrar(T0 + i * HORA_MS, Resultado::Sessao, NADA, true, false, None);
        }
        assert_eq!(d.horas.len(), HORAS_GUARDADAS);
        // horas de relógio, não horas com amostra (revisão M2): uma amostra 3 dias depois
        d.registrar(T0 + 130 * HORA_MS, Resultado::Sessao, NADA, true, false, None);
        assert_eq!(d.horas.len(), 1);
    }

    #[test]
    fn carrega_as_horas_do_arquivo_de_antes() {
        // revisão I3: o arquivo da abertura anterior volta, sem as de mais de 48 h
        let mut a = DiagnosticoLeitura::default();
        a.registrar(T0 - 50 * HORA_MS, Resultado::Sessao, NADA, true, false, None);
        a.registrar(T0 - 20 * HORA_MS, Resultado::SemConversa, NADA, true, false, None);
        a.registrar(T0 - 2 * HORA_MS, Resultado::Sessao, NADA, true, false, None);
        let texto = a.para_gravar(T0 - 2 * HORA_MS, true).unwrap();
        let mut d = DiagnosticoLeitura::default();
        d.carregar(&texto, T0);
        assert_eq!(d.horas.len(), 2, "a de 50 h atrás sai");
        assert_eq!(d.horas[0].sem_conversa, 1);
        assert_eq!(d.estado().estado, "sem_uso", "o estado recomeça");
        d.registrar(T0, Resultado::Sessao, NADA, true, false, None);
        assert_eq!(d.horas.len(), 3);
        // arquivo estragado: nada
        let mut d = DiagnosticoLeitura::default();
        d.carregar("{ não é json", T0);
        d.carregar("[]", T0);
        assert!(d.horas.is_empty());
    }

    #[test]
    fn grava_no_maximo_uma_vez_por_minuto_e_sem_titulos() {
        let mut d = DiagnosticoLeitura::default();
        assert_eq!(d.para_gravar(T0, false), None, "nada a gravar");
        d.registrar(T0, Resultado::Sessao, NADA, true, false, None);
        let texto = d.para_gravar(T0, false).expect("a primeira");
        assert!(texto.contains("\"estado\": \"ok\"") && texto.contains("\"consultas\": 1"), "{texto}");
        d.registrar(T0 + 5 * S, Resultado::Sessao, NADA, true, false, None);
        assert_eq!(d.para_gravar(T0 + 5 * S, false), None, "menos de 1 min");
        assert!(d.para_gravar(T0 + 5 * S, true).is_some(), "o estado mudou: grava já");
        d.registrar(T0 + 10 * S, Resultado::Sessao, NADA, true, false, None);
        assert!(d.para_gravar(T0 + 70 * S, false).is_some());
        // só números e os nomes dos campos
        let campos = ["estado", "desde", "horas", "inicio", "consultas", "sessao", "sem_sessao", "sem_conversa", "atrasadas", "seguradas", "recomecos", "ok", "null"];
        let v: serde_json::Value = serde_json::from_str(&texto).unwrap();
        fn textos(v: &serde_json::Value, out: &mut Vec<String>) {
            match v {
                serde_json::Value::String(s) => out.push(s.clone()),
                serde_json::Value::Object(m) => m.iter().for_each(|(k, x)| {
                    out.push(k.clone());
                    textos(x, out)
                }),
                serde_json::Value::Array(a) => a.iter().for_each(|x| textos(x, out)),
                _ => {}
            }
        }
        let mut t = vec![];
        textos(&v, &mut t);
        assert!(t.iter().all(|x| campos.contains(&x.as_str())), "{t:?}");
    }
}
