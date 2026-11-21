// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Sondas da rodada 26 (só medição, só em teste, ignoradas por padrão): como o Chat e o Cowork do
//! app do Claude aparecem na UI Automation, se o botão de enviar muda de estado, se a barra lateral
//! diz que uma sessão está trabalhando e se o título do terminal traz o nome da sessão.
//!
//! Privacidade: nenhum título, nome de conversa, texto de mensagem ou da caixa é mostrado. Só saem
//! o sufixo de app do nome de um `Document` ("Claude Code", "Claude"…), o tamanho do resto, os
//! tipos e as classes dos elementos, o `AutomationId`, o `IsEnabled` e os nomes de botões que são
//! rótulos do app (enviar, parar…). O texto da caixa nunca é pedido (nada de `ValuePattern`).

/// Palavras de rótulo de botão do app. Um nome só sai inteiro se **todas** as palavras dele estão
/// aqui (fora "Model:" e "Permission mode:", que saem só com o primeiro pedaço); o resto sai só
/// como tamanho, para nenhum título da barra lateral nem linha da conversa escapar.
const ROTULOS: &[&str] = &[
    "send", "message", "enviar", "mensagem", "stop", "response", "parar", "resposta", "interromper", "submit",
    "cancel", "cancelar", "attach", "anexar", "add", "adicionar", "files", "connectors", "and", "more", "use",
    "voice", "mode", "dictate", "microphone", "tools", "upload", "queue", "this", "task", "nested", "session",
    "1", "2", "3",
];

/// Palavras de estado que a barra lateral pode pôr no fim do nome de um item.
const ESTADOS: &[&str] = &[
    "running", "executando", "rodando", "working", "trabalhando", "thinking", "pensando", "waiting", "aguardando",
    "esperando", "needs", "precisa", "idle", "unread", "done", "concluído", "concluída", "error", "erro", "paused",
    "pausado", "pausada", "awaiting", "input", "approval", "aprovação", "permission",
];

/// O nome de um botão, se é um rótulo do app (todas as palavras em `ROTULOS`); "Model: …" e
/// "Permission mode: …" saem só com o pedaço antes dos dois-pontos; o resto, só o tamanho.
pub fn rotulo_ou_tamanho(nome: &str) -> String {
    let m = nome.trim().to_lowercase();
    for prefixo in ["model:", "permission mode:"] {
        if m.starts_with(prefixo) {
            return format!("{:?} <{} car.>", &nome.trim()[..prefixo.len()], m.chars().count() - prefixo.chars().count());
        }
    }
    let palavras: Vec<&str> = m.split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).collect();
    if !palavras.is_empty() && palavras.iter().all(|p| ROTULOS.contains(p)) {
        format!("{:?}", nome.trim())
    } else {
        format!("<{} car.>", nome.chars().count())
    }
}

/// O sufixo de app do nome de um `Document` ("… - Claude Code" → "Claude Code") e o tamanho do
/// resto. Sem sufixo com "claude" ou "cowork": só o tamanho.
pub fn sufixo_do_documento(nome: &str) -> String {
    let n = nome.trim();
    if n.is_empty() {
        return "<sem nome>".into();
    }
    for sep in [" - ", " — ", " · ", " | "] {
        if let Some((resto, suf)) = n.rsplit_once(sep) {
            let s = suf.trim().to_lowercase();
            if ["claude", "claude code", "cowork"].contains(&s.as_str()) {
                return format!("<{} car.>{sep}{suf}", resto.chars().count());
            }
        }
    }
    let m = n.to_lowercase();
    if ["claude", "claude code", "cowork", "chat"].contains(&m.as_str()) {
        return format!("{n:?}");
    }
    format!("<{} car., sem sufixo de app>", n.chars().count())
}

/// Se o nome de um item termina numa palavra de estado: a forma, com o título trocado pelo
/// tamanho ("<23 car.>, Running"). Senão, `None`.
pub fn estado_no_fim(nome: &str) -> Option<String> {
    let n = nome.trim_end();
    let m = n.to_lowercase();
    if m.len() != n.len() {
        return None;
    }
    // palavras que podem vir depois do estado ("needs your input"); outras seriam do título
    let ligacao = ["your", "you", "for", "de", "sua", "você", "voce", "a", "o"];
    for e in ESTADOS {
        // a palavra de estado inteira no fim, com só palavras de estado ou de ligação depois
        let Some(i) = m.rfind(e) else { continue };
        let inteira_antes = m[..i].chars().next_back().is_some_and(|c| !c.is_alphanumeric());
        let inteira_depois = m[i + e.len()..].chars().next().is_none_or(|c| !c.is_alphanumeric());
        let depois = &n[i..];
        let palavras: Vec<String> = depois.to_lowercase().split_whitespace().map(|p| p.trim_matches(|c: char| !c.is_alphanumeric()).to_string()).collect();
        let so_estado = palavras.iter().all(|p| p.is_empty() || ligacao.contains(&p.as_str()) || ESTADOS.contains(&p.as_str()));
        if i == 0 || !inteira_antes || !inteira_depois || palavras.len() > 4 || !so_estado {
            continue;
        }
        let antes = &n[..i];
        let titulo = antes.trim_end_matches(|c: char| !c.is_alphanumeric());
        let sep = &antes[titulo.len()..];
        return Some(format!("<{} car.>{sep:?}{depois}", titulo.chars().count()));
    }
    None
}

/// Palavras de nome de tela do app (r27): o resto de um "… - Claude" só sai inteiro se todas as
/// palavras dele estão aqui ("New chat", "Projects"); senão, só o tamanho.
const TELAS: &[&str] = &[
    "new", "chat", "chats", "nova", "novo", "conversa", "conversas", "claude", "projects", "project", "projetos",
    "recents", "recentes", "settings", "configurações", "ajustes", "artifacts", "home", "início", "customize",
    "code", "cowork", "search", "buscar", "library", "biblioteca",
];

/// O nome de um `Document` do app (r27): como o `sufixo_do_documento`, mas o resto sai inteiro se
/// é um nome de tela (`TELAS`).
pub fn tela_do_documento(nome: &str) -> String {
    let n = nome.trim();
    if let Some((resto, suf)) = n.rsplit_once(" - ") {
        let palavras: Vec<String> =
            resto.to_lowercase().split(|c: char| !c.is_alphanumeric()).filter(|p| !p.is_empty()).map(str::to_string).collect();
        if !palavras.is_empty() && palavras.iter().all(|p| TELAS.contains(&p.as_str())) && suf.trim().to_lowercase().starts_with("claude") {
            return format!("{:?} - {}", resto.trim(), suf.trim());
        }
    }
    sufixo_do_documento(nome)
}

/// Um pedaço de caminho parece um id (uuid ou hexadecimal/letras e números longos).
fn parece_id(p: &str) -> bool {
    let so_id = p.chars().all(|c| c.is_ascii_hexdigit() || c == '-' || c == '_');
    (p.len() >= 16 && so_id) || (p.len() >= 20 && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_'))
}

/// A forma de um endereço (r27): o esquema, o host se é do app (`claude.ai`, `localhost`…) e os
/// pedaços do caminho que são palavras de tela; um id vira `<id>`, o resto só o tamanho. Sem
/// consulta nem fragmento. Vazio: `<sem url>`.
pub fn forma_da_url(url: &str) -> String {
    let u = url.trim();
    if u.is_empty() {
        return "<sem url>".into();
    }
    let (esquema, resto) = u.split_once("://").unwrap_or(("", u));
    let resto = resto.split(['?', '#']).next().unwrap_or("");
    let (host, caminho) = resto.split_once('/').unwrap_or((resto, ""));
    let hosts = ["claude.ai", "www.claude.ai", "localhost", "claude.com", "app.claude.ai"];
    let host = if hosts.contains(&host.to_lowercase().as_str()) || host.starts_with("localhost:") {
        host.to_string()
    } else {
        format!("<host {} car.>", host.chars().count())
    };
    let pedacos: Vec<String> = caminho
        .split('/')
        .filter(|p| !p.is_empty())
        .map(|p| {
            if TELAS.contains(&p.to_lowercase().as_str()) || ["code", "session", "sessions", "local", "project", "c", "p"].contains(&p) {
                p.to_string()
            } else if parece_id(p) {
                "<id>".into()
            } else {
                format!("<{} car.>", p.chars().count())
            }
        })
        .collect();
    format!("{esquema}://{host}/{}", pedacos.join("/"))
}

/// O id do fim de um endereço (o último pedaço que parece id), só para comparar em memória.
pub fn id_da_url(url: &str) -> Option<String> {
    let resto = url.split(['?', '#']).next()?;
    resto.split('/').filter(|p| parece_id(p)).last().map(str::to_lowercase)
}

/// A primeira letra de um título de janela, se não é letra nem número (o ícone de estado do
/// Claude Code no terminal, por exemplo): o código dela. Letra ou número: "letra".
pub fn primeiro_simbolo(titulo: &str) -> String {
    match titulo.chars().next() {
        None => "<vazio>".into(),
        Some(c) if c.is_alphanumeric() => "letra".into(),
        Some(c) => format!("U+{:04X}", c as u32),
    }
}

#[cfg(all(test, windows))]
mod medir {
    use super::*;
    use std::collections::BTreeMap;
    use std::ffi::c_void;
    use std::io::Write;
    use std::mem::ManuallyDrop;
    use std::time::{Duration, Instant};

    use windows::core::w;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_I4};
    use windows::Win32::UI::Accessibility::*;
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;

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

    struct Cliente {
        auto: IUIAutomation,
        cache: IUIAutomationCacheRequest,
        botoes: IUIAutomationCondition,
        docs: IUIAutomationCondition,
        itens: IUIAutomationCondition,
        links: IUIAutomationCondition,
        textos: IUIAutomationCondition,
    }

    fn tipo(id: i32) -> String {
        match id {
            50000 => "Button".into(),
            50004 => "Edit".into(),
            50007 => "ListItem".into(),
            50008 => "List".into(),
            50005 => "Hyperlink".into(),
            50024 => "TreeItem".into(),
            50026 => "Group".into(),
            50030 => "Document".into(),
            50033 => "Pane".into(),
            50020 => "Text".into(),
            50018 => "TabItem".into(),
            50019 => "Tab".into(),
            50032 => "Window".into(),
            50025 => "Custom".into(),
            50021 => "ToolBar".into(),
            50011 => "MenuItem".into(),
            50013 => "RadioButton".into(),
            outro => outro.to_string(),
        }
    }

    impl Cliente {
        fn novo() -> Cliente {
            // SAFETY: COM nesta thread; VARIANTs inteiros.
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                let auto: IUIAutomation = CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER).unwrap();
                let cache = auto.CreateCacheRequest().unwrap();
                for p in [
                    UIA_NamePropertyId,
                    UIA_ControlTypePropertyId,
                    UIA_ClassNamePropertyId,
                    UIA_AutomationIdPropertyId,
                    UIA_IsEnabledPropertyId,
                    UIA_IsOffscreenPropertyId,
                    UIA_ProcessIdPropertyId,
                    UIA_HasKeyboardFocusPropertyId,
                    UIA_ValueValuePropertyId,
                ] {
                    cache.AddProperty(p).unwrap();
                }
                let c = |t: UIA_CONTROLTYPE_ID| auto.CreatePropertyCondition(UIA_ControlTypePropertyId, &variant_i4(t.0)).unwrap();
                let botoes = c(UIA_ButtonControlTypeId);
                let docs = c(UIA_DocumentControlTypeId);
                let itens = auto
                    .CreateOrCondition(
                        &auto.CreateOrCondition(&c(UIA_ButtonControlTypeId), &c(UIA_ListItemControlTypeId)).unwrap(),
                        &auto.CreateOrCondition(&c(UIA_HyperlinkControlTypeId), &c(UIA_TreeItemControlTypeId)).unwrap(),
                    )
                    .unwrap();
                let itens = auto
                    .CreateOrCondition(&itens, &auto.CreateOrCondition(&c(UIA_TabItemControlTypeId), &c(UIA_RadioButtonControlTypeId)).unwrap())
                    .unwrap();
                let links = c(UIA_HyperlinkControlTypeId);
                let textos = c(UIA_TextControlTypeId);
                Cliente { auto, cache, botoes, docs, itens, links, textos }
            }
        }
    }

    fn nome(e: &IUIAutomationElement) -> String {
        unsafe { e.CachedName() }.map(|b| b.to_string()).unwrap_or_default()
    }
    fn tipo_de(e: &IUIAutomationElement) -> i32 {
        unsafe { e.CachedControlType() }.map(|t| t.0).unwrap_or(0)
    }
    fn classe(e: &IUIAutomationElement) -> String {
        let c = unsafe { e.CachedClassName() }.map(|b| b.to_string()).unwrap_or_default();
        c.chars().take(30).collect()
    }
    fn auto_id(e: &IUIAutomationElement) -> String {
        let a = unsafe { e.CachedAutomationId() }.map(|b| b.to_string()).unwrap_or_default();
        a.chars().take(30).collect()
    }
    fn ativo(e: &IUIAutomationElement) -> bool {
        unsafe { e.CachedIsEnabled() }.is_ok_and(|b| b.as_bool())
    }
    fn fora(e: &IUIAutomationElement) -> bool {
        unsafe { e.CachedIsOffscreen() }.is_ok_and(|b| b.as_bool())
    }

    fn todos(cl: &Cliente, raiz: &IUIAutomationElement, cond: &IUIAutomationCondition) -> Vec<IUIAutomationElement> {
        // SAFETY: interfaces COM válidas.
        unsafe {
            let Ok(a) = raiz.FindAllBuildCache(TreeScope_Descendants, cond, &cl.cache) else { return vec![] };
            (0..a.Length().unwrap_or(0)).filter_map(|i| a.GetElement(i).ok()).collect()
        }
    }

    fn descreve_botao(e: &IUIAutomationElement) -> String {
        let id = auto_id(e);
        let id = if id.is_empty() { String::new() } else { format!(" #{id}") };
        format!("{}{}{}", rotulo_ou_tamanho(&nome(e)), id, if ativo(e) { " ativo" } else { " INATIVO" })
    }

    /// Os botões ao lado de `e` (nos irmãos dele: o próprio irmão, se é botão, ou os botões dentro
    /// dele, se são poucos), com o nome cru (só para achar enviar/parar) e a descrição que sai.
    fn botoes_ao_lado(cl: &Cliente, e: &IUIAutomationElement) -> Option<Vec<(String, String)>> {
        // SAFETY: interfaces COM válidas; um elemento que sumiu só faz a chamada falhar.
        unsafe {
            let w = cl.auto.ControlViewWalker().ok()?;
            let pai = w.GetParentElementBuildCache(e, &cl.cache).ok()?;
            let filhos = pai.FindAllBuildCache(TreeScope_Children, &cl.auto.CreateTrueCondition().ok()?, &cl.cache).ok()?;
            let mut v = vec![];
            for i in 0..filhos.Length().ok()? {
                let f = filhos.GetElement(i).ok()?;
                if cl.auto.CompareElements(&f, e).is_ok_and(|b| b.as_bool()) {
                    continue;
                }
                let bs = if tipo_de(&f) == UIA_ButtonControlTypeId.0 { vec![f] } else { todos(cl, &f, &cl.botoes) };
                if bs.len() <= 14 {
                    v.extend(bs.iter().filter(|b| !fora(b)).map(|b| (nome(b), descreve_botao(b))));
                }
            }
            Some(v)
        }
    }

    /// Onde fica o botão de enviar ou parar em relação ao campo: o primeiro ancestral do campo que
    /// o contém (nível), quantos níveis abaixo dele o botão está e quantos filhos o ancestral tem.
    fn rota_do_enviar(cl: &Cliente, campo: &IUIAutomationElement) -> String {
        // SAFETY: interfaces COM válidas; um elemento que sumiu só faz a chamada falhar.
        unsafe {
            let Ok(w) = cl.auto.ControlViewWalker() else { return "sem walker".into() };
            let mut a = campo.clone();
            for nivel in 1..=4 {
                let Ok(p) = w.GetParentElementBuildCache(&a, &cl.cache) else { break };
                a = p;
                let bs = todos(cl, &a, &cl.botoes);
                let filhos = a
                    .FindAllBuildCache(TreeScope_Children, &cl.auto.CreateTrueCondition().unwrap(), &cl.cache)
                    .map(|f| f.Length().unwrap_or(0))
                    .unwrap_or(0);
                let mut achados = vec![];
                for b in &bs {
                    let n = nome(b).to_lowercase();
                    if !["send", "enviar", "stop", "parar", "submit", "interromper"].iter().any(|k| n.contains(k)) {
                        continue;
                    }
                    let mut x = b.clone();
                    let mut prof = 0;
                    while prof < 12 {
                        if cl.auto.CompareElements(&x, &a).is_ok_and(|v| v.as_bool()) {
                            break;
                        }
                        let Ok(p) = w.GetParentElementBuildCache(&x, &cl.cache) else { break };
                        x = p;
                        prof += 1;
                    }
                    let pai = w.GetParentElementBuildCache(b, &cl.cache).ok();
                    let pai = pai.map(|p| format!("{}[{}]", tipo(tipo_de(&p)), classe(&p))).unwrap_or_default();
                    achados.push(format!("{} profundidade {prof} (pai {pai})", descreve_botao(b)));
                }
                if !achados.is_empty() {
                    return format!("nível {nivel} {}[{}] ({filhos} filhos, {} botões): {}", tipo(tipo_de(&a)), classe(&a), bs.len(), achados.join(" | "));
                }
            }
            "não achado até o nível 4".into()
        }
    }

    /// Uma amostra do app do Claude: o caminho do foco até a raiz, os `Document`s, os botões perto
    /// da caixa de mensagem e os itens com estado no fim do nome.
    fn amostra_claude(cl: &Cliente, hwnd: isize, caixa: &mut Option<(IUIAutomationElement, usize)>) -> Vec<String> {
        let mut linhas = vec![];
        // SAFETY: interfaces COM válidas; elementos que sumiram só fazem a chamada falhar.
        unsafe {
            let Ok(raiz) = cl.auto.ElementFromHandleBuildCache(HWND(hwnd as *mut c_void), &cl.cache) else {
                return vec!["sem raiz".into()];
            };
            let processo = raiz.CachedProcessId().unwrap_or(0);
            // 1. o foco e o caminho até a raiz
            if let Ok(f) = cl.auto.GetFocusedElementBuildCache(&cl.cache) {
                if f.CachedProcessId().unwrap_or(-1) == processo {
                    let mut cadeia = vec![];
                    let subir = cl.auto.ControlViewWalker().unwrap();
                    let mut e = f.clone();
                    let mut niveis = 0;
                    let mut doc = String::from("nenhum");
                    loop {
                        let t = tipo_de(&e);
                        if t == UIA_DocumentControlTypeId.0 && doc == "nenhum" {
                            doc = sufixo_do_documento(&nome(&e));
                        }
                        if cadeia.len() < 14 {
                            cadeia.push(format!("{}[{}]{}", tipo(t), classe(&e), if auto_id(&e).is_empty() { String::new() } else { format!("#{}", auto_id(&e)) }));
                        }
                        niveis += 1;
                        let Ok(p) = subir.GetParentElementBuildCache(&e, &cl.cache) else { break };
                        if niveis > 60 {
                            break;
                        }
                        e = p;
                    }
                    linhas.push(format!("foco: {} (níveis {niveis}); 1º Document acima: {doc}", tipo(tipo_de(&f))));
                    linhas.push(format!("  cadeia: {}", cadeia.join(" < ")));
                    // a caixa: o foco num Edit; sobe até um nível em que os irmãos do caminho têm
                    // um botão de enviar ou parar (os botões da caixa ficam ao lado do editor)
                    if tipo_de(&f) == UIA_EditControlTypeId.0 {
                        linhas.push(format!("  rota do enviar: {}", rota_do_enviar(cl, &f)));
                        let mut a = f.clone();
                        for nivel in 1..=5 {
                            let Some(v) = botoes_ao_lado(cl, &a) else { break };
                            let tem = v.iter().any(|(n, _)| {
                                let n = n.to_lowercase();
                                ["send", "enviar", "stop", "parar", "submit", "interromper"].iter().any(|k| n.contains(k))
                            });
                            if tem {
                                *caixa = Some((a.clone(), nivel));
                                break;
                            }
                            let Ok(p) = subir.GetParentElementBuildCache(&a, &cl.cache) else { break };
                            a = p;
                        }
                    }
                } else {
                    linhas.push("foco: fora do app do Claude".into());
                }
            }
            // 2. os botões da caixa guardada (a última em que o foco esteve)
            match caixa {
                Some((c, nivel)) => match c.BuildUpdatedCache(&cl.cache).ok().and_then(|c2| botoes_ao_lado(cl, &c2)) {
                    Some(v) => {
                        let v: Vec<String> = v.into_iter().map(|(_, d)| d).collect();
                        linhas.push(format!("caixa (irmãos do nível {nivel} acima do Edit, {} botões): {}", v.len(), v.join(" | ")));
                    }
                    None => {
                        linhas.push("caixa: sumiu (troca de conversa?)".into());
                        *caixa = None;
                    }
                },
                None => linhas.push("caixa: ainda não achada (clique na caixa de mensagem)".into()),
            }
            // 3. os Documents
            let docs: Vec<String> = todos(cl, &raiz, &cl.docs)
                .iter()
                .map(|d| format!("{}{}", sufixo_do_documento(&nome(d)), if fora(d) { " (fora de vista)" } else { "" }))
                .collect();
            linhas.push(format!("documents: {}", docs.join(" | ")));
            // 4. itens com estado no fim do nome (barra lateral) e as abas do app
            let mut estados: BTreeMap<String, u32> = BTreeMap::new();
            let mut abas = vec![];
            for it in todos(cl, &raiz, &cl.itens) {
                let n = nome(&it);
                if let Some(forma) = estado_no_fim(&n) {
                    // só a forma sem o tamanho, para contar
                    let chave = format!(
                        "{}[{}]{} {}",
                        tipo(tipo_de(&it)),
                        classe(&it),
                        if auto_id(&it).is_empty() { String::new() } else { format!("#{}", auto_id(&it)) },
                        forma
                    );
                    *estados.entry(chave).or_default() += 1;
                }
                if ["chat", "cowork", "code"].contains(&n.trim().to_lowercase().as_str()) {
                    abas.push(format!("{}:{}{}", n.trim(), tipo(tipo_de(&it)), if ativo(&it) { "" } else { " inativo" }));
                }
            }
            linhas.push(format!("itens com estado no fim: {estados:?}"));
            linhas.push(format!("abas do app: {}", abas.join(" ")));
        }
        linhas
    }

    /// Uma amostra da janela em foco, se é um terminal: o processo, a classe, o símbolo do começo
    /// do título e se ele contém o nome de teste da sessão (`WT_SONDA_NOME`). O título não sai.
    fn amostra_terminal(nome_teste: &str) -> Option<String> {
        let j = crate::win::janela_em_foco()?;
        let terminais = ["windowsterminal", "openconsole", "conhost", "powershell", "pwsh", "cmd"];
        if !terminais.contains(&j.processo.as_str()) && j.classe != "ConsoleWindowClass" {
            return None;
        }
        let t = j.titulo.to_lowercase();
        Some(format!(
            "terminal: processo {} classe {}; título com {} car., começa com {}; contém o nome de teste: {}; contém \"claude\": {}",
            j.processo,
            j.classe,
            j.titulo.chars().count(),
            primeiro_simbolo(&j.titulo),
            !nome_teste.is_empty() && t.contains(&nome_teste.to_lowercase()),
            t.contains("claude"),
        ))
    }

    /// Uma vez: cada caixa de mensagem (`Edit` com classe ProseMirror) do app do Claude e os botões
    /// dos ancestrais dela, nível a nível (até 30 botões por nível), com o `IsEnabled`.
    /// `cargo test -p sinais sonda_caixa_r26 -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sonda_caixa_r26() {
        let cl = Cliente::novo();
        // SAFETY: só procura a janela pela classe e o título; interfaces COM válidas.
        unsafe {
            let h = FindWindowW(w!("Chrome_WidgetWin_1"), w!("Claude")).expect("app do Claude aberto");
            let raiz = cl.auto.ElementFromHandleBuildCache(h, &cl.cache).unwrap();
            let edit = cl.auto.CreatePropertyCondition(UIA_ControlTypePropertyId, &variant_i4(UIA_EditControlTypeId.0)).unwrap();
            let subir = cl.auto.ControlViewWalker().unwrap();
            let edits = todos(&cl, &raiz, &edit);
            println!("{} campos Edit", edits.len());
            for (k, ed) in edits.iter().enumerate() {
                println!("Edit {k}: [{}] foco {} fora {}", classe(ed), ed.CachedHasKeyboardFocus().is_ok_and(|b| b.as_bool()), fora(ed));
                let mut a = ed.clone();
                for nivel in 1..=7 {
                    let Ok(p) = subir.GetParentElementBuildCache(&a, &cl.cache) else { break };
                    a = p;
                    let bs = todos(&cl, &a, &cl.botoes);
                    if bs.len() > 30 {
                        println!("  nível {nivel} {}[{}]: {} botões (muitos); os filhos:", tipo(tipo_de(&a)), classe(&a), bs.len());
                        let filhos = a.FindAllBuildCache(TreeScope_Children, &cl.auto.CreateTrueCondition().unwrap(), &cl.cache).unwrap();
                        let n = filhos.Length().unwrap_or(0);
                        // os últimos filhos (a caixa fica no fim da conversa)
                        for i in n.saturating_sub(8)..n {
                            let f = filhos.GetElement(i).unwrap();
                            let bf = todos(&cl, &f, &cl.botoes);
                            let v: Vec<String> = if bf.len() <= 14 {
                                bf.iter().map(|b| format!("{}[{}]{}", descreve_botao(b), classe(b), if fora(b) { " fora" } else { "" })).collect()
                            } else {
                                vec![]
                            };
                            println!("    filho {i}/{n} {}[{}]: {} botões: {}", tipo(tipo_de(&f)), classe(&f), bf.len(), v.join(" | "));
                        }
                        break;
                    }
                    let v: Vec<String> =
                        bs.iter().map(|b| format!("{}[{}]{}", descreve_botao(b), classe(b), if fora(b) { " fora" } else { "" })).collect();
                    println!("  nível {nivel} {}[{}]: {} botões: {}", tipo(tipo_de(&a)), classe(&a), bs.len(), v.join(" | "));
                }
            }
        }
    }

    /// Com o VS Code ou o Windows Terminal em foco: se o título da janela contém o nome de teste e
    /// quais elementos da UI Automation (abas, painéis) têm o nome de teste no nome. Só saem os
    /// tipos e as classes; nenhum nome.
    fn amostra_editor(cl: &Cliente, nome_teste: &str) -> Option<String> {
        let j = crate::win::janela_em_foco()?;
        if !["code", "windowsterminal"].contains(&j.processo.as_str()) || nome_teste.is_empty() {
            return None;
        }
        let no_titulo = j.titulo.to_lowercase().contains(&nome_teste.to_lowercase());
        // SAFETY: interfaces COM válidas; o HWND pode ter sumido, e aí a chamada só falha.
        let achados = unsafe {
            let raiz = cl.auto.ElementFromHandle(HWND(j.hwnd as *mut c_void)).ok()?;
            let alvo = windows::core::BSTR::from(nome_teste);
            let v = VARIANT::from(alvo);
            let cond = cl
                .auto
                .CreatePropertyConditionEx(UIA_NamePropertyId, &v, PropertyConditionFlags(PropertyConditionFlags_IgnoreCase.0 | PropertyConditionFlags_MatchSubstring.0))
                .ok()?;
            let t = Instant::now();
            let a = raiz.FindAllBuildCache(TreeScope_Descendants, &cond, &cl.cache).ok()?;
            let mut v = vec![];
            for i in 0..a.Length().unwrap_or(0).min(8) {
                let e = a.GetElement(i).ok()?;
                v.push(format!("{}[{}]{}", tipo(tipo_de(&e)), classe(&e), if fora(&e) { " fora" } else { "" }));
            }
            format!("{} ({} ms)", v.join(" | "), t.elapsed().as_millis())
        };
        Some(format!(
            "{}: título com {} car., começa com {}, contém o nome de teste: {no_titulo}; elementos com o nome de teste: {achados}",
            j.processo,
            j.titulo.chars().count(),
            primeiro_simbolo(&j.titulo)
        ))
    }

    /// Amostra a cada `WT_SONDA_PASSO_MS` (padrão 1000) por `WT_SONDA_S` segundos (padrão 600) e
    /// escreve em `WT_SONDA_SAIDA` (ou na tela) só quando algo muda, com o segundo da mudança.
    /// `cargo test -p sinais sonda_r26 -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sonda_r26() {
        let segundos: u64 = std::env::var("WT_SONDA_S").ok().and_then(|v| v.parse().ok()).unwrap_or(600);
        let passo: u64 = std::env::var("WT_SONDA_PASSO_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1000);
        let nome_teste = std::env::var("WT_SONDA_NOME").unwrap_or_default();
        let mut saida: Box<dyn Write> = match std::env::var("WT_SONDA_SAIDA") {
            Ok(p) => Box::new(std::fs::File::create(p).unwrap()),
            Err(_) => Box::new(std::io::stdout()),
        };
        let cl = Cliente::novo();
        let mut uia = crate::uia::Uia::novo();
        let inicio = Instant::now();
        let mut anterior: Vec<String> = vec![];
        let mut caixa = None;
        let mut tempos = vec![];
        while inicio.elapsed() < Duration::from_secs(segundos) {
            let t = Instant::now();
            let mut linhas = vec![];
            let fg = crate::win::janela_em_foco();
            linhas.push(format!(
                "em foco: {}",
                fg.as_ref().map(|j| format!("{} [{}]", j.processo, j.classe)).unwrap_or_else(|| "nada".into())
            ));
            if let Some(l) = amostra_terminal(&nome_teste) {
                linhas.push(l);
            }
            if let Some(l) = amostra_editor(&cl, &nome_teste) {
                linhas.push(l);
            }
            // SAFETY: só procura a janela pela classe e o título.
            match unsafe { FindWindowW(w!("Chrome_WidgetWin_1"), w!("Claude")) } {
                Ok(h) if !h.0.is_null() => {
                    linhas.extend(amostra_claude(&cl, h.0 as isize, &mut caixa));
                    // o que o app leria (regra A): sem conversa, lendo ou escrevendo
                    let em_foco = fg.as_ref().is_some_and(|j| j.hwnd == h.0 as isize);
                    let lida = uia.as_mut().and_then(|u| u.ler(h.0 as isize, em_foco));
                    linhas.push(format!(
                        "Uia::ler: {}",
                        match lida {
                            None => "sem conversa",
                            Some(c) if c.escrevendo => "ESCREVENDO",
                            Some(_) => "lendo",
                        }
                    ));
                }
                _ => linhas.push("app do Claude: janela não achada".into()),
            }
            tempos.push(t.elapsed().as_millis());
            if linhas != anterior {
                let _ = writeln!(saida, "--- {:>4} s", inicio.elapsed().as_secs());
                for l in &linhas {
                    let _ = writeln!(saida, "{l}");
                }
                let _ = saida.flush();
                anterior = linhas;
            }
            std::thread::sleep(Duration::from_millis(passo));
        }
        tempos.sort_unstable();
        let n = tempos.len();
        let _ = writeln!(saida, "=== {n} amostras; consulta: mediana {} ms, p90 {} ms, máx {} ms", tempos[n / 2], tempos[n * 9 / 10], tempos[n - 1]);
    }
    /// O `Value` (em cache) de um `Document` ou `Hyperlink`: no Chromium, o endereço. Nunca de um
    /// campo (`Edit`), onde o `Value` é o texto.
    fn endereco(e: &IUIAutomationElement) -> String {
        let t = tipo_de(e);
        if t != UIA_DocumentControlTypeId.0 && t != UIA_HyperlinkControlTypeId.0 {
            return String::new();
        }
        // SAFETY: interface COM válida, com o cache pedido.
        unsafe {
            let Ok(mut v) = e.GetCachedPropertyValue(UIA_ValueValuePropertyId) else { return String::new() };
            let texto = if v.Anonymous.Anonymous.vt == windows::Win32::System::Variant::VT_BSTR {
                v.Anonymous.Anonymous.Anonymous.bstrVal.to_string()
            } else {
                String::new()
            };
            let _ = windows::Win32::System::Variant::VariantClear(&mut v);
            texto
        }
    }

    fn classe_inteira(e: &IUIAutomationElement) -> String {
        unsafe { e.CachedClassName() }.map(|b| b.to_string()).unwrap_or_default()
    }

    /// O link do item da barra lateral: ele mesmo ou um ancestral (até 3), ou um filho.
    fn link_do_item(cl: &Cliente, it: &IUIAutomationElement) -> String {
        // SAFETY: interfaces COM válidas; elementos que sumiram só fazem a chamada falhar.
        unsafe {
            let subir = cl.auto.ControlViewWalker().unwrap();
            let mut e = it.clone();
            for _ in 0..4 {
                if tipo_de(&e) == UIA_HyperlinkControlTypeId.0 {
                    return endereco(&e);
                }
                let Ok(p) = subir.GetParentElementBuildCache(&e, &cl.cache) else { break };
                e = p;
            }
        }
        todos(cl, it, &cl.links).first().map(endereco).unwrap_or_default()
    }

    /// Uma amostra do Chat (r27): os `Document`s (tela e forma do endereço), os itens da barra
    /// lateral (quantos, quantos com " Running", se o do "Running" é a conversa aberta), os botões
    /// de parar por documento e o chip de tarefa em segundo plano. Nada de título.
    fn amostra_chat(cl: &Cliente, hwnd: isize) -> Vec<String> {
        let mut linhas = vec![];
        // SAFETY: interfaces COM válidas; elementos que sumiram só fazem a chamada falhar.
        unsafe {
            let Ok(raiz) = cl.auto.ElementFromHandleBuildCache(HWND(hwnd as *mut c_void), &cl.cache) else {
                return vec!["sem raiz".into()];
            };
            let subir = cl.auto.ControlViewWalker().unwrap();
            // o Document que contém o foco
            let mut doc_do_foco = None;
            if let Ok(f) = cl.auto.GetFocusedElementBuildCache(&cl.cache) {
                let mut e = f;
                for _ in 0..60 {
                    if tipo_de(&e) == UIA_DocumentControlTypeId.0 && !nome(&e).trim().is_empty() {
                        doc_do_foco = Some(nome(&e));
                        break;
                    }
                    let Ok(p) = subir.GetParentElementBuildCache(&e, &cl.cache) else { break };
                    e = p;
                }
            }
            // os Documents
            let docs = todos(cl, &raiz, &cl.docs);
            let mut abertos: Vec<(String, Option<String>)> = vec![];
            for d in &docs {
                let n = nome(d);
                let url = endereco(d);
                if n.trim().is_empty() && url.is_empty() {
                    continue;
                }
                let paradas: Vec<String> = todos(cl, d, &cl.botoes)
                    .iter()
                    .filter(|b| {
                        let m = nome(b).trim().to_lowercase();
                        m.starts_with("stop") || m.starts_with("parar") || m.starts_with("interromper")
                    })
                    .map(|b| format!("{}{}", rotulo_ou_tamanho(&nome(b)), if fora(b) { " fora" } else { "" }))
                    .collect();
                let chips = todos(cl, d, &cl.itens)
                    .iter()
                    .chain(todos(cl, d, &cl.textos).iter())
                    .filter(|e| nome(e).to_lowercase().contains("background task"))
                    .count();
                linhas.push(format!(
                    "document: {} | url {} |{}{} paradas [{}] | chips de tarefa em segundo plano {chips}",
                    tela_do_documento(&n),
                    forma_da_url(&url),
                    if fora(d) { " fora de vista |" } else { "" },
                    if doc_do_foco.as_deref() == Some(n.as_str()) { " FOCO dentro |" } else { "" },
                    paradas.join(", "),
                ));
                if !fora(d) && n.trim_end().ends_with(" - Claude") {
                    abertos.push((n.trim_end().trim_end_matches(" - Claude").trim().to_lowercase(), id_da_url(&url)));
                }
            }
            // a barra lateral: Buttons com a classe do item
            let itens: Vec<IUIAutomationElement> =
                todos(cl, &raiz, &cl.botoes).into_iter().filter(|b| classe_inteira(b).starts_with("w-full shrink-0 border-none")).collect();
            let mut running = vec![];
            let mut link_da_aberta = 0;
            let mut iguais = 0;
            let mut formas: BTreeMap<String, u32> = BTreeMap::new();
            for it in &itens {
                let n = nome(it);
                let url = link_do_item(cl, it);
                *formas.entry(forma_da_url(&url)).or_default() += 1;
                if let Some(id) = id_da_url(&url) {
                    if abertos.iter().any(|(_, a)| a.as_deref() == Some(id.as_str())) {
                        link_da_aberta += 1;
                    }
                }
                let sem = n.strip_suffix(" Running");
                let base = sem.unwrap_or(&n).trim().to_lowercase();
                let igual = abertos.iter().any(|(t, _)| *t == base);
                iguais += igual as u32;
                if let Some(sem) = sem {
                    running.push(format!(
                        "<{} car.> Running (fora {}, título = conversa aberta: {igual}, link {})",
                        sem.chars().count(),
                        fora(it),
                        forma_da_url(&url)
                    ));
                }
            }
            linhas.push(format!(
                "barra lateral: {} itens, {link_da_aberta} com o link da conversa aberta, {iguais} com o título dela; links: {formas:?}",
                itens.len()
            ));
            linhas.push(format!("running: [{}]", running.join("; ")));
        }
        linhas
    }

    /// Rodada 27: o Chat do app do Claude. Amostra a cada `WT_SONDA_PASSO_MS` (padrão 1000) por
    /// `WT_SONDA_S` segundos (padrão 900) e escreve em `WT_SONDA_SAIDA` (ou na tela) só quando algo
    /// muda. `cargo test -p sinais sonda_r27 -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn sonda_r27() {
        let segundos: u64 = std::env::var("WT_SONDA_S").ok().and_then(|v| v.parse().ok()).unwrap_or(900);
        let passo: u64 = std::env::var("WT_SONDA_PASSO_MS").ok().and_then(|v| v.parse().ok()).unwrap_or(1000);
        let mut saida: Box<dyn Write> = match std::env::var("WT_SONDA_SAIDA") {
            Ok(p) => Box::new(std::fs::File::create(p).unwrap()),
            Err(_) => Box::new(std::io::stdout()),
        };
        let cl = Cliente::novo();
        let mut uia = crate::uia::Uia::novo();
        let inicio = Instant::now();
        let mut anterior: Vec<String> = vec![];
        let mut tempos = vec![];
        while inicio.elapsed() < Duration::from_secs(segundos) {
            let t = Instant::now();
            let mut linhas = vec![];
            let fg = crate::win::janela_em_foco();
            linhas.push(format!("em foco: {}", fg.as_ref().map(|j| format!("{} [{}]", j.processo, j.classe)).unwrap_or_else(|| "nada".into())));
            // SAFETY: só procura a janela pela classe e o título.
            match unsafe { FindWindowW(w!("Chrome_WidgetWin_1"), w!("Claude")) } {
                Ok(h) if !h.0.is_null() => {
                    linhas.extend(amostra_chat(&cl, h.0 as isize));
                    let em_foco = fg.as_ref().is_some_and(|j| j.hwnd == h.0 as isize);
                    let lida = uia.as_mut().and_then(|u| u.ler(h.0 as isize, em_foco));
                    linhas.push(format!(
                        "Uia::ler: {}",
                        match lida {
                            None => "sem conversa",
                            Some(c) if c.escrevendo => "ESCREVENDO",
                            Some(_) => "lendo",
                        }
                    ));
                }
                _ => linhas.push("app do Claude: janela não achada".into()),
            }
            let ms = t.elapsed().as_millis();
            tempos.push(ms);
            if linhas != anterior {
                let _ = writeln!(saida, "--- {:>4} s ({ms} ms)", inicio.elapsed().as_secs());
                for l in &linhas {
                    let _ = writeln!(saida, "{l}");
                }
                let _ = saida.flush();
                anterior = linhas;
            }
            std::thread::sleep(Duration::from_millis(passo));
        }
        tempos.sort_unstable();
        let n = tempos.len();
        let _ = writeln!(saida, "=== {n} amostras; consulta: mediana {} ms, p90 {} ms, máx {} ms", tempos[n / 2], tempos[n * 9 / 10], tempos[n - 1]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sonda_nao_mostra_titulos() {
        assert_eq!(sufixo_do_documento("Projeto: tela - Claude Code"), "<13 car.> - Claude Code");
        assert_eq!(sufixo_do_documento("Receita de bolo"), "<15 car., sem sufixo de app>");
        assert_eq!(sufixo_do_documento(""), "<sem nome>");
        assert_eq!(estado_no_fim("Spec 018: algo… Running").as_deref(), Some("<14 car.>\"… \"Running"));
        assert_eq!(estado_no_fim("Spec 018: algo"), None);
        assert_eq!(estado_no_fim("Plano doneRelatório em x"), None, "palavra inteira");
        assert_eq!(estado_no_fim("Bug running tests"), None, "depois do estado vem título");
        assert_eq!(estado_no_fim("Bug, needs your input").as_deref(), Some("<3 car.>\", \"needs your input"));
        assert_eq!(rotulo_ou_tamanho("Send message"), "\"Send message\"");
        assert_eq!(rotulo_ou_tamanho("Stop response"), "\"Stop response\"");
        assert_eq!(rotulo_ou_tamanho("Model: Opus 5.5 High"), "\"Model:\" <14 car.>");
        assert_eq!(rotulo_ou_tamanho("Cliente X"), "<9 car.>");
        // títulos e linhas de ferramenta com uma palavra da lista não saem
        assert_eq!(rotulo_ou_tamanho("Plano 2: dock"), "<13 car.>");
        assert_eq!(rotulo_ou_tamanho("git add -A"), "<10 car.>");
        assert_eq!(rotulo_ou_tamanho("planilha.xlsx"), "<13 car.>");
        assert_eq!(sufixo_do_documento("A - Claude Code de B"), "<20 car., sem sufixo de app>");
        assert_eq!(estado_no_fim("Bug ao lidar"), None, "palavra que só começa com um estado");
        assert_eq!(estado_no_fim("Corrigir erros"), None);
        assert_eq!(primeiro_simbolo("✳ algo"), "U+2733");
        assert_eq!(primeiro_simbolo("PowerShell"), "letra");
    }

    #[test]
    fn sonda_r27_nao_mostra_titulos_nem_enderecos() {
        assert_eq!(tela_do_documento("New chat - Claude"), "\"New chat\" - Claude");
        assert_eq!(tela_do_documento("Receita de bolo - Claude"), "<15 car.> - Claude");
        assert_eq!(tela_do_documento("Projeto: login - Claude Code"), "<14 car.> - Claude Code");
        assert_eq!(tela_do_documento("Claude"), "\"Claude\"");
        assert_eq!(
            forma_da_url("https://claude.ai/chat/0f5c2a1e-1234-4abc-9def-00112233aabb?x=segredo#f"),
            "https://claude.ai/chat/<id>"
        );
        assert_eq!(forma_da_url("https://claude.ai/new"), "https://claude.ai/new");
        assert_eq!(forma_da_url("https://exemplo.com/receita-de-bolo"), "https://<host 11 car.>/<15 car.>");
        assert_eq!(forma_da_url("https://claude.ai/project/abcdef0123456789abcd/minha-pasta"), "https://claude.ai/project/<id>/<11 car.>");
        assert_eq!(forma_da_url(""), "<sem url>");
        assert_eq!(id_da_url("https://claude.ai/chat/0F5C2A1E-1234-4abc-9def-00112233aabb").as_deref(), Some("0f5c2a1e-1234-4abc-9def-00112233aabb"));
        assert_eq!(id_da_url("https://claude.ai/new"), None);
    }
}
