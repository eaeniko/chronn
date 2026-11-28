// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Ícone da bandeja: menu, clique, ícone de "rodando" e dica com a contagem.
use nucleo::idioma::{ta, tfa};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle,
};

use crate::janelas;

pub const ID: &str = "principal";

const ICONE_PARADO: Image<'static> = tauri::include_image!("icons/32x32.png");
const ICONE_RODANDO: Image<'static> = tauri::include_image!("icons/rodando/32x32.png");

/// Ícone mostrado agora: 0 = nenhum ainda, 1 = parado, 2 = rodando.
static ICONE_ATUAL: AtomicU8 = AtomicU8::new(0);

/// O item do dock no menu (um só, com o texto do modo) e o texto mostrado agora (0 = nenhum
/// ainda, 1 = sempre à vista, 2 = só com pergunta).
static ITEM_DOCK: OnceLock<MenuItem<tauri::Wry>> = OnceLock::new();
static ITEM_DOCK_MODO: AtomicU8 = AtomicU8::new(0);
/// O "Trazer o dock para cá" (spec 006), para trocar o idioma com o menu montado.
static ITEM_TRAZER: OnceLock<MenuItem<tauri::Wry>> = OnceLock::new();

/// Os itens de texto fixo (Abrir, Pausar tudo, Sobre, Sair), para trocar o idioma com o menu
/// montado (rodada 23), e o idioma deles (0 = português, 1 = inglês).
static ITENS: OnceLock<[MenuItem<tauri::Wry>; 4]> = OnceLock::new();
static ITENS_IDIOMA: AtomicU8 = AtomicU8::new(0);

/// Os textos dos itens fixos, na ordem de `ITENS`.
pub fn textos_dos_itens() -> [String; 4] {
    [ta("bandeja.abrir").to_string(), ta("bandeja.pausar_tudo").to_string(), texto_do_sobre(), ta("bandeja.sair").to_string()]
}

/// O idioma mudou: o menu já montado recebe os textos novos (os fixos e o do dock).
fn atualizar_idioma() {
    let quero = if nucleo::idioma::atual() == nucleo::idioma::Idioma::En { 1 } else { 0 };
    if ITENS_IDIOMA.swap(quero, Ordering::SeqCst) == quero {
        return;
    }
    if let Some(itens) = ITENS.get() {
        for (item, texto) in itens.iter().zip(textos_dos_itens()) {
            let _ = item.set_text(texto);
        }
    }
    if let Some(item) = ITEM_DOCK.get() {
        let _ = item.set_text(texto_do_item_dock(ITEM_DOCK_MODO.load(Ordering::SeqCst) == 2));
    }
    if let Some(item) = ITEM_TRAZER.get() {
        let _ = item.set_text(ta("bandeja.dock_trazer"));
    }
}

/// O texto do item do dock (revisão da rodada 12): "Mostrar/esconder dock" com o dock "Sempre à
/// vista"; "Mostrar o dock agora" com ele "Só quando houver uma pergunta".
pub fn texto_do_item_dock(so_perguntas: bool) -> &'static str {
    ta(if so_perguntas { "bandeja.dock_agora" } else { "bandeja.dock_alternar" })
}

/// Troca o texto do item do dock quando o modo muda (`janelas::aplicar_dock`).
pub fn atualizar_item_dock(_app: &AppHandle, so_perguntas: bool) {
    let quero = if so_perguntas { 2 } else { 1 };
    if ITEM_DOCK_MODO.swap(quero, Ordering::SeqCst) == quero {
        return;
    }
    if let Some(item) = ITEM_DOCK.get() {
        let _ = item.set_text(texto_do_item_dock(so_perguntas));
    }
}

/// "Chronn · N rodando", ou só "Chronn" (o nome de exibição, `APP_NOME`).
pub fn dica(rodando: usize) -> String {
    if rodando == 0 {
        crate::perfil::nome_na_tela()
    } else {
        tfa("bandeja.rodando", &[&crate::perfil::nome_na_tela(), &rodando.to_string()])
    }
}

/// O item "Sobre o Chronn · alpha" da bandeja (o canal da rodada 25).
pub fn texto_do_sobre() -> String {
    format!("{} · {}", tfa("bandeja.sobre", &[]), nucleo::CANAL)
}

pub fn criar(app: &AppHandle) -> tauri::Result<()> {
    let [t_abrir, t_pausar, t_sobre, t_sair] = textos_dos_itens();
    ITENS_IDIOMA.store(if nucleo::idioma::atual() == nucleo::idioma::Idioma::En { 1 } else { 0 }, Ordering::SeqCst);
    let abrir = MenuItem::with_id(app, "abrir", t_abrir, true, None::<&str>)?;
    let pausar = MenuItem::with_id(app, "pausar_tudo", t_pausar, true, None::<&str>)?;
    // um item só para o dock, com o texto do modo (`texto_do_item_dock`, trocado em
    // `atualizar_item_dock`): no "Só quando houver uma pergunta", "Mostrar o dock agora" o mostra
    // até um clique fora ou 30 s sem o mouse em cima
    let dock = MenuItem::with_id(app, "dock", texto_do_item_dock(false), true, None::<&str>)?;
    let _ = ITEM_DOCK.set(dock.clone());
    // o resgate (spec 006): o dock no monitor do mouse, por cima de tudo
    let trazer = MenuItem::with_id(app, "trazer_dock", ta("bandeja.dock_trazer"), true, None::<&str>)?;
    let _ = ITEM_TRAZER.set(trazer.clone());
    ITEM_DOCK_MODO.store(1, Ordering::SeqCst);
    let sep = PredefinedMenuItem::separator(app)?;
    // rodada 18: a versão, os créditos e os links, em Ajustes → Sobre
    let sobre = MenuItem::with_id(app, "sobre", t_sobre, true, None::<&str>)?;
    let sair = MenuItem::with_id(app, "sair", t_sair, true, None::<&str>)?;
    let _ = ITENS.set([abrir.clone(), pausar.clone(), sobre.clone(), sair.clone()]);
    let menu = Menu::with_items(app, &[&abrir, &pausar, &dock, &trazer, &sep, &sobre, &sair])?;
    TrayIconBuilder::with_id(ID)
        .icon(ICONE_PARADO)
        .tooltip(crate::perfil::nome_na_tela())
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, evento| match evento.id().as_ref() {
            "abrir" => janelas::mostrar_principal(app),
            "pausar_tudo" => crate::comandos::pausar_tudo_agora(app),
            "dock" => janelas::item_dock(app),
            "trazer_dock" => janelas::trazer_dock(app),
            "sobre" => janelas::mostrar_sobre(app),
            "sair" => janelas::sair(app),
            _ => {}
        })
        .on_tray_icon_event(|icone, evento| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = evento {
                janelas::mostrar_principal(icone.app_handle());
            }
        })
        .build(app)?;
    ICONE_ATUAL.store(1, Ordering::SeqCst);
    Ok(())
}

/// O ícone `rodando` quando há algo ligado, e a dica com a contagem.
pub fn atualizar(app: &AppHandle, rodando: usize) {
    // a troca de idioma vale na hora também no menu já montado (rodada 23)
    atualizar_idioma();
    let Some(b) = app.tray_by_id(ID) else { return };
    let _ = b.set_tooltip(Some(dica(rodando)));
    let quero = if rodando > 0 { 2 } else { 1 };
    if ICONE_ATUAL.swap(quero, Ordering::SeqCst) != quero {
        let _ = b.set_icon(Some(if rodando > 0 { ICONE_RODANDO } else { ICONE_PARADO }));
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::APP_NOME;

    #[test]
    fn item_do_dock_com_o_texto_do_modo() {
        assert_eq!(texto_do_item_dock(false), "Mostrar/esconder dock");
        assert_eq!(texto_do_item_dock(true), "Mostrar o dock agora");
        assert_eq!(ta("bandeja.dock_trazer"), "Trazer o dock para cá");
    }

    #[test]
    fn item_sobre_com_o_nome() {
        assert_eq!(texto_do_sobre(), format!("Sobre o Chronn · {}", nucleo::CANAL));
        assert_eq!(texto_do_sobre(), "Sobre o Chronn · alpha");
    }

    #[test]
    fn dica_conta_os_rodando() {
        assert_eq!(dica(0), APP_NOME);
        assert_eq!(dica(3), format!("{APP_NOME} · 3 rodando"));
    }
}
