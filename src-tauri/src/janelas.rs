// Chronn — © 2026 Nikolai Cinotti (@eaeniko). Veja LICENSE.
//! Janelas: a principal (título, mostrar, fechar e recriar) e o dock (posição, tamanho e etapa).
//! As contas de posição são funções puras (pixels físicos), testadas sem Tauri.
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use nucleo::config::{
    ConfigApp, AO_FECHAR_BANDEJA, AO_FECHAR_SAIR, AO_MINIMIZAR_BANDEJA, AO_MINIMIZAR_BARRA, AO_MINIMIZAR_DOCK, DOCK_SO_PERGUNTAS,
};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WebviewWindowBuilder, Window,
};

use crate::estado_app::{trava, Estado, PedidoNovoTimer};
use crate::bandeja;

pub const PRINCIPAL: &str = "main";
/// Evento para a janela principal: o X foi clicado (`PedidoFechar`).
pub const EVENTO_PEDIR_FECHAR: &str = "pedir-fechar";
/// Evento para a janela principal: a janela foi minimizada (`PedidoFechar`, com a ação do minimizar).
pub const EVENTO_PEDIR_MINIMIZAR: &str = "pedir-minimizar";
/// Evento para a janela principal: abrir nos Ajustes (o ⚙ do dock).
pub const EVENTO_ABRIR_AJUSTES: &str = "abrir-ajustes";
/// Evento para a janela principal: há um "+ Novo timer…" esperando (`novo_timer_pedido`).
pub const EVENTO_NOVO_TIMER: &str = "novo-timer";
/// Evento para a janela principal: o dock pediu o "Ligar a um timer" de uma sessão (rodada 16).
pub const EVENTO_LIGAR_SESSAO: &str = "ligar-sessao";
/// Quanto o X espera a página responder antes de fechar sozinho.
pub const ESPERA_DA_PAGINA: Duration = Duration::from_secs(2);

/// O que o X da janela principal faz (`ConfigApp::ao_fechar`; valor desconhecido = perguntar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AoFechar {
    Perguntar,
    /// A janela é destruída (o WebView sai da memória); a bandeja e o dock continuam.
    Bandeja,
    Sair,
}

impl AoFechar {
    pub fn de(valor: &str) -> AoFechar {
        match valor {
            AO_FECHAR_BANDEJA => AoFechar::Bandeja,
            AO_FECHAR_SAIR => AoFechar::Sair,
            _ => AoFechar::Perguntar,
        }
    }

    pub fn valor(self) -> &'static str {
        match self {
            AoFechar::Perguntar => nucleo::config::AO_FECHAR_PERGUNTAR,
            AoFechar::Bandeja => AO_FECHAR_BANDEJA,
            AoFechar::Sair => AO_FECHAR_SAIR,
        }
    }

    /// A página não respondeu ao X (ainda carregando ou quebrada): "sair" sai; o resto vai
    /// para a bandeja, que segue contando.
    pub fn sem_resposta(self) -> AoFechar {
        match self {
            AoFechar::Sair => AoFechar::Sair,
            _ => AoFechar::Bandeja,
        }
    }
}

/// O que o minimizar da janela principal faz (`ConfigApp::ao_minimizar`; valor desconhecido =
/// perguntar).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AoMinimizar {
    Perguntar,
    /// Como o X com "Manter na bandeja": a janela é destruída; a bandeja e o dock continuam.
    Bandeja,
    /// Minimiza e mostra o dock.
    Dock,
    /// Minimiza na barra de tarefas, e só.
    Barra,
}

impl AoMinimizar {
    pub fn de(valor: &str) -> AoMinimizar {
        match valor {
            AO_MINIMIZAR_BANDEJA => AoMinimizar::Bandeja,
            AO_MINIMIZAR_DOCK => AoMinimizar::Dock,
            AO_MINIMIZAR_BARRA => AoMinimizar::Barra,
            _ => AoMinimizar::Perguntar,
        }
    }

    pub fn valor(self) -> &'static str {
        match self {
            AoMinimizar::Perguntar => nucleo::config::AO_MINIMIZAR_PERGUNTAR,
            AoMinimizar::Bandeja => AO_MINIMIZAR_BANDEJA,
            AoMinimizar::Dock => AO_MINIMIZAR_DOCK,
            AoMinimizar::Barra => AO_MINIMIZAR_BARRA,
        }
    }

    /// A página não respondeu: "bandeja" fecha sem ela; "perguntar" minimiza na barra.
    pub fn sem_resposta(self) -> AoMinimizar {
        match self {
            AoMinimizar::Bandeja => AoMinimizar::Bandeja,
            _ => AoMinimizar::Barra,
        }
    }
}

// ---- X e minimizar da janela principal ----
// Nenhum dos dois age direto: a página grava o que estiver pendente (um campo dos Ajustes ainda
// sem `change`) e responde com `fechar_principal` / `minimizar_principal` (ou com
// `fechar_visto` / `minimizar_visto`, quando vai perguntar). Sem resposta em
// `ESPERA_DA_PAGINA`, o Rust faz sozinho (`sem_resposta`).

/// O que vai para a página nos eventos `pedir-fechar` e `pedir-minimizar`.
#[derive(Clone, Debug, Serialize)]
pub struct PedidoFechar {
    pub id: u64,
    /// "perguntar", "bandeja", "sair" (fechar) ou "barra" (minimizar).
    pub acao: &'static str,
}

/// Um pedido esperando a página (0 = nenhum).
pub struct Pedido(AtomicU64);

static PROXIMO_PEDIDO: AtomicU64 = AtomicU64::new(1);

impl Pedido {
    const fn novo() -> Pedido {
        Pedido(AtomicU64::new(0))
    }

    /// Abre um pedido e devolve o id dele.
    pub fn abrir(&self) -> u64 {
        let id = PROXIMO_PEDIDO.fetch_add(1, Ordering::SeqCst);
        self.0.store(id, Ordering::SeqCst);
        id
    }

    /// A página respondeu (ou vai perguntar).
    pub fn responder(&self) {
        self.0.store(0, Ordering::SeqCst);
    }

    /// O prazo do pedido `id` venceu sem resposta? (e fecha o pedido)
    pub fn vencer(&self, id: u64) -> bool {
        self.0.compare_exchange(id, 0, Ordering::SeqCst, Ordering::SeqCst).is_ok()
    }

    pub fn aberto(&self) -> bool {
        self.0.load(Ordering::SeqCst) != 0
    }
}

static PEDIDO_FECHAR: Pedido = Pedido::novo();
static PEDIDO_MINIMIZAR: Pedido = Pedido::novo();
/// O próprio app minimizou a janela (a resposta "Barra de tarefas") neste instante (ms desde
/// 1970; 0 = nunca): os avisos de minimizado que chegarem logo depois não perguntam de novo.
static MINIMIZOU_SOZINHO_EM: AtomicU64 = AtomicU64::new(0);
/// Quanto tempo depois do próprio minimizar os avisos são ignorados.
pub const FOLGA_DO_MINIMIZAR: Duration = Duration::from_secs(2);

fn agora_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Um aviso de minimizado em `agora` (ms) é do minimizar que o próprio app fez em `feito`?
pub fn minimizado_pelo_app(feito: u64, agora: u64) -> bool {
    feito != 0 && agora >= feito && agora - feito < FOLGA_DO_MINIMIZAR.as_millis() as u64
}

/// Emite o pedido e, se a página não responder a tempo, faz `sem_resposta` sozinho.
fn pedir_a_pagina(app: &AppHandle, pedido: &'static Pedido, evento: &'static str, acao: &'static str, sem_resposta: impl FnOnce(&AppHandle) + Send + 'static) {
    let id = pedido.abrir();
    let _ = app.emit_to(PRINCIPAL, evento, PedidoFechar { id, acao });
    let app = app.clone();
    let _ = std::thread::Builder::new().name("worktimer-pedido".into()).spawn(move || {
        std::thread::sleep(ESPERA_DA_PAGINA);
        if pedido.vencer(id) {
            log_debug!("worktimer: a página não respondeu a {evento}; seguindo sem ela");
            sem_resposta(&app);
        }
    });
}

/// O X da janela principal, com a config `ao_fechar` já lida.
pub fn pedir_fechar(app: &AppHandle, janela: &Window, modo: AoFechar) {
    if modo == AoFechar::Perguntar {
        // fechar pela barra de tarefas com a janela minimizada (ou atrás de outras): o
        // diálogo precisa aparecer
        let _ = janela.unminimize();
        let _ = janela.show();
        let _ = janela.set_focus();
    }
    pedir_a_pagina(app, &PEDIDO_FECHAR, EVENTO_PEDIR_FECHAR, modo.valor(), move |app| {
        // sem a página: "sair" sai; o resto vai para a bandeja
        fechar_com(app, modo.sem_resposta());
    });
}

/// A página respondeu ao X (ou vai perguntar ao usuário): o Rust não fecha sozinho.
pub fn pedido_respondido() {
    PEDIDO_FECHAR.responder();
}

/// A janela principal foi minimizada (`janela.is_minimized()`), com a config `ao_minimizar` já
/// lida. "Dock" mostra o dock e "barra" deixa como está; "bandeja" e "perguntar" passam pela
/// página (com "perguntar", a janela volta para o diálogo aparecer).
pub fn ao_minimizar(app: &AppHandle, janela: &Window, modo: AoMinimizar) {
    // o minimizar que o próprio app pediu, ou um pedido ainda esperando a página
    if minimizado_pelo_app(MINIMIZOU_SOZINHO_EM.load(Ordering::SeqCst), agora_ms()) || PEDIDO_MINIMIZAR.aberto() {
        return;
    }
    match modo {
        AoMinimizar::Dock => mostrar_dock(app),
        AoMinimizar::Barra => {}
        AoMinimizar::Perguntar | AoMinimizar::Bandeja => {
            if modo == AoMinimizar::Perguntar {
                let _ = janela.unminimize();
                let _ = janela.show();
                let _ = janela.set_focus();
            }
            pedir_a_pagina(app, &PEDIDO_MINIMIZAR, EVENTO_PEDIR_MINIMIZAR, modo.valor(), move |app| {
                minimizar_com(app, modo.sem_resposta())
            });
        }
    }
}

/// A página respondeu ao minimizar (ou vai perguntar): o Rust não age sozinho.
pub fn minimizar_respondido() {
    PEDIDO_MINIMIZAR.responder();
}

/// "Bandeja": destrói a janela principal (como o X). "Barra": minimiza sem perguntar de novo.
/// "Dock": minimiza e mostra o dock (com o dock "Só quando houver uma pergunta", só minimiza:
/// `mostrar_dock`).
pub fn minimizar_com(app: &AppHandle, modo: AoMinimizar) {
    match modo {
        AoMinimizar::Bandeja => fechar_principal(app),
        AoMinimizar::Barra | AoMinimizar::Dock => {
            if let Some(j) = app.get_webview_window(PRINCIPAL) {
                MINIMIZOU_SOZINHO_EM.store(agora_ms(), Ordering::SeqCst);
                let _ = j.minimize();
            }
            if modo == AoMinimizar::Dock {
                mostrar_dock(app);
            }
        }
        AoMinimizar::Perguntar => {}
    }
}

/// "Bandeja": destrói a janela principal. "Sair": encerra o app. "Perguntar": nada.
pub fn fechar_com(app: &AppHandle, modo: AoFechar) {
    match modo {
        AoFechar::Sair => sair(app),
        AoFechar::Bandeja => fechar_principal(app),
        AoFechar::Perguntar => {}
    }
}

/// Encerra o app (o "Sair" da bandeja e o "sair" do X).
pub fn sair(app: &AppHandle) {
    guardar_lugar_principal(app);
    app.exit(0);
}
/// Argumento que o "Iniciar com o Windows" põe na linha de comando: abre só o dock e a bandeja
/// (com o dock "Só quando houver uma pergunta", só a bandeja).
pub const ARG_BANDEJA: &str = "--bandeja";

/// O app foi aberto pelo Windows (com `--bandeja`)? Então a janela principal não é criada.
pub fn so_bandeja(args: impl IntoIterator<Item = String>) -> bool {
    args.into_iter().skip(1).any(|a| a == ARG_BANDEJA)
}

/// A janela principal é criada (e mostrada) na partida? Sempre, menos quando o Windows abriu
/// o app (`--bandeja`); mas o primeiro uso (nenhum timer ainda) precisa dela mesmo assim.
pub fn principal_na_partida(so_bandeja: bool, primeiro_uso: bool) -> bool {
    !so_bandeja || primeiro_uso
}
pub const DOCK: &str = "dock";
/// Dock expandido: tamanho padrão e mínimo (lógicos).
pub const EXPANDIDO_PADRAO: (f64, f64) = (320.0, 420.0);
pub const EXPANDIDO_MIN: (f64, f64) = (280.0, 240.0);
/// Dock recolhido antes de a página medir o conteúdo (lógico).
pub const RECOLHIDO_INICIAL: (f64, f64) = (240.0, 46.0);
/// Distância da borda da tela quando não há posição salva (lógica).
pub const MARGEM: f64 = 16.0;
pub use crate::lugar_dock::{dentro, Lado, Ret};
use crate::lugar_dock::{self as lugar, Monitor};

/// Título da janela do dock (não aparece na tela; o Windows mostra em listas de janelas).
pub fn titulo_dock() -> String {
    format!("{} — dock", crate::perfil::nome_na_tela())
}

/// "● N rodando · Chronn", ou só "Chronn" (o nome de exibição, `APP_NOME`).
pub fn titulo_principal(rodando: usize) -> String {
    let nome = crate::perfil::nome_na_tela();
    if rodando == 0 {
        nome
    } else {
        nucleo::idioma::tfa("janela.rodando", &[&rodando.to_string(), &nome])
    }
}

// ---- com Tauri ----

fn ret_de(j: &WebviewWindow) -> Option<Ret> {
    let p = j.outer_position().ok()?;
    let s = j.outer_size().ok()?;
    Some(Ret { x: p.x, y: p.y, w: s.width as i32, h: s.height as i32 })
}

/// Os monitores agora, com a tela inteira, a área útil e a escala (spec 005). No Windows, direto
/// do Win32: não passa pela thread principal, e o laço também usa.
pub fn monitores(app: &AppHandle) -> Vec<Monitor> {
    #[cfg(windows)]
    {
        let _ = app;
        win::monitores()
    }
    #[cfg(not(windows))]
    {
        let ret = |p: tauri::PhysicalPosition<i32>, s: tauri::PhysicalSize<u32>| Ret { x: p.x, y: p.y, w: s.width as i32, h: s.height as i32 };
        let principal = app.primary_monitor().ok().flatten().map(|m| *m.position());
        app.available_monitors()
            .map(|v| {
                v.iter()
                    .map(|m| Monitor {
                        tela: ret(*m.position(), *m.size()),
                        util: ret(m.work_area().position, m.work_area().size),
                        escala: m.scale_factor(),
                        principal: principal == Some(*m.position()),
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Um tamanho lógico em pixels físicos, na escala dada.
fn fisico(t: (f64, f64), escala: f64) -> (i32, i32) {
    ((t.0 * escala).round() as i32, (t.1 * escala).round() as i32)
}

pub fn dock(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(DOCK)
}

/// Mostra a janela principal; se ela foi fechada para a bandeja, recria. Foi a pessoa: a janela
/// não se esconde de novo depois de um diálogo do dock.
pub fn mostrar_principal(app: &AppHandle) {
    serie_do_dock().zerar();
    abrir_principal(app, None, false);
}

/// O duplo clique no dock: como `mostrar_principal`, mas a janela vem para o monitor do dock
/// (spec 005, US 005.4).
pub fn mostrar_principal_pelo_dock(app: &AppHandle) {
    serie_do_dock().zerar();
    abrir_principal(app, None, true);
}

/// Mostra a janela principal nos Ajustes (recria se preciso): o ⚙ do dock, no monitor dele.
pub fn mostrar_ajustes(app: &AppHandle) {
    serie_do_dock().zerar();
    abrir_principal(app, Some(PEDIDO_AJUSTES), true);
}

/// O "Sobre o …" da bandeja (rodada 18): a janela principal em Ajustes → Sobre.
pub fn mostrar_sobre(app: &AppHandle) {
    serie_do_dock().zerar();
    abrir_principal(app, Some(PEDIDO_SOBRE), false);
}

/// Uma recriação por vez (dois cliques rápidos na bandeja não criam duas).
static RECRIANDO: AtomicBool = AtomicBool::new(false);
/// O ⚙ do dock pediu os Ajustes e a página ainda não buscou (`ajustes_pedidos`). Guardado
/// aqui, não num script da página: vale também para o ⚙ clicado enquanto a janela nasce, e
/// um recarregamento não volta para os Ajustes.
/// 0: nada; `PEDIDO_AJUSTES`: os Ajustes; `PEDIDO_SOBRE`: Ajustes → Sobre (rodada 18).
static AJUSTES_PEDIDOS: AtomicU8 = AtomicU8::new(0);
const PEDIDO_AJUSTES: u8 = 1;
const PEDIDO_SOBRE: u8 = 2;

/// A página pergunta (ao abrir e ao receber `abrir-ajustes`) se deve ir para os Ajustes:
/// "ajustes", "sobre" (a seção) ou nada.
pub fn ajustes_pedidos() -> Option<&'static str> {
    match AJUSTES_PEDIDOS.swap(0, Ordering::SeqCst) {
        PEDIDO_AJUSTES => Some("ajustes"),
        PEDIDO_SOBRE => Some("sobre"),
        _ => None,
    }
}

/// O "+ Novo timer…" da pergunta do dock, esperando a página buscar (como o ⚙): vale também se a
/// janela principal ainda vai nascer. Um pedido novo troca o anterior.
static NOVO_TIMER: Mutex<Option<PedidoNovoTimer>> = Mutex::new(None);

/// Guarda o pedido, mostra (ou recria) a janela principal e avisa a página.
pub fn pedir_novo_timer(app: &AppHandle, pedido: PedidoNovoTimer) {
    // o pedido repetido (dois cliques) troca o anterior sem contar outro diálogo: a página avisa
    // o fim uma vez só (o A10 da revisão de 25/11; re-revisão, R7)
    let repetido = NOVO_TIMER.lock().unwrap_or_else(|p| p.into_inner()).replace(pedido).is_some();
    if !repetido {
        lembrar_antes_do_dialogo(app);
    }
    abrir_principal(app, None, true);
    let _ = app.emit_to(PRINCIPAL, EVENTO_NOVO_TIMER, ());
}

// ---- diálogos do dock na janela principal (rodada 16) ----
// Nenhum diálogo abre dentro do dock: o "Escolher timer…", o "Outro timer…", o "Trocar" e o
// "+ Novo timer…" abrem na janela principal, que aparece na frente com o foco. Se ela estava
// escondida (ou fechada para a bandeja), volta a ficar assim quando o diálogo fecha.

/// O "Ligar a um timer" pedido pelo dock: a sessão, o que mostrar se ela não estiver na lista
/// das recentes, e se a lista oferece o "+ Novo timer…" (só com a pergunta da sessão).
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct PedidoLigar {
    pub sessao_id: String,
    pub pergunta_id: Option<u64>,
    pub nome: Option<String>,
    pub cwd: Option<String>,
    pub timer_id: Option<i64>,
    pub com_novo: bool,
    /// O "Sempre que for na pasta" da pergunta de sessão nova (rodada 27): marcado ou não, e a
    /// pasta escolhida na lista. Nulos (o "Trocar", a página antiga): como antes.
    #[serde(default)]
    pub sempre: Option<bool>,
    #[serde(default)]
    pub pasta: Option<String>,
}

static LIGAR: Mutex<Option<PedidoLigar>> = Mutex::new(None);

/// Guarda o pedido do dock, mostra (ou recria) a janela principal na frente e avisa a página.
pub fn pedir_ligar_sessao(app: &AppHandle, pedido: PedidoLigar) {
    // o pedido repetido troca o anterior sem contar outro diálogo (o A10; re-revisão, R7)
    let repetido = LIGAR.lock().unwrap_or_else(|p| p.into_inner()).replace(pedido).is_some();
    if !repetido {
        lembrar_antes_do_dialogo(app);
    }
    abrir_principal(app, None, true);
    let _ = app.emit_to(PRINCIPAL, EVENTO_LIGAR_SESSAO, ());
}

/// A página busca o pedido (ao abrir e ao receber `ligar-sessao`); vale uma vez.
pub fn ligar_sessao_pedido() -> Option<PedidoLigar> {
    LIGAR.lock().unwrap_or_else(|p| p.into_inner()).take()
}

/// Como a janela principal estava quando o dock pediu um diálogo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AntesDoDialogo {
    /// À vista (também minimizada): fica como o diálogo a deixar.
    AVista,
    /// Existia, escondida: volta a se esconder.
    Escondida,
    /// Fechada para a bandeja (o WebView fora da memória): é recriada e volta a fechar.
    Fechada,
}

/// O que fazer com a janela principal quando o diálogo pedido pelo dock fecha.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DepoisDoDialogo {
    Nada,
    Esconder,
    Fechar,
}

pub fn antes_do_dialogo(existe: bool, visivel: bool) -> AntesDoDialogo {
    match (existe, visivel) {
        (false, _) => AntesDoDialogo::Fechada,
        (true, false) => AntesDoDialogo::Escondida,
        (true, true) => AntesDoDialogo::AVista,
    }
}

pub fn depois_do_dialogo(antes: Option<AntesDoDialogo>) -> DepoisDoDialogo {
    match antes {
        Some(AntesDoDialogo::Escondida) => DepoisDoDialogo::Esconder,
        Some(AntesDoDialogo::Fechada) => DepoisDoDialogo::Fechar,
        Some(AntesDoDialogo::AVista) | None => DepoisDoDialogo::Nada,
    }
}

/// Os diálogos pedidos pelo dock abertos agora (cada pedido conta um, e a página avisa quando o
/// dele acaba) e como a janela estava quando o primeiro abriu (revisão da rodada 16: um controle
/// explícito, sem prazo).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SerieDoDock {
    pub abertos: u32,
    pub antes: Option<AntesDoDialogo>,
}

impl SerieDoDock {
    /// O dock pediu um diálogo; `agora`: como a janela está neste instante.
    pub fn abriu(&mut self, agora: AntesDoDialogo) {
        if self.abertos == 0 {
            self.antes = Some(agora);
        }
        self.abertos += 1;
    }

    /// O diálogo de um pedido acabou. Quando acaba o último, a janela volta como estava.
    pub fn fechou(&mut self) -> DepoisDoDialogo {
        if self.abertos == 0 {
            return DepoisDoDialogo::Nada;
        }
        self.abertos -= 1;
        if self.abertos > 0 {
            return DepoisDoDialogo::Nada;
        }
        depois_do_dialogo(self.antes.take())
    }

    /// A pessoa mostrou a janela (bandeja, duplo clique no dock, ⚙) ou ela foi fechada: o estado de
    /// antes não vale mais (ela fica como você a deixou).
    pub fn zerar(&mut self) {
        *self = SerieDoDock::default();
    }
}

static SERIE_DO_DOCK: Mutex<SerieDoDock> = Mutex::new(SerieDoDock { abertos: 0, antes: None });

fn serie_do_dock() -> std::sync::MutexGuard<'static, SerieDoDock> {
    SERIE_DO_DOCK.lock().unwrap_or_else(|p| p.into_inner())
}

fn lembrar_antes_do_dialogo(app: &AppHandle) {
    let j = app.get_webview_window(PRINCIPAL);
    let agora = antes_do_dialogo(j.is_some(), j.is_some_and(|j| j.is_visible().unwrap_or(false)));
    serie_do_dock().abriu(agora);
}

/// A página avisa que o diálogo de um pedido do dock acabou (de qualquer jeito): com o último, a
/// janela volta a ficar como estava.
pub fn dialogo_do_dock_fechou(app: &AppHandle) {
    let depois = serie_do_dock().fechou();
    match depois {
        DepoisDoDialogo::Nada => {}
        // a janela estava emprestada ao diálogo: o lugar dela não é gravado (revisão, I3)
        DepoisDoDialogo::Esconder => {
            if let Some(j) = app.get_webview_window(PRINCIPAL) {
                let _ = j.hide();
            }
            // escondida, ela não segura a vez das dicas
            crate::comandos::liberar_dicas_da_janela(app, PRINCIPAL);
        }
        DepoisDoDialogo::Fechar => fechar_principal_e(app, false),
    }
}

/// A página busca o pedido (ao abrir e ao receber `novo-timer`); vale uma vez.
pub fn novo_timer_pedido() -> Option<PedidoNovoTimer> {
    NOVO_TIMER.lock().unwrap_or_else(|p| p.into_inner()).take()
}

/// `pelo_dock`: o pedido veio do dock; a janela que está noutro monitor vem para o dele (US 005.4).
fn abrir_principal(app: &AppHandle, pedido: Option<u8>, pelo_dock: bool) {
    let ajustes = pedido.is_some();
    if let Some(p) = pedido {
        AJUSTES_PEDIDOS.store(p, Ordering::SeqCst);
    }
    if let Some(j) = app.get_webview_window(PRINCIPAL) {
        // só a escondida vem para o monitor do dock: a que a pessoa já vê fica onde está (revisão,
        // I3); maximizada ou minimizada, também fica
        if pelo_dock && !j.is_visible().unwrap_or(true) && !j.is_maximized().unwrap_or(false) && !j.is_minimized().unwrap_or(false) {
            if let Some(r) = ret_de(&j).and_then(|r| perto_do_dock(app, r)) {
                por_principal(&j, r, j.scale_factor().ok());
            }
        }
        let _ = j.show();
        let _ = j.unminimize();
        let _ = j.set_focus();
        if ajustes {
            let _ = app.emit_to(PRINCIPAL, EVENTO_ABRIR_AJUSTES, ());
        }
        return;
    }
    if RECRIANDO.swap(true, Ordering::SeqCst) {
        return;
    }
    // Criar janela no laço de eventos (bandeja, comando síncrono) trava o WebView2 no Windows:
    // a criação vai numa thread própria.
    let app = app.clone();
    let r = std::thread::Builder::new().name("worktimer-janela".into()).spawn(move || {
        if let Err(e) = criar_principal(&app, pelo_dock) {
            log_debug!("worktimer: não consegui recriar a janela principal: {e}");
        }
        RECRIANDO.store(false, Ordering::SeqCst);
    });
    if r.is_err() {
        RECRIANDO.store(false, Ordering::SeqCst);
    }
}

/// Cria a janela principal a partir do `tauri.conf.json` (mesmo label, tamanho, opções e
/// argumentos do WebView2) e mostra. Lá ela tem `"create": false`: o Tauri não a cria na
/// partida; quem cria é o setup (fora do `--bandeja`) ou `mostrar_principal`, quando precisa.
/// Fora do setup, chamar numa thread própria (no laço de eventos o WebView2 trava).
/// Só falha se a janela não nasce; mostrar e focar são tentados sem derrubar nada.
/// Ela nasce no lugar salvo (se ainda aparece num monitor); `pelo_dock`, no monitor do dock
/// (spec 005, US 005.4).
pub fn criar_principal(app: &AppHandle, pelo_dock: bool) -> tauri::Result<()> {
    let mut conf = app
        .config()
        .app
        .windows
        .iter()
        .find(|w| w.label == PRINCIPAL)
        .cloned()
        .ok_or(tauri::Error::WindowNotFound)?;
    // o título vem do nome de exibição (no tauri.conf.json fica vazio)
    conf.title = crate::perfil::nome_na_tela();
    let j = WebviewWindowBuilder::from_config(app, &conf)?.build()?;
    // ainda escondida: vai para o lugar dela antes de aparecer
    let (alvo, escala, maximizada) = lugar_ao_nascer(app, &j, pelo_dock);
    // título com a contagem de rodando, como a janela que foi fechada
    crate::laco::avisar(app);
    let _ = j.show();
    // de novo depois de aparecer: se a troca de escala do monitor só chegou agora, o tamanho
    // volta ao certo (revisão, I2)
    if let Some(r) = alvo {
        por_principal(&j, r, escala);
    }
    if maximizada {
        let _ = j.maximize();
    }
    let _ = j.set_focus();
    Ok(())
}

/// "Manter na bandeja": destrói a janela principal (o WebView dela sai da memória). Os diálogos
/// do dock que estavam nela acabaram junto.
pub fn fechar_principal(app: &AppHandle) {
    fechar_principal_e(app, true);
}

/// `guardar`: grava o lugar antes (não, no fim de um diálogo do dock com a janela emprestada).
fn fechar_principal_e(app: &AppHandle, guardar: bool) {
    if guardar {
        guardar_lugar_principal(app);
    }
    serie_do_dock().zerar();
    crate::comandos::liberar_dicas_da_janela(app, PRINCIPAL);
    if let Some(j) = app.get_webview_window(PRINCIPAL) {
        let _ = j.destroy();
    }
}

pub fn atualizar_titulo(app: &AppHandle, rodando: usize) {
    if let Some(j) = app.get_webview_window(PRINCIPAL) {
        let _ = j.set_title(&titulo_principal(rodando));
    }
}

// ---- dock opcional (rodada 12) ----
// Em Ajustes, o dock fica "Sempre à vista" (o padrão) ou "Só quando houver uma pergunta": aí ele
// fica escondido e aparece sozinho com uma pergunta do núcleo (sessão nova, volta da inatividade,
// app fechado; os lembretes "esperando você" não contam), sem roubar o foco, e se esconde de novo
// quando não sobra nenhuma. A bandeja tem "Mostrar o dock agora". Quem decide é `dock_visivel`.

/// Quando o dock aparece (`ConfigApp::dock_modo`; valor desconhecido = sempre).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModoDock {
    /// "Sempre à vista": o dock de sempre (a bandeja pode escondê-lo).
    Sempre,
    /// "Só quando houver uma pergunta".
    SoPerguntas,
}

impl ModoDock {
    pub fn de(valor: &str) -> ModoDock {
        if valor == DOCK_SO_PERGUNTAS {
            ModoDock::SoPerguntas
        } else {
            ModoDock::Sempre
        }
    }
}

/// O "Mostrar o dock agora" (modo escondido) vale até um clique fora ou até isto sem o mouse em
/// cima do dock, o que vier primeiro.
pub const MOSTRAR_AGORA_MS: u64 = 30_000;

/// O que a bandeja pediu do dock.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PedidoDock {
    #[default]
    Nenhum,
    /// "Sempre à vista", escondido pelo "Mostrar/esconder dock" da bandeja.
    Escondido,
    /// "Mostrar o dock agora" no modo escondido: `desde` é o pedido ou a última saída do mouse
    /// (ms); `clicou_fora`, o dock perdeu o foco.
    MostrarAgora { desde: u64, mouse_em_cima: bool, clicou_fora: bool },
}

/// O dock aparece? Com pergunta pendente, sempre, nos dois modos (nada se perde; revisão da
/// rodada 12, I1: também com o dock escondido pela bandeja, que volta a se esconder quando não
/// sobra pergunta). Sem pergunta: "Sempre à vista", sim, menos escondido pela bandeja; "Só quando
/// houver uma pergunta", só pelo "Mostrar o dock agora" ainda valendo (sem clique fora, e com o
/// mouse em cima ou há menos de 30 s).
pub fn dock_visivel(modo: ModoDock, perguntas: usize, pedido: PedidoDock, agora: u64) -> bool {
    match modo {
        ModoDock::Sempre => perguntas > 0 || pedido != PedidoDock::Escondido,
        ModoDock::SoPerguntas => {
            perguntas > 0
                || match pedido {
                    PedidoDock::MostrarAgora { desde, mouse_em_cima, clicou_fora } => {
                        !clicou_fora && (mouse_em_cima || agora.saturating_sub(desde) < MOSTRAR_AGORA_MS)
                    }
                    _ => false,
                }
        }
    }
}

static PEDIDO_DOCK: Mutex<PedidoDock> = Mutex::new(PedidoDock::Nenhum);

fn pedido_dock() -> std::sync::MutexGuard<'static, PedidoDock> {
    PEDIDO_DOCK.lock().unwrap_or_else(|p| p.into_inner())
}

/// O modo do dock e as perguntas pendentes agora (sem núcleo: sempre à vista, sem perguntas).
/// Chamar sem a trava do núcleo.
fn modo_e_perguntas(app: &AppHandle) -> (ModoDock, usize) {
    match app.try_state::<Estado>() {
        Some(e) => {
            let n = trava(&e);
            let modo = n.banco.config_app().map(|c| ModoDock::de(&c.dock_modo)).unwrap_or(ModoDock::Sempre);
            (modo, n.motor.perguntas.len())
        }
        None => (ModoDock::Sempre, 0),
    }
}

/// Mostra ou esconde o dock conforme `dock_visivel` (a cada mudança de estado, a cada ciclo do
/// laço e a cada pedido da bandeja). Ao aparecer sozinho, não pede o foco. Chamar sem a trava do
/// núcleo.
pub fn aplicar_dock(app: &AppHandle) {
    let (modo, perguntas) = modo_e_perguntas(app);
    let agora = agora_ms();
    let quer = {
        let mut p = pedido_dock();
        let quer = dock_visivel(modo, perguntas, *p, agora);
        // o "Mostrar o dock agora" que venceu sai (uma pergunta depois não o revive)
        if matches!(*p, PedidoDock::MostrarAgora { .. }) && !dock_visivel(modo, 0, *p, agora) {
            *p = PedidoDock::Nenhum;
        }
        quer
    };
    let Some(d) = dock(app) else { return };
    if d.is_visible().unwrap_or(false) != quer {
        if quer {
            mostrar_sem_ativar(&d);
        } else {
            let _ = d.hide();
            // o aviso do dock não fica sozinho na tela
            esconder_aviso(app);
        }
    }
    bandeja::atualizar_item_dock(app, modo == ModoDock::SoPerguntas);
}

/// Mostra o dock **sem ativá-lo**: a pergunta chega logo depois do Enter no Claude, e o foco não
/// pode sair de lá (revisão da rodada 12). O `show()` do Tauri não serve sozinho: no Windows, o
/// tao só usa `SW_SHOWNOACTIVATE` na criação da janela; depois, `show()` é `ShowWindow(SW_SHOW)`,
/// que ativa. Aqui a janela aparece com `SW_SHOWNOACTIVATE`, e o `show()` em seguida só acerta o
/// estado do tao (sem ele, o próximo `set_size`/`set_position` a esconderia de novo); numa janela
/// já visível, o `SW_SHOW` não faz nada. É o único lugar em que o dock aparece (`aplicar_dock`);
/// o "Mostrar o dock agora" da bandeja pede o foco depois, de propósito.
fn mostrar_sem_ativar(d: &WebviewWindow) {
    #[cfg(windows)]
    if let Ok(h) = d.hwnd() {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{ShowWindow, SW_SHOWNOACTIVATE};
        // SAFETY: a janela é do próprio app e existe (o `hwnd` acabou de vir dela)
        unsafe {
            let _ = ShowWindow(HWND(h.0), SW_SHOWNOACTIVATE);
        }
    }
    let _ = d.show();
}

/// O item do dock na bandeja, um só (revisão da rodada 12), com o texto do modo
/// (`bandeja::texto_do_item_dock`). "Sempre à vista": "Mostrar/esconder dock" esconde ou mostra
/// (com uma pergunta pendente, o escondido aparece mesmo assim). "Só quando houver uma pergunta":
/// "Mostrar o dock agora", até o próximo clique fora ou 30 s sem o mouse em cima; aí o dock pega o
/// foco (foi pedido), para o clique fora valer.
pub fn item_dock(app: &AppHandle) {
    let (modo, _) = modo_e_perguntas(app);
    match modo {
        ModoDock::Sempre => {
            let visivel = dock(app).is_some_and(|d| d.is_visible().unwrap_or(false));
            *pedido_dock() = if visivel { PedidoDock::Escondido } else { PedidoDock::Nenhum };
            aplicar_dock(app);
        }
        ModoDock::SoPerguntas => {
            *pedido_dock() = PedidoDock::MostrarAgora { desde: agora_ms(), mouse_em_cima: false, clicou_fora: false };
            aplicar_dock(app);
            if let Some(d) = dock(app) {
                let _ = d.set_focus();
            }
        }
    }
}

/// O minimizar "Dock": mostra o dock se ele está "Sempre à vista" (tira o escondido pela
/// bandeja); no modo escondido, nada (só minimiza).
pub fn mostrar_dock(app: &AppHandle) {
    let (modo, _) = modo_e_perguntas(app);
    if modo == ModoDock::Sempre {
        *pedido_dock() = PedidoDock::Nenhum;
        aplicar_dock(app);
    }
}

/// O dock perdeu o foco (um clique fora): o "Mostrar o dock agora" acaba.
pub fn dock_perdeu_foco(app: &AppHandle) {
    // o clique no "Desfazer" do aviso tira o foco do dock: não é um clique fora
    if aviso_visivel(app) {
        return;
    }
    {
        let mut p = pedido_dock();
        if let PedidoDock::MostrarAgora { clicou_fora, .. } = &mut *p {
            *clicou_fora = true;
        } else {
            return;
        }
    }
    aplicar_dock(app);
}

/// O mouse entrou no dock ou saiu dele (a página avisa): os 30 s do "Mostrar o dock agora"
/// contam da saída.
pub fn dock_mouse(app: &AppHandle, dentro: bool) {
    {
        let mut p = pedido_dock();
        if let PedidoDock::MostrarAgora { desde, mouse_em_cima, .. } = &mut *p {
            *mouse_em_cima = dentro;
            if !dentro {
                *desde = agora_ms();
            }
        } else {
            return;
        }
    }
    aplicar_dock(app);
}

/// Lado da tela em que o dock está agora (no monitor dele, pela tela inteira).
pub fn lado_do_dock(app: &AppHandle) -> Lado {
    dock(app).and_then(|d| ret_de(&d)).map(|r| lugar::lado_no_monitor(r, &monitores(app))).unwrap_or_default()
}

/// O tamanho do expandido (físico) na escala dada: o salvo em pixels lógicos (spec 005); o de
/// antes, já físico; ou o padrão.
fn tamanho_expandido(cfg: &ConfigApp, escala: f64) -> (i32, i32) {
    match (cfg.dock_larg_l, cfg.dock_alt_l, cfg.dock_larg, cfg.dock_alt) {
        (Some(w), Some(h), _, _) if w > 0.0 && h > 0.0 => fisico((w, h), escala),
        (_, _, Some(w), Some(h)) if w > 0 && h > 0 => (w, h),
        _ => fisico(EXPANDIDO_PADRAO, escala),
    }
}

/// O último retângulo que o app pediu ao dock: o `Moved` desse pedido não é da pessoa (US 005.3).
static PEDIDO_RET: Mutex<Option<Ret>> = Mutex::new(None);
/// O retângulo do arrastar da pessoa (e se o dock estava expandido nele), esperando o
/// `dock_guardar` da página para virar a escolha.
static ESCOLHA_PENDENTE: Mutex<Option<(Ret, bool)>> = Mutex::new(None);
/// O dock está fora da escolha (a abertura ou o "fora dos monitores" o puseram noutro lugar):
/// o laço tenta a escolha a cada ciclo até ela valer (revisão, I1).
static FORA_DA_ESCOLHA: AtomicBool = AtomicBool::new(false);
/// A última linha "movido sem pedido" do log (ms), para no máximo uma por segundo.
static MOVIDO_LOG: AtomicU64 = AtomicU64::new(0);
/// Quando o app mexeu no dock por último (o relógio monotônico): os `Moved` de logo depois (o 1º
/// passo entre escalas, o ajuste de escala do tao, a volta do minimizado) não são "o Windows"
/// (re-revisão, R8).
static APP_MEXEU: AtomicU64 = AtomicU64::new(0);
/// A página do dock mede de novo no próximo ciclo (depois de o dock voltar do minimizado: o tamanho
/// pedido enquanto ele estava minimizado se perdeu; re-revisão, R4).
static MEDIR_DEPOIS: AtomicBool = AtomicBool::new(false);
/// Evento para o dock: medir o conteúdo de novo e mandar o tamanho, mesmo que seja o mesmo (o dock
/// trocou de escala ou voltou do minimizado).
pub const EVENTO_DOCK_MEDIR: &str = "dock-medir";

fn travar<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|p| p.into_inner())
}

/// Põe o dock no retângulo, numa chamada só (a posição e o tamanho juntos, sem o pulo do A11), e
/// lembra o pedido. `assincrono`: da thread do dock (os comandos); do laço, síncrono, para um
/// pedido velho não chegar depois de um novo da thread principal (revisão, M9). `escala`: a escala
/// em que o tamanho foi medido, quando ele vem do monitor de antes (vai para a do destino; nulo, o
/// tamanho já é o do destino; re-revisão, R1). Devolve se o dock trocou de escala.
fn por_dock(d: &WebviewWindow, r: Ret, assincrono: bool, escala: Option<f64>) -> bool {
    APP_MEXEU.store(monotonico(), Ordering::SeqCst);
    #[cfg(windows)]
    if let Ok(h) = d.hwnd() {
        let h = h.0 as isize;
        let (r, muda) = win::no_destino(h, r, escala);
        *travar(&PEDIDO_RET) = Some(r);
        win::por_no_lugar(h, r, assincrono, muda);
        return muda;
    }
    let _ = (assincrono, escala);
    *travar(&PEDIDO_RET) = Some(r);
    let _ = d.set_position(PhysicalPosition::new(r.x, r.y));
    let _ = d.set_size(PhysicalSize::new(r.w as u32, r.h as u32));
    false
}

/// A página do dock mede o conteúdo de novo (o dock trocou de escala ou voltou do minimizado).
fn pedir_medida(app: &AppHandle) {
    let _ = app.emit_to(DOCK, EVENTO_DOCK_MEDIR, ());
}

/// Troca o tamanho do dock segurando a borda do lado em que ele está, dentro da tela inteira do
/// monitor dele (pode ficar em cima da barra; sem monitor, não se move). Devolve o lado.
fn redimensionar(app: &AppHandle, d: &WebviewWindow, w: i32, h: i32) -> Lado {
    let Some(r) = ret_de(d) else { return Lado::default() };
    // minimizado: o conteúdo chega quando ele voltar (revisão, M8)
    if lugar::minimizada(r) {
        return Lado::default();
    }
    let ms = monitores(app);
    let novo = lugar::redimensionado(r, w, h, &ms);
    if novo != r {
        por_dock(d, novo, true, None);
    }
    lugar::lado_no_monitor(novo, &ms)
}

fn ajustar_limites(d: &WebviewWindow, expandido: bool) {
    let _ = d.set_resizable(expandido);
    let min = expandido.then(|| LogicalSize::new(EXPANDIDO_MIN.0, EXPANDIDO_MIN.1));
    let _ = d.set_min_size(min);
}

/// A escolha guardada na config (o retângulo inteiro; numa config de antes, o canto com `inicial`).
fn escolha_da_config(cfg: &ConfigApp, inicial: (i32, i32)) -> Option<Ret> {
    lugar::escolha_guardada(cfg.dock_x, cfg.dock_y, cfg.dock_w, cfg.dock_h, inicial)
}

/// Onde e de que tamanho o dock abre (pura): a escolha ancorada pela borda dela, no tamanho da
/// etapa na escala do monitor dela; sem o monitor da escolha, a posição padrão do principal, na
/// escala dele. O `bool`: o dock abriu fora da escolha (ela existe, mas o monitor dela não está).
fn abertura(cfg: &ConfigApp, ms: &[Monitor]) -> (Ret, bool) {
    let escala_principal = lugar::principal(ms).map_or(1.0, |m| m.escala);
    let escolha = escolha_da_config(cfg, fisico(RECOLHIDO_INICIAL, escala_principal));
    let da_escolha = escolha.and_then(|e| lugar::monitor_visivel(e, ms));
    let escala = da_escolha.or_else(|| lugar::principal(ms)).map_or(1.0, |m| m.escala);
    let (w, h) = if cfg.dock_expandido { tamanho_expandido(cfg, escala) } else { fisico(RECOLHIDO_INICIAL, escala) };
    let (x, y) = lugar::posicao_inicial(escolha, w, h, ms, (MARGEM * escala).round() as i32);
    (Ret { x, y, w, h }, escolha.is_some() && da_escolha.is_none())
}

/// Põe o dock na escolha (ancorada pela borda que ela segura), no tamanho e na etapa salvos, e
/// mostra (se o modo dele deixa). Sem o monitor da escolha, a posição padrão do principal; a
/// escolha fica guardada, e o laço a tenta a cada ciclo até o monitor chegar (US 006.4, I1).
pub fn restaurar_dock(app: &AppHandle, cfg: &ConfigApp) {
    let Some(d) = dock(app) else { return };
    let ms = monitores(app);
    let (r, fora) = abertura(cfg, &ms);
    FORA_DA_ESCOLHA.store(fora, Ordering::SeqCst);
    // a lista que a abertura usou: um monitor que chega antes do 1º ciclo do laço conta como
    // mudança (revisão, I1)
    travar(&VIGIA).monitores = Some(ms);
    ajustar_limites(&d, cfg.dock_expandido);
    por_dock(&d, r, true, None);
    anotar(if fora { "aberto (fora da escolha: o monitor dela não está)" } else { "aberto" }, Some(r));
    // "Sempre à vista" aparece já; no modo escondido, só com uma pergunta (o "Iniciar com o
    // Windows" abre então só a bandeja)
    aplicar_dock(app);
}

/// Troca a etapa. Expandido: o tamanho salvo (ou 320×420). Recolhido: o tamanho chega
/// depois, pelo `ajustar_recolhido`, quando a página mede o conteúdo.
pub fn mudar_etapa(app: &AppHandle, cfg: &ConfigApp) -> Lado {
    let Some(d) = dock(app) else { return Lado::default() };
    ajustar_limites(&d, cfg.dock_expandido);
    if cfg.dock_expandido {
        let (w, h) = tamanho_expandido(cfg, d.scale_factor().unwrap_or(1.0));
        redimensionar(app, &d, w, h)
    } else {
        lado_do_dock(app)
    }
}

/// Tamanho do recolhido medido pela página (lógico). Ignorado com o dock expandido.
pub fn ajustar_recolhido(app: &AppHandle, cfg: &ConfigApp, larg: f64, alt: f64) -> Lado {
    let Some(d) = dock(app) else { return Lado::default() };
    if cfg.dock_expandido || !(larg > 0.0 && alt > 0.0) {
        return lado_do_dock(app);
    }
    let escala = d.scale_factor().unwrap_or(1.0);
    let (w, h) = ((larg * escala).ceil() as i32, (alt * escala).ceil() as i32);
    redimensionar(app, &d, w, h)
}

/// O botão principal do mouse está apertado agora?
fn botao_apertado() -> bool {
    #[cfg(windows)]
    {
        win::botao_apertado()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// O dock se moveu ou mudou de tamanho (`main.rs`, na thread do dock). No laço de mover ou
/// redimensionar do Windows (ou com o botão do mouse apertado) e fora do que o app pediu, é a
/// pessoa arrastando (ou redimensionando o expandido): o retângulo fica como a escolha pendente,
/// com a etapa de agora (US 005.3). Fora do pedido e sem a pessoa, o Windows mexeu: uma linha no
/// log (revisão, M12). O aviso vai junto.
pub fn dock_mexeu(app: &AppHandle) {
    if let Some(d) = dock(app) {
        if let Some(r) = ret_de(&d) {
            let pedido = *travar(&PEDIDO_RET);
            if lugar::e_da_pessoa(a_pessoa_mexe(&d), r, pedido) {
                let expandido = d.is_resizable().unwrap_or(false);
                *travar(&ESCOLHA_PENDENTE) = Some((r, expandido));
            } else if pedido != Some(r) && monotonico().saturating_sub(APP_MEXEU.load(Ordering::SeqCst)) >= 1_000 {
                let agora = agora_ms();
                if agora.saturating_sub(MOVIDO_LOG.swap(agora, Ordering::SeqCst)) >= 1_000 {
                    anotar("movido sem pedido (o Windows)", Some(r));
                }
            }
        }
    }
    reposicionar_aviso(app);
}

/// A pessoa está movendo ou redimensionando a janela? O laço de mover do Windows desta thread
/// (`GUI_INMOVESIZE`: vale também com "mostrar o conteúdo ao arrastar" desligado; revisão, M5) ou
/// o botão do mouse apertado.
fn a_pessoa_mexe(d: &WebviewWindow) -> bool {
    #[cfg(windows)]
    if let Ok(h) = d.hwnd() {
        return win::em_mover(h.0 as isize) || botao_apertado();
    }
    let _ = d;
    botao_apertado()
}

/// O `dock_guardar` da página (meio segundo depois do último movimento): a escolha pendente, se
/// ela pode valer (nunca minimizada nem fora dos monitores), com a escala do monitor dela e se o
/// dock estava expandido nela.
pub fn tirar_escolha(app: &AppHandle) -> Option<(Ret, f64, bool)> {
    let (r, expandido) = travar(&ESCOLHA_PENDENTE).take()?;
    let ms = monitores(app);
    if !lugar::pode_gravar(r, &ms) {
        return None;
    }
    // o arrastar acabou: o que está na tela agora é o pedido (o `Moved` dele não é da pessoa;
    // revisão, M6)
    *travar(&PEDIDO_RET) = dock(app).and_then(|d| ret_de(&d)).or(Some(r));
    FORA_DA_ESCOLHA.store(false, Ordering::SeqCst);
    anotar("arrastado", Some(r));
    Some((r, lugar::monitor_de(r, &ms).map_or(1.0, |m| m.escala), expandido))
}

// ---- avisos fora do dock (rodada 15) ----
// As faixas do dock (o "Flow encerrado. Desfazer", um erro) aparecem numa janela pequena só delas,
// sem borda, sempre por cima e sem pegar o foco, logo abaixo do dock e centralizada nele (acima,
// se embaixo não couber na área do monitor do dock); ela acompanha o dock quando ele se move. O
// dock não muda de tamanho por causa de um aviso (no lugar do crescer da rodada 14). Com o dock
// escondido, o aviso vai para a janela principal.

/// A janela dos avisos do dock (`tauri.conf.json`).
pub const AVISO: &str = "aviso";
/// Evento para a janela do aviso: o aviso a mostrar (`AvisoDoDock`).
pub const EVENTO_AVISO_MOSTRAR: &str = "aviso-mostrar";
/// Evento para o dock: o botão do aviso (o id) foi clicado, na janela do aviso ou na principal.
pub const EVENTO_AVISO_CLICADO: &str = "aviso-clicado";
/// Evento para a janela principal: um aviso do dock com o dock escondido (`AvisoDoDock`).
pub const EVENTO_AVISO_DO_DOCK: &str = "aviso-do-dock";
/// Distância entre o dock e o aviso (lógica).
pub const MARGEM_AVISO: f64 = 6.0;

/// Um aviso do dock: o texto, o botão (o "Desfazer"), quanto tempo fica e o tema do dock.
#[derive(Clone, Debug, PartialEq, Serialize, serde::Deserialize)]
pub struct AvisoDoDock {
    pub id: u64,
    pub texto: String,
    pub botao: Option<String>,
    /// O segundo botão (rodada 16: o "Não mostrar mais dicas" de uma dica).
    #[serde(default)]
    pub botao2: Option<String>,
    pub ms: u64,
    pub tema: Option<String>,
    /// O idioma do texto (o `lang` da página do aviso; rodada 23).
    #[serde(default)]
    pub lang: Option<String>,
}

/// Onde fica o aviso de `w`×`h`: logo abaixo do dock, centralizado nele; se não couber embaixo
/// na área (o monitor do dock), logo acima; e sempre dentro da área nas laterais. Se não couber
/// nem embaixo nem em cima sem tapar o dock (um expandido alto), ao lado dele, do lado com mais
/// espaço, centralizado na altura (revisão da rodada 15).
pub fn posicao_do_aviso(dock: Ret, w: i32, h: i32, area: Ret, margem: i32) -> (i32, i32) {
    let x = dock.x + (dock.w - w) / 2;
    let abaixo = dock.y + dock.h + margem;
    if abaixo + h <= area.y + area.h {
        return dentro(x, abaixo, w, h, area);
    }
    let acima = dock.y - margem - h;
    if acima >= area.y {
        return dentro(x, acima, w, h, area);
    }
    let (a_direita, a_esquerda) = (area.x + area.w - (dock.x + dock.w), dock.x - area.x);
    let x = if a_direita >= a_esquerda { dock.x + dock.w + margem } else { dock.x - margem - w };
    dentro(x, dock.y + (dock.h - h) / 2, w, h, area)
}

/// O aviso à vista (ou a caminho) e o tamanho medido pela página do aviso (lógico).
type AvisoMedido = (AvisoDoDock, Option<(f64, f64)>);

static AVISO_ATUAL: Mutex<Option<AvisoMedido>> = Mutex::new(None);

fn aviso_atual_trava() -> std::sync::MutexGuard<'static, Option<AvisoMedido>> {
    AVISO_ATUAL.lock().unwrap_or_else(|p| p.into_inner())
}

/// O aviso que a página do aviso precisa mostrar (ela pergunta ao abrir: o evento pode ter
/// chegado antes dela).
pub fn aviso_atual() -> Option<AvisoDoDock> {
    aviso_atual_trava().as_ref().map(|(a, _)| a.clone())
}

/// Mostra um aviso do dock (um de cada vez: o novo toma o lugar). Com o dock escondido, ele vai
/// para a janela principal. Devolve se ficou no dock.
pub fn mostrar_aviso(app: &AppHandle, aviso: AvisoDoDock) -> bool {
    let no_dock = dock(app).is_some_and(|d| d.is_visible().unwrap_or(false));
    if !no_dock {
        esconder_aviso(app);
        let _ = app.emit_to(PRINCIPAL, EVENTO_AVISO_DO_DOCK, aviso);
        return false;
    }
    *aviso_atual_trava() = Some((aviso.clone(), None));
    // o prazo conta aqui, no núcleo: a janela fecha mesmo se a página do dock recarregar no meio
    // (revisão da rodada 15); um aviso novo, com outro id, não é fechado por este
    let (app2, id, prazo) = (app.clone(), aviso.id, prazo_do_aviso(aviso.ms));
    std::thread::spawn(move || {
        std::thread::sleep(prazo);
        fechar_aviso(&app2, id);
    });
    let _ = app.emit_to(AVISO, EVENTO_AVISO_MOSTRAR, aviso);
    true
}

/// Quanto tempo a janela do aviso fica, no máximo: o prazo do aviso, entre 1 s e 1 min.
pub fn prazo_do_aviso(ms: u64) -> Duration {
    Duration::from_millis(ms.clamp(1_000, 60_000))
}

/// A página do aviso mediu o aviso `id`: a janela fica desse tamanho, no lugar dela, e aparece
/// sem pegar o foco.
pub fn tamanho_do_aviso(app: &AppHandle, id: u64, larg: f64, alt: f64) {
    {
        let mut a = aviso_atual_trava();
        match a.as_mut() {
            Some((v, t)) if v.id == id && larg > 0.0 && alt > 0.0 => *t = Some((larg, alt)),
            _ => return,
        }
    }
    if posicionar_aviso(app) {
        if let Some(j) = app.get_webview_window(AVISO) {
            mostrar_sem_ativar(&j);
        }
    }
}

/// Põe a janela do aviso no lugar (abaixo ou acima do dock) e no tamanho medido. Falso se não há
/// aviso medido ou janela.
fn posicionar_aviso(app: &AppHandle) -> bool {
    let Some((larg, alt)) = aviso_atual_trava().as_ref().and_then(|(_, t)| *t) else { return false };
    let (Some(j), Some(d)) = (app.get_webview_window(AVISO), dock(app)) else { return false };
    let Some(r) = ret_de(&d) else { return false };
    // a área útil do monitor do dock (achado pela tela inteira: o dock pode estar em cima da
    // barra) e a escala dele (spec 005)
    let ms = monitores(app);
    let Some(m) = lugar::monitor_de(r, &ms) else { return false };
    let (w, h) = ((larg * m.escala).ceil() as i32, (alt * m.escala).ceil() as i32);
    let (x, y) = posicao_do_aviso(r, w, h, m.util, (MARGEM_AVISO * m.escala).round() as i32);
    #[cfg(windows)]
    if let Ok(hw) = j.hwnd() {
        let (r, muda) = win::no_destino(hw.0 as isize, Ret { x, y, w, h }, None);
        win::por_no_lugar(hw.0 as isize, r, true, muda);
        return true;
    }
    let _ = j.set_size(PhysicalSize::new(w as u32, h as u32));
    let _ = j.set_position(PhysicalPosition::new(x, y));
    true
}

/// O dock se moveu ou mudou de tamanho: o aviso à vista vai junto.
pub fn reposicionar_aviso(app: &AppHandle) {
    if aviso_visivel(app) {
        posicionar_aviso(app);
    }
}

/// Fecha o aviso `id` (venceu, ou o "Desfazer" deu certo); outro aviso que tomou o lugar fica.
pub fn fechar_aviso(app: &AppHandle, id: u64) {
    if aviso_atual_trava().as_ref().is_some_and(|(a, _)| a.id == id) {
        esconder_aviso(app);
    }
}

fn esconder_aviso(app: &AppHandle) {
    *aviso_atual_trava() = None;
    if let Some(j) = app.get_webview_window(AVISO) {
        let _ = j.hide();
    }
}

/// O aviso está à vista (o "clique fora" do "Mostrar o dock agora" não vale enquanto isso: o
/// clique no "Desfazer" tira o foco do dock).
pub fn aviso_visivel(app: &AppHandle) -> bool {
    app.get_webview_window(AVISO).is_some_and(|j| j.is_visible().unwrap_or(false))
}

/// Um botão do aviso foi clicado: o dock faz o que ele faz (o "Desfazer" é dele). `qual`: 1 ou 2.
pub fn aviso_clicado(app: &AppHandle, id: u64, qual: u8) {
    let _ = app.emit_to(DOCK, EVENTO_AVISO_CLICADO, serde_json::json!({ "id": id, "qual": qual }));
}

// ---- a janela principal no lugar dela (spec 005, US 005.4) ----

/// A janela principal se moveu ou mudou de tamanho: o laço grava o lugar no próximo ciclo.
static PRINCIPAL_MEXEU: AtomicBool = AtomicBool::new(false);

/// `main.rs`: a janela principal se moveu ou mudou de tamanho.
pub fn principal_mexeu() {
    PRINCIPAL_MEXEU.store(true, Ordering::SeqCst);
}

/// O laço, a cada ciclo: grava o lugar da janela principal se ela mexeu (fora da thread dela).
pub fn guardar_principal_se_mexeu(app: &AppHandle) {
    if PRINCIPAL_MEXEU.swap(false, Ordering::SeqCst) {
        guardar_lugar_principal(app);
    }
}

/// A janela principal está emprestada a um diálogo do dock (estava fechada ou escondida, e o dock
/// a trouxe): o lugar de agora não é o da pessoa (revisão, I3).
fn emprestada() -> bool {
    let s = serie_do_dock();
    s.abertos > 0 && matches!(s.antes, Some(AntesDoDialogo::Fechada | AntesDoDialogo::Escondida))
}

/// Grava o lugar da janela principal: o retângulo fora de maximizada e se ela está maximizada.
/// Minimizada, escondida ou emprestada a um diálogo do dock, fica o último. Chamar sem a trava do
/// núcleo.
pub fn guardar_lugar_principal(app: &AppHandle) {
    if emprestada() {
        return;
    }
    let Some(j) = app.get_webview_window(PRINCIPAL) else { return };
    if j.is_minimized().unwrap_or(false) || !j.is_visible().unwrap_or(false) {
        return;
    }
    let max = j.is_maximized().unwrap_or(false);
    let ms = monitores(app);
    let r = if max { normal_da_maximizada(&j, &ms) } else { ret_de(&j) };
    let Some(r) = r.filter(|r| lugar::pode_gravar(*r, &ms)) else { return };
    if let Some(e) = app.try_state::<Estado>() {
        if let Err(e) = trava(&e).guardar_lugar_principal(r.x, r.y, r.w, r.h, max) {
            log_debug!("worktimer: o lugar da janela principal: {e}");
        }
    }
}

/// O retângulo de antes de maximizar (o `rcNormalPosition`, que vem em coordenadas da área de
/// trabalho do monitor da janela).
fn normal_da_maximizada(j: &WebviewWindow, ms: &[Monitor]) -> Option<Ret> {
    #[cfg(windows)]
    {
        let h = j.hwnd().ok()?;
        let r = win::normal(h.0 as isize)?;
        let m = lugar::monitor_de(ret_de(j)?, ms)?;
        Some(lugar::da_area_de_trabalho(r, m))
    }
    #[cfg(not(windows))]
    {
        let _ = (j, ms);
        None
    }
}

/// A janela principal (fora de maximizada) noutro monitor que o do dock: o centro da área útil do
/// monitor do dock.
fn perto_do_dock(app: &AppHandle, principal: Ret) -> Option<Ret> {
    let d = dock(app)?;
    if !d.is_visible().unwrap_or(false) {
        return None;
    }
    lugar::principal_perto_do_dock(principal, ret_de(&d)?, &monitores(app))
}

/// Põe a janela principal no retângulo (síncrono: ela aparece logo depois, já no lugar).
/// `escala`: a escala em que o tamanho foi medido (vai para a do destino).
fn por_principal(j: &WebviewWindow, r: Ret, escala: Option<f64>) {
    #[cfg(windows)]
    if let Ok(h) = j.hwnd() {
        let (r, muda) = win::no_destino(h.0 as isize, r, escala);
        win::por_no_lugar(h.0 as isize, r, false, muda);
        return;
    }
    let _ = escala;
    let _ = j.set_position(PhysicalPosition::new(r.x, r.y));
    let _ = j.set_size(PhysicalSize::new(r.w as u32, r.h as u32));
}

/// A janela principal recém-criada (ainda escondida) vai para o lugar salvo, se ele ainda aparece
/// num monitor; pedida pelo dock, para o monitor do dock. Devolve o lugar, a escala em que o
/// tamanho dele foi medido e se ela estava maximizada.
fn lugar_ao_nascer(app: &AppHandle, j: &WebviewWindow, pelo_dock: bool) -> (Option<Ret>, Option<f64>, bool) {
    let cfg = app.try_state::<Estado>().and_then(|e| trava(&e).banco.config_app().ok()).unwrap_or_default();
    let ms = monitores(app);
    let salvo = match (cfg.principal_x, cfg.principal_y, cfg.principal_w, cfg.principal_h) {
        (Some(x), Some(y), Some(w), Some(h)) if w > 0 && h > 0 => Some(Ret { x, y, w, h }),
        _ => None,
    };
    let mut alvo = lugar::lugar_da_principal(salvo, &ms);
    // o tamanho salvo foi medido no monitor dele; o de agora, no da janela
    let mut escala = alvo.and_then(|a| lugar::monitor_visivel(a, &ms)).map(|m| m.escala);
    if pelo_dock {
        let base = alvo.or_else(|| ret_de(j));
        if alvo.is_none() {
            escala = j.scale_factor().ok();
        }
        if let Some(p) = base.and_then(|r| perto_do_dock(app, r)) {
            alvo = Some(p);
        }
    }
    if let Some(r) = alvo {
        por_principal(j, r, escala);
    }
    (alvo, escala, cfg.principal_max && alvo.is_some())
}

// ---- a saúde do dock (spec 006) ----

/// O que o laço lembra entre um ciclo e outro: os monitores, a hora do ciclo e a última foto do
/// log.
struct Vigia {
    monitores: Option<Vec<Monitor>>,
    ultimo_ciclo: Option<u64>,
    foto: Option<lugar::FotoDoDock>,
}

static VIGIA: Mutex<Vigia> = Mutex::new(Vigia { monitores: None, ultimo_ciclo: None, foto: None });
/// O sinal de vida da página do dock (um `Mutex` só dele: o `dock_vivo` chega pela thread
/// principal, e nenhuma trava fica presa enquanto o laço fala com o Tauri).
static VIDA: Mutex<Option<lugar::Vida>> = Mutex::new(None);
/// Evento para o dock: o sinal de vida (a página responde com `dock_vivo`).
pub const EVENTO_DOCK_PING: &str = "dock-ping";

/// A página do dock respondeu ao ping.
pub fn dock_respondeu() {
    if let Some(v) = travar(&VIDA).as_mut() {
        v.respondeu(monotonico());
    }
}

/// O dock está minimizado? E oculto (outra área de trabalho, `DWMWA_CLOAKED`)?
fn minimizado_e_oculto(d: &WebviewWindow) -> (bool, bool) {
    #[cfg(windows)]
    if let Ok(h) = d.hwnd() {
        let h = h.0 as isize;
        return (win::minimizada(h), win::oculta(h));
    }
    let _ = d;
    (false, false)
}

/// Põe o dock (e o aviso à vista) por cima de tudo de novo, sem ativar (US 006.1).
fn reafirmar_por_cima(app: &AppHandle) {
    #[cfg(windows)]
    for label in [DOCK, AVISO] {
        if let Some(j) = app.get_webview_window(label) {
            if !j.is_visible().unwrap_or(false) {
                continue;
            }
            if let Ok(h) = j.hwnd() {
                win::por_cima(h.0 as isize);
            }
        }
    }
    #[cfg(not(windows))]
    let _ = app;
}

/// A volta para a escolha (US 006.4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Volta {
    /// Nunca houve um arrastar: não há para onde voltar.
    SemEscolha,
    /// O monitor da escolha não está (ainda): tentar de novo.
    SemMonitor,
    JaLa,
    Movido(Ret),
}

/// A escolha da config, se houver (o tamanho de antes vem de `inicial`).
fn escolha_agora(app: &AppHandle, inicial: (i32, i32)) -> Option<Ret> {
    let cfg = app.try_state::<Estado>().and_then(|e| trava(&e).banco.config_app().ok())?;
    escolha_da_config(&cfg, inicial)
}

/// O dock volta para a escolha, se o monitor dela está aqui, no mesmo tamanho lógico.
fn recolocar_na_escolha(app: &AppHandle, d: &WebviewWindow, ms: &[Monitor]) -> Volta {
    let Some(r) = ret_de(d) else { return Volta::SemMonitor };
    let Some(escolha) = escolha_agora(app, (r.w, r.h)) else { return Volta::SemEscolha };
    let Some((x, y)) = lugar::lugar_da_escolha(escolha, r.w, r.h, ms) else { return Volta::SemMonitor };
    if (x, y) == (r.x, r.y) {
        return Volta::JaLa;
    }
    let novo = Ret { x, y, w: r.w, h: r.h };
    if por_dock(d, novo, false, d.scale_factor().ok()) {
        pedir_medida(app);
    }
    Volta::Movido(novo)
}

/// Um relógio que só anda para a frente (ms desde a 1ª chamada), para o sinal de vida.
fn monotonico() -> u64 {
    static INICIO: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    INICIO.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64
}

/// Esconde e mostra o dock sem ativar: a janela que reaparece vem para a área de trabalho de agora
/// (o plano B quando o `IVirtualDesktopManager` não diz qual é; revisão, M2).
fn reaparecer(d: &WebviewWindow) {
    APP_MEXEU.store(monotonico(), Ordering::SeqCst);
    let _ = d.hide();
    mostrar_sem_ativar(d);
}

/// O último `reaparecer` do laço (monotônico): no máximo um por minuto (re-revisão, R3).
static REAPARECEU: AtomicU64 = AtomicU64::new(0);

/// A cada ciclo do laço (5 s), sem a trava do núcleo: com o dock que deve estar à vista,
/// - restaura o minimizado e traz o oculto para a área de trabalho de agora (US 006.2);
/// - com a troca dos monitores ou o PC acordando, volta para a escolha (US 006.4);
/// - reafirma o "sempre por cima" (US 006.1);
/// - manda o sinal de vida e recarrega a página calada (US 006.2);
/// - grava no log o que mudou (US 006.5).
pub fn cuidar_do_dock(app: &AppHandle) {
    let agora = agora_ms();
    let Some(d) = dock(app) else { return };
    let ms = monitores(app);
    let visivel = d.is_visible().unwrap_or(false);
    let (minimizado, oculto) = if visivel { minimizado_e_oculto(&d) } else { (false, false) };
    let (mudaram, acordou) = {
        let mut v = travar(&VIGIA);
        let mudaram = v.monitores.as_ref().is_some_and(|a| *a != ms);
        let acordou = lugar::acordou(v.ultimo_ciclo, agora);
        v.monitores = Some(ms.clone());
        v.ultimo_ciclo = Some(agora);
        (mudaram, acordou)
    };
    if acordou {
        anotar("acordou", None);
    }
    if mudaram {
        anotar("monitores mudaram", None);
    }
    // a troca vale até a escolha ser aplicada, também se o dock estava escondido (re-revisão, R6)
    if mudaram || acordou {
        FORA_DA_ESCOLHA.store(true, Ordering::SeqCst);
    }
    // o dock voltou do minimizado no ciclo anterior: a página mede de novo (R4)
    if visivel && !minimizado && MEDIR_DEPOIS.swap(false, Ordering::SeqCst) {
        pedir_medida(app);
    }
    if visivel {
        #[cfg(windows)]
        if let Ok(h) = d.hwnd() {
            let h = h.0 as isize;
            if minimizado {
                APP_MEXEU.store(monotonico(), Ordering::SeqCst);
                win::restaurar_sem_ativar(h);
                MEDIR_DEPOIS.store(true, Ordering::SeqCst);
                anotar("restaurado (estava minimizado)", None);
            }
            if oculto {
                match win::trazer_para_a_area_de_agora(h) {
                    win::Area::Movido => anotar("trazido para a área de trabalho de agora", None),
                    // oculto na área de agora (a nuvem do próprio app, uma animação): nada
                    win::Area::JaAqui => {}
                    win::Area::NaoSei => {
                        let mono = monotonico();
                        if mono.saturating_sub(REAPARECEU.load(Ordering::SeqCst)) >= 60_000 {
                            REAPARECEU.store(mono, Ordering::SeqCst);
                            reaparecer(&d);
                            anotar("reaparecido (noutra área de trabalho, sem saber qual)", None);
                        }
                    }
                }
            }
        }
        if !minimizado && lugar::tentar_a_escolha(FORA_DA_ESCOLHA.load(Ordering::SeqCst), mudaram, acordou) {
            match recolocar_na_escolha(app, &d, &ms) {
                // sem o monitor dela: tenta de novo no próximo ciclo
                Volta::SemMonitor => {}
                Volta::SemEscolha | Volta::JaLa => FORA_DA_ESCOLHA.store(false, Ordering::SeqCst),
                Volta::Movido(r) => {
                    FORA_DA_ESCOLHA.store(false, Ordering::SeqCst);
                    anotar("recolocado na escolha", Some(r));
                }
            }
        }
        // quase fora da tela: para dentro do monitor mais perto. Uma lasca à vista foi a pessoa (o
        // lugar trazido para dentro vira a escolha); sem encostar em monitor nenhum, o monitor dele
        // sumiu (a escolha fica, e ele volta quando o monitor voltar; re-revisão, R2)
        if !minimizado {
            if let Some(r) = ret_de(&d) {
                if let Some((x, y)) = lugar::de_volta_a_tela(r, &ms) {
                    let lasca = lugar::so_uma_lasca(r, &ms);
                    let novo = Ret { x, y, ..r };
                    if por_dock(&d, novo, false, d.scale_factor().ok()) {
                        pedir_medida(app);
                    }
                    if lasca {
                        FORA_DA_ESCOLHA.store(false, Ordering::SeqCst);
                        if let Some(e) = app.try_state::<Estado>() {
                            let _ = trava(&e).guardar_escolha_dock(x, y, r.w, r.h, None, false);
                        }
                        anotar("só uma lasca à vista: para dentro da tela (vira a escolha)", Some(novo));
                    } else {
                        FORA_DA_ESCOLHA.store(escolha_agora(app, (r.w, r.h)).is_some(), Ordering::SeqCst);
                        anotar("fora dos monitores: de volta à tela", Some(novo));
                    }
                }
            }
        }
        // só com o dock na faixa da barra: o resto das janelas "sempre por cima" (um vídeo em PiP,
        // uma apresentação) continua por cima dele (revisão, M7)
        if ret_de(&d).is_some_and(|r| lugar::na_faixa_da_barra(r, &ms)) {
            reafirmar_por_cima(app);
        }
    }
    // o sinal de vida: só com o dock à vista de verdade (escondido, minimizado ou noutra área, a
    // página pode ficar parada sem estar quebrada)
    let a_vista = visivel && !minimizado && !oculto;
    let (recarregar, de_novo) = {
        let mono = monotonico();
        let mut v = travar(&VIDA);
        let v = v.get_or_insert_with(|| lugar::Vida::nova(mono));
        // os pings de logo antes de o PC dormir não contam
        if acordou {
            v.em_pausa(mono);
        }
        let de_novo = v.sem_resposta_desde_recarregar;
        if a_vista {
            (v.ping(mono), de_novo)
        } else {
            v.em_pausa(mono);
            (false, de_novo)
        }
    };
    if a_vista {
        if recarregar {
            anotar(
                if de_novo {
                    "recarregado de novo (o último recarregar não resolveu: o navegador do WebView2 pode ter caído)"
                } else {
                    "recarregado (a página não respondia)"
                },
                None,
            );
            let _ = d.reload();
        } else {
            let _ = app.emit_to(DOCK, EVENTO_DOCK_PING, ());
        }
    }
    let foto = lugar::FotoDoDock { visivel, ret: ret_de(&d), minimizado, oculto, monitores: ms };
    let nova = {
        let mut v = travar(&VIGIA);
        let nova = v.foto.as_ref() != Some(&foto);
        if nova {
            v.foto = Some(foto.clone());
        }
        nova
    };
    if nova {
        escrever_no_log(&lugar::linha_da_foto(&hora_do_log(), &foto));
    }
}

/// A janela em primeiro plano trocou (o gancho do Windows, na thread principal): com o dock à
/// vista e encostado na faixa da barra, ele volta por cima dela (US 006.1). De novo um pouco
/// depois: a barra às vezes sobe depois do evento.
pub fn trocou_o_primeiro_plano(app: &AppHandle) {
    let Some(d) = dock(app) else { return };
    if !d.is_visible().unwrap_or(false) {
        return;
    }
    let Some(r) = ret_de(&d) else { return };
    if !lugar::na_faixa_da_barra(r, &monitores(app)) {
        return;
    }
    reafirmar_por_cima(app);
    let app = app.clone();
    let _ = std::thread::Builder::new().name("worktimer-por-cima".into()).spawn(move || {
        std::thread::sleep(Duration::from_millis(400));
        reafirmar_por_cima(&app);
    });
}

/// Liga o gancho da troca de primeiro plano (no setup, na thread principal).
pub fn vigiar_primeiro_plano(app: &AppHandle) {
    #[cfg(windows)]
    win::vigiar_primeiro_plano(app);
    #[cfg(not(windows))]
    let _ = app;
}

/// O "Trazer o dock para cá" da bandeja (US 006.3): o dock aparece no monitor do mouse (ou no
/// principal), na posição padrão, restaurado, na área de trabalho de agora, por cima de tudo e com
/// a página recarregada (cobre a pintura em branco). Esse lugar vira a escolha.
pub fn trazer_dock(app: &AppHandle) {
    let (modo, _) = modo_e_perguntas(app);
    *pedido_dock() = match modo {
        ModoDock::Sempre => PedidoDock::Nenhum,
        ModoDock::SoPerguntas => PedidoDock::MostrarAgora { desde: agora_ms(), mouse_em_cima: false, clicou_fora: false },
    };
    let Some(d) = dock(app) else { return };
    // o resgate é a escolha agora: um arrastar velho não a desfaz (revisão, M6)
    *travar(&ESCOLHA_PENDENTE) = None;
    FORA_DA_ESCOLHA.store(false, Ordering::SeqCst);
    let ms = monitores(app);
    let mouse = cursor();
    let escala = mouse.and_then(|p| lugar::monitor_do_ponto(p, &ms)).or_else(|| lugar::principal(&ms)).map_or(1.0, |m| m.escala);
    let (minimizado, _) = minimizado_e_oculto(&d);
    #[cfg(windows)]
    if let Ok(h) = d.hwnd() {
        let h = h.0 as isize;
        if minimizado {
            APP_MEXEU.store(monotonico(), Ordering::SeqCst);
            win::restaurar_sem_ativar(h);
        }
        // o primeiro plano agora é a janela da bandeja, do próprio app: sem saber a área de
        // trabalho, o plano B (revisão, M2; re-revisão, R3)
        if win::trazer_para_a_area_de_agora(h) == win::Area::NaoSei && win::oculta(h) {
            reaparecer(&d);
        }
    }
    // o tamanho de agora vai na escala do monitor do mouse (R1); minimizado, o inicial de lá
    let (w, h, de) = match ret_de(&d) {
        Some(r) if !minimizado && r.w > 0 && r.h > 0 => (r.w, r.h, d.scale_factor().ok()),
        _ => {
            let (w, h) = fisico(RECOLHIDO_INICIAL, escala);
            (w, h, None)
        }
    };
    let (w, h) = match de {
        Some(de) => {
            let r = lugar::na_escala(Ret { x: 0, y: 0, w, h }, de, escala);
            (r.w, r.h)
        }
        None => (w, h),
    };
    let Some((x, y)) = lugar::posicao_do_resgate(mouse, w, h, &ms, (MARGEM * escala).round() as i32) else { return };
    let novo = Ret { x, y, w, h };
    por_dock(&d, novo, true, None);
    aplicar_dock(app);
    reafirmar_por_cima(app);
    let _ = d.reload();
    if let Some(e) = app.try_state::<Estado>() {
        if let Err(e) = trava(&e).guardar_escolha_dock(x, y, w, h, None, false) {
            log_debug!("worktimer: o resgate do dock: {e}");
        }
    }
    anotar("resgate (bandeja)", Some(novo));
    if modo == ModoDock::SoPerguntas {
        let _ = d.set_focus();
    }
}

fn cursor() -> Option<(i32, i32)> {
    #[cfg(windows)]
    {
        win::cursor()
    }
    #[cfg(not(windows))]
    {
        None
    }
}

// ---- o log do dock (US 006.5) ----

static LOG: Mutex<()> = Mutex::new(());

fn hora_do_log() -> String {
    chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Uma ação no log do dock (arrastado, recolocado, resgate…): a hora e números, nunca títulos.
fn anotar(acao: &str, ret: Option<Ret>) {
    escrever_no_log(&lugar::linha_da_acao(&hora_do_log(), acao, ret));
}

/// `~\.worktimer\dock.log`; passou de 256 KB, vira o `dock.log.1` (um só).
fn escrever_no_log(linha: &str) {
    use std::io::Write;
    let _g = travar(&LOG);
    let base = fila::caminhos::base();
    let arquivo = base.join("dock.log");
    if std::fs::metadata(&arquivo).is_ok_and(|m| m.len() > lugar::LOG_MAX_BYTES) {
        let _ = std::fs::rename(&arquivo, base.join("dock.log.1"));
    }
    if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(&arquivo) {
        let _ = writeln!(f, "{linha}");
    }
}

// ---- Win32 ----

/// As chamadas do Windows do dock e da janela principal. Os HWND andam como `isize` (o tipo do
/// Tauri é de outra versão do crate `windows`).
#[cfg(windows)]
mod win {
    use super::{Monitor, Ret};
    use std::sync::OnceLock;
    use tauri::AppHandle;
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, MonitorFromRect, HDC, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
    use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
    use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForWindow, MDT_EFFECTIVE_DPI};
    use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};
    use windows::Win32::UI::Shell::{IVirtualDesktopManager, VirtualDesktopManager};
    use windows::Win32::UI::WindowsAndMessaging::{
        GetCursorPos, GetForegroundWindow, GetGUIThreadInfo, GetSystemMetrics, GetWindowPlacement, GetWindowThreadProcessId,
        IsIconic, SetWindowPos, ShowWindowAsync, GUITHREADINFO, GUI_INMOVESIZE,
        EVENT_SYSTEM_FOREGROUND, HWND_TOPMOST, MONITORINFOF_PRIMARY, SM_SWAPBUTTON, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE,
        SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE, SWP_NOZORDER, SW_SHOWNOACTIVATE, WINDOWPLACEMENT, WINEVENT_OUTOFCONTEXT,
        WINEVENT_SKIPOWNPROCESS,
    };

    fn hwnd(h: isize) -> HWND {
        HWND(h as *mut core::ffi::c_void)
    }

    fn ret(r: RECT) -> Ret {
        Ret { x: r.left, y: r.top, w: r.right - r.left, h: r.bottom - r.top }
    }

    unsafe extern "system" fn um_monitor(m: HMONITOR, _: HDC, _: *mut RECT, dados: LPARAM) -> BOOL {
        // SAFETY: `dados` é o `&mut Vec` de `monitores`, vivo durante o EnumDisplayMonitors
        let v = unsafe { &mut *(dados.0 as *mut Vec<Monitor>) };
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        // SAFETY: `m` veio do Windows agora; `mi` é nosso, do tamanho certo
        if unsafe { GetMonitorInfoW(m, &mut mi) }.as_bool() {
            let (mut dx, mut dy) = (96u32, 96u32);
            // SAFETY: idem
            let escala = if unsafe { GetDpiForMonitor(m, MDT_EFFECTIVE_DPI, &mut dx, &mut dy) }.is_ok() && dx > 0 {
                dx as f64 / 96.0
            } else {
                1.0
            };
            v.push(Monitor { tela: ret(mi.rcMonitor), util: ret(mi.rcWork), escala, principal: mi.dwFlags & MONITORINFOF_PRIMARY != 0 });
        }
        BOOL(1)
    }

    /// Os monitores agora (o app é "per monitor": os retângulos vêm em pixels físicos).
    pub fn monitores() -> Vec<Monitor> {
        let mut v: Vec<Monitor> = Vec::new();
        // SAFETY: o callback só usa o `v` desta pilha, durante a chamada
        unsafe {
            let _ = EnumDisplayMonitors(None, None, Some(um_monitor), LPARAM(&mut v as *mut Vec<Monitor> as isize));
        }
        v
    }

    /// O DPI do monitor em que o retângulo cai e o da janela agora (0: desconhecido).
    fn dpis(h: isize, r: Ret) -> (u32, u32) {
        let rc = RECT { left: r.x, top: r.y, right: r.x + r.w, bottom: r.y + r.h };
        // SAFETY: só leituras
        unsafe {
            let m = MonitorFromRect(&rc, MONITOR_DEFAULTTONEAREST);
            let (mut dx, mut dy) = (0u32, 0u32);
            let destino = if GetDpiForMonitor(m, MDT_EFFECTIVE_DPI, &mut dx, &mut dy).is_ok() { dx } else { 0 };
            (destino, GetDpiForWindow(hwnd(h)))
        }
    }

    /// O retângulo no tamanho do destino: com `escala` (a escala em que o tamanho foi medido), o
    /// mesmo tamanho lógico na escala do monitor de destino (re-revisão, R1). E se a janela muda
    /// de escala indo para lá.
    pub fn no_destino(h: isize, r: Ret, escala: Option<f64>) -> (Ret, bool) {
        let (destino, agora) = dpis(h, r);
        let r = match escala {
            Some(e) if destino != 0 => super::lugar::na_escala(r, e, destino as f64 / 96.0),
            _ => r,
        };
        (r, destino != 0 && agora != 0 && destino != agora)
    }

    /// Posição e tamanho numa chamada só, sem mexer na ordem nem ativar. `assincrono`: o pedido
    /// vai para a fila da thread da janela (os comandos não esperam por ela).
    ///
    /// Indo para um monitor de outra escala (`muda`, do `no_destino`), duas chamadas: a 1ª só move
    /// (é nela que o Windows manda o `WM_DPICHANGED`, e o tao refaz o tamanho), e a 2ª põe o
    /// tamanho final, já em pixels do destino. Numa chamada só, ele seria escalado de novo
    /// (revisão, I2).
    pub fn por_no_lugar(h: isize, r: Ret, assincrono: bool, muda: bool) {
        let mut f = SWP_NOZORDER | SWP_NOACTIVATE;
        if assincrono {
            f |= SWP_ASYNCWINDOWPOS;
        }
        // SAFETY: a janela é do próprio app
        unsafe {
            if muda {
                let _ = SetWindowPos(hwnd(h), None, r.x, r.y, 0, 0, f | SWP_NOSIZE);
            }
            let _ = SetWindowPos(hwnd(h), None, r.x, r.y, r.w, r.h, f);
        }
    }

    /// A thread da janela está no laço de mover ou redimensionar do Windows (a pessoa arrastando)?
    pub fn em_mover(h: isize) -> bool {
        let mut gi = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
        // SAFETY: só leituras; `gi` é nosso, com o tamanho certo
        unsafe {
            let t = GetWindowThreadProcessId(hwnd(h), None);
            t != 0 && GetGUIThreadInfo(t, &mut gi).is_ok() && (gi.flags & GUI_INMOVESIZE).0 != 0
        }
    }

    /// De novo por cima de tudo ("sempre por cima"), sem ativar, sem mover e sem esperar.
    pub fn por_cima(h: isize) {
        // SAFETY: a janela é do próprio app
        unsafe {
            let _ = SetWindowPos(
                hwnd(h),
                Some(HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_NOOWNERZORDER | SWP_ASYNCWINDOWPOS,
            );
        }
    }

    /// O botão principal do mouse (o esquerdo, ou o direito com os botões trocados) está apertado.
    pub fn botao_apertado() -> bool {
        // SAFETY: só leituras do estado do mouse
        unsafe {
            let vk = if GetSystemMetrics(SM_SWAPBUTTON) != 0 { VK_RBUTTON } else { VK_LBUTTON };
            (GetAsyncKeyState(vk.0 as i32) as u16 & 0x8000) != 0
        }
    }

    pub fn minimizada(h: isize) -> bool {
        // SAFETY: só leitura
        unsafe { IsIconic(hwnd(h)) }.as_bool()
    }

    pub fn restaurar_sem_ativar(h: isize) {
        // SAFETY: a janela é do próprio app; assíncrono, não espera a thread dela
        unsafe {
            let _ = ShowWindowAsync(hwnd(h), SW_SHOWNOACTIVATE);
        }
    }

    /// Oculta pelo Windows (outra área de trabalho virtual, `DWMWA_CLOAKED`).
    pub fn oculta(h: isize) -> bool {
        let mut c: u32 = 0;
        // SAFETY: `c` é um u32, do tamanho que o atributo pede
        let ok = unsafe { DwmGetWindowAttribute(hwnd(h), DWMWA_CLOAKED, &mut c as *mut u32 as *mut core::ffi::c_void, 4) }.is_ok();
        ok && c != 0
    }

    /// A janela e a área de trabalho de agora (re-revisão, R3).
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Area {
        JaAqui,
        Movido,
        /// Não deu para saber (o primeiro plano sem área, um erro) ou o mover não pegou.
        NaoSei,
    }

    /// Fora da área de trabalho de agora: vai para a da janela em primeiro plano, e confere depois.
    pub fn trazer_para_a_area_de_agora(h: isize) -> Area {
        thread_local! {
            static COM: () = {
                // SAFETY: uma vez por thread; já iniciado noutro modo (a thread principal), segue
                // valendo o de antes
                let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
            };
        }
        COM.with(|_| {});
        // SAFETY: uma janela do próprio app, com o COM desta thread iniciado
        unsafe {
            let Ok(m) = CoCreateInstance::<_, IVirtualDesktopManager>(&VirtualDesktopManager, None, CLSCTX_INPROC_SERVER) else {
                return Area::NaoSei;
            };
            let aqui = |m: &IVirtualDesktopManager| m.IsWindowOnCurrentVirtualDesktop(hwnd(h)).map(|b| b.as_bool());
            match aqui(&m) {
                Ok(true) => return Area::JaAqui,
                Ok(false) => {}
                Err(_) => return Area::NaoSei,
            }
            let fg = GetForegroundWindow();
            if fg.is_invalid() {
                return Area::NaoSei;
            }
            let Ok(id) = m.GetWindowDesktopId(fg) else { return Area::NaoSei };
            if id == windows::core::GUID::zeroed() || m.MoveWindowToDesktop(hwnd(h), &id).is_err() {
                return Area::NaoSei;
            }
            // o "deu certo" sem mover nada (a área do primeiro plano não é a de agora) não conta
            if aqui(&m).is_ok_and(|b| b) {
                Area::Movido
            } else {
                Area::NaoSei
            }
        }
    }

    pub fn cursor() -> Option<(i32, i32)> {
        let mut p = POINT::default();
        // SAFETY: `p` é nosso
        unsafe { GetCursorPos(&mut p) }.ok()?;
        Some((p.x, p.y))
    }

    /// O retângulo de antes de maximizar (coordenadas da área de trabalho).
    pub fn normal(h: isize) -> Option<Ret> {
        let mut wp = WINDOWPLACEMENT { length: std::mem::size_of::<WINDOWPLACEMENT>() as u32, ..Default::default() };
        // SAFETY: `wp` é nosso, com o tamanho certo
        unsafe { GetWindowPlacement(hwnd(h), &mut wp) }.ok()?;
        Some(ret(wp.rcNormalPosition))
    }

    static APP: OnceLock<AppHandle> = OnceLock::new();

    unsafe extern "system" fn primeiro_plano(_: HWINEVENTHOOK, _: u32, _: HWND, _: i32, _: i32, _: u32, _: u32) {
        if let Some(app) = APP.get() {
            // nada de pânico atravessando o Windows
            let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| super::trocou_o_primeiro_plano(app)));
        }
    }

    /// O gancho fica até o app sair (o Windows o tira junto).
    pub fn vigiar_primeiro_plano(app: &AppHandle) {
        if APP.set(app.clone()).is_err() {
            return;
        }
        // SAFETY: fora do processo (`WINEVENT_OUTOFCONTEXT`): o callback roda nesta thread, que tem
        // o laço de mensagens do app
        unsafe {
            let _ = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_FOREGROUND,
                None,
                Some(primeiro_plano),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::APP_NOME;

    #[test]
    fn serie_de_dialogos_do_dock_conta_e_zera() {
        use AntesDoDialogo::*;
        let mut s = SerieDoDock::default();
        // escondida, dois pedidos (o "Ligar a um timer" e, de dentro dele, o "+ Novo timer…")
        s.abriu(Escondida);
        s.abriu(AVista);
        assert_eq!(s.fechou(), DepoisDoDialogo::Nada, "o primeiro acabou, o segundo está aberto");
        assert_eq!(s.fechou(), DepoisDoDialogo::Esconder, "com o último, volta como estava antes do primeiro");
        assert_eq!(s, SerieDoDock::default());
        // um aviso a mais (uma página recarregada) não esconde nada
        assert_eq!(s.fechou(), DepoisDoDialogo::Nada);
        // fechada para a bandeja: volta a fechar
        s.abriu(Fechada);
        assert_eq!(s.fechou(), DepoisDoDialogo::Fechar);
        // a pessoa mostrou a janela no meio (ou ela foi fechada): fica como está
        s.abriu(Escondida);
        s.zerar();
        assert_eq!(s.fechou(), DepoisDoDialogo::Nada);
        // à vista: fica
        s.abriu(AVista);
        assert_eq!(s.fechou(), DepoisDoDialogo::Nada);
    }

    #[test]
    fn dialogo_do_dock_deixa_a_janela_como_estava() {
        use AntesDoDialogo::*;
        assert_eq!(antes_do_dialogo(true, true), AVista);
        assert_eq!(antes_do_dialogo(true, false), Escondida);
        assert_eq!(antes_do_dialogo(false, false), Fechada);
        assert_eq!(antes_do_dialogo(false, true), Fechada);
        // escondida volta a se esconder; fechada para a bandeja volta a fechar; à vista fica
        assert_eq!(depois_do_dialogo(Some(Escondida)), DepoisDoDialogo::Esconder);
        assert_eq!(depois_do_dialogo(Some(Fechada)), DepoisDoDialogo::Fechar);
        assert_eq!(depois_do_dialogo(Some(AVista)), DepoisDoDialogo::Nada);
        assert_eq!(depois_do_dialogo(None), DepoisDoDialogo::Nada, "um diálogo que não veio do dock");
    }

    #[test]
    fn o_aviso_fecha_sozinho_no_prazo_dele() {
        assert_eq!(prazo_do_aviso(10_000), Duration::from_secs(10));
        assert_eq!(prazo_do_aviso(4_000), Duration::from_secs(4));
        assert_eq!(prazo_do_aviso(0), Duration::from_secs(1), "nunca some antes de dar para ler");
        assert_eq!(prazo_do_aviso(u64::MAX), Duration::from_secs(60), "nunca fica para sempre");
    }

    #[test]
    fn aviso_abaixo_do_dock_centralizado_ou_acima_se_nao_couber() {
        let dock = |x, y| Ret { x, y, w: 300, h: 46 };
        // embaixo, centralizado no dock, com a margem
        assert_eq!(posicao_do_aviso(dock(800, 20), 220, 34, TELA, 6), (840, 72));
        // o aviso mais largo que o dock: centralizado também (passa dos dois lados)
        assert_eq!(posicao_do_aviso(dock(800, 20), 420, 34, TELA, 6), (740, 72));
        // o dock embaixo da tela: o aviso vai para cima
        assert_eq!(posicao_do_aviso(dock(800, 990), 220, 34, TELA, 6), (840, 950));
        // cabe embaixo bem no limite: fica embaixo
        assert_eq!(posicao_do_aviso(dock(800, 954), 220, 34, TELA, 6), (840, 1006));
        // nas bordas laterais, não sai da tela
        assert_eq!(posicao_do_aviso(dock(0, 20), 420, 34, TELA, 6), (0, 72));
        assert_eq!(posicao_do_aviso(dock(1620, 20), 420, 34, TELA, 6), (1500, 72));
        // o dock expandido embaixo à direita: acima e encostado na borda
        assert_eq!(posicao_do_aviso(Ret { x: 1600, y: 620, w: 320, h: 420 }, 420, 34, TELA, 6), (1500, 580));
        // o expandido quase da altura da área: nem embaixo nem em cima sem tapar o dock, então ao
        // lado, do lado com mais espaço, centralizado na altura (revisão)
        let alto = |x| Ret { x, y: 20, w: 320, h: 1000 };
        assert_eq!(posicao_do_aviso(alto(1500), 420, 34, TELA, 6), (1074, 503), "à esquerda");
        assert_eq!(posicao_do_aviso(alto(100), 420, 34, TELA, 6), (426, 503), "à direita");
        // no segundo monitor, a área é a dele
        assert_eq!(posicao_do_aviso(dock(2900, 20), 420, 34, SEGUNDA, 6), (2780, 72));
        assert_eq!(posicao_do_aviso(dock(1930, 940), 220, 34, SEGUNDA, 6), (1970, 900));
    }

    #[test]
    fn dock_sempre_a_vista_ou_so_com_pergunta() {
        use ModoDock::*;
        let t0 = 1_000_000;
        let agora_mostra = |desde, mouse, fora| PedidoDock::MostrarAgora { desde, mouse_em_cima: mouse, clicou_fora: fora };
        // sempre à vista: aparece, a não ser escondido pela bandeja
        assert!(dock_visivel(Sempre, 0, PedidoDock::Nenhum, t0));
        assert!(!dock_visivel(Sempre, 0, PedidoDock::Escondido, t0));
        assert!(dock_visivel(Sempre, 2, PedidoDock::Escondido, t0), "escondido pela bandeja: a pergunta mostra");
        assert!(dock_visivel(Sempre, 0, agora_mostra(0, false, true), t0), "pedido do outro modo: vale o normal");
        // só com pergunta: escondido sem pergunta, à vista com uma
        assert!(!dock_visivel(SoPerguntas, 0, PedidoDock::Nenhum, t0));
        assert!(dock_visivel(SoPerguntas, 1, PedidoDock::Nenhum, t0));
        assert!(!dock_visivel(SoPerguntas, 0, PedidoDock::Escondido, t0));
        assert!(dock_visivel(SoPerguntas, 1, PedidoDock::Escondido, t0), "a pergunta nunca se perde");
        // "Mostrar o dock agora": 30 s sem o mouse em cima, ou até o clique fora
        assert!(dock_visivel(SoPerguntas, 0, agora_mostra(t0, false, false), t0 + 29_999));
        assert!(!dock_visivel(SoPerguntas, 0, agora_mostra(t0, false, false), t0 + 30_000));
        assert!(dock_visivel(SoPerguntas, 0, agora_mostra(t0, true, false), t0 + 120_000), "com o mouse em cima, fica");
        assert!(!dock_visivel(SoPerguntas, 0, agora_mostra(t0, true, true), t0 + 1), "clicou fora");
        assert!(dock_visivel(SoPerguntas, 1, agora_mostra(t0, false, true), t0 + 1), "clicou fora com pergunta: fica");
        // relógio que voltou: não quebra
        assert!(dock_visivel(SoPerguntas, 0, agora_mostra(t0, false, false), t0 - 5));
        // o valor da config
        assert_eq!(ModoDock::de("perguntas"), SoPerguntas);
        assert_eq!(ModoDock::de("sempre"), Sempre);
        assert_eq!(ModoDock::de("outro"), Sempre);
    }

    const TELA: Ret = Ret { x: 0, y: 0, w: 1920, h: 1040 };
    const SEGUNDA: Ret = Ret { x: 1920, y: 0, w: 1280, h: 984 };

    #[test]
    fn so_bandeja_quando_o_windows_abre_o_app() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(so_bandeja(args(&["C:\\x\\work-timer.exe", "--bandeja"])));
        assert!(!so_bandeja(args(&["C:\\x\\work-timer.exe"])));
        assert!(!so_bandeja(args(&["--bandeja"])), "o primeiro é o executável");
        // o Tauri não cria a janela principal na partida e ela nasce escondida: quem cria e
        // mostra é o setup
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let janelas = conf["app"]["windows"].as_array().unwrap();
        let main = janelas.iter().find(|w| w["label"] == PRINCIPAL).unwrap();
        assert_eq!(main["create"], false);
        assert_eq!(main["visible"], false);
        let dock = janelas.iter().find(|w| w["label"] == DOCK).unwrap();
        assert_eq!(dock["create"], serde_json::Value::Null, "o dock o Tauri cria");
    }

    #[test]
    fn principal_na_partida_so_fora_da_bandeja_ou_no_primeiro_uso() {
        assert!(principal_na_partida(false, false));
        assert!(principal_na_partida(false, true));
        assert!(!principal_na_partida(true, false), "Iniciar com o Windows: só bandeja e dock");
        assert!(principal_na_partida(true, true), "primeiro uso abre a janela mesmo assim");
    }

    #[test]
    fn sem_resposta_da_pagina_so_sair_sai() {
        assert_eq!(AoFechar::Sair.sem_resposta(), AoFechar::Sair);
        assert_eq!(AoFechar::Bandeja.sem_resposta(), AoFechar::Bandeja);
        assert_eq!(AoFechar::Perguntar.sem_resposta(), AoFechar::Bandeja);
        for m in [AoFechar::Perguntar, AoFechar::Bandeja, AoFechar::Sair] {
            assert_eq!(AoFechar::de(m.valor()), m);
        }
    }

    #[test]
    fn ajustes_pedidos_vale_uma_vez() {
        assert_eq!(ajustes_pedidos(), None);
        AJUSTES_PEDIDOS.store(PEDIDO_AJUSTES, Ordering::SeqCst);
        assert_eq!(ajustes_pedidos(), Some("ajustes"));
        assert_eq!(ajustes_pedidos(), None, "um recarregamento não volta para os Ajustes");
        // o "Sobre o …" da bandeja: a seção Sobre
        AJUSTES_PEDIDOS.store(PEDIDO_SOBRE, Ordering::SeqCst);
        assert_eq!(ajustes_pedidos(), Some("sobre"));
        assert_eq!(ajustes_pedidos(), None);
    }

    #[test]
    fn ao_fechar_desconhecido_pergunta() {
        assert_eq!(AoFechar::de("perguntar"), AoFechar::Perguntar);
        assert_eq!(AoFechar::de("bandeja"), AoFechar::Bandeja);
        assert_eq!(AoFechar::de("sair"), AoFechar::Sair);
        assert_eq!(AoFechar::de(""), AoFechar::Perguntar);
        assert_eq!(AoFechar::de(&ConfigApp::default().ao_fechar), AoFechar::Perguntar);
    }

    /// WebView2 só divide o ambiente entre janelas com os mesmos argumentos; a principal
    /// recriada usa os da config, então basta todas as janelas da config terem os mesmos.
    /// Os padrões são os que o wry 0.57 passa quando não há argumentos (o `autoplay` dele
    /// vem ligado), mais `--disable-gpu`.
    #[test]
    fn todas_as_janelas_sem_gpu_e_com_os_argumentos_padrao() {
        let conf: serde_json::Value = serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        let janelas = conf["app"]["windows"].as_array().unwrap();
        // a principal, o dock e a dos avisos do dock (rodada 15)
        assert_eq!(janelas.len(), 3);
        for j in janelas {
            assert_eq!(
                j["additionalBrowserArgs"],
                "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection --autoplay-policy=no-user-gesture-required --disable-gpu",
                "{}",
                j["label"]
            );
        }
    }

    #[test]
    fn titulo_conta_os_rodando() {
        assert_eq!(titulo_principal(0), APP_NOME);
        assert_eq!(titulo_principal(2), format!("● 2 rodando · {APP_NOME}"));
        assert_eq!(titulo_dock(), format!("{APP_NOME} — dock"));
    }

    #[test]
    fn ao_minimizar_le_a_config_e_sem_resposta() {
        for m in [AoMinimizar::Perguntar, AoMinimizar::Bandeja, AoMinimizar::Dock, AoMinimizar::Barra] {
            assert_eq!(AoMinimizar::de(m.valor()), m);
        }
        assert_eq!(AoMinimizar::de(""), AoMinimizar::Perguntar);
        assert_eq!(AoMinimizar::de(&ConfigApp::default().ao_minimizar), AoMinimizar::Perguntar);
        // sem a página: "bandeja" fecha mesmo assim; "perguntar" minimiza na barra
        assert_eq!(AoMinimizar::Bandeja.sem_resposta(), AoMinimizar::Bandeja);
        assert_eq!(AoMinimizar::Perguntar.sem_resposta(), AoMinimizar::Barra);
    }

    #[test]
    fn o_proprio_minimizar_nao_pergunta_de_novo_por_2_s() {
        assert!(!minimizado_pelo_app(0, 5_000), "nunca minimizou");
        assert!(minimizado_pelo_app(10_000, 10_000));
        assert!(minimizado_pelo_app(10_000, 11_999));
        assert!(!minimizado_pelo_app(10_000, 12_000), "passou a folga: a pessoa minimizou de novo");
        assert!(!minimizado_pelo_app(10_000, 9_000), "relógio voltou");
    }

    fn mon(x: i32, escala: f64, principal: bool) -> Monitor {
        Monitor { tela: Ret { x, y: 0, w: 1920, h: 1080 }, util: Ret { x, y: 0, w: 1920, h: 1032 }, escala, principal }
    }

    #[test]
    fn abertura_na_escala_do_monitor_da_escolha() {
        let ms = [mon(0, 1.0, true), mon(1920, 1.25, false)];
        // a escolha em cima da barra do M2, a 125%: a pílula inicial de 240×46 lógicos vira 300×58
        let cfg = ConfigApp { dock_x: Some(3000), dock_y: Some(1022), dock_w: Some(375), dock_h: Some(58), ..ConfigApp::default() };
        let (r, fora) = abertura(&cfg, &ms);
        assert!(!fora);
        assert_eq!(r, Ret { x: 3075, y: 1022, w: 300, h: 58 }, "a borda direita e a de baixo ficam");
        // o M2 ainda não chegou: o padrão do principal, na escala dele, e "fora da escolha" (I1)
        let (r, fora) = abertura(&cfg, &ms[..1]);
        assert!(fora);
        assert_eq!(r, Ret { x: 1664, y: 16, w: 240, h: 46 });
        // sem escolha, nunca "fora"
        assert!(!abertura(&ConfigApp::default(), &ms).1);
        // o expandido no M2: o tamanho lógico salvo, na escala de lá
        let exp = ConfigApp { dock_expandido: true, dock_larg_l: Some(320.0), dock_alt_l: Some(420.0), ..cfg.clone() };
        let r = abertura(&exp, &ms).0;
        assert_eq!((r.w, r.h), (400, 525));
    }

    #[test]
    fn tamanho_do_expandido_logico_ou_de_antes() {
        let c = ConfigApp::default();
        assert_eq!(tamanho_expandido(&c, 1.5), (480, 630), "o padrão, na escala");
        let antes = ConfigApp { dock_larg: Some(500), dock_alt: Some(600), ..c.clone() };
        assert_eq!(tamanho_expandido(&antes, 1.5), (500, 600), "o de antes já é físico");
        let novo = ConfigApp { dock_larg_l: Some(330.0), dock_alt_l: Some(450.0), ..antes };
        assert_eq!(tamanho_expandido(&novo, 1.5), (495, 675), "o lógico vale primeiro");
    }

    #[test]
    fn pedido_vence_so_se_ninguem_respondeu() {
        static P: Pedido = Pedido::novo();
        let id = P.abrir();
        assert!(P.aberto());
        P.responder();
        assert!(!P.aberto() && !P.vencer(id), "respondido: não age sozinho");
        let id = P.abrir();
        let outro = P.abrir();
        assert!(!P.vencer(id), "um pedido mais novo vale");
        assert!(P.vencer(outro));
        assert!(!P.aberto());
    }
}
