// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! UI Automation da janela do app do Claude: a conversa aberta e se você está escrevendo nela.
//! Nunca entra em pânico: falhas viram `None`.
//!
//! O custo não cresce com a conversa:
//! 1. com a janela em foco, sobe-se do elemento com foco até o primeiro `Document` de conversa
//!    (~10 idas ao outro processo, com tipo e nome em cache);
//! 2. com a janela em foco e o foco fora de uma conversa (a barra lateral, um botão: é quando se
//!    troca de conversa ou se abre outra ao lado), a busca na árvore inteira (`FindAll`, ~50 ms
//!    numa conversa longa), para não valer uma lista velha;
//! 3. com a janela fora de foco (só debaixo do mouse: a conversa não troca), os `Document`s de
//!    conversa guardados da última busca, conferidos com uma ida cada (nome e se estão à vista).
//!    Só valem os de uma busca feita na última amostra em foco ou depois (cada amostra em foco
//!    descarta a lista guardada);
//!    a busca inteira só se a guarda falhar, não mostrar conversa ou passar de
//!    `REFAZER_DOCUMENTOS`. A escolha é `conversa::caminho_da_consulta`.
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};

use windows::core::Interface;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_I4};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationCacheRequest, IUIAutomationCondition,
    IUIAutomationElement, TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_DocumentControlTypeId,
    UIA_IsOffscreenPropertyId, UIA_NamePropertyId, UIA_ProcessIdPropertyId,
};
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

use crate::conversa::{
    caminho_da_consulta, conversa_acima, e_campo_de_texto, ler_conversa, titulo_da_conversa, Caminho, ConversaLida, Documento,
    LIMITE_CONSULTA,
};

/// Subir do elemento com foco até a conversa dele: no app do Claude são ~8 níveis.
const MAX_NIVEIS: usize = 64;
/// Os `Document`s de conversa guardados valem por isto; depois, a busca inteira roda de novo
/// (uma conversa que apareceu ao lado, na vista dividida, entra no máximo com este atraso).
pub const REFAZER_DOCUMENTOS: Duration = Duration::from_secs(30);

/// Os `Document`s de conversa achados na última busca inteira.
struct Guardados {
    hwnd: isize,
    elementos: Vec<IUIAutomationElement>,
    desde: Instant,
}

pub struct Uia {
    auto: IUIAutomation,
    so_documentos: IUIAutomationCondition,
    /// Dos `Document`s: o nome e se está à vista.
    nome_e_vista: IUIAutomationCacheRequest,
    /// Do elemento com foco: o tipo (campo de texto?) e o processo (é o app do Claude?).
    tipo_e_processo: IUIAutomationCacheRequest,
    /// Dos ancestrais do foco: o tipo e o nome (só o de um `Document` importa).
    tipo_e_nome: IUIAutomationCacheRequest,
    guardados: Option<Guardados>,
    /// Quantas buscas inteiras rodaram (para medir).
    pub buscas: u64,
    /// Quantas leituras foram por cada caminho (para medir): pelo foco, guardados, busca inteira.
    pub por_caminho: [u64; 3],
}

fn variant_i4(v: i32) -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_I4,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { lVal: v },
            }),
        },
    }
}

fn pedido(auto: &IUIAutomation, props: &[windows::Win32::UI::Accessibility::UIA_PROPERTY_ID]) -> Option<IUIAutomationCacheRequest> {
    // SAFETY: interface COM válida.
    unsafe {
        let c = auto.CreateCacheRequest().ok()?;
        for p in props {
            c.AddProperty(*p).ok()?;
        }
        Some(c)
    }
}

impl Uia {
    /// Inicia o COM nesta thread (MTA, o recomendado para clientes de UI Automation) e cria o cliente.
    pub fn novo() -> Option<Uia> {
        // SAFETY: chamadas COM na thread que as inicia; o VARIANT é um inteiro (nada a liberar).
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let auto: IUIAutomation = CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER)
                .or_else(|_| CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER))
                .ok()?;
            // um app travado não segura a consulta pelos 20 s padrão
            if let Ok(a2) = auto.cast::<IUIAutomation2>() {
                let ms = LIMITE_CONSULTA.as_millis() as u32;
                let _ = a2.SetConnectionTimeout(ms);
                let _ = a2.SetTransactionTimeout(ms);
            }
            let so_documentos =
                auto.CreatePropertyCondition(UIA_ControlTypePropertyId, &variant_i4(UIA_DocumentControlTypeId.0)).ok()?;
            let nome_e_vista = pedido(&auto, &[UIA_NamePropertyId, UIA_IsOffscreenPropertyId])?;
            let tipo_e_processo = pedido(&auto, &[UIA_ControlTypePropertyId, UIA_ProcessIdPropertyId])?;
            let tipo_e_nome = pedido(&auto, &[UIA_ControlTypePropertyId, UIA_NamePropertyId])?;
            Some(Uia { auto, so_documentos, nome_e_vista, tipo_e_processo, tipo_e_nome, guardados: None, buscas: 0, por_caminho: [0; 3] })
        }
    }

    /// A conversa aberta na janela `hwnd` e se você está escrevendo nela. `em_foco`: a janela é a
    /// que tem o foco (só então o elemento com foco é consultado: debaixo do mouse sem o foco, ele
    /// está em outro programa, que pode estar travado). O texto do campo nunca é lido.
    pub fn ler(&mut self, hwnd: isize, em_foco: bool) -> Option<ConversaLida> {
        // Com a janela em foco, a lista guardada é descartada: pelo foco ela não é renovada, e a
        // conversa pode trocar ou dividir (por um atalho, com o foco já dentro da nova). A primeira
        // amostra fora de foco faz a busca inteira de novo; a busca em foco guarda a dela.
        if em_foco {
            self.guardados = None;
        }
        let pelo_foco = if em_foco { self.pelo_foco(hwnd) } else { None };
        // os guardados só são conferidos com a janela fora de foco
        let guardados = if em_foco { None } else { self.guardados_validos(hwnd) };
        let caminho = caminho_da_consulta(em_foco, pelo_foco.is_some(), guardados.is_some());
        self.por_caminho[caminho as usize] += 1;
        match caminho {
            Caminho::PeloFoco => pelo_foco,
            Caminho::Guardados => ler_conversa(&guardados?),
            // sem o foco numa conversa: a única à vista (com várias, nenhuma, como sempre)
            Caminho::BuscaInteira => ler_conversa(&self.buscar(hwnd)?),
        }
    }

    /// Sobe do elemento com foco (se ele é do app) até o primeiro `Document` de conversa.
    fn pelo_foco(&self, hwnd: isize) -> Option<ConversaLida> {
        let mut processo = 0u32;
        // SAFETY: só lê o processo dono da janela; um HWND que sumiu dá 0.
        unsafe { GetWindowThreadProcessId(HWND(hwnd as *mut c_void), Some(&mut processo)) };
        // SAFETY: interfaces COM válidas.
        unsafe {
            let foco = self.auto.GetFocusedElementBuildCache(&self.tipo_e_processo).ok()?;
            if processo == 0 || foco.CachedProcessId().ok()? as u32 != processo {
                return None;
            }
            let tipo = foco.CachedControlType().ok()?.0;
            let campo = e_campo_de_texto(tipo);
            let subir = self.auto.ControlViewWalker().ok()?;
            let mut acima: Vec<(bool, String)> = vec![];
            // o próprio foco pode ser o Document da conversa (o clique na lista de mensagens)
            if tipo == UIA_DocumentControlTypeId.0 {
                acima.push((true, foco.CurrentName().map(|b| b.to_string()).unwrap_or_default()));
            }
            let mut e = foco;
            for _ in 0..MAX_NIVEIS {
                if let Some(t) = conversa_acima(&acima) {
                    return Some(ConversaLida { titulo: t.to_string(), escrevendo: campo });
                }
                let Ok(pai) = subir.GetParentElementBuildCache(&e, &self.tipo_e_nome) else { break };
                let doc = pai.CachedControlType().is_ok_and(|t| t.0 == UIA_DocumentControlTypeId.0);
                let nome = if doc { pai.CachedName().map(|b| b.to_string()).unwrap_or_default() } else { String::new() };
                acima.push((doc, nome));
                e = pai;
            }
            conversa_acima(&acima).map(|t| ConversaLida { titulo: t.to_string(), escrevendo: campo })
        }
    }

    /// Os `Document`s de conversa guardados, conferidos agora, se ainda valem: da mesma janela,
    /// com menos de `REFAZER_DOCUMENTOS`, todos existindo e algum à vista.
    fn guardados_validos(&self, hwnd: isize) -> Option<Vec<Documento>> {
        let g = self.guardados.as_ref().filter(|g| g.hwnd == hwnd && g.desde.elapsed() < REFAZER_DOCUMENTOS)?;
        self.conferir(&g.elementos).filter(|v| !v.is_empty())
    }

    /// Nome atual dos guardados, sem os que saíram de vista. Um que sumiu: `None` (busca de novo).
    fn conferir(&self, elementos: &[IUIAutomationElement]) -> Option<Vec<Documento>> {
        let mut v = vec![];
        for e in elementos {
            // SAFETY: interfaces COM válidas; o elemento pode ter sumido, e aí a chamada só falha.
            let e = unsafe { e.BuildUpdatedCache(&self.nome_e_vista) }.ok()?;
            if let Some(d) = documento(&e) {
                v.push(d);
            }
        }
        Some(v)
    }

    /// A busca inteira dos `Document`s da janela; guarda os de conversa.
    fn buscar(&mut self, hwnd: isize) -> Option<Vec<Documento>> {
        self.guardados = None;
        self.buscas += 1;
        // SAFETY: interfaces COM válidas; o HWND pode ter sumido, e aí a chamada só falha.
        unsafe {
            let raiz = self.auto.ElementFromHandle(HWND(hwnd as *mut c_void)).ok()?;
            let achados = raiz.FindAllBuildCache(TreeScope_Descendants, &self.so_documentos, &self.nome_e_vista).ok()?;
            let mut docs = vec![];
            let mut elementos = vec![];
            for i in 0..achados.Length().ok()? {
                let e = achados.GetElement(i).ok()?;
                if let Some(d) = documento(&e) {
                    docs.push(d);
                    elementos.push(e);
                }
            }
            self.guardados = Some(Guardados { hwnd, elementos, desde: Instant::now() });
            Some(docs)
        }
    }
}

/// O `Document` (com nome e visibilidade em cache) se é uma conversa à vista.
fn documento(e: &IUIAutomationElement) -> Option<Documento> {
    // SAFETY: interface COM válida, com o cache pedido.
    unsafe {
        let nome = e.CachedName().map(|b| b.to_string()).unwrap_or_default();
        let fora = e.CachedIsOffscreen().is_ok_and(|b| b.as_bool());
        (!fora && titulo_da_conversa(&nome).is_some()).then_some(Documento { nome, ..Default::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversa::LeitorConversa;
    use windows::core::w;
    use windows::Win32::UI::WindowsAndMessaging::{FindWindowW, GetForegroundWindow};

    fn janela_do_claude() -> isize {
        // SAFETY: só procura a janela pelo nome da classe e o título.
        unsafe { FindWindowW(w!("Chrome_WidgetWin_1"), w!("Claude")) }.expect("app do Claude aberto").0 as isize
    }

    fn em_foco(hwnd: isize) -> bool {
        // SAFETY: só lê a janela em primeiro plano.
        unsafe { GetForegroundWindow() }.0 as isize == hwnd
    }

    fn mediana(mut v: Vec<u128>) -> (u128, u128) {
        v.sort_unstable();
        (v[v.len() / 2], v[v.len() - 1])
    }

    /// Contra o app do Claude aberto (janela "Claude"): mede a consulta e diz se reconheceu uma
    /// conversa, sem mostrar o título. `cargo test -p sinais fumaça -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fumaça_conversa_do_app_do_claude() {
        let hwnd = janela_do_claude();
        let mut uia = Uia::novo().expect("UI Automation");
        // a consulta de cada caminho: pelo foco (com a janela em foco), pelos guardados e a busca inteira
        let (mut pelo_foco, mut guardados, mut busca) = (vec![], vec![], vec![]);
        let mut lida = None;
        for _ in 0..10 {
            let t = Instant::now();
            lida = uia.ler(hwnd, true);
            pelo_foco.push(t.elapsed().as_millis());
            let t = Instant::now();
            let _ = uia.ler(hwnd, false);
            guardados.push(t.elapsed().as_millis());
            let t = Instant::now();
            let _ = uia.buscar(hwnd);
            busca.push(t.elapsed().as_millis());
        }
        let foco = em_foco(hwnd);
        println!("janela em foco: {foco}; reconheceu: {}, escrevendo: {}", lida.is_some(), lida.as_ref().is_some_and(|c| c.escrevendo));
        let ((a, am), (b, bm), (c, cm)) = (mediana(pelo_foco), mediana(guardados), mediana(busca));
        println!("com em_foco: mediana {a} ms, máx {am} ms; sem em_foco (guardados): mediana {b} ms, máx {bm} ms; busca inteira: mediana {c} ms, máx {cm} ms");
        let mut l = LeitorConversa::novo();
        let mut tempos = vec![];
        for _ in 0..10 {
            let t = Instant::now();
            let _ = l.ler(hwnd, foco, LIMITE_CONSULTA);
            tempos.push(t.elapsed().as_millis());
        }
        let (m, mx) = mediana(tempos);
        println!("pelo leitor: mediana {m} ms, máx {mx} ms");
    }

    /// Amostra o app do Claude a cada 2 s por `WT_SONDA_S` segundos (padrão 120), como o laço (com
    /// a janela em foco ou não), e conta as amostras em cada situação, sem mostrar títulos nem
    /// texto. `cargo test -p sinais sonda_escrevendo -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sonda_escrevendo() {
        let hwnd = janela_do_claude();
        let mut uia = Uia::novo().expect("UI Automation");
        let segundos: u64 = std::env::var("WT_SONDA_S").ok().and_then(|v| v.parse().ok()).unwrap_or(120);
        let (mut sem, mut lendo, mut escrevendo, mut foco, mut tempos) = (0, 0, 0, 0, vec![]);
        for _ in 0..segundos / 2 {
            let f = em_foco(hwnd);
            foco += f as u32;
            let t = Instant::now();
            match uia.ler(hwnd, f) {
                None => sem += 1,
                Some(c) if c.escrevendo => escrevendo += 1,
                Some(_) => lendo += 1,
            }
            tempos.push(t.elapsed().as_millis());
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
        tempos.sort_unstable();
        let n = tempos.len();
        let acima = tempos.iter().filter(|t| **t > LIMITE_CONSULTA.as_millis()).count();
        println!(
            "amostras {n} ({foco} com a janela em foco): sem conversa {sem}, lendo {lendo}, escrevendo {escrevendo}; mediana {} ms, p90 {} ms, máx {} ms, acima de 300 ms {acima}; buscas inteiras {}; pelo foco / guardados / busca: {:?}",
            tempos[n / 2],
            tempos[n * 9 / 10],
            tempos[n - 1],
            uia.buscas,
            uia.por_caminho
        );
    }
}
