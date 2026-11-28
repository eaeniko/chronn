// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! As contas puras do lugar do dock e da janela principal (specs 005 e 006), em pixels físicos,
//! testadas sem Tauri e sem Windows.
//!
//! - O monitor de uma janela é achado pela **tela inteira** (com a barra de tarefas), nunca só pela
//!   área útil: o dock em cima da barra continua no monitor dele (A01).
//! - O dock pode ficar em cima da barra (decisão do usuário, 27/11): o `dentro` e o `ancorar` do
//!   dock usam a tela inteira; a área útil vale para a posição padrão, o resgate e o aviso (A02).
//! - Só a posição escolhida arrastando é gravada, com o retângulo inteiro (A04 e A07).
use serde::Serialize;

/// Retângulo em pixels físicos.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ret {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Um monitor: a tela inteira, a área útil (a tela sem a barra de tarefas) e a escala.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Monitor {
    pub tela: Ret,
    pub util: Ret,
    pub escala: f64,
    pub principal: bool,
}

/// Lado da tela em que o dock está: o conteúdo se alinha a ele e o tamanho cresce para longe dele.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Lado {
    pub direita: bool,
    pub embaixo: bool,
}

/// Quanto de uma janela precisa aparecer num monitor para uma posição salva valer (físico).
pub const VISIVEL_MIN: i32 = 32;
/// O Windows põe a janela minimizada em (−32000, −32000).
const MINIMIZADA: i32 = -30_000;

fn limitar(v: i32, min: i32, max: i32) -> i32 {
    if max < min {
        min
    } else {
        v.clamp(min, max)
    }
}

/// Posição de uma janela `w`×`h` em (x, y), empurrada para dentro da área.
pub fn dentro(x: i32, y: i32, w: i32, h: i32, area: Ret) -> (i32, i32) {
    (limitar(x, area.x, area.x + area.w - w), limitar(y, area.y, area.y + area.h - h))
}

/// Pelo centro da janela: está na metade direita? na de baixo?
pub fn lado(r: Ret, area: Ret) -> Lado {
    Lado { direita: 2 * r.x + r.w > 2 * area.x + area.w, embaixo: 2 * r.y + r.h > 2 * area.y + area.h }
}

/// Nova posição ao trocar o tamanho para `w`×`h`: segura a borda do lado em que a janela está
/// (direita e/ou de baixo) e não deixa sair da área.
pub fn ancorar(r: Ret, w: i32, h: i32, area: Ret) -> (i32, i32) {
    let l = lado(r, area);
    let x = if l.direita { r.x + r.w - w } else { r.x };
    let y = if l.embaixo { r.y + r.h - h } else { r.y };
    dentro(x, y, w, h, area)
}

fn intersecao(a: Ret, b: Ret) -> (i32, i32) {
    let w = (a.x + a.w).min(b.x + b.w) - a.x.max(b.x);
    let h = (a.y + a.h).min(b.y + b.h) - a.y.max(b.y);
    (w.max(0), h.max(0))
}

fn area(p: (i32, i32)) -> i64 {
    p.0 as i64 * p.1 as i64
}

/// Distância (ao quadrado) do centro de `r` até o retângulo `a` (0 se o centro está dentro).
fn distancia(r: Ret, a: Ret) -> i64 {
    let (cx, cy) = (r.x as i64 * 2 + r.w as i64, r.y as i64 * 2 + r.h as i64);
    let dx = (a.x as i64 * 2 - cx).max(cx - (a.x as i64 + a.w as i64) * 2).max(0);
    let dy = (a.y as i64 * 2 - cy).max(cy - (a.y as i64 + a.h as i64) * 2).max(0);
    dx * dx + dy * dy
}

/// O monitor da janela, pela tela inteira: o que mais a contém; sem interseção, o mais perto
/// (como o `MONITOR_DEFAULTTONEAREST` do Windows). Só `None` sem monitor nenhum.
pub fn monitor_de(r: Ret, ms: &[Monitor]) -> Option<&Monitor> {
    let melhor = ms.iter().map(|m| (m, area(intersecao(r, m.tela)))).filter(|(_, a)| *a > 0).max_by_key(|(_, a)| *a);
    match melhor {
        Some((m, _)) => Some(m),
        None => ms.iter().min_by_key(|m| distancia(r, m.tela)),
    }
}

/// O monitor em que a janela aparece de verdade: pelo menos 32×32 px (ou a janela inteira, se
/// menor) dentro da tela dele. Para a posição salva valer.
pub fn monitor_visivel(r: Ret, ms: &[Monitor]) -> Option<&Monitor> {
    ms.iter()
        .map(|m| (m, intersecao(r, m.tela)))
        .filter(|(_, (w, h))| *w >= VISIVEL_MIN.min(r.w) && *h >= VISIVEL_MIN.min(r.h))
        .max_by_key(|(_, p)| area(*p))
        .map(|(m, _)| m)
}

/// O monitor principal (ou o primeiro, se nenhum diz que é).
pub fn principal(ms: &[Monitor]) -> Option<&Monitor> {
    ms.iter().find(|m| m.principal).or_else(|| ms.first())
}

/// O monitor que contém o ponto (pela tela inteira).
pub fn monitor_do_ponto(p: (i32, i32), ms: &[Monitor]) -> Option<&Monitor> {
    ms.iter().find(|m| p.0 >= m.tela.x && p.0 < m.tela.x + m.tela.w && p.1 >= m.tela.y && p.1 < m.tela.y + m.tela.h)
}

/// A posição padrão do dock num monitor: o canto de cima à direita da área útil, com margem.
pub fn posicao_padrao(w: i32, h: i32, m: &Monitor, margem: i32) -> (i32, i32) {
    let u = m.util;
    dentro(u.x + u.w - w - margem, u.y + margem, w, h, u)
}

/// A janela está minimizada (o Windows a põe em −32000)?
pub fn minimizada(r: Ret) -> bool {
    r.x <= MINIMIZADA || r.y <= MINIMIZADA
}

/// O dock troca de tamanho para `w`×`h`: segura a borda do lado dele e fica dentro da **tela
/// inteira** do monitor dele (pode ficar em cima da barra). Sem monitor nenhum, ou minimizado, não
/// se move.
pub fn redimensionado(r: Ret, w: i32, h: i32, ms: &[Monitor]) -> Ret {
    let (x, y) = match monitor_de(r, ms) {
        Some(m) if !minimizada(r) => ancorar(r, w, h, m.tela),
        _ => (r.x, r.y),
    };
    Ret { x, y, w, h }
}

/// O dock que quase não aparece em monitor nenhum (o monitor dele sumiu e o Windows não o tirou
/// de lá, ou só uma lasca ficou à vista): volta para dentro da tela do monitor **mais perto**, nunca
/// para outro (revisão, M3). `None` se ele aparece, se está minimizado ou sem monitor.
pub fn de_volta_a_tela(r: Ret, ms: &[Monitor]) -> Option<(i32, i32)> {
    if minimizada(r) || monitor_visivel(r, ms).is_some() {
        return None;
    }
    let m = monitor_de(r, ms)?;
    Some(dentro(r.x, r.y, r.w, r.h, m.tela))
}

/// Só uma lasca do dock aparece (ele encosta num monitor, mas menos de 32 px): foi a pessoa que o
/// empurrou para a borda. Sem encostar em monitor nenhum, o monitor dele sumiu (revisão, R2).
pub fn so_uma_lasca(r: Ret, ms: &[Monitor]) -> bool {
    !minimizada(r) && monitor_visivel(r, ms).is_none() && ms.iter().any(|m| area(intersecao(r, m.tela)) > 0)
}

/// O tamanho físico `r.w`×`r.h`, medido na escala `de`, na escala `para` (o mesmo tamanho lógico):
/// o dock levado de um monitor para outro de outra escala não fica cortado (revisão, R1).
pub fn na_escala(r: Ret, de: f64, para: f64) -> Ret {
    if de <= 0.0 || para <= 0.0 || de == para {
        return r;
    }
    let f = para / de;
    Ret { w: (r.w as f64 * f).round() as i32, h: (r.h as f64 * f).round() as i32, ..r }
}

/// Tentar a volta para a escolha neste ciclo? Com os monitores que mudaram, o PC que acordou ou o
/// dock ainda fora da escolha (a abertura ou o "fora dos monitores" caíram noutro lugar: o
/// monitor dela pode chegar a qualquer ciclo; revisão, I1).
pub fn tentar_a_escolha(fora_da_escolha: bool, mudaram: bool, acordou: bool) -> bool {
    fora_da_escolha || mudaram || acordou
}

/// O lado do dock no monitor dele (pela tela inteira).
pub fn lado_no_monitor(r: Ret, ms: &[Monitor]) -> Lado {
    monitor_de(r, ms).map(|m| lado(r, m.tela)).unwrap_or_default()
}

/// A escolha (o retângulo que a pessoa deixou arrastando) no tamanho `w`×`h` de agora: ancorada
/// pelo lado dela, dentro da tela do monitor dela. `None` se o monitor dela não está mais aqui.
pub fn lugar_da_escolha(escolha: Ret, w: i32, h: i32, ms: &[Monitor]) -> Option<(i32, i32)> {
    let m = monitor_visivel(escolha, ms)?;
    Some(ancorar(escolha, w, h, m.tela))
}

/// Posição ao abrir: a escolha, se o monitor dela ainda existe; senão a posição padrão do
/// principal (a escolha não se perde: ela volta quando o monitor voltar).
pub fn posicao_inicial(escolha: Option<Ret>, w: i32, h: i32, ms: &[Monitor], margem: i32) -> (i32, i32) {
    if let Some(p) = escolha.and_then(|e| lugar_da_escolha(e, w, h, ms)) {
        return p;
    }
    match principal(ms) {
        Some(m) => posicao_padrao(w, h, m, margem),
        None => escolha.map(|e| (e.x, e.y)).unwrap_or((0, 0)),
    }
}

/// A escolha guardada na config: o retângulo inteiro (spec 005) ou, numa config de antes, o canto
/// com o tamanho inicial.
pub fn escolha_guardada(x: Option<i32>, y: Option<i32>, w: Option<i32>, h: Option<i32>, inicial: (i32, i32)) -> Option<Ret> {
    let (x, y) = x.zip(y)?;
    let (w, h) = match w.zip(h) {
        Some((w, h)) if w > 0 && h > 0 => (w, h),
        _ => inicial,
    };
    Some(Ret { x, y, w, h })
}

/// O movimento (ou a troca de tamanho) é da pessoa? O botão do mouse está apertado **e** o
/// retângulo não é o que o próprio app pediu por último.
pub fn e_da_pessoa(botao_apertado: bool, r: Ret, pedido: Option<Ret>) -> bool {
    botao_apertado && pedido != Some(r)
}

/// O retângulo pode virar a escolha gravada? Nunca minimizado (−32000) nem fora dos monitores.
pub fn pode_gravar(r: Ret, ms: &[Monitor]) -> bool {
    r.x > MINIMIZADA && r.y > MINIMIZADA && r.w > 0 && r.h > 0 && monitor_visivel(r, ms).is_some()
}

/// O dock encosta na faixa da barra de tarefas (a tela menos a área útil) do monitor dele?
pub fn na_faixa_da_barra(r: Ret, ms: &[Monitor]) -> bool {
    monitor_de(r, ms).is_some_and(|m| area(intersecao(r, m.tela)) > area(intersecao(r, m.util)))
}

/// O `rcNormalPosition` (a janela maximizada antes de maximizar) vem em coordenadas da área de
/// trabalho: a área útil do monitor da janela é a origem (como o Chromium converte; revisão, M4).
pub fn da_area_de_trabalho(normal: Ret, m: &Monitor) -> Ret {
    Ret { x: normal.x + m.util.x - m.tela.x, y: normal.y + m.util.y - m.tela.y, ..normal }
}

/// O resgate da bandeja: a posição padrão do monitor do mouse (ou do principal).
pub fn posicao_do_resgate(mouse: Option<(i32, i32)>, w: i32, h: i32, ms: &[Monitor], margem: i32) -> Option<(i32, i32)> {
    let m = mouse.and_then(|p| monitor_do_ponto(p, ms)).or_else(|| principal(ms))?;
    Some(posicao_padrao(w, h, m, margem))
}

// ---- a janela principal (US 005.4) ----

/// Onde a janela principal recriada nasce: o lugar salvo, se ele aparece num monitor (o tamanho
/// cortado à área útil dele); senão nada (o padrão do Windows).
pub fn lugar_da_principal(salvo: Option<Ret>, ms: &[Monitor]) -> Option<Ret> {
    let s = salvo?;
    let m = monitor_visivel(s, ms)?;
    let (w, h) = (s.w.min(m.util.w), s.h.min(m.util.h));
    let (x, y) = dentro(s.x, s.y, w, h, m.tela);
    Some(Ret { x, y, w, h })
}

/// A janela principal pedida pelo dock: se ela está noutro monitor (ou em nenhum), o centro da
/// área útil do monitor do dock, no mesmo tamanho (cortado à área). Mesmo monitor: nada.
pub fn principal_perto_do_dock(principal: Ret, dock: Ret, ms: &[Monitor]) -> Option<Ret> {
    let md = monitor_de(dock, ms)?;
    if monitor_visivel(principal, ms).is_some_and(|mp| mp.tela == md.tela) {
        return None;
    }
    let u = md.util;
    let (w, h) = (principal.w.min(u.w), principal.h.min(u.h));
    Some(Ret { x: u.x + (u.w - w) / 2, y: u.y + (u.h - h) / 2, w, h })
}

// ---- a saúde do dock (spec 006) ----

/// O ciclo anterior do laço foi há mais disto: o PC dormiu (ou o relógio pulou).
pub const ACORDOU_MS: u64 = 30_000;

/// O PC acordou? (o ciclo anterior há mais de 30 s; o relógio que voltou não conta)
pub fn acordou(anterior: Option<u64>, agora: u64) -> bool {
    anterior.is_some_and(|a| agora > a && agora - a > ACORDOU_MS)
}

/// O sinal de vida da página do dock: o Rust manda um "ping" a cada ciclo e a página responde.
/// Sem resposta por 30 s (com 3 pings ou mais sem volta), a página é recarregada, no máximo uma
/// vez a cada 2 min.
///
/// As horas são de um relógio que só anda para a frente (não o de parede; revisão, M10).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Vida {
    /// A última resposta (ou o começo da conta: a abertura, o último recarregar, o dock à vista).
    pub ultima: u64,
    pub sem_volta: u32,
    pub recarregou: Option<u64>,
    /// Recarregou e a página ainda não respondeu: o próximo recarregar não resolveu (o processo do
    /// navegador do WebView2 pode ter caído; revisão, M11).
    pub sem_resposta_desde_recarregar: bool,
}

pub const VIDA_PRAZO_MS: u64 = 30_000;
pub const VIDA_PINGS_MIN: u32 = 3;
pub const VIDA_INTERVALO_MS: u64 = 120_000;

impl Vida {
    pub fn nova(agora: u64) -> Vida {
        Vida { ultima: agora, sem_volta: 0, recarregou: None, sem_resposta_desde_recarregar: false }
    }

    /// A página respondeu.
    pub fn respondeu(&mut self, agora: u64) {
        self.recomecar(agora);
        self.sem_resposta_desde_recarregar = false;
    }

    fn recomecar(&mut self, agora: u64) {
        self.ultima = agora;
        self.sem_volta = 0;
    }

    /// O dock não está à vista (escondido, minimizado, noutra área) ou o PC acordou: a conta
    /// recomeça.
    pub fn em_pausa(&mut self, agora: u64) {
        self.recomecar(agora);
    }

    /// Um ping saiu. Devolve se a página deve ser recarregada agora (e já conta o recarregar).
    pub fn ping(&mut self, agora: u64) -> bool {
        self.sem_volta = self.sem_volta.saturating_add(1);
        let calada = self.sem_volta >= VIDA_PINGS_MIN && agora.saturating_sub(self.ultima) >= VIDA_PRAZO_MS;
        let pode = self.recarregou.is_none_or(|r| agora.saturating_sub(r) >= VIDA_INTERVALO_MS);
        if calada && pode {
            self.recarregou = Some(agora);
            self.recomecar(agora);
            self.sem_resposta_desde_recarregar = true;
            return true;
        }
        false
    }
}

/// O que o log do dock registra a cada mudança (só números).
#[derive(Clone, Debug, PartialEq)]
pub struct FotoDoDock {
    pub visivel: bool,
    pub ret: Option<Ret>,
    pub minimizado: bool,
    pub oculto: bool,
    pub monitores: Vec<Monitor>,
}

fn ret_txt(r: Ret) -> String {
    format!("{},{},{}x{}", r.x, r.y, r.w, r.h)
}

/// A linha do log para a foto (`hora` já formatada). Nunca títulos: só números.
pub fn linha_da_foto(hora: &str, f: &FotoDoDock) -> String {
    let ms: Vec<String> = f
        .monitores
        .iter()
        .map(|m| format!("{}{}/util {}@{}", if m.principal { "*" } else { "" }, ret_txt(m.tela), ret_txt(m.util), m.escala))
        .collect();
    format!(
        "{hora} estado visivel={} ret={} min={} oculto={} monitores=[{}]",
        f.visivel as u8,
        f.ret.map(ret_txt).unwrap_or_else(|| "-".into()),
        f.minimizado as u8,
        f.oculto as u8,
        ms.join(" | ")
    )
}

/// A linha do log para uma ação (arrastado, recolocado, resgate, recarregado, acordou…).
pub fn linha_da_acao(hora: &str, acao: &str, ret: Option<Ret>) -> String {
    match ret {
        Some(r) => format!("{hora} {acao} ret={}", ret_txt(r)),
        None => format!("{hora} {acao}"),
    }
}

/// O log passa disto e vira o `.1` (um só).
pub const LOG_MAX_BYTES: u64 = 256 * 1024;

#[cfg(test)]
mod testes {
    use super::*;

    /// O setup do relato: dois monitores de 1920×1080 lado a lado, a barra de 48 px embaixo nos dois.
    fn m1() -> Monitor {
        Monitor { tela: Ret { x: 0, y: 0, w: 1920, h: 1080 }, util: Ret { x: 0, y: 0, w: 1920, h: 1032 }, escala: 1.0, principal: true }
    }
    fn m2() -> Monitor {
        Monitor {
            tela: Ret { x: 1920, y: 0, w: 1920, h: 1080 },
            util: Ret { x: 1920, y: 0, w: 1920, h: 1032 },
            escala: 1.0,
            principal: false,
        }
    }
    /// O 2º sem barra de tarefas (a área útil é a tela inteira) e a 125%.
    fn m2_sem_barra() -> Monitor {
        Monitor { util: m2().tela, escala: 1.25, ..m2() }
    }
    fn dois() -> Vec<Monitor> {
        vec![m1(), m2()]
    }

    #[test]
    fn dock_em_cima_da_barra_do_segundo_monitor_e_do_segundo() {
        // A01: o dock inteiro em cima da barra do M2 (y 1034, 46 de altura)
        let r = Ret { x: 3000, y: 1034, w: 300, h: 46 };
        assert_eq!(monitor_de(r, &dois()).map(|m| m.tela), Some(m2().tela));
        // a pergunta chega (400×330): o cartão cresce para cima a partir da pílula, no M2
        let novo = redimensionado(r, 400, 330, &dois());
        assert_eq!(novo, Ret { x: 2900, y: 750, w: 400, h: 330 });
        // respondida, a pílula volta exatamente para cima da barra
        assert_eq!(redimensionado(novo, 300, 46, &dois()), r);
        // um ▶ (uma linha a mais) e o total crescendo: fica em cima da barra
        assert_eq!(redimensionado(r, 320, 70, &dois()), Ret { x: 2980, y: 1010, w: 320, h: 70 });
        assert!(na_faixa_da_barra(r, &dois()));
    }

    #[test]
    fn dock_com_uma_faixa_na_area_util_nao_sobe() {
        // A02: 10 px na área útil, o resto em cima da barra
        let r = Ret { x: 1500, y: 1022, w: 300, h: 46 };
        assert_eq!(redimensionado(r, 320, 46, &dois()), Ret { x: 1480, y: 1022, w: 320, h: 46 });
        assert!(na_faixa_da_barra(r, &dois()));
        // longe da barra: não encosta
        assert!(!na_faixa_da_barra(Ret { x: 1500, y: 900, w: 300, h: 46 }, &dois()));
        // mas nunca passa da tela inteira
        assert_eq!(redimensionado(Ret { x: 1500, y: 1040, w: 300, h: 40 }, 300, 60, &dois()).y, 1020);
    }

    #[test]
    fn sem_intersecao_vale_o_monitor_mais_perto_e_sem_monitor_nao_move() {
        let r = Ret { x: 4000, y: 500, w: 300, h: 46 };
        assert_eq!(monitor_de(r, &dois()).map(|m| m.tela), Some(m2().tela));
        let r = Ret { x: -500, y: 500, w: 300, h: 46 };
        assert_eq!(monitor_de(r, &dois()).map(|m| m.tela), Some(m1().tela));
        assert_eq!(monitor_de(r, &[]), None);
        assert_eq!(redimensionado(r, 400, 90, &[]), Ret { x: -500, y: 500, w: 400, h: 90 });
        // na divisa, o que mais contém
        assert_eq!(monitor_de(Ret { x: 1800, y: 0, w: 300, h: 46 }, &dois()).map(|m| m.tela), Some(m2().tela));
        assert_eq!(monitor_de(Ret { x: 1700, y: 0, w: 300, h: 46 }, &dois()).map(|m| m.tela), Some(m1().tela));
    }

    #[test]
    fn segundo_monitor_sem_barra() {
        let ms = [m1(), m2_sem_barra()];
        let r = Ret { x: 3000, y: 1034, w: 300, h: 46 };
        assert!(!na_faixa_da_barra(r, &ms), "sem barra, nada a reafirmar por causa dela");
        assert_eq!(redimensionado(r, 400, 330, &ms), Ret { x: 2900, y: 750, w: 400, h: 330 });
        // a escala é a do monitor dele (a conversão fica na abertura, em `janelas::abertura`)
        assert_eq!(monitor_de(r, &ms).map(|m| m.escala), Some(1.25));
    }

    #[test]
    fn minimizado_so_troca_o_tamanho() {
        let r = Ret { x: -32000, y: -32000, w: 160, h: 28 };
        assert!(minimizada(r));
        assert_eq!(redimensionado(r, 320, 70, &dois()), Ret { x: -32000, y: -32000, w: 320, h: 70 });
        assert_eq!(de_volta_a_tela(r, &dois()), None, "minimizado: quem cuida é a saúde do dock");
    }

    #[test]
    fn fora_dos_monitores_volta_ao_mais_perto() {
        // o M2 sumiu e o dock ficou onde ele estava: vai para dentro do M1 (o único)
        assert_eq!(de_volta_a_tela(Ret { x: 3000, y: 500, w: 300, h: 46 }, &[m1()]), Some((1620, 500)));
        // só 20 px à vista embaixo do M2: volta para dentro do M2, não para o principal (M3)
        assert_eq!(de_volta_a_tela(Ret { x: 3000, y: 1060, w: 300, h: 46 }, &dois()), Some((3000, 1034)));
        // à vista: nada
        assert_eq!(de_volta_a_tela(Ret { x: 3000, y: 1034, w: 300, h: 46 }, &dois()), None);
        assert_eq!(de_volta_a_tela(Ret { x: 3000, y: 500, w: 300, h: 46 }, &[]), None);
    }

    #[test]
    fn a_lasca_e_da_pessoa_e_o_monitor_que_sumiu_nao() {
        // 20 px à vista embaixo do M2: a pessoa o empurrou para lá
        assert!(so_uma_lasca(Ret { x: 3000, y: 1060, w: 300, h: 46 }, &dois()));
        // o M2 sumiu: o dock não encosta em monitor nenhum
        assert!(!so_uma_lasca(Ret { x: 3000, y: 500, w: 300, h: 46 }, &[m1()]));
        // à vista, ou minimizado: não
        assert!(!so_uma_lasca(Ret { x: 3000, y: 1034, w: 300, h: 46 }, &dois()));
        assert!(!so_uma_lasca(Ret { x: -32000, y: -32000, w: 160, h: 28 }, &dois()));
    }

    #[test]
    fn o_tamanho_vai_junto_na_escala_do_destino() {
        let r = Ret { x: 3000, y: 1034, w: 300, h: 46 };
        // do M1 a 100% para o M2 a 125%: os mesmos 300×46 lógicos
        assert_eq!(na_escala(r, 1.0, 1.25), Ret { w: 375, h: 58, ..r });
        // e de volta
        assert_eq!(na_escala(Ret { w: 375, h: 58, ..r }, 1.25, 1.0), Ret { w: 300, h: 46, ..r });
        assert_eq!(na_escala(r, 1.5, 1.5), r);
        assert_eq!(na_escala(r, 0.0, 1.25), r, "escala desconhecida: fica");
    }

    #[test]
    fn a_escolha_e_tentada_enquanto_o_dock_esta_fora_dela() {
        // I1: a abertura caiu no padrão; o 2º monitor chega uns segundos depois, sem "mudaram"
        assert!(tentar_a_escolha(true, false, false));
        assert!(tentar_a_escolha(false, true, false));
        assert!(tentar_a_escolha(false, false, true));
        assert!(!tentar_a_escolha(false, false, false), "na escolha e nada mudou: não mexe");
        // o ciclo em que o monitor chegou: a escolha vale de novo
        let e = Ret { x: 3000, y: 1034, w: 300, h: 46 };
        assert_eq!(lugar_da_escolha(e, 240, 46, &[m1()]), None);
        assert_eq!(lugar_da_escolha(e, 240, 46, &dois()), Some((3060, 1034)));
    }

    #[test]
    fn o_retangulo_de_antes_de_maximizar_pela_area_do_monitor_dele() {
        let n = Ret { x: 100, y: 50, w: 1200, h: 800 };
        // a barra embaixo: a área útil começa no canto da tela, nada muda
        assert_eq!(da_area_de_trabalho(n, &m1()), n);
        // a barra em cima (48 px) e à esquerda (60 px) no monitor da janela
        let m = Monitor { util: Ret { x: 1980, y: 48, w: 1860, h: 1032 }, ..m2() };
        assert_eq!(da_area_de_trabalho(Ret { x: 200, ..n }, &m), Ret { x: 260, y: 98, w: 1200, h: 800 });
    }

    #[test]
    fn ancorar_segura_a_borda_mais_perto() {
        let tela = Ret { x: 0, y: 0, w: 1920, h: 1040 };
        let r = Ret { x: 1604, y: 16, w: 300, h: 46 };
        assert_eq!(lado(r, tela), Lado { direita: true, embaixo: false });
        assert_eq!(ancorar(r, 320, 420, tela), (1584, 16));
        let r = Ret { x: 20, y: 980, w: 300, h: 46 };
        assert_eq!(ancorar(r, 320, 420, tela), (20, 606));
        let r = Ret { x: 900, y: 0, w: 100, h: 46 };
        assert_eq!(ancorar(r, 1100, 46, tela), (820, 0));
        let r = Ret { x: 0, y: 0, w: 100, h: 46 };
        assert_eq!(ancorar(r, 400, 46, Ret { x: 0, y: 0, w: 300, h: 500 }), (0, 0));
    }

    #[test]
    fn posicao_inicial_com_a_escolha_em_cima_da_barra() {
        let e = Ret { x: 3000, y: 1034, w: 300, h: 46 };
        // o app abre com a pílula de 240×46: a borda direita e a de baixo ficam
        assert_eq!(posicao_inicial(Some(e), 240, 46, &dois(), 16), (3060, 1034));
        // o 2º monitor ainda não voltou (login, DisplayPort): o padrão do principal
        assert_eq!(posicao_inicial(Some(e), 240, 46, &[m1()], 16), (1664, 16));
        // sem escolha: o padrão, na área útil
        assert_eq!(posicao_inicial(None, 300, 46, &dois(), 16), (1604, 16));
        // a margem em pixels físicos vem de fora (a 150%, 24)
        assert_eq!(posicao_inicial(None, 480, 69, &dois(), 24), (1416, 24));
        // aparece só uma lasquinha: o padrão
        assert_eq!(posicao_inicial(Some(Ret { x: 1910, y: 10, w: 300, h: 46 }), 300, 46, &[m1()], 16), (1604, 16));
    }

    #[test]
    fn a_escolha_grande_volta_como_a_pilula_no_mesmo_canto() {
        // A07: a pessoa arrastou com o cartão da pergunta aberto (400×330, embaixo à direita)
        let e = Ret { x: 1500, y: 700, w: 400, h: 330 };
        // reabre com a pílula: a borda direita (1900) e a de baixo (1030) ficam
        assert_eq!(posicao_inicial(Some(e), 300, 46, &dois(), 16), (1600, 984));
        // em cima e à esquerda: o canto de cima à esquerda fica
        let e = Ret { x: 40, y: 30, w: 400, h: 330 };
        assert_eq!(posicao_inicial(Some(e), 300, 46, &dois(), 16), (40, 30));
    }

    #[test]
    fn a_escolha_de_antes_usa_o_tamanho_inicial() {
        assert_eq!(escolha_guardada(Some(10), Some(20), None, None, (240, 46)), Some(Ret { x: 10, y: 20, w: 240, h: 46 }));
        assert_eq!(escolha_guardada(Some(10), Some(20), Some(300), Some(50), (240, 46)), Some(Ret { x: 10, y: 20, w: 300, h: 50 }));
        assert_eq!(escolha_guardada(Some(10), Some(20), Some(0), Some(50), (240, 46)).map(|r| r.w), Some(240));
        assert_eq!(escolha_guardada(None, Some(20), Some(300), Some(50), (240, 46)), None);
    }

    #[test]
    fn so_o_arrastar_da_pessoa_grava() {
        let pedido = Ret { x: 3000, y: 1034, w: 300, h: 46 };
        assert!(!e_da_pessoa(false, Ret { x: 1520, y: 702, ..pedido }, Some(pedido)), "o app ou o Windows moveram");
        assert!(!e_da_pessoa(true, pedido, Some(pedido)), "o próprio pedido, com um clique por acaso");
        assert!(e_da_pessoa(true, Ret { x: 2000, ..pedido }, Some(pedido)));
        assert!(e_da_pessoa(true, pedido, None));
        // nunca minimizado nem fora dos monitores
        assert!(!pode_gravar(Ret { x: -32000, y: -32000, w: 160, h: 28 }, &dois()));
        assert!(!pode_gravar(Ret { x: 5000, y: 100, w: 300, h: 46 }, &dois()));
        assert!(pode_gravar(pedido, &dois()), "em cima da barra vale");
        assert!(!pode_gravar(pedido, &[m1()]), "o 2º monitor saiu da lista");
    }

    #[test]
    fn a_escolha_volta_quando_o_monitor_volta() {
        let e = Ret { x: 3000, y: 1034, w: 300, h: 46 };
        assert_eq!(lugar_da_escolha(e, 300, 46, &[m1()]), None, "o 2º apagou");
        assert_eq!(lugar_da_escolha(e, 300, 46, &dois()), Some((3000, 1034)), "voltou");
        assert_eq!(lugar_da_escolha(e, 320, 70, &dois()), Some((2980, 1010)), "no tamanho de agora");
    }

    #[test]
    fn resgate_no_monitor_do_mouse() {
        assert_eq!(posicao_do_resgate(Some((2500, 500)), 300, 46, &dois(), 16), Some((3524, 16)));
        assert_eq!(posicao_do_resgate(Some((100, 500)), 300, 46, &dois(), 16), Some((1604, 16)));
        assert_eq!(posicao_do_resgate(Some((9000, 500)), 300, 46, &dois(), 16), Some((1604, 16)), "fora: o principal");
        assert_eq!(posicao_do_resgate(None, 300, 46, &dois(), 16), Some((1604, 16)));
        assert_eq!(posicao_do_resgate(None, 300, 46, &[], 16), None);
        // o principal é o marcado, não o primeiro da lista
        assert_eq!(principal(&[m2(), m1()]).map(|m| m.tela), Some(m1().tela));
    }

    #[test]
    fn a_janela_principal_volta_ao_lugar_dela() {
        let s = Ret { x: 2200, y: 100, w: 1200, h: 800 };
        assert_eq!(lugar_da_principal(Some(s), &dois()), Some(s));
        assert_eq!(lugar_da_principal(Some(s), &[m1()]), None, "o 2º não está: o padrão");
        assert_eq!(lugar_da_principal(None, &dois()), None);
        // maior que o monitor: cortada à área útil
        let grande = Ret { x: 0, y: 0, w: 2500, h: 1200 };
        assert_eq!(lugar_da_principal(Some(grande), &dois()), Some(Ret { x: 0, y: 0, w: 1920, h: 1032 }));
    }

    #[test]
    fn pedida_pelo_dock_a_janela_vai_para_o_monitor_do_dock() {
        let dock = Ret { x: 3000, y: 1034, w: 300, h: 46 };
        let p = Ret { x: 100, y: 100, w: 1200, h: 800 };
        assert_eq!(principal_perto_do_dock(p, dock, &dois()), Some(Ret { x: 2280, y: 116, w: 1200, h: 800 }));
        // já no monitor do dock: fica
        assert_eq!(principal_perto_do_dock(Ret { x: 2000, ..p }, dock, &dois()), None);
        // fora de tudo: vai também
        assert!(principal_perto_do_dock(Ret { x: 9000, ..p }, dock, &dois()).is_some());
        assert_eq!(principal_perto_do_dock(p, dock, &[]), None);
    }

    #[test]
    fn acordou_depois_de_30_s_sem_ciclo() {
        assert!(!acordou(None, 100_000));
        assert!(!acordou(Some(95_000), 100_000));
        assert!(!acordou(Some(70_000), 100_000), "30 s ainda não");
        assert!(acordou(Some(69_999), 100_000));
        assert!(!acordou(Some(200_000), 100_000), "o relógio voltou");
    }

    #[test]
    fn sem_sinal_de_vida_recarrega_uma_vez_a_cada_2_min() {
        let t0 = 1_000_000;
        let mut v = Vida::nova(t0);
        // responde sempre: nunca recarrega
        for i in 1..20 {
            assert!(!v.ping(t0 + i * 5_000));
            v.respondeu(t0 + i * 5_000 + 10);
        }
        // calada: 30 s e mais de 3 pings sem volta
        let t1 = t0 + 100_000;
        let mut v = Vida::nova(t1);
        let mut recarregou = vec![];
        for i in 1..=60 {
            if v.ping(t1 + i * 5_000) {
                recarregou.push(i * 5_000);
            }
        }
        assert_eq!(recarregou, vec![30_000, 150_000, 270_000], "a 1ª depois de 30 s; depois, a cada 2 min");
        // um só ping atrasado (o laço demorou) não recarrega
        let mut v = Vida::nova(t0);
        assert!(!v.ping(t0 + 60_000));
        // escondido: a conta recomeça
        let mut v = Vida::nova(t0);
        for i in 1..=5 {
            v.ping(t0 + i * 5_000);
        }
        v.em_pausa(t0 + 26_000);
        assert!(!v.ping(t0 + 31_000));
        // o recarregar que não resolve fica marcado até a página responder (M11)
        let mut v = Vida::nova(t0);
        let mut i = 1;
        while !v.ping(t0 + i * 5_000) {
            i += 1;
        }
        assert!(v.sem_resposta_desde_recarregar);
        v.ping(t0 + i * 5_000 + 5_000);
        assert!(v.sem_resposta_desde_recarregar, "um ping sem volta não tira a marca");
        v.respondeu(t0 + i * 5_000 + 6_000);
        assert!(!v.sem_resposta_desde_recarregar);
    }

    #[test]
    fn o_log_tem_so_numeros() {
        let f = FotoDoDock { visivel: true, ret: Some(Ret { x: 3000, y: 1034, w: 300, h: 46 }), minimizado: false, oculto: false, monitores: dois() };
        assert_eq!(
            linha_da_foto("2026-11-27 18:00:00", &f),
            "2026-11-27 18:00:00 estado visivel=1 ret=3000,1034,300x46 min=0 oculto=0 monitores=[*0,0,1920x1080/util 0,0,1920x1032@1 | 1920,0,1920x1080/util 1920,0,1920x1032@1]"
        );
        assert_eq!(linha_da_acao("h", "resgate", None), "h resgate");
        assert_eq!(linha_da_acao("h", "arrastado", Some(Ret { x: 1, y: 2, w: 3, h: 4 })), "h arrastado ret=1,2,3x4");
    }
}
