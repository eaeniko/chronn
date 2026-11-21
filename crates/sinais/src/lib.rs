// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Sinais do Windows: ociosidade, janela em foco, janela sob o mouse e a conversa aberta no
//! app do Claude. Privacidade: títulos só são devolvidos ao chamador; nunca são logados.

pub mod conversa;
#[cfg(test)]
mod sonda;
#[cfg(windows)]
pub mod uia;
#[cfg(windows)]
pub mod win;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JanelaInfo {
    pub processo: String,
    pub titulo: String,
    /// Classe da janela (ex.: `Chrome_WidgetWin_1` no app do Claude).
    pub classe: String,
    pub pid: u32,
    /// O `HWND` da janela raiz, para a UI Automation.
    pub hwnd: isize,
    /// É a janela com o foco (e não só a que está debaixo do mouse).
    pub em_foco: bool,
}

/// Só o nome do arquivo, minúsculo, sem `.exe`.
pub fn normalizar_processo(caminho_exe: &str) -> String {
    let nome = caminho_exe.rsplit(['\\', '/']).next().unwrap_or(caminho_exe);
    let minusculo = nome.to_lowercase();
    minusculo.strip_suffix(".exe").unwrap_or(&minusculo).to_string()
}

/// Janela sob o mouse se ele mexeu nos últimos 30 000 ms; senão, a em foco. A escolhida diz se
/// é a janela com o foco (`em_foco`).
pub fn escolher_janela_em_uso(
    foco: Option<JanelaInfo>,
    sob_mouse: Option<JanelaInfo>,
    mouse_mexeu_ha_ms: Option<i64>,
) -> Option<JanelaInfo> {
    let hwnd_foco = foco.as_ref().map(|j| j.hwnd);
    let escolhida = match mouse_mexeu_ha_ms {
        Some(ms) if ms <= 30_000 => sob_mouse.or(foco),
        _ => foco,
    };
    escolhida.map(|j| JanelaInfo { em_foco: Some(j.hwnd) == hwnd_foco, ..j })
}

/// Lembra a última posição do mouse e quando ela mudou.
#[cfg(windows)]
pub struct Amostrador {
    ultima_pos: Option<(i32, i32)>,
    mexeu_em_ms: Option<i64>,
}

#[cfg(windows)]
impl Amostrador {
    pub fn novo() -> Self {
        Self { ultima_pos: None, mexeu_em_ms: None }
    }

    /// Devolve (ociosidade em ms, janela em uso).
    pub fn amostrar(&mut self, agora_ms: i64) -> (i64, Option<JanelaInfo>) {
        let pos = win::posicao_mouse();
        if self.ultima_pos.is_some_and(|p| p != pos) {
            self.mexeu_em_ms = Some(agora_ms);
        }
        self.ultima_pos = Some(pos);
        let mexeu_ha = self.mexeu_em_ms.map(|t| agora_ms - t);
        let janela =
            escolher_janela_em_uso(win::janela_em_foco(), win::janela_sob_mouse(), mexeu_ha);
        (win::ociosidade_ms(), janela)
    }
}

#[cfg(windows)]
impl Default for Amostrador {
    fn default() -> Self {
        Self::novo()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn j(p: &str) -> JanelaInfo {
        JanelaInfo { processo: p.into(), titulo: "t".into(), classe: String::new(), pid: 1, hwnd: p.len() as isize, em_foco: false }
    }

    fn focada(p: &str) -> JanelaInfo {
        JanelaInfo { em_foco: true, ..j(p) }
    }

    #[test]
    fn mouse_recente_escolhe_janela_sob_mouse() {
        let r = escolher_janela_em_uso(Some(j("foco")), Some(j("mouse")), Some(10_000));
        assert_eq!(r, Some(j("mouse")), "debaixo do mouse, sem o foco");
        // o mouse sobre a própria janela com foco
        let r = escolher_janela_em_uso(Some(j("foco")), Some(j("foco")), Some(10_000));
        assert_eq!(r, Some(focada("foco")));
    }

    #[test]
    fn mouse_parado_escolhe_foco() {
        let r = escolher_janela_em_uso(Some(j("foco")), Some(j("mouse")), Some(45_000));
        assert_eq!(r, Some(focada("foco")));
        let r = escolher_janela_em_uso(Some(j("foco")), Some(j("mouse")), None);
        assert_eq!(r, Some(focada("foco")));
    }

    #[test]
    fn normaliza_nome_do_processo() {
        assert_eq!(
            normalizar_processo("C:\\Users\\x\\AppData\\Local\\Programs\\Microsoft VS Code\\Code.exe"),
            "code"
        );
    }
}
