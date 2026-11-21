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
//!
//! O "escrevendo" (regra A, rodada 26) pede também o botão de enviar ativo: com o foco na caixa,
//! ele é procurado nos irmãos seguintes do campo e de até 3 ancestrais dele (a conversa fica antes
//! da caixa; os botões, depois), com `IRMAOS_DA_CAIXA` irmãos por nível e no máximo
//! `TETO_DA_CAIXA` de procura (passou: "lendo", sem tirar a conversa da amostra).
//!
//! O Chat (rodada 27): a conversa do Chat é um `Document` `"<título> - Claude"` cujo endereço (o
//! `Value`) é `https://claude.ai/chat/<id>`. O `Value` só é pedido de um `Document`: na busca, a
//! condição já é só `Document` (vai no cache); na subida pelo foco, só do `Document` de página do
//! Chat achado (nunca no cache dos ancestrais). Num campo (`Edit`), o `Value` seria o texto. Os
//! sinais do Chat (`sinais_do_chat`) vão em duas fases: primeiro só os `Document`s; os botões só
//! com a aba do Chat à vista, e deles só o nome dos itens da barra lateral e do "Stop response".
use std::ffi::c_void;
use std::mem::ManuallyDrop;
use std::time::{Duration, Instant};

use windows::core::{Interface, BSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::{VariantClear, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BSTR, VT_I4};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, CUIAutomation8, IUIAutomation, IUIAutomation2, IUIAutomationCacheRequest, IUIAutomationCondition,
    IUIAutomationElement, PropertyConditionFlags, PropertyConditionFlags_IgnoreCase, PropertyConditionFlags_None,
    TreeScope_Descendants, TreeScope_Subtree, UIA_ButtonControlTypeId, UIA_ClassNamePropertyId, UIA_PROPERTY_ID,
    UIA_ControlTypePropertyId, UIA_DocumentControlTypeId, UIA_IsEnabledPropertyId, UIA_IsOffscreenPropertyId,
    UIA_NamePropertyId, UIA_ProcessIdPropertyId, UIA_ValueValuePropertyId,
};
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

use crate::conversa::{
    aba_do_chat, caminho_da_consulta, classes_dos_itens, conversa_acima, conversa_da_pagina, e_botao_de_enviar,
    e_campo_de_texto, e_pagina_do_chat, escrevendo_na_caixa, ler_conversa, nomes_de_parar_resposta, sinais_do_chat,
    Acima, Caminho, ConversaLida, Documento, PecaDoChat, SinaisChat, LIMITE_CONSULTA,
};

/// Subir do elemento com foco até a conversa dele: no app do Claude são ~8 níveis.
const MAX_NIVEIS: usize = 64;
/// Os `Document`s de conversa guardados valem por isto; depois, a busca inteira roda de novo
/// (uma conversa que apareceu ao lado, na vista dividida, entra no máximo com este atraso).
pub const REFAZER_DOCUMENTOS: Duration = Duration::from_secs(30);
/// Ancestrais do campo cujos irmãos seguintes também são conferidos (além dos do próprio campo):
/// no Code o botão de enviar fica nos irmãos do 2º ancestral (às vezes do 3º); no Chat, nos do 1º
/// (medido na rodada 26).
const ANCESTRAIS_DA_CAIXA: usize = 3;
/// Irmãos seguintes conferidos em cada nível (no Code, o botão fica no 2º depois do editor).
const IRMAOS_DA_CAIXA: usize = 8;
/// Tempo máximo da procura do botão (medida: 3 a 13 ms). Passou: "não achado" ("lendo"), para a
/// amostra inteira não passar do `LIMITE_CONSULTA` e virar "não sei".
const TETO_DA_CAIXA: Duration = Duration::from_millis(100);

/// Os `Document`s de conversa achados na última busca inteira.
struct Guardados {
    hwnd: isize,
    elementos: Vec<IUIAutomationElement>,
    desde: Instant,
}

pub struct Uia {
    auto: IUIAutomation,
    so_documentos: IUIAutomationCondition,
    /// Dos `Document`s (só deles): o nome, se está à vista e o endereço (o `Value`).
    nome_e_vista: IUIAutomationCacheRequest,
    /// Dos `Button`s (a 2ª fase dos sinais do Chat): só a classe, para achar os itens da barra
    /// lateral sem ler o nome dos outros botões.
    so_classe: IUIAutomationCacheRequest,
    /// Dos itens da barra lateral e do "Stop response" (já filtrados pela classe ou pelo nome
    /// exato): o nome e a classe. Nunca o `Value`.
    pecas: IUIAutomationCacheRequest,
    /// Do elemento com foco: o tipo (campo de texto?) e o processo (é o app do Claude?).
    tipo_e_processo: IUIAutomationCacheRequest,
    /// Dos ancestrais do foco: o tipo e o nome (só o de um `Document` importa).
    tipo_e_nome: IUIAutomationCacheRequest,
    so_botoes: IUIAutomationCondition,
    /// Dos botões ao lado da caixa: o nome (é o de enviar?) e se está ativo.
    nome_e_ativo: IUIAutomationCacheRequest,
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

/// Uma condição "a propriedade `prop` é igual ao texto `texto`". O `VARIANT` (um BSTR) é liberado
/// aqui: a UI Automation guarda uma cópia.
fn condicao_de_texto(
    auto: &IUIAutomation,
    prop: UIA_PROPERTY_ID,
    texto: &str,
    flags: PropertyConditionFlags,
) -> Option<IUIAutomationCondition> {
    let mut v = VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_BSTR,
                wReserved1: 0,
                wReserved2: 0,
                wReserved3: 0,
                Anonymous: VARIANT_0_0_0 { bstrVal: ManuallyDrop::new(BSTR::from(texto)) },
            }),
        },
    };
    // SAFETY: interface COM válida; o `VARIANT` é um BSTR nosso, liberado pelo `VariantClear`.
    unsafe {
        let c = auto.CreatePropertyConditionEx(prop, &v, flags).ok();
        let _ = VariantClear(&mut v);
        c
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
            // o `Value` vai no cache só aqui: a condição da busca é só `Document`
            let nome_e_vista = pedido(&auto, &[UIA_NamePropertyId, UIA_IsOffscreenPropertyId, UIA_ValueValuePropertyId])?;
            let tipo_e_processo = pedido(&auto, &[UIA_ControlTypePropertyId, UIA_ProcessIdPropertyId])?;
            let tipo_e_nome = pedido(&auto, &[UIA_ControlTypePropertyId, UIA_NamePropertyId])?;
            let so_botoes =
                auto.CreatePropertyCondition(UIA_ControlTypePropertyId, &variant_i4(UIA_ButtonControlTypeId.0)).ok()?;
            let nome_e_ativo = pedido(&auto, &[UIA_NamePropertyId, UIA_IsEnabledPropertyId])?;
            let so_classe = pedido(&auto, &[UIA_ClassNamePropertyId])?;
            let pecas = pedido(&auto, &[UIA_NamePropertyId, UIA_ClassNamePropertyId])?;
            Some(Uia {
                auto,
                so_documentos,
                nome_e_vista,
                so_classe,
                pecas,
                tipo_e_processo,
                tipo_e_nome,
                so_botoes,
                nome_e_ativo,
                guardados: None,
                buscas: 0,
                por_caminho: [0; 3],
            })
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
            let o_campo = foco.clone();
            // o botão de enviar só é procurado com a conversa achada e o foco num campo
            let lida = |(chat, t): (Option<String>, &str)| ConversaLida {
                titulo: t.to_string(),
                escrevendo: escrevendo_na_caixa(campo, if campo { self.enviar_ativo(&o_campo) } else { None }),
                chat,
            };
            let subir = self.auto.ControlViewWalker().ok()?;
            let mut acima: Vec<Acima> = vec![];
            // o próprio foco pode ser o Document da conversa (o clique na lista de mensagens)
            if tipo == UIA_DocumentControlTypeId.0 {
                let nome = foco.CurrentName().map(|b| b.to_string()).unwrap_or_default();
                let endereco = if e_pagina_do_chat(&nome) { endereco_atual(&foco, tipo) } else { String::new() };
                acima.push((true, nome, endereco));
            }
            let mut e = foco;
            for _ in 0..MAX_NIVEIS {
                if let Some(c) = conversa_acima(&acima) {
                    return Some(lida(c));
                }
                let Ok(pai) = subir.GetParentElementBuildCache(&e, &self.tipo_e_nome) else { break };
                let tipo_pai = pai.CachedControlType().map(|t| t.0).unwrap_or(0);
                let doc = tipo_pai == UIA_DocumentControlTypeId.0;
                let nome = if doc { pai.CachedName().map(|b| b.to_string()).unwrap_or_default() } else { String::new() };
                // o endereço só deste `Document` (página do Chat), pedido agora: nunca no cache
                let endereco = if doc && e_pagina_do_chat(&nome) { endereco_atual(&pai, tipo_pai) } else { String::new() };
                acima.push((doc, nome, endereco));
                e = pai;
            }
            conversa_acima(&acima).map(lida)
        }
    }

    /// O botão de enviar ao lado do campo: ativo ou não; `None` se não foi achado (no Code, com a
    /// caixa vazia, ele não existe; com o Claude trabalhando, o "Stop" fica no lugar). Procura nos
    /// irmãos seguintes do campo e de até `ANCESTRAIS_DA_CAIXA` ancestrais, por no máximo
    /// `TETO_DA_CAIXA`.
    fn enviar_ativo(&self, campo: &IUIAutomationElement) -> Option<bool> {
        let inicio = Instant::now();
        // SAFETY: interfaces COM válidas; um elemento que sumiu só faz a chamada falhar.
        unsafe {
            let andar = self.auto.ControlViewWalker().ok()?;
            let mut p = campo.clone();
            for nivel in 0..=ANCESTRAIS_DA_CAIXA {
                let mut irmao = andar.GetNextSiblingElement(&p).ok();
                for _ in 0..IRMAOS_DA_CAIXA {
                    let Some(i) = irmao else { break };
                    if inicio.elapsed() > TETO_DA_CAIXA {
                        return None;
                    }
                    if let Ok(bs) = i.FindAllBuildCache(TreeScope_Subtree, &self.so_botoes, &self.nome_e_ativo) {
                        for k in 0..bs.Length().unwrap_or(0) {
                            let Ok(b) = bs.GetElement(k) else { continue };
                            if e_botao_de_enviar(&b.CachedName().map(|n| n.to_string()).unwrap_or_default()) {
                                return Some(b.CachedIsEnabled().is_ok_and(|v| v.as_bool()));
                            }
                        }
                    }
                    irmao = andar.GetNextSiblingElement(&i).ok();
                }
                if nivel < ANCESTRAIS_DA_CAIXA {
                    p = andar.GetParentElement(&p).ok()?;
                }
            }
            None
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

/// O `Document` (com nome, visibilidade e endereço em cache) se é uma conversa à vista, do Code
/// ou do Chat. O endereço só é guardado numa página do Chat.
fn documento(e: &IUIAutomationElement) -> Option<Documento> {
    // SAFETY: interface COM válida, com o cache pedido.
    unsafe {
        let nome = e.CachedName().map(|b| b.to_string()).unwrap_or_default();
        let fora = e.CachedIsOffscreen().is_ok_and(|b| b.as_bool());
        if fora {
            return None;
        }
        let endereco = if e_pagina_do_chat(&nome) { texto_do_variant(e.GetCachedPropertyValue(UIA_ValueValuePropertyId).ok()) } else { String::new() };
        conversa_da_pagina(&nome, &endereco).is_some().then_some(Documento { nome, endereco, ..Default::default() })
    }
}

/// O texto de um `VARIANT` (um BSTR), que é liberado aqui. Outro tipo (ou nada): vazio.
fn texto_do_variant(v: Option<VARIANT>) -> String {
    let Some(mut v) = v else { return String::new() };
    // SAFETY: o `VARIANT` veio da UI Automation; o BSTR só é lido com o tipo certo, e o
    // `VariantClear` libera o que ele tiver.
    unsafe {
        let texto =
            if v.Anonymous.Anonymous.vt == VT_BSTR { v.Anonymous.Anonymous.Anonymous.bstrVal.to_string() } else { String::new() };
        let _ = VariantClear(&mut v);
        texto
    }
}

/// O endereço (o `Value`) de um `Document`, pedido agora só dele. De outro tipo, nada: num campo,
/// o `Value` é o texto.
fn endereco_atual(e: &IUIAutomationElement, tipo: i32) -> String {
    if tipo != UIA_DocumentControlTypeId.0 {
        return String::new();
    }
    // SAFETY: interface COM válida; um elemento que sumiu só faz a chamada falhar.
    texto_do_variant(unsafe { e.GetCurrentPropertyValue(UIA_ValueValuePropertyId) }.ok())
}

impl Uia {
    /// Os sinais do Chat na janela `hwnd` do app do Claude (rodada 27), à vista ou não, em duas
    /// fases. `None` se a consulta falhar.
    /// 1. Só os `Document`s (nome, visibilidade e endereço em cache: a condição é só `Document`).
    ///    Sem a aba do Chat à vista (`aba_do_chat`), nenhum botão é pedido.
    /// 2. Com ela, os `Button`s só com a classe em cache (os itens da barra lateral, pelo começo da
    ///    classe) e, numa segunda busca, o nome só dos botões com a classe exata de um item ou com
    ///    o nome exato do "Stop response" (`nomes_de_parar_resposta`). O nome dos outros botões
    ///    (os das mensagens) nunca vem para cá.
    pub fn sinais_do_chat(&self, hwnd: isize) -> Option<SinaisChat> {
        // SAFETY: interfaces COM válidas; o HWND pode ter sumido, e aí a chamada só falha.
        unsafe {
            let raiz = self.auto.ElementFromHandle(HWND(hwnd as *mut c_void)).ok()?;
            let docs = raiz.FindAllBuildCache(TreeScope_Descendants, &self.so_documentos, &self.nome_e_vista).ok()?;
            let mut pecas = vec![];
            for i in 0..docs.Length().ok()? {
                let Ok(e) = docs.GetElement(i) else { continue };
                let nome = e.CachedName().map(|b| b.to_string()).unwrap_or_default();
                let fora = e.CachedIsOffscreen().is_ok_and(|b| b.as_bool());
                let endereco = if !fora && e_pagina_do_chat(&nome) {
                    texto_do_variant(e.GetCachedPropertyValue(UIA_ValueValuePropertyId).ok())
                } else {
                    String::new()
                };
                pecas.push(PecaDoChat::Pagina { nome, endereco, fora });
            }
            if !aba_do_chat(&pecas) {
                return Some(SinaisChat::default());
            }
            pecas.extend(self.botoes_do_chat(&raiz)?);
            Some(sinais_do_chat(&pecas))
        }
    }

    /// A 2ª fase dos sinais do Chat: os itens da barra lateral e o "Stop response", com o nome.
    fn botoes_do_chat(&self, raiz: &IUIAutomationElement) -> Option<Vec<PecaDoChat>> {
        // SAFETY: interfaces COM válidas; um elemento que sumiu só faz a chamada falhar.
        unsafe {
            let todos = raiz.FindAllBuildCache(TreeScope_Descendants, &self.so_botoes, &self.so_classe).ok()?;
            let mut classes = vec![];
            for i in 0..todos.Length().ok()? {
                let Ok(e) = todos.GetElement(i) else { continue };
                classes.push(e.CachedClassName().map(|b| b.to_string()).unwrap_or_default());
            }
            let classes = classes_dos_itens(classes.iter().map(String::as_str));
            let mut quais: Vec<Option<IUIAutomationCondition>> = vec![];
            for c in &classes {
                quais.push(Some(condicao_de_texto(&self.auto, UIA_ClassNamePropertyId, c, PropertyConditionFlags_None)?));
            }
            for n in nomes_de_parar_resposta() {
                quais.push(Some(condicao_de_texto(&self.auto, UIA_NamePropertyId, n, PropertyConditionFlags_IgnoreCase)?));
            }
            let quais = self.auto.CreateOrConditionFromNativeArray(&quais).ok()?;
            let condicao = self.auto.CreateAndCondition(&self.so_botoes, &quais).ok()?;
            let achados = raiz.FindAllBuildCache(TreeScope_Descendants, &condicao, &self.pecas).ok()?;
            let mut v = vec![];
            for i in 0..achados.Length().ok()? {
                let Ok(e) = achados.GetElement(i) else { continue };
                let nome = e.CachedName().map(|b| b.to_string()).unwrap_or_default();
                let classe = e.CachedClassName().map(|b| b.to_string()).unwrap_or_default();
                v.push(PecaDoChat::Botao { nome, classe });
            }
            Some(v)
        }
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

    /// Contra o app do Claude aberto: mede os sinais do Chat (rodada 27) e diz o que achou, sem
    /// mostrar títulos nem endereços. `cargo test -p sinais fumaça_sinais_do_chat -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn fumaça_sinais_do_chat() {
        let hwnd = janela_do_claude();
        let uia = Uia::novo().expect("UI Automation");
        let mut tempos = vec![];
        let mut ultimo = None;
        for _ in 0..20 {
            let t = Instant::now();
            ultimo = uia.sinais_do_chat(hwnd);
            tempos.push(t.elapsed().as_millis());
            std::thread::sleep(Duration::from_millis(200));
        }
        match &ultimo {
            None => println!("sinais: não sei"),
            Some(s) => println!(
                "aba do Chat: {}; conversa aberta: {}; Stop response: {}; barra lateral: {}; rodando: {}",
                s.aba_do_chat,
                s.aberta.is_some(),
                s.aberta.as_ref().is_some_and(|a| a.parando),
                s.barra,
                s.rodando.len()
            ),
        }
        tempos.sort_unstable();
        let n = tempos.len();
        println!("direto: mediana {} ms, p90 {} ms, máx {} ms", tempos[n / 2], tempos[n * 9 / 10], tempos[n - 1]);
        let mut l = LeitorConversa::do_chat();
        let mut tempos = vec![];
        let mut sem = 0;
        for _ in 0..20 {
            let t = Instant::now();
            sem += l.ler(hwnd, false, LIMITE_CONSULTA).is_none() as u32;
            tempos.push(t.elapsed().as_millis());
            std::thread::sleep(Duration::from_millis(200));
        }
        tempos.sort_unstable();
        println!(
            "pelo leitor: mediana {} ms, p90 {} ms, máx {} ms, sem resposta {sem} de {n}",
            tempos[n / 2],
            tempos[n * 9 / 10],
            tempos[n - 1]
        );
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

    /// Rodada 27, ao vivo: a cada `WT_SONDA_PASSO_MS` (padrão 1000) por `WT_SONDA_S` segundos
    /// (padrão 600), o que o próprio app leria do Chat (`sinais_do_chat` e `ler`), escrito em
    /// `WT_SONDA_SAIDA` (ou na tela) só quando muda. Sem títulos: só sim/não e contagens.
    /// `cargo test -p sinais sonda_chat_ao_vivo -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sonda_chat_ao_vivo() {
        use std::io::Write;
        let segundos: u64 = std::env::var("WT_SONDA_S").ok().and_then(|v| v.parse().ok()).unwrap_or(600);
        let passo: u64 = std::env::var("WT_SONDA_PASSO_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1000);
        let mut saida: Box<dyn Write> = match std::env::var("WT_SONDA_SAIDA") {
            Ok(p) => Box::new(std::fs::File::create(p).unwrap()),
            Err(_) => Box::new(std::io::stdout()),
        };
        let mut uia = Uia::novo().expect("UI Automation");
        let inicio = Instant::now();
        let (mut anterior, mut tempos) = (String::new(), vec![]);
        while inicio.elapsed() < Duration::from_secs(segundos) {
            // SAFETY: só procura a janela pela classe e o título.
            let h = unsafe { FindWindowW(w!("Chrome_WidgetWin_1"), w!("Claude")) }.ok().filter(|h| !h.0.is_null());
            let linha = match h {
                None => "janela do Claude não achada".to_string(),
                Some(h) => {
                    let hwnd = h.0 as isize;
                    let t = Instant::now();
                    let s = uia.sinais_do_chat(hwnd);
                    tempos.push(t.elapsed().as_millis());
                    let sinais = match &s {
                        None => "sinais: não sei".to_string(),
                        Some(s) => format!(
                            "aba do Chat {}, conversa aberta {}, Stop response {}, barra {}, rodando {}",
                            s.aba_do_chat,
                            s.aberta.is_some(),
                            s.aberta.as_ref().is_some_and(|a| a.parando),
                            s.barra,
                            s.rodando.len()
                        ),
                    };
                    let lida = match uia.ler(hwnd, em_foco(hwnd)) {
                        None => "sem conversa".to_string(),
                        Some(c) => format!("{} {}", if c.chat.is_some() { "Chat" } else { "Code" }, if c.escrevendo { "ESCREVENDO" } else { "lendo" }),
                    };
                    format!("{sinais} | ler: {lida}")
                }
            };
            if linha != anterior {
                let _ = writeln!(saida, "{:>4} s  {linha}", inicio.elapsed().as_secs());
                let _ = saida.flush();
                anterior = linha;
            }
            std::thread::sleep(Duration::from_millis(passo));
        }
        tempos.sort_unstable();
        let n = tempos.len().max(1);
        let _ = writeln!(
            saida,
            "=== {} consultas dos sinais: mediana {} ms, p90 {} ms, máx {} ms",
            tempos.len(),
            tempos.get(n / 2).copied().unwrap_or(0),
            tempos.get(n * 9 / 10).copied().unwrap_or(0),
            tempos.last().copied().unwrap_or(0)
        );
    }
}
