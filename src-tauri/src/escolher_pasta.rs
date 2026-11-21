// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! O seletor de pasta do Windows (rodada 15: "Pastas deste timer" e "Trocar por uma subpasta…").
//! O diálogo comum do sistema (`IFileOpenDialog` com `FOS_PICKFOLDERS`), numa thread própria
//! (COM de apartamento único), modal à janela principal.

/// Abre o seletor numa pasta (`inicial`, se existir) e devolve a pasta escolhida, ou `None` se a
/// pessoa cancelou. `dono`: a janela que fica bloqueada enquanto o seletor está aberto (o HWND
/// como número, para atravessar a thread).
#[cfg(windows)]
pub fn escolher_pasta(dono: Option<isize>, inicial: Option<String>, titulo: String) -> Result<Option<String>, String> {
    std::thread::spawn(move || unsafe { abrir(dono, inicial.as_deref(), &titulo) })
        .join()
        .map_err(|_| nucleo::idioma::ta("erro.seletor_parou").to_string())?
}

#[cfg(not(windows))]
pub fn escolher_pasta(_dono: Option<isize>, _inicial: Option<String>, _titulo: String) -> Result<Option<String>, String> {
    Err(nucleo::idioma::ta("erro.seletor_so_windows").into())
}

/// SAFETY: chamada numa thread nova, que inicia e encerra o COM dela; os objetos COM saem antes do
/// `CoUninitialize` (o bloco interno).
#[cfg(windows)]
unsafe fn abrir(dono: Option<isize>, inicial: Option<&str>, titulo: &str) -> Result<Option<String>, String> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{ERROR_CANCELLED, HWND};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        COINIT_DISABLE_OLE1DDE,
    };
    use windows::Win32::UI::Shell::{
        FileOpenDialog, IFileOpenDialog, IShellItem, SHCreateItemFromParsingName, FOS_FORCEFILESYSTEM, FOS_PATHMUSTEXIST,
        FOS_PICKFOLDERS, SIGDN_FILESYSPATH,
    };

    let iniciou = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED | COINIT_DISABLE_OLE1DDE) }.is_ok();
    let r = (|| -> windows::core::Result<Option<String>> {
        let d: IFileOpenDialog = unsafe { CoCreateInstance(&FileOpenDialog, None, CLSCTX_INPROC_SERVER)? };
        unsafe {
            let o = d.GetOptions()?;
            d.SetOptions(o | FOS_PICKFOLDERS | FOS_FORCEFILESYSTEM | FOS_PATHMUSTEXIST)?;
            d.SetTitle(&HSTRING::from(titulo))?;
            if let Some(p) = inicial.filter(|p| std::path::Path::new(p).is_dir()) {
                if let Ok(item) = SHCreateItemFromParsingName::<_, _, IShellItem>(&HSTRING::from(p), None) {
                    let _ = d.SetFolder(&item);
                }
            }
            if let Err(e) = d.Show(dono.map(|h| HWND(h as *mut _))) {
                return if e.code() == ERROR_CANCELLED.to_hresult() { Ok(None) } else { Err(e) };
            }
            let item = d.GetResult()?;
            let nome = item.GetDisplayName(SIGDN_FILESYSPATH)?;
            let pasta = nome.to_string().map_err(|_| windows::core::Error::from_thread());
            CoTaskMemFree(Some(nome.0 as *const _));
            pasta.map(Some)
        }
    })();
    if iniciou {
        unsafe { CoUninitialize() };
    }
    r.map_err(|e| nucleo::idioma::tfa("erro.seletor_abrir", &[&e.message().to_string()]))
}
