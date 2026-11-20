// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Leitura barata dos sinais do Windows. Nunca entra em pânico: falhas viram None / 0.
use crate::{JanelaInfo, normalizar_processo};
use windows::Win32::Foundation::{CloseHandle, HWND, POINT};
use windows::Win32::System::SystemInformation::GetTickCount64;
use windows::Win32::System::Threading::{
    OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, QueryFullProcessImageNameW,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::{
    GA_ROOT, GetAncestor, GetClassNameW, GetCursorPos, GetForegroundWindow, GetWindowTextW,
    GetWindowThreadProcessId, WindowFromPoint,
};
use windows::core::PWSTR;

/// Milissegundos desde a última entrada (teclado/mouse) do usuário.
pub fn ociosidade_ms() -> i64 {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    // SAFETY: `info` é válido e com cbSize correto.
    if !unsafe { GetLastInputInfo(&mut info) }.as_bool() {
        return 0;
    }
    // dwTime é um tick de 32 bits: diferença com aritmética circular.
    let agora = unsafe { GetTickCount64() } as u32;
    agora.wrapping_sub(info.dwTime) as i64
}

pub fn posicao_mouse() -> (i32, i32) {
    let mut p = POINT::default();
    // SAFETY: endereço válido de um POINT.
    if unsafe { GetCursorPos(&mut p) }.is_ok() { (p.x, p.y) } else { (0, 0) }
}

pub fn janela_em_foco() -> Option<JanelaInfo> {
    // SAFETY: sem pré-condições; HWND nulo é tratado em `info_da_janela`.
    info_da_janela(unsafe { GetForegroundWindow() })
}

pub fn janela_sob_mouse() -> Option<JanelaInfo> {
    let (x, y) = posicao_mouse();
    // WindowFromPoint pode devolver um widget filho (ex.: Electron): sobe até a raiz.
    let filha = unsafe { WindowFromPoint(POINT { x, y }) };
    if filha.0.is_null() {
        return None;
    }
    let raiz = unsafe { GetAncestor(filha, GA_ROOT) };
    info_da_janela(if raiz.0.is_null() { filha } else { raiz })
}

fn info_da_janela(hwnd: HWND) -> Option<JanelaInfo> {
    if hwnd.0.is_null() {
        return None;
    }
    let mut buf = [0u16; 512];
    // SAFETY: buffer válido; o retorno é o nº de caracteres copiados.
    let n = unsafe { GetWindowTextW(hwnd, &mut buf) };
    let titulo = String::from_utf16_lossy(&buf[..n.max(0) as usize]);
    let mut cls = [0u16; 256];
    // SAFETY: buffer válido; o retorno é o nº de caracteres copiados.
    let n = unsafe { GetClassNameW(hwnd, &mut cls) };
    let classe = String::from_utf16_lossy(&cls[..n.max(0) as usize]);

    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return None;
    }
    let processo = normalizar_processo(&caminho_do_processo(pid)?);
    Some(JanelaInfo { processo, titulo, classe, pid, hwnd: hwnd.0 as isize, em_foco: false })
}

fn caminho_do_processo(pid: u32) -> Option<String> {
    // SAFETY: o handle aberto é sempre fechado antes de retornar.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut tam = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut tam);
        let _ = CloseHandle(h);
        r.ok()?;
        Some(String::from_utf16_lossy(&buf[..tam as usize]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore]
    fn fumaça_le_sinais_reais() {
        assert!(ociosidade_ms() < 3_600_000);
        assert!(janela_em_foco().is_some());
    }
}
