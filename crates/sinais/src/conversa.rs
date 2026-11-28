// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Conversa aberta no app do Claude. A UI Automation da janela do app tem um `Document` por
//! conversa à vista, com o Name `"<título da sessão> - Claude Code"`; o título é o mesmo nome
//! que o app guarda por sessão. No Chat (rodada 27), o Name é `"<título> - Claude"` e o endereço
//! (o `Value` do `Document`) é `https://claude.ai/chat/<id>`. Privacidade: os títulos só vão para
//! o chamador comparar, em memória; o `Value` só é pedido de um `Document` (num campo, seria o
//! texto).
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use crate::JanelaInfo;

/// Final do Name do `Document` de uma conversa do Claude Code no app do Claude.
pub const SUFIXO: &str = " - Claude Code";
/// Final do Name do `Document` de uma página do Chat do app do Claude (rodada 27).
pub const SUFIXO_CHAT: &str = " - Claude";
/// Começo do endereço de uma conversa do Chat (host exato).
pub const ENDERECO_CHAT: &str = "https://claude.ai/chat/";
/// Começo do endereço de qualquer página do app do Claude (host exato): sem ele, um `Document`
/// "… - Claude" não é a aba do Chat (uma prévia no painel do Code, por exemplo).
pub const ENDERECO_CLAUDE: &str = "https://claude.ai/";
/// Final exato do nome do item da barra lateral do Chat com o Claude respondendo nele.
pub const RODANDO: &str = " Running";
/// Começo da classe do item da barra lateral (um `Button`) de uma conversa (medido na rodada 27).
pub const CLASSE_DO_ITEM: &str = "w-full shrink-0 border-none";

/// Consulta que demora mais que isto conta como "conversa não reconhecida" naquela amostra.
pub const LIMITE_CONSULTA: Duration = Duration::from_millis(300);
/// Consulta pendente há mais que isto: a thread travou; o leitor abandona ela e começa outra.
pub const LIMITE_TRAVADA: Duration = Duration::from_secs(30);
/// Spec 001: uma consulta que passa do limite (ou ainda pendente) devolve a última conversa lida
/// da mesma janela por até isto, em vez de "não sei" (numa conversa longa, a busca inteira pode
/// passar dos 300 ms de vez em quando, e a leitura caía).
pub const SEGURAR_ULTIMA: Duration = Duration::from_secs(15);

/// Um `Document` da janela e se o elemento com foco está dentro dele.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Documento {
    pub nome: String,
    /// O endereço da página (o `Value` do `Document`), só lido nas páginas do Chat.
    pub endereco: String,
    pub tem_foco: bool,
    /// O foco do teclado está num campo de texto (`Edit`) dentro deste documento: no app do
    /// Claude, é a caixa de mensagem da conversa.
    pub campo_com_foco: bool,
}

/// A conversa aberta lida na janela: o título e se você está escrevendo nela (o foco do teclado
/// na caixa de mensagem dessa conversa, com o botão de enviar ativo: `escrevendo`). O texto da
/// caixa nunca é lido.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConversaLida {
    pub titulo: String,
    pub escrevendo: bool,
    /// O id da conversa do Chat (minúsculo); `None` numa conversa do Code.
    pub chat: Option<String>,
}

/// `ControlType` da UI Automation de um campo de texto (`UIA_EditControlTypeId`). A caixa de
/// mensagem do app do Claude (um editor ProseMirror) aparece assim.
pub const TIPO_CAMPO: i32 = 50004;

/// O elemento com foco é um campo de texto?
pub fn e_campo_de_texto(tipo: i32) -> bool {
    tipo == TIPO_CAMPO
}

/// Nome do botão de enviar da caixa de mensagem: "Send" no Code, "Send message" no Chat (medido na
/// rodada 26); os mesmos em português e em outros idiomas, se o app for traduzido.
const ENVIAR: &[&str] = &[
    "send", "send message", "enviar", "enviar mensagem", "envoyer", "envoyer le message", "senden",
    "nachricht senden", "invia", "invia messaggio",
];

/// O botão é o de enviar da caixa ("Send", "Send message"), pelo nome inteiro: nem o "Stop" que
/// fica no lugar dele com o Claude trabalhando, nem outro que só começa igual ("Send feedback").
pub fn e_botao_de_enviar(nome: &str) -> bool {
    ENVIAR.contains(&nome.trim().to_lowercase().as_str())
}

/// "escrevendo" (regra A, rodada 26): o foco do teclado na caixa de mensagem **e** o botão de
/// enviar dela ativo. Ao abrir uma conversa, o app já põe o foco na caixa vazia; o botão só se
/// ativa com algo escrito (no Code ele nem existe com a caixa vazia; no Chat fica inativo). Com o
/// Claude trabalhando, o "Stop" fica no lugar dele. `enviar_ativo`: `None` se o botão não foi
/// achado. Sem ler o texto da caixa: só o `IsEnabled` do botão.
pub fn escrevendo_na_caixa(campo_com_foco: bool, enviar_ativo: Option<bool>) -> bool {
    campo_com_foco && enviar_ativo == Some(true)
}

/// A janela é a do app do Claude (Electron), e não um terminal rodando o `claude.exe`.
pub fn e_app_do_claude(j: &JanelaInfo) -> bool {
    j.processo == "claude" && j.classe.starts_with("Chrome_WidgetWin")
}

/// Título da sessão a partir do Name do `Document`: sem o sufixo `" - Claude Code"` e sem
/// espaços nas pontas. `None` se o documento não é de uma conversa do Claude Code.
pub fn titulo_da_conversa(nome_documento: &str) -> Option<&str> {
    let t = nome_documento.trim_end().strip_suffix(SUFIXO)?.trim();
    (!t.is_empty()).then_some(t)
}

/// A página é do Chat (qualquer uma: conversa, tela inicial, Projetos): o Name termina em
/// `" - Claude"`, e não em `" - Claude Code"`.
pub fn e_pagina_do_chat(nome_documento: &str) -> bool {
    let n = nome_documento.trim_end();
    n.ends_with(SUFIXO_CHAT) && !n.ends_with(SUFIXO)
}

/// O id da conversa do Chat pelo endereço: `https://claude.ai/chat/<id>` (host exato, sem
/// diferenciar maiúsculas no começo, ignorando a consulta e o fragmento), em minúsculas. A tela
/// inicial (`/new`), os Projetos e as outras páginas: `None`.
pub fn id_do_chat(endereco: &str) -> Option<String> {
    let u = endereco.trim();
    let u = &u[..u.find(['?', '#']).unwrap_or(u.len())];
    let comeco = u.get(..ENDERECO_CHAT.len())?;
    if !comeco.eq_ignore_ascii_case(ENDERECO_CHAT) {
        return None;
    }
    let id = u[ENDERECO_CHAT.len()..].trim_end_matches('/');
    let valido = !id.is_empty() && id.len() <= 64 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
    valido.then(|| id.to_ascii_lowercase())
}

/// A conversa do Chat de uma página: `(id, título)`. O Name termina em `" - Claude"` (e não em
/// `" - Claude Code"`), o título não é vazio e o endereço é o de uma conversa (`id_do_chat`).
pub fn chat_da_pagina<'a>(nome: &'a str, endereco: &str) -> Option<(String, &'a str)> {
    if !e_pagina_do_chat(nome) {
        return None;
    }
    let t = nome.trim_end().strip_suffix(SUFIXO_CHAT)?.trim();
    if t.is_empty() {
        return None;
    }
    Some((id_do_chat(endereco)?, t))
}

/// A conversa de uma página: do Code (`(None, título)`) ou do Chat (`(Some(id), título)`).
pub fn conversa_da_pagina<'a>(nome: &'a str, endereco: &str) -> Option<(Option<String>, &'a str)> {
    if let Some(t) = titulo_da_conversa(nome) {
        return Some((None, t));
    }
    chat_da_pagina(nome, endereco).map(|(id, t)| (Some(id), t))
}

/// O `Button` é um item (uma conversa) da barra lateral, pela classe.
pub fn e_item_da_barra(classe: &str) -> bool {
    classe.starts_with(CLASSE_DO_ITEM)
}

/// O título de um item da barra lateral com o Claude respondendo: só com o fim exato
/// `" Running"`, sem os espaços das pontas e não vazio.
pub fn titulo_rodando(nome: &str) -> Option<&str> {
    let t = nome.strip_suffix(RODANDO)?.trim();
    (!t.is_empty()).then_some(t)
}

/// Nome do botão que fica no lugar do de enviar com o Claude respondendo no Chat (medido na
/// rodada 27), e os equivalentes nos idiomas de `ENVIAR`.
const PARAR_RESPOSTA: &[&str] = &[
    "stop response", "parar resposta", "interromper resposta", "arrêter la réponse", "antwort stoppen",
    "interrompi risposta",
];

/// O botão é o "Stop response" da conversa do Chat, pelo nome inteiro: nunca "Stop", "Stop this
/// task" ou "Stop 1 nested session" (do Code).
pub fn e_parar_resposta(nome: &str) -> bool {
    PARAR_RESPOSTA.contains(&nome.trim().to_lowercase().as_str())
}

/// Os nomes do "Stop response" (`e_parar_resposta`), para a busca pedir só esses botões pelo nome
/// exato (sem diferenciar maiúsculas), sem ler o nome dos outros.
pub fn nomes_de_parar_resposta() -> &'static [&'static str] {
    PARAR_RESPOSTA
}

/// O endereço é de uma página do app do Claude (`https://claude.ai/…`, host exato, sem
/// diferenciar maiúsculas no começo).
pub fn e_endereco_do_claude(endereco: &str) -> bool {
    endereco.trim().get(..ENDERECO_CLAUDE.len()).is_some_and(|c| c.eq_ignore_ascii_case(ENDERECO_CLAUDE))
}

/// A aba do Chat está à vista: há um `Document` à vista que é uma página do Chat
/// (`e_pagina_do_chat`) com o endereço do app do Claude. É a 1ª fase dos sinais (só os
/// `Document`s): sem ela, nenhum botão da janela é pedido (na aba do Code, os nomes dos botões
/// da transcrição vêm das respostas).
pub fn aba_do_chat(pecas: &[PecaDoChat]) -> bool {
    pecas.iter().any(|p| {
        matches!(p, PecaDoChat::Pagina { nome, endereco, fora: false } if e_pagina_do_chat(nome) && e_endereco_do_claude(endereco))
    })
}

/// No máximo tantas classes de item da barra lateral entram na busca dos botões (2ª fase).
pub const MAX_CLASSES_DO_ITEM: usize = 8;

/// As classes inteiras (sem repetir) dos itens da barra lateral (`e_item_da_barra`) entre as
/// classes dos `Button`s da janela, no máximo `MAX_CLASSES_DO_ITEM`. A 2ª fase dos sinais pede o
/// nome só dos botões com uma destas classes (uma condição de classe exata: a UI Automation não
/// compara pelo começo).
pub fn classes_dos_itens<'a>(classes: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut v: Vec<String> = vec![];
    for c in classes {
        if v.len() >= MAX_CLASSES_DO_ITEM {
            break;
        }
        if e_item_da_barra(c) && !v.iter().any(|x| x == c) {
            v.push(c.to_string());
        }
    }
    v
}

/// Os sinais do Chat numa amostra (rodada 27): se a aba do Chat está à vista, a conversa aberta
/// (e se o Claude responde nela) e os títulos dos itens da barra lateral com o Claude respondendo.
/// Os títulos ficam só em memória.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SinaisChat {
    /// Há um `Document` à vista que é uma página do Chat (`e_pagina_do_chat`) com o endereço do
    /// app do Claude (`aba_do_chat`).
    pub aba_do_chat: bool,
    /// A conversa do Chat à vista (só uma).
    pub aberta: Option<ChatAberto>,
    /// Os títulos (`titulo_rodando`) dos itens da barra lateral; só com `aba_do_chat`.
    pub rodando: Vec<String>,
    /// Algum item da barra lateral foi achado. Sem nenhum (a barra fechada), `rodando` não diz
    /// nada das conversas que não estão abertas.
    pub barra: bool,
    /// Os títulos (sem o `" Running"`) que aparecem em dois ou mais itens da barra lateral, em
    /// minúsculas: o " Running" de um deles não diz qual conversa responde.
    pub repetidos: Vec<String>,
}

/// A conversa do Chat à vista.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatAberto {
    pub id: String,
    pub titulo: String,
    /// Há na janela o botão "Stop response" (`e_parar_resposta`): o Claude responde nela.
    pub parando: bool,
}

/// Um elemento da janela que importa para os sinais do Chat.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PecaDoChat {
    /// Um `Document`: o nome, o endereço (só o de uma página do Chat) e se está fora de vista.
    Pagina { nome: String, endereco: String, fora: bool },
    /// Um `Button`: o nome e a classe.
    Botao { nome: String, classe: String },
}

/// Os sinais do Chat a partir dos `Document`s e dos `Button`s da janela. Sem página do Chat à
/// vista (a aba do Code; `aba_do_chat`), só `aba_do_chat: false`. A conversa aberta é a única
/// conversa do Chat à vista (com a aba do Chat, a janela só tem o `Document` do Chat, e o "Stop
/// response" é dela); com duas ou mais, nenhuma.
pub fn sinais_do_chat(pecas: &[PecaDoChat]) -> SinaisChat {
    if !aba_do_chat(pecas) {
        return SinaisChat::default();
    }
    let mut conversas: Vec<(String, String)> = vec![];
    let (mut parando, mut barra, mut rodando) = (false, false, vec![]);
    let mut titulos: Vec<String> = vec![];
    for p in pecas {
        match p {
            PecaDoChat::Pagina { nome, endereco, fora: false } if e_pagina_do_chat(nome) => {
                if let Some((id, t)) = chat_da_pagina(nome, endereco) {
                    if !conversas.iter().any(|(i, _)| *i == id) {
                        conversas.push((id, t.to_string()));
                    }
                }
            }
            PecaDoChat::Pagina { .. } => {}
            PecaDoChat::Botao { nome, classe } => {
                parando |= e_parar_resposta(nome);
                if e_item_da_barra(classe) {
                    barra = true;
                    let r = titulo_rodando(nome);
                    if let Some(t) = r {
                        rodando.push(t.to_string());
                    }
                    let t = r.unwrap_or(nome).trim().to_lowercase();
                    if !t.is_empty() {
                        titulos.push(t);
                    }
                }
            }
        }
    }
    let aberta = match conversas.as_slice() {
        [(id, titulo)] => Some(ChatAberto { id: id.clone(), titulo: titulo.clone(), parando }),
        _ => None,
    };
    titulos.sort();
    let mut repetidos: Vec<String> = titulos.windows(2).filter(|w| w[0] == w[1]).map(|w| w[0].clone()).collect();
    repetidos.dedup();
    SinaisChat { aba_do_chat: true, aberta, rodando, barra, repetidos }
}

/// Índice (em `docs`) da conversa aberta: a única conversa à vista; com várias (vista dividida),
/// a que contém o elemento com foco; sem foco dentro de nenhuma, nenhuma. Com duas que contêm o
/// foco (um documento dentro do outro), a mais interna: a busca devolve a árvore em pré-ordem, e
/// a de dentro vem depois da que a contém (spec 001).
fn indice_da_conversa(docs: &[Documento]) -> Option<usize> {
    let conversas: Vec<(usize, bool)> = docs
        .iter()
        .enumerate()
        .filter(|(_, d)| conversa_da_pagina(&d.nome, &d.endereco).is_some())
        .map(|(i, d)| (i, d.tem_foco))
        .collect();
    match conversas.as_slice() {
        [] => None,
        [(i, _)] => Some(*i),
        varias => varias.iter().rev().find(|(_, foco)| *foco).map(|(i, _)| *i),
    }
}

/// Título da conversa aberta (`indice_da_conversa`).
pub fn conversa_aberta(docs: &[Documento]) -> Option<String> {
    let i = indice_da_conversa(docs)?;
    conversa_da_pagina(&docs[i].nome, &docs[i].endereco).map(|(_, t)| t.to_string())
}

/// A conversa aberta e se o foco do teclado está num campo de texto dela ("escrevendo"). O campo
/// de outro documento (outra conversa, um painel lateral) não conta.
pub fn ler_conversa(docs: &[Documento]) -> Option<ConversaLida> {
    let i = indice_da_conversa(docs)?;
    let (chat, titulo) = conversa_da_pagina(&docs[i].nome, &docs[i].endereco)?;
    Some(ConversaLida { titulo: titulo.to_string(), escrevendo: docs[i].campo_com_foco, chat })
}

/// Um elemento no caminho do foco para cima: `(é Document, nome, endereço)`. O endereço só é lido
/// num `Document` que é página do Chat.
pub type Acima = (bool, String, String);

/// Subindo do foco (`acima`: do mais perto ao mais longe), o primeiro `Document` que é uma
/// conversa, do Code ou do Chat: `(id do Chat, título)`. Um `Document` interno da conversa
/// (prévia, diff) fica no caminho e não conta: o foco nele conta para a conversa que o contém
/// (spec 5.5; até a rodada 5, com duas conversas à vista, dava "não sei").
pub fn conversa_acima(acima: &[Acima]) -> Option<(Option<String>, &str)> {
    acima.iter().find_map(|(doc, nome, endereco)| if *doc { conversa_da_pagina(nome, endereco) } else { None })
}

/// Por onde a consulta acha a conversa aberta.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caminho {
    /// Subindo do elemento com foco até a conversa (barato).
    PeloFoco,
    /// Os `Document`s de conversa guardados da última busca, conferidos (barato).
    Guardados,
    /// A busca na árvore inteira (cara numa conversa longa).
    BuscaInteira,
}

/// A escolha do caminho. Com a janela em foco: a conversa acima do foco, se há; senão a busca
/// inteira (o foco na barra lateral ou num botão é quando se troca de conversa ou se abre outra
/// ao lado: uma lista guardada estaria velha). Com a janela fora de foco (só debaixo do mouse, a
/// conversa não troca): os guardados, se ainda valem; senão a busca inteira.
pub fn caminho_da_consulta(em_foco: bool, conversa_acima_do_foco: bool, guardados_validos: bool) -> Caminho {
    match (em_foco, conversa_acima_do_foco, guardados_validos) {
        (true, true, _) => Caminho::PeloFoco,
        (true, false, _) => Caminho::BuscaInteira,
        (false, _, true) => Caminho::Guardados,
        (false, _, false) => Caminho::BuscaInteira,
    }
}

/// A thread da consulta e os canais dela.
struct Canais<T> {
    pedidos: Sender<(u64, isize, bool)>,
    respostas: Receiver<(u64, Option<T>)>,
    thread: JoinHandle<()>,
}

/// Faz a consulta numa thread própria e espera no máximo `limite` pela resposta. Enquanto uma
/// consulta atrasada não volta, as próximas amostras nem perguntam (e ficam sem conversa). Se ela
/// não volta em `limite_travada`, a thread é abandonada e outra começa (sem reiniciar o app). No
/// máximo uma abandonada fica viva: enquanto ela não volta, não nasce outra e a amostra é "não sei".
/// Atrasada ou pendente, a amostra devolve a última conversa lida da mesma janela (no mesmo estado
/// de foco) por até `segurar` (spec 001); uma consulta que respondeu "sem conversa" não segura nada.
pub struct LeitorConversa<T = ConversaLida> {
    iniciar: Box<dyn Fn() -> Option<Canais<T>> + Send>,
    canais: Option<Canais<T>>,
    abandonada: Option<JoinHandle<()>>,
    prox: u64,
    /// A consulta atrasada: o id, desde quando, a janela e se estava em foco.
    pendente: Option<(u64, Instant, isize, bool)>,
    pub limite_travada: Duration,
    /// Quantas vezes a thread da consulta foi trocada (travou ou caiu).
    pub recomecos: u32,
    /// Quantas consultas passaram do limite.
    pub atrasadas: u64,
    /// A última resposta com conversa: (janela, em foco, a conversa, quando).
    ultima: Option<(isize, bool, T, Instant)>,
    pub segurar: Duration,
    /// Quantas amostras devolveram a última conversa no lugar de uma consulta lenta.
    pub seguradas: u64,
}

impl<T: Clone + Send + 'static> LeitorConversa<T> {
    /// `fabrica` roda na thread da consulta (ex.: inicia o COM e o cliente de UI Automation)
    /// e devolve a função que acha a conversa aberta na janela `hwnd`. Uma thread nova chama de novo.
    pub fn com<F, C>(fabrica: F) -> LeitorConversa<T>
    where
        F: Fn() -> C + Send + Sync + 'static,
        C: FnMut(isize, bool) -> Option<T>,
    {
        let fabrica = Arc::new(fabrica);
        let iniciar = move || -> Option<Canais<T>> {
            let (tx_pedido, rx_pedido) = mpsc::channel::<(u64, isize, bool)>();
            let (tx_resposta, respostas) = mpsc::channel();
            let fabrica = Arc::clone(&fabrica);
            let thread = std::thread::Builder::new()
                .name("worktimer-conversa".into())
                .spawn(move || {
                    let mut consultar = fabrica();
                    for (id, hwnd, em_foco) in rx_pedido {
                        if tx_resposta.send((id, consultar(hwnd, em_foco))).is_err() {
                            break;
                        }
                    }
                })
                .ok()?;
            Some(Canais { pedidos: tx_pedido, respostas, thread })
        };
        let canais = iniciar();
        LeitorConversa {
            iniciar: Box::new(iniciar),
            canais,
            abandonada: None,
            prox: 1,
            pendente: None,
            limite_travada: LIMITE_TRAVADA,
            recomecos: 0,
            atrasadas: 0,
            ultima: None,
            segurar: SEGURAR_ULTIMA,
            seguradas: 0,
        }
    }

    /// A última conversa lida desta janela, no mesmo estado de foco, se é de até `segurar` atrás.
    fn segurada(&mut self, hwnd: isize, em_foco: bool) -> Option<T> {
        let (h, f, v, quando) = self.ultima.as_ref()?;
        if *h != hwnd || *f != em_foco || quando.elapsed() > self.segurar {
            return None;
        }
        let v = v.clone();
        self.seguradas += 1;
        Some(v)
    }

    /// Troca a thread da consulta. A antiga fica como a abandonada: se um dia voltar, sai sozinha
    /// (ninguém mais escuta).
    fn recomecar(&mut self) {
        if let Some(c) = self.canais.take() {
            self.abandonada = Some(c.thread);
        }
        self.canais = (self.iniciar)();
        self.pendente = None;
        self.recomecos += 1;
    }

    /// Ainda há uma consulta abandonada presa.
    fn abandonada_viva(&self) -> bool {
        self.abandonada.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// Conversa aberta na janela, ou `None` (não reconhecida, falhou ou passou do `limite`).
    /// `em_foco`: a janela é a que tem o foco (só então o elemento com foco é consultado).
    pub fn ler(&mut self, hwnd: isize, em_foco: bool, limite: Duration) -> Option<T> {
        if let Some((p, desde, h, f)) = self.pendente {
            // resposta atrasada de uma amostra anterior: libera a thread e passa a ser a última
            // lida daquela janela (revisão I2: "sem conversa" apaga a guardada, e outra conversa
            // toma o lugar dela)
            if let Some(c) = &self.canais {
                while let Ok((id, r)) = c.respostas.try_recv() {
                    if id == p {
                        self.pendente = None;
                        self.ultima = r.map(|v| (h, f, v, Instant::now()));
                    }
                }
            }
            if self.pendente.is_some() {
                if desde.elapsed() < self.limite_travada || self.abandonada_viva() {
                    return self.segurada(hwnd, em_foco);
                }
                self.recomecar();
            }
        }
        if self.canais.is_none() {
            self.recomecar();
        }
        let id = self.prox;
        self.prox += 1;
        let c = self.canais.as_ref()?;
        if c.pedidos.send((id, hwnd, em_foco)).is_err() {
            self.canais = None;
            return None;
        }
        match c.respostas.recv_timeout(limite) {
            Ok((rid, r)) if rid == id => {
                self.ultima = r.as_ref().map(|v| (hwnd, em_foco, v.clone(), Instant::now()));
                r
            }
            Ok(_) => None,
            Err(RecvTimeoutError::Timeout) => {
                self.pendente = Some((id, Instant::now(), hwnd, em_foco));
                self.atrasadas += 1;
                self.segurada(hwnd, em_foco)
            }
            Err(RecvTimeoutError::Disconnected) => {
                self.canais = None;
                None
            }
        }
    }
}

#[cfg(windows)]
impl LeitorConversa<ConversaLida> {
    /// Leitor com a UI Automation do Windows.
    pub fn novo() -> LeitorConversa<ConversaLida> {
        LeitorConversa::com(|| {
            let mut uia = crate::uia::Uia::novo();
            move |hwnd: isize, em_foco: bool| uia.as_mut().and_then(|u| u.ler(hwnd, em_foco))
        })
    }
}

#[cfg(windows)]
impl LeitorConversa<SinaisChat> {
    /// Leitor dos sinais do Chat (rodada 27), com a UI Automation numa thread própria: consultado
    /// a cada ciclo com a janela do app do Claude, mesmo fora de uso.
    pub fn do_chat() -> LeitorConversa<SinaisChat> {
        LeitorConversa::com(|| {
            let uia = crate::uia::Uia::novo();
            move |hwnd: isize, _: bool| uia.as_ref().and_then(|u| u.sinais_do_chat(hwnd))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(nome: &str, tem_foco: bool) -> Documento {
        Documento { nome: nome.into(), tem_foco, ..Default::default() }
    }

    fn escrevendo(nome: &str) -> Documento {
        Documento { nome: nome.into(), tem_foco: true, campo_com_foco: true, ..Default::default() }
    }

    #[test]
    fn escrevendo_so_com_o_foco_no_campo_da_conversa_aberta() {
        let lida = |t: &str, e: bool| Some(ConversaLida { titulo: t.into(), escrevendo: e, chat: None });
        // uma conversa à vista, foco na caixa de mensagem dela
        let docs = [doc("", false), escrevendo("Flow: nomes - Claude Code")];
        assert_eq!(ler_conversa(&docs), lida("Flow: nomes", true));
        // foco fora do campo (na lista de mensagens, ou em outro programa): lendo
        let docs = [doc("", false), doc("Flow: nomes - Claude Code", true)];
        assert_eq!(ler_conversa(&docs), lida("Flow: nomes", false));
        // vista dividida: escrevendo na B
        let docs = [doc("A - Claude Code", false), escrevendo("B - Claude Code")];
        assert_eq!(ler_conversa(&docs), lida("B", true));
        // o campo de um documento que não é conversa (um painel lateral) não marca nada
        let docs = [escrevendo("Notas"), doc("A - Claude Code", false)];
        assert_eq!(ler_conversa(&docs), lida("A", false));
        assert_eq!(ler_conversa(&[]), None);
        assert!(e_campo_de_texto(TIPO_CAMPO));
        assert!(!e_campo_de_texto(50030), "Document não é campo");
    }

    #[test]
    fn escrevendo_so_com_o_botao_de_enviar_ativo() {
        // regra A: o foco na caixa e o botão de enviar ativo
        assert!(escrevendo_na_caixa(true, Some(true)), "uma letra na caixa");
        assert!(!escrevendo_na_caixa(true, Some(false)), "caixa vazia no Chat: o botão fica inativo");
        assert!(!escrevendo_na_caixa(true, None), "caixa vazia no Code (sem botão) ou o Claude trabalhando (Stop)");
        assert!(!escrevendo_na_caixa(false, Some(true)), "rascunho na caixa, mas o foco fora dela");
    }

    #[test]
    fn reconhece_o_botao_de_enviar_pelo_nome() {
        assert!(e_botao_de_enviar("Send"));
        assert!(e_botao_de_enviar("Send message"));
        assert!(e_botao_de_enviar(" Enviar mensagem "));
        assert!(e_botao_de_enviar("ENVIAR"));
        // o botão de parar fica no lugar dele com o Claude trabalhando
        assert!(!e_botao_de_enviar("Stop"));
        assert!(!e_botao_de_enviar("Stop response"));
        assert!(!e_botao_de_enviar("Sender"));
        assert!(!e_botao_de_enviar("Resend"));
        assert!(!e_botao_de_enviar("Send feedback"));
        assert!(!e_botao_de_enviar("Enviar arquivo"));
        assert!(!e_botao_de_enviar(""));
    }

    #[test]
    fn extrai_o_titulo_do_name_do_documento() {
        assert_eq!(
            titulo_da_conversa("Projeto: tela de login - Claude Code"),
            Some("Projeto: tela de login")
        );
        assert_eq!(titulo_da_conversa("  Título com espaço   - Claude Code "), Some("Título com espaço"));
        // o documento raiz da página não tem nome; uma conversa comum do Claude não é Claude Code
        assert_eq!(titulo_da_conversa(""), None);
        assert_eq!(titulo_da_conversa("Receita de bolo - Claude"), None);
        assert_eq!(titulo_da_conversa(" - Claude Code"), None);
        // o sufixo é o do fim; um título que contém o sufixo no meio fica inteiro
        assert_eq!(titulo_da_conversa("A - Claude Code e B - Claude Code"), Some("A - Claude Code e B"));
    }

    #[test]
    fn uma_conversa_a_vista_vale_sem_foco() {
        let docs = [doc("", false), doc("Flow: nomes - Claude Code", false)];
        assert_eq!(conversa_aberta(&docs), Some("Flow: nomes".into()));
        assert_eq!(conversa_aberta(&[doc("", true)]), None);
        assert_eq!(conversa_aberta(&[]), None);
    }

    #[test]
    fn vista_dividida_usa_a_conversa_com_foco() {
        let docs = [doc("", false), doc("A - Claude Code", false), doc("B - Claude Code", true)];
        assert_eq!(conversa_aberta(&docs), Some("B".into()));
        // o foco no documento raiz (ou em outro programa) não escolhe nenhuma
        let docs = [doc("", true), doc("A - Claude Code", false), doc("B - Claude Code", false)];
        assert_eq!(conversa_aberta(&docs), None);
    }

    #[test]
    fn app_do_claude_e_o_electron_e_nao_o_terminal() {
        let j = |processo: &str, classe: &str| JanelaInfo {
            processo: processo.into(),
            titulo: "Claude".into(),
            classe: classe.into(),
            pid: 1,
            hwnd: 1,
            em_foco: true,
        };
        assert!(e_app_do_claude(&j("claude", "Chrome_WidgetWin_1")));
        assert!(!e_app_do_claude(&j("claude", "ConsoleWindowClass")));
        assert!(!e_app_do_claude(&j("code", "Chrome_WidgetWin_1")));
    }

    #[test]
    fn caminho_da_consulta_nunca_usa_os_guardados_com_a_janela_em_foco() {
        use Caminho::*;
        assert_eq!(caminho_da_consulta(true, true, true), PeloFoco);
        assert_eq!(caminho_da_consulta(true, true, false), PeloFoco);
        // em foco, o foco fora de uma conversa (barra lateral, botão da vista dividida): busca inteira
        assert_eq!(caminho_da_consulta(true, false, true), BuscaInteira);
        assert_eq!(caminho_da_consulta(true, false, false), BuscaInteira);
        // fora de foco: os guardados, se ainda valem
        assert_eq!(caminho_da_consulta(false, false, true), Guardados);
        assert_eq!(caminho_da_consulta(false, false, false), BuscaInteira);
    }

    #[test]
    fn foco_num_documento_interno_sobe_ate_a_conversa() {
        let a = |doc: bool, nome: &str| (doc, nome.to_string(), String::new());
        // do foco para cima: um grupo, um Document interno (prévia), outro grupo, a conversa B, a raiz
        let acima = [a(false, ""), a(true, "Prévia do diff"), a(false, ""), a(true, "B - Claude Code"), a(true, "")];
        assert_eq!(conversa_acima(&acima), Some((None, "B")), "a prévia fica no caminho");
        // o próprio foco é o Document da conversa
        assert_eq!(conversa_acima(&[a(true, "A - Claude Code")]), Some((None, "A")));
        // um elemento que não é Document, com o nome de uma conversa, não conta
        assert_eq!(conversa_acima(&[a(false, "A - Claude Code"), a(true, "")]), None);
        assert_eq!(conversa_acima(&[]), None, "foco fora da janela");
    }

    const URL: &str = "https://claude.ai/chat/0A1B2C3D-4e5f-6789-abcd-ef0123456789";
    const ID: &str = "0a1b2c3d-4e5f-6789-abcd-ef0123456789";

    #[test]
    fn reconhece_a_conversa_do_chat_pelo_nome_e_pelo_endereco() {
        assert_eq!(chat_da_pagina("Receita de bolo - Claude", URL), Some((ID.to_string(), "Receita de bolo")));
        assert_eq!(chat_da_pagina("  Receita  - Claude ", &format!("{URL}?x=1#fim")), Some((ID.to_string(), "Receita")));
        assert_eq!(chat_da_pagina("Receita - Claude", &format!("{URL}/")), Some((ID.to_string(), "Receita")));
        // a tela inicial, os Projetos e outras páginas não são conversa
        assert_eq!(chat_da_pagina("Nova conversa - Claude", "https://claude.ai/new"), None);
        assert_eq!(chat_da_pagina("Projetos - Claude", "https://claude.ai/projects"), None);
        assert_eq!(chat_da_pagina("Receita - Claude", "https://claude.ai/chat/"), None);
        assert_eq!(chat_da_pagina("Receita - Claude", "https://claude.ai/chat/a/b"), None);
        // host exato
        assert_eq!(chat_da_pagina("Receita - Claude", &URL.replace("claude.ai", "claude.ai.exemplo.com")), None);
        assert_eq!(chat_da_pagina("Receita - Claude", &URL.replace("claude.ai", "x.claude.ai")), None);
        assert_eq!(chat_da_pagina("Receita - Claude", &URL.replace("https", "http")), None);
        // o Code e o título vazio não são do Chat
        assert_eq!(chat_da_pagina("Sessão - Claude Code", URL), None);
        assert_eq!(chat_da_pagina(" - Claude", URL), None);
        assert_eq!(chat_da_pagina("Receita - Claude", ""), None);
        assert!(e_pagina_do_chat("Nova conversa - Claude"));
        assert!(!e_pagina_do_chat("Sessão - Claude Code"));
        assert!(!e_pagina_do_chat("Claude"));
        // a conversa da página: do Code (sem id) ou do Chat
        assert_eq!(conversa_da_pagina("A - Claude Code", URL), Some((None, "A")));
        assert_eq!(conversa_da_pagina("A - Claude", URL), Some((Some(ID.into()), "A")));
        let a = |doc: bool, nome: &str, url: &str| (doc, nome.to_string(), url.to_string());
        let acima = [a(false, "", ""), a(true, "Inicial - Claude", "https://claude.ai/new"), a(true, "A - Claude", URL)];
        assert_eq!(conversa_acima(&acima), Some((Some(ID.into()), "A")));
        assert_eq!(conversa_acima(&[a(false, "A - Claude", URL)]), None, "só um Document");
    }

    #[test]
    fn ler_conversa_aceita_a_do_chat() {
        let chat = Documento { nome: "Receita - Claude".into(), endereco: URL.into(), tem_foco: true, campo_com_foco: true };
        assert_eq!(ler_conversa(&[chat]), Some(ConversaLida { titulo: "Receita".into(), escrevendo: true, chat: Some(ID.into()) }));
        let inicial = Documento { nome: "Nova - Claude".into(), endereco: "https://claude.ai/new".into(), ..Default::default() };
        assert_eq!(ler_conversa(&[inicial]), None, "a tela inicial não é conversa");
    }

    #[test]
    fn running_e_stop_response_so_pelo_nome_exato() {
        assert_eq!(titulo_rodando("Receita de bolo Running"), Some("Receita de bolo"));
        assert_eq!(titulo_rodando(" Receita  Running"), Some("Receita"));
        assert_eq!(titulo_rodando("Receita"), None);
        assert_eq!(titulo_rodando("Receita Running "), None, "só o fim exato");
        assert_eq!(titulo_rodando("Receita running"), None);
        assert_eq!(titulo_rodando(" Running"), None);
        assert_eq!(titulo_rodando("1 background task running"), None);
        assert!(e_item_da_barra("w-full shrink-0 border-none flex"));
        assert!(!e_item_da_barra("cds-reset -ms-1 inline-flex"));
        assert!(e_parar_resposta("Stop response"));
        assert!(e_parar_resposta(" stop RESPONSE "));
        assert!(e_parar_resposta("Parar resposta"));
        for nao in ["Stop", "Stop this task", "Stop 1 nested session", "Send message", "Stop responses", ""] {
            assert!(!e_parar_resposta(nao), "{nao}");
        }
    }

    fn pagina(nome: &str, endereco: &str) -> PecaDoChat {
        PecaDoChat::Pagina { nome: nome.into(), endereco: endereco.into(), fora: false }
    }
    fn botao(nome: &str, classe: &str) -> PecaDoChat {
        PecaDoChat::Botao { nome: nome.into(), classe: classe.into() }
    }
    const ITEM: &str = "w-full shrink-0 border-none text-left";

    #[test]
    fn sinais_do_chat_pela_barra_e_pelo_stop_response() {
        let s = sinais_do_chat(&[
            pagina("Receita - Claude", URL),
            botao("Receita Running", ITEM),
            botao("Outra conversa Running", ITEM),
            botao("Parada", ITEM),
            botao("Stop response", "x"),
            // o chip e um botão "Running" sem a classe do item não contam
            botao("Fora da barra Running", "cds-reset"),
        ]);
        assert_eq!(
            s,
            SinaisChat {
                aba_do_chat: true,
                aberta: Some(ChatAberto { id: ID.into(), titulo: "Receita".into(), parando: true }),
                rodando: vec!["Receita".into(), "Outra conversa".into()],
                barra: true,
                repetidos: vec![],
            }
        );
        // a tela inicial: a aba do Chat, sem conversa aberta
        let s = sinais_do_chat(&[pagina("Nova - Claude", "https://claude.ai/new"), botao("Outra Running", ITEM)]);
        assert_eq!((s.aba_do_chat, s.aberta, s.rodando, s.barra), (true, None, vec!["Outra".to_string()], true));
        // sem o Stop response, a aberta não responde; sem itens, a barra está fechada
        let s = sinais_do_chat(&[pagina("Receita - Claude", URL), botao("Stop", "x")]);
        assert_eq!(s.aberta.map(|a| a.parando), Some(false));
        assert!(!s.barra);
    }

    #[test]
    fn sinais_do_chat_com_a_aba_do_code_nao_dizem_nada() {
        let s = sinais_do_chat(&[
            pagina("Sessão - Claude Code", "https://claude.ai/abcdefg/123"),
            botao("Sessão Running", ITEM),
            botao("Stop response", "x"),
        ]);
        assert_eq!(s, SinaisChat::default());
        // a página do Chat fora de vista não é a aba do Chat
        let s = sinais_do_chat(&[PecaDoChat::Pagina { nome: "Receita - Claude".into(), endereco: URL.into(), fora: true }]);
        assert!(!s.aba_do_chat);
        // duas conversas do Chat à vista: nenhuma é a aberta
        let outra = URL.replace("0A1B", "9999");
        let s = sinais_do_chat(&[pagina("A - Claude", URL), pagina("B - Claude", &outra)]);
        assert!(s.aba_do_chat && s.aberta.is_none());
    }

    #[test]
    fn titulos_repetidos_na_barra_rodando_ou_nao() {
        let s = sinais_do_chat(&[
            pagina("Nova - Claude", "https://claude.ai/new"),
            botao("Receita Running", ITEM),
            botao("receita", ITEM),
            botao("Plano", ITEM),
            botao("Viagem Running", ITEM),
            botao("Plano", ITEM),
            botao("Plano", ITEM),
            // fora da barra não conta
            botao("Viagem", "cds-reset"),
        ]);
        assert_eq!(s.repetidos, ["plano", "receita"]);
        assert_eq!(s.rodando, ["Receita", "Viagem"]);
    }

    #[test]
    fn sem_a_aba_do_chat_nenhum_botao_e_pedido() {
        // a 1ª fase (só os `Document`s) decide se a 2ª (os botões) roda: na aba do Code, não
        let code = [pagina("Sessão - Claude Code", "https://claude.ai/abcdefg/123")];
        assert!(!aba_do_chat(&code));
        assert!(!aba_do_chat(&[]), "sem Document nenhum");
        assert!(!aba_do_chat(&[PecaDoChat::Pagina { nome: "Receita - Claude".into(), endereco: URL.into(), fora: true }]));
        assert!(aba_do_chat(&[pagina("Nova - Claude", "https://claude.ai/new")]), "a tela inicial é a aba do Chat");
        assert!(aba_do_chat(&[pagina("Receita - Claude", URL)]));
        // os botões não entram na decisão
        assert!(!aba_do_chat(&[botao("Receita - Claude", ITEM), botao("Stop response", "x")]));
    }

    #[test]
    fn a_aba_do_chat_pede_o_endereco_do_claude() {
        assert!(e_endereco_do_claude("https://claude.ai/new"));
        assert!(e_endereco_do_claude(" HTTPS://Claude.ai/chat/x"));
        for nao in ["", "https://claude.ai", "http://claude.ai/new", "https://claude.ai.exemplo.com/", "https://exemplo.com/claude.ai/"] {
            assert!(!e_endereco_do_claude(nao), "{nao}");
        }
        // uma página "… - Claude" de outro endereço (uma prévia no painel do Code): não é a aba
        // do Chat, e nada muda (nem os itens da barra, nem o "Stop response")
        let s = sinais_do_chat(&[pagina("Prévia - Claude", "https://exemplo.com/"), botao("Stop response", "x"), botao("A", ITEM)]);
        assert_eq!(s, SinaisChat::default());
        let s = sinais_do_chat(&[pagina("Prévia - Claude", ""), botao("A", ITEM)]);
        assert_eq!(s, SinaisChat::default(), "sem endereço também não");
    }

    #[test]
    fn classes_dos_itens_sem_repetir_e_com_teto() {
        let classes = ["cds-reset", "w-full shrink-0 border-none a", "x", "w-full shrink-0 border-none a", "w-full shrink-0 border-none b"];
        assert_eq!(classes_dos_itens(classes), ["w-full shrink-0 border-none a", "w-full shrink-0 border-none b"]);
        assert!(classes_dos_itens(["cds-reset", ""]).is_empty());
        let muitas: Vec<String> = (0..20).map(|i| format!("{CLASSE_DO_ITEM} {i}")).collect();
        assert_eq!(classes_dos_itens(muitas.iter().map(String::as_str)).len(), MAX_CLASSES_DO_ITEM);
        // os nomes do "Stop response" pedidos pela busca são os que `e_parar_resposta` aceita
        assert!(nomes_de_parar_resposta().iter().all(|n| e_parar_resposta(n)));
        assert!(nomes_de_parar_resposta().contains(&"stop response"));
    }

    #[test]
    fn leitor_travado_recomeca_com_thread_nova() {
        let (tx, rx) = mpsc::channel::<()>();
        let rx = std::sync::Mutex::new(Some(rx));
        // a primeira thread trava para sempre; as seguintes respondem
        let mut l = LeitorConversa::com(move || {
            let trava = rx.lock().unwrap().take();
            move |_: isize, _: bool| {
                if let Some(r) = &trava {
                    let _ = r.recv();
                }
                Some("ok".to_string())
            }
        });
        l.limite_travada = Duration::from_millis(30);
        assert_eq!(l.ler(1, true, Duration::from_millis(20)), None);
        assert_eq!(l.ler(1, true, Duration::from_millis(20)), None, "pendente, ainda dentro do prazo");
        std::thread::sleep(Duration::from_millis(40));
        assert_eq!(l.ler(1, true, Duration::from_secs(5)), Some("ok".into()));
        assert_eq!(l.recomecos, 1);
        drop(tx);
    }

    #[test]
    fn leitor_deixa_no_maximo_uma_consulta_abandonada_viva() {
        // toda consulta espera uma liberação; as liberações saem na ordem em que as threads pedem
        let (tx, rx) = mpsc::channel::<()>();
        let rx = Arc::new(std::sync::Mutex::new(rx));
        let mut l = LeitorConversa::com(move || {
            let rx = Arc::clone(&rx);
            move |_: isize, _: bool| {
                let _ = rx.lock().unwrap().recv();
                Some("ok".to_string())
            }
        });
        l.limite_travada = Duration::from_millis(20);
        let curto = Duration::from_millis(5);
        assert_eq!(l.ler(1, true, curto), None);
        // muitos prazos com a primeira presa: só uma troca
        for _ in 0..6 {
            std::thread::sleep(Duration::from_millis(25));
            assert_eq!(l.ler(1, true, curto), None);
        }
        assert_eq!(l.recomecos, 1);
        // a abandonada volta e sai; a próxima troca pode acontecer
        tx.send(()).unwrap();
        for _ in 0..500 {
            if !l.abandonada_viva() {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        assert!(!l.abandonada_viva());
        std::thread::sleep(Duration::from_millis(25));
        assert_eq!(l.ler(1, true, curto), None);
        assert_eq!(l.recomecos, 2);
        drop(tx);
    }

    #[test]
    fn documento_dentro_de_documento_vale_o_mais_interno() {
        // a busca em pré-ordem: a mãe (que contém o foco) antes da filha (que também contém)
        let docs = [doc("", true), doc("Mãe - Claude Code", true), doc("Filha - Claude Code", true)];
        assert_eq!(conversa_aberta(&docs), Some("Filha".into()));
        // a filha sem o foco: a mãe
        let docs = [doc("Mãe - Claude Code", true), doc("Filha - Claude Code", false)];
        assert_eq!(conversa_aberta(&docs), Some("Mãe".into()));
    }

    #[test]
    fn foco_num_painel_da_conversa_conta_para_ela() {
        // medido em 26/11: o `Document` da conversa é a página inteira (RootWebArea, com o título
        // da sessão à vista); o terminal, o navegador e a prévia do lado ficam dentro dele
        let a = |doc: bool, nome: &str| (doc, nome.to_string(), String::new());
        let terminal = [a(false, ""), a(false, ""), a(true, "Saída do terminal"), a(false, ""), a(true, "Sessão aninhada - Claude Code")];
        assert_eq!(conversa_acima(&terminal), Some((None, "Sessão aninhada")));
        let previa = [a(true, "notas.md"), a(false, ""), a(true, "Plano - Claude Code")];
        assert_eq!(conversa_acima(&previa), Some((None, "Plano")));
    }

    #[test]
    fn consulta_lenta_segura_a_ultima_conversa_por_pouco_tempo() {
        // a 1ª consulta responde; as seguintes esperam uma liberação
        let (tx, rx) = mpsc::channel::<()>();
        let rx = Arc::new(std::sync::Mutex::new(rx));
        let n = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let n2 = Arc::clone(&n);
        let mut l = LeitorConversa::com(move || {
            let rx = Arc::clone(&rx);
            let n = Arc::clone(&n2);
            move |_: isize, _: bool| {
                if n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) > 0 {
                    let _ = rx.lock().unwrap().recv();
                }
                Some("A".to_string())
            }
        });
        l.segurar = Duration::from_millis(300);
        let curto = Duration::from_millis(10);
        assert_eq!(l.ler(1, true, Duration::from_secs(5)), Some("A".into()));
        // passou do limite: a última da mesma janela
        assert_eq!(l.ler(1, true, curto), Some("A".into()));
        // ainda pendente: também
        assert_eq!(l.ler(1, true, curto), Some("A".into()));
        // outra janela ou outro estado de foco: nada
        assert_eq!(l.ler(2, true, curto), None);
        assert_eq!(l.ler(1, false, curto), None);
        assert_eq!(l.seguradas, 2);
        // passado o tempo de segurar: "não sei"
        std::thread::sleep(Duration::from_millis(350));
        assert_eq!(l.ler(1, true, curto), None);
        drop(tx);
    }

    #[test]
    fn a_resposta_atrasada_troca_a_guardada() {
        // revisão I2: a 1ª responde "A"; a 2ª atrasa e diz "sem conversa"; a 3ª atrasa de novo
        let (tx, rx) = mpsc::channel::<()>();
        let rx = Arc::new(std::sync::Mutex::new(rx));
        let n = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let n2 = Arc::clone(&n);
        let mut l = LeitorConversa::com(move || {
            let rx = Arc::clone(&rx);
            let n = Arc::clone(&n2);
            move |_: isize, _: bool| match n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => Some("A".to_string()),
                1 => {
                    let _ = rx.lock().unwrap().recv();
                    None
                }
                _ => {
                    let _ = rx.lock().unwrap().recv();
                    Some("B".to_string())
                }
            }
        });
        let curto = Duration::from_millis(10);
        assert_eq!(l.ler(1, true, Duration::from_secs(5)), Some("A".into()));
        assert_eq!(l.ler(1, true, curto), Some("A".into()), "atrasou: segura a A");
        // a atrasada volta com "sem conversa"; a próxima consulta atrasa: nada de A
        tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        assert_eq!(l.ler(1, true, curto), None);
        // a que atrasou volta com B: a próxima lenta segura a B
        tx.send(()).unwrap();
        std::thread::sleep(Duration::from_millis(50));
        let r = l.ler(1, true, curto);
        assert!(r == Some("B".into()) || r.is_none(), "{r:?}");
        assert_ne!(r, Some("A".into()));
        drop(tx);
    }

    #[test]
    fn sem_conversa_na_resposta_nao_segura_nada() {
        let n = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let n2 = Arc::clone(&n);
        let (tx, rx) = mpsc::channel::<()>();
        let rx = Arc::new(std::sync::Mutex::new(rx));
        let mut l = LeitorConversa::com(move || {
            let n = Arc::clone(&n2);
            let rx = Arc::clone(&rx);
            move |_: isize, _: bool| match n.fetch_add(1, std::sync::atomic::Ordering::SeqCst) {
                0 => Some("A".to_string()),
                1 => None,
                _ => {
                    let _ = rx.lock().unwrap().recv();
                    Some("A".to_string())
                }
            }
        });
        let curto = Duration::from_millis(10);
        assert_eq!(l.ler(1, true, Duration::from_secs(5)), Some("A".into()));
        // a página trocou e a consulta disse "sem conversa": a próxima lenta não devolve a velha
        assert_eq!(l.ler(1, true, Duration::from_secs(5)), None);
        assert_eq!(l.ler(1, true, curto), None);
        assert_eq!(l.seguradas, 0);
        drop(tx);
    }

    #[test]
    fn leitor_responde_dentro_do_limite() {
        let mut l = LeitorConversa::com(|| |hwnd: isize, _: bool| Some(format!("janela {hwnd}")));
        assert_eq!(l.ler(7, true, Duration::from_secs(5)), Some("janela 7".into()));
        assert_eq!(l.ler(8, true, Duration::from_secs(5)), Some("janela 8".into()));
    }

    #[test]
    fn leitor_lento_conta_como_nao_reconhecida_e_nao_acumula_pedidos() {
        let (tx, rx) = mpsc::channel::<()>();
        let rx = Arc::new(std::sync::Mutex::new(rx));
        let mut l = LeitorConversa::com(move || {
            let rx = Arc::clone(&rx);
            move |_: isize, _: bool| {
                // segura a consulta até o teste soltar
                let _ = rx.lock().unwrap().recv();
                Some("tarde".to_string())
            }
        });
        assert_eq!(l.ler(1, true, Duration::from_millis(20)), None, "passou do limite");
        assert_eq!(l.ler(1, true, Duration::from_millis(20)), None, "ainda ocupada: nem pergunta");
        assert_eq!(l.atrasadas, 1, "só a que passou do limite conta");
        tx.send(()).unwrap();
        // a resposta atrasada é descartada; a próxima consulta responde na hora
        tx.send(()).unwrap();
        let mut r = None;
        for _ in 0..500 {
            r = l.ler(1, true, Duration::from_secs(5));
            if r.is_some() {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        assert_eq!(r, Some("tarde".into()));
    }
}
